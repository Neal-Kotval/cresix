#!/usr/bin/env python3
"""Real-process regression for Cresix Cloud connected mode.

This test intentionally uses only the Python standard library. It starts the
real Cloud and connector binaries around a small, test-controlled C6-compatible
HTTP upstream, then exercises the account, catalog, relay, and revocation
boundaries. Credentials stay in a private temporary directory and are never
printed.
"""

from __future__ import annotations

import contextlib
import http.cookiejar
import http.server
import json
import os
import pathlib
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
TARGET = ROOT / "target" / "debug"
TIMEOUT_SECONDS = 15
LOCAL_TOKEN = "local_test_credential_00000000000000000000000000000000"


class RegressionFailure(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RegressionFailure(message)


def private_write(path: pathlib.Path, value: str) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        stream.write(value)
    require(stat.S_IMODE(path.stat().st_mode) == 0o600, f"{path.name} is not mode 0600")


def unused_loopback_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def wait_until(predicate: Any, description: str, timeout: float = TIMEOUT_SECONDS) -> None:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            if predicate():
                return
        except Exception as error:  # expected while a process is becoming ready
            last_error = error
        time.sleep(0.1)
    detail = f": {type(last_error).__name__}" if last_error else ""
    raise RegressionFailure(f"timed out waiting for {description}{detail}")


class LocalC6Handler(http.server.BaseHTTPRequestHandler):
    server_version = "C6Regression/1"

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def _json(self, status: int, body: Any, *, cookie: bool = False) -> None:
        encoded = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        if cookie:
            self.send_header("Set-Cookie", "local_session=must-not-cross; Path=/; HttpOnly")
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self) -> None:
        if self.path == "/api/v1/projects":
            if self.headers.get("Authorization") != f"Bearer {LOCAL_TOKEN}":
                self._json(401, {"error": "unauthenticated"})
                return
            workspace_id = str(self.server.workspace_id)  # type: ignore[attr-defined]
            project_id = str(self.server.project_id)  # type: ignore[attr-defined]
            self._json(
                200,
                {
                    "projects": [
                        {
                            "id": project_id,
                            "workspaceId": workspace_id,
                            "slug": "weeknote",
                            "name": "Weeknote",
                            "description": "Connected-mode regression fixture",
                            "defaultBranch": "main",
                            "headSha": "0123456789abcdef",
                            "publishedSha": None,
                            "role": "owner",
                            "updatedAt": "2026-07-31T12:00:00Z",
                        }
                    ]
                },
            )
            return
        self._json(404, {"error": "not_found"})

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode("utf-8")
        self._json(
            201,
            {
                "path": self.path,
                "body": body,
                "headers": {name.lower(): value for name, value in self.headers.items()},
            },
            cookie=True,
        )


@dataclass
class ApiResponse:
    status: int
    headers: Any
    body: Any


class CloudClient:
    def __init__(self, origin: str) -> None:
        self.origin = origin
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar))

    def request(
        self,
        method: str,
        path: str,
        body: Any | None = None,
        *,
        csrf: str | None = None,
        origin: str | None = None,
        headers: dict[str, str] | None = None,
    ) -> ApiResponse:
        request_headers = {"Accept": "application/json"}
        if body is not None:
            request_headers["Content-Type"] = "application/json"
        if origin is not None:
            request_headers["Origin"] = origin
        if csrf is not None:
            request_headers["X-C6-CSRF"] = csrf
        if headers:
            request_headers.update(headers)
        request = urllib.request.Request(
            self.origin + path,
            method=method,
            headers=request_headers,
            data=None if body is None else json.dumps(body).encode("utf-8"),
        )
        try:
            response = self.opener.open(request, timeout=5)
        except urllib.error.HTTPError as error:
            response = error
        raw = response.read()
        content_type = response.headers.get("Content-Type", "")
        decoded = json.loads(raw) if raw and "json" in content_type else raw
        return ApiResponse(response.status, response.headers, decoded)


def process_output(process: subprocess.Popen[str], secrets: list[str]) -> str:
    if process.stderr is None:
        return ""
    output = ""
    if process.poll() is not None:
        output = process.stderr.read()[-2000:]
    for secret in secrets:
        output = output.replace(secret, "[REDACTED]")
    return output


def stop_process(process: subprocess.Popen[str] | None) -> None:
    if process is None or process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def run() -> None:
    subprocess.run(
        ["cargo", "build", "--quiet", "-p", "c6-cloud", "-p", "c6-connector"],
        cwd=ROOT,
        check=True,
    )

    cloud_process: subprocess.Popen[str] | None = None
    connector_process: subprocess.Popen[str] | None = None
    local_server: http.server.ThreadingHTTPServer | None = None
    local_thread: threading.Thread | None = None
    secrets: list[str] = [LOCAL_TOKEN]

    try:
        with tempfile.TemporaryDirectory(prefix="c6-cloud-regression-") as raw_temp:
            temp = pathlib.Path(raw_temp)
            cloud_data = temp / "cloud"
            cloud_data.mkdir(mode=0o700)
            cloud_port = unused_loopback_port()
            cloud_origin = f"http://127.0.0.1:{cloud_port}"

            workspace_id = uuid.uuid4()
            project_id = uuid.uuid4()
            local_server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), LocalC6Handler)
            local_server.workspace_id = workspace_id  # type: ignore[attr-defined]
            local_server.project_id = project_id  # type: ignore[attr-defined]
            local_port = int(local_server.server_address[1])
            local_thread = threading.Thread(target=local_server.serve_forever, daemon=True)
            local_thread.start()

            environment = os.environ.copy()
            environment.update(
                {
                    "C6_CLOUD_BIND": "127.0.0.1",
                    "C6_CLOUD_PORT": str(cloud_port),
                    "C6_CLOUD_PUBLIC_ORIGIN": cloud_origin,
                    "C6_CLOUD_DATA_DIR": str(cloud_data),
                    "C6_CLOUD_WEB_DIR": str(temp / "missing-web-is-fine-for-api"),
                    "RUST_LOG": "warn",
                }
            )
            cloud_process = subprocess.Popen(
                [str(TARGET / "c6-cloud")],
                cwd=ROOT,
                env=environment,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
            )
            client = CloudClient(cloud_origin)
            wait_until(
                lambda: client.request("GET", "/api/v1/status").status == 200,
                "Cloud API readiness",
            )
            require(cloud_process.poll() is None, "Cloud exited during startup")

            bootstrap_path = cloud_data / "bootstrap-token"
            wait_until(bootstrap_path.exists, "private bootstrap credential file")
            require(stat.S_IMODE(bootstrap_path.stat().st_mode) == 0o600, "bootstrap file is not 0600")
            bootstrap_token = bootstrap_path.read_text(encoding="utf-8").strip()
            secrets.append(bootstrap_token)

            claim = client.request(
                "POST",
                "/api/v1/bootstrap/claim",
                {"bootstrapToken": bootstrap_token, "handle": "regression", "displayName": "Regression Owner"},
                origin=cloud_origin,
            )
            require(claim.status == 201, f"bootstrap claim returned {claim.status}")
            csrf = claim.body["csrfToken"]
            secrets.append(csrf)
            require(not bootstrap_path.exists(), "consumed bootstrap credential file still exists")

            session = client.request("GET", "/api/v1/session")
            require(session.status == 200, f"authenticated session returned {session.status}")
            require(session.body["account"]["handle"] == "regression", "session account mismatch")

            no_csrf = client.request(
                "POST",
                "/api/v1/workspaces",
                {"namespace": "regression-lab", "name": "Regression Lab"},
                origin=cloud_origin,
            )
            require(no_csrf.status == 403, f"mutation without CSRF returned {no_csrf.status}")
            bad_origin = client.request(
                "POST",
                "/api/v1/workspaces",
                {"namespace": "regression-lab", "name": "Regression Lab"},
                csrf=csrf,
                origin="https://attacker.invalid",
            )
            require(bad_origin.status == 403, f"cross-origin mutation returned {bad_origin.status}")

            workspace = client.request(
                "POST",
                "/api/v1/workspaces",
                {"namespace": "regression-lab", "name": "Regression Lab"},
                csrf=csrf,
                origin=cloud_origin,
            )
            require(workspace.status == 201, f"workspace creation returned {workspace.status}")
            cloud_workspace_id = workspace.body["id"]
            listed = client.request("GET", "/api/v1/workspaces")
            require(listed.status == 200 and len(listed.body["workspaces"]) == 1, "workspace list mismatch")

            install = client.request(
                "POST",
                "/api/v1/installations",
                {"localServerId": str(uuid.uuid4()), "label": "Laptop regression"},
                csrf=csrf,
                origin=cloud_origin,
            )
            require(install.status == 201, f"installation registration returned {install.status}")
            installation_id = install.body["installation"]["id"]
            route_id = install.body["installation"]["routeId"]
            connector_token = install.body["connectorToken"]
            secrets.append(connector_token)

            binding = client.request(
                "POST",
                f"/api/v1/workspaces/{cloud_workspace_id}/bindings",
                {"installationId": installation_id, "localWorkspaceId": str(workspace_id)},
                csrf=csrf,
                origin=cloud_origin,
            )
            require(binding.status == 201, f"workspace binding returned {binding.status}")
            binding_id = binding.body["id"]

            offline = client.request("GET", f"/relay/{route_id}/status")
            require(offline.status == 502, f"offline relay returned {offline.status}")
            require(offline.body.get("code") == "relay_unavailable", "offline relay error is not stable")

            connector_token_path = temp / "connector-token"
            local_token_path = temp / "local-token"
            connector_config_path = temp / "connector.toml"
            private_write(connector_token_path, connector_token + "\n")
            private_write(local_token_path, LOCAL_TOKEN + "\n")
            private_write(
                connector_config_path,
                "\n".join(
                    [
                        f'cloud_origin = "{cloud_origin}"',
                        f'local_origin = "http://127.0.0.1:{local_port}"',
                        f'installation_id = "{installation_id}"',
                        f'binding_id = "{binding_id}"',
                        f'local_workspace_id = "{workspace_id}"',
                        f'cloud_credential_file = "{connector_token_path}"',
                        f'local_credential_file = "{local_token_path}"',
                        "allow_insecure_cloud_loopback = true",
                        "catalog_interval_seconds = 10",
                        "request_timeout_seconds = 5",
                        "max_in_flight = 2",
                        "",
                    ]
                ),
            )
            connector_process = subprocess.Popen(
                [str(TARGET / "c6-connector"), "--config", str(connector_config_path)],
                cwd=ROOT,
                env={**os.environ, "RUST_LOG": "warn"},
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
            )

            directory_path = "/api/v1/directory/regression-lab/weeknote"
            wait_until(
                lambda: client.request("GET", directory_path).status == 200,
                "connector catalog publication",
            )
            directory = client.request("GET", directory_path)
            require(directory.body["project"]["headSha"] == "0123456789abcdef", "catalog projection mismatch")
            require(directory.body["installation"]["connectionState"] == "connected", "installation is not connected")

            def relay_ready() -> bool:
                response = client.request(
                    "POST",
                    f"/relay/{route_id}/apps/echo?mode=connected",
                    {"hello": "relay"},
                    headers={
                        "X-Regression-Marker": "preserved",
                        "Forwarded": "for=192.0.2.1",
                        "X-C6-Relay-Route": "must-be-stripped",
                    },
                )
                return response.status == 201

            wait_until(relay_ready, "WebSocket relay registration")
            relayed = client.request(
                "POST",
                f"/relay/{route_id}/apps/echo?mode=connected",
                {"hello": "relay"},
                headers={
                    "X-Regression-Marker": "preserved",
                    "Forwarded": "for=192.0.2.1",
                    "X-C6-Relay-Route": "must-be-stripped",
                },
            )
            require(relayed.status == 201, f"relayed request returned {relayed.status}")
            require(relayed.body["path"] == "/apps/echo?mode=connected", "relay target changed")
            require(json.loads(relayed.body["body"]) == {"hello": "relay"}, "relay body changed")
            upstream_headers = relayed.body["headers"]
            require(upstream_headers.get("x-regression-marker") == "preserved", "safe header was lost")
            require("cookie" not in upstream_headers, "Cloud account cookie crossed into local C6")
            require("forwarded" not in upstream_headers, "forwarding identity header crossed relay")
            require("x-c6-relay-route" not in upstream_headers, "internal relay header crossed relay")
            require(relayed.headers.get("Set-Cookie") is None, "local Set-Cookie crossed into Cloud origin")
            require(any(cookie.name == "c6_cloud_session" for cookie in client.jar), "Cloud session cookie was overwritten")

            revoked = client.request(
                "DELETE",
                f"/api/v1/installations/{installation_id}",
                csrf=csrf,
                origin=cloud_origin,
            )
            require(revoked.status == 200, f"installation revocation returned {revoked.status}")
            require(revoked.body["installation"]["connectionState"] == "revoked", "revocation state mismatch")

            after_revoke = client.request("GET", f"/relay/{route_id}/status")
            require(after_revoke.status == 502, f"revoked relay returned {after_revoke.status}")
            heartbeat = client.request(
                "POST",
                f"/api/v1/installations/{installation_id}/heartbeat",
                headers={"Authorization": f"Bearer {connector_token}"},
            )
            require(heartbeat.status == 401, f"revoked connector credential returned {heartbeat.status}")
            require(client.request("GET", directory_path).status == 404, "revoked installation remains discoverable")

            stop_process(connector_process)
            connector_process = None
            require(client.request("GET", f"/relay/{route_id}/status").status == 502, "stopped connector route is available")

            stop_process(cloud_process)
            cloud_process = None
    except Exception as error:
        details = []
        if cloud_process is not None:
            output = process_output(cloud_process, secrets)
            if output:
                details.append(f"cloud stderr (redacted): {output}")
        if connector_process is not None:
            output = process_output(connector_process, secrets)
            if output:
                details.append(f"connector stderr (redacted): {output}")
        suffix = "\n" + "\n".join(details) if details else ""
        raise RegressionFailure(f"{error}{suffix}") from error
    finally:
        stop_process(connector_process)
        stop_process(cloud_process)
        if local_server is not None:
            local_server.shutdown()
            local_server.server_close()
        if local_thread is not None:
            local_thread.join(timeout=5)


if __name__ == "__main__":
    try:
        run()
    except Exception as error:
        print(f"c6 cloud connected regression: FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
    print("c6 cloud connected regression: passed")
