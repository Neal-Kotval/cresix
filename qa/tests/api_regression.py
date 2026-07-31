#!/usr/bin/env python3
"""Black-box security and persistence tests for the C6 HTTP control plane."""

from __future__ import annotations

import concurrent.futures
import http.client
import json
import os
import pathlib
import socket
import sqlite3
import stat
import subprocess
import tempfile
import time
import unittest
import urllib.parse
from dataclasses import dataclass
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
SERVER = ROOT / "target" / "debug" / "c6-server"
BOOTSTRAP_TOKEN = "qa-bootstrap-token-that-is-never-a-real-secret"


@dataclass
class Response:
    status: int
    body: Any
    headers: dict[str, str]
    set_cookies: list[str]


class Client:
    def __init__(self, port: int, origin: str | None = None) -> None:
        self.port = port
        self.origin = origin or f"http://127.0.0.1:{port}"
        self.cookie: str | None = None
        self.csrf: str | None = None

    def request(
        self,
        method: str,
        path: str,
        body: Any = None,
        *,
        headers: dict[str, str] | None = None,
        authenticate: bool = True,
    ) -> Response:
        encoded = None if body is None else json.dumps(body).encode()
        request_headers = {"Accept": "application/json"}
        if method not in {"GET", "HEAD", "OPTIONS"}:
            request_headers["Origin"] = self.origin
        if encoded is not None:
            request_headers["Content-Type"] = "application/json"
        if authenticate and self.cookie:
            request_headers["Cookie"] = self.cookie
        if authenticate and self.csrf and method not in {"GET", "HEAD", "OPTIONS"}:
            request_headers["X-C6-CSRF"] = self.csrf
        request_headers.update(headers or {})
        connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)
        connection.request(method, path, body=encoded, headers=request_headers)
        raw = connection.getresponse()
        payload = raw.read()
        raw_headers = raw.getheaders()
        response_headers: dict[str, str] = {}
        for key, value in raw_headers:
            lowered = key.lower()
            response_headers[lowered] = f"{response_headers[lowered]}, {value}" if lowered in response_headers else value
        set_cookies = [value for key, value in raw_headers if key.lower() == "set-cookie"]
        connection.close()
        try:
            decoded = json.loads(payload) if payload else None
        except json.JSONDecodeError:
            decoded = payload.decode("utf-8", errors="replace")
        return Response(raw.status, decoded, response_headers, set_cookies)

    def adopt_session(self, response: Response) -> None:
        pairs = [value.split(";", 1)[0] for value in response.set_cookies]
        self.cookie = "; ".join(pairs)
        self.csrf = response.body["session"]["csrfToken"]


class Server:
    def __init__(
        self,
        data_dir: pathlib.Path,
        *,
        secure_cookie: bool = False,
        supplied_bootstrap: bool = True,
        extra_environment: dict[str, str] | None = None,
    ) -> None:
        self.data_dir = data_dir
        self.port = self._free_port()
        scheme = "https" if secure_cookie else "http"
        self.base_url = f"{scheme}://127.0.0.1:{self.port}"
        self.process: subprocess.Popen[bytes] | None = None
        self.log = data_dir.parent / f"server-{self.port}.log"
        self.supplied_bootstrap = supplied_bootstrap
        self.extra_environment = extra_environment or {}

    @staticmethod
    def _free_port() -> int:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            return int(listener.getsockname()[1])

    def start(self) -> None:
        if not SERVER.is_file():
            raise AssertionError(f"missing server binary: {SERVER}; qa/run.sh builds it first")
        environment = os.environ.copy()
        environment.update(
            {
                "C6_DATA_DIR": str(self.data_dir),
                "C6_PORT": str(self.port),
                "C6_PUBLIC_BASE_URL": self.base_url,
                "RUST_LOG": "c6_server=warn",
            }
        )
        if self.supplied_bootstrap:
            environment["C6_BOOTSTRAP_TOKEN"] = BOOTSTRAP_TOKEN
        else:
            environment.pop("C6_BOOTSTRAP_TOKEN", None)
        environment.update(self.extra_environment)
        log_handle = self.log.open("wb")
        self.process = subprocess.Popen(
            [str(SERVER)], cwd=ROOT, env=environment, stdout=log_handle, stderr=subprocess.STDOUT
        )
        log_handle.close()
        client = Client(self.port, self.base_url)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AssertionError("C6 server exited during startup (process output suppressed)")
            try:
                if client.request("GET", "/healthz", authenticate=False).status == 200:
                    return
            except (ConnectionError, OSError):
                pass
            time.sleep(0.05)
        raise AssertionError("C6 server did not become healthy (process output suppressed)")

    def stop(self) -> None:
        if not self.process:
            return
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=2)
        self.process = None

    def restart(self) -> None:
        self.stop()
        self.start()


def identity(token: str, name: str, key_suffix: str) -> dict[str, str]:
    return {
        "token": token,
        "displayName": name,
        "deviceLabel": f"{name} test device",
        "publicKey": f"qa-public-key-{key_suffix}-abcdefghijklmnopqrstuvwxyz",
    }


class ControlPlaneAcceptance(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="c6-api-qa-")
        self.root = pathlib.Path(self.temporary.name)
        self.data = self.root / "data"
        self.server = Server(self.data)
        self.server.start()
        self.client = Client(self.server.port, self.server.base_url)

    def tearDown(self) -> None:
        self.server.stop()
        self.temporary.cleanup()

    def claim(self) -> Response:
        response = self.client.request(
            "POST", "/api/v1/bootstrap/claim", identity(BOOTSTRAP_TOKEN, "QA Owner", "owner")
        )
        self.assertEqual(response.status, 201, response.body)
        self.client.adopt_session(response)
        return response

    def workspace(self) -> str:
        response = self.client.request(
            "POST", "/api/v1/workspaces", {"slug": "qa-team", "name": "QA Team"}
        )
        self.assertEqual(response.status, 201, response.body)
        return str(response.body["id"])

    def project(self, workspace_id: str) -> str:
        response = self.client.request(
            "POST",
            "/api/v1/projects",
            {"workspaceId": workspace_id, "slug": "qa-app", "name": "QA App"},
        )
        self.assertEqual(response.status, 201, response.body)
        return str(response.body["id"])

    def invite_peer(self, workspace_id: str, role: str = "reader") -> tuple[Client, str]:
        invitation = self.client.request(
            "POST",
            "/api/v1/invites",
            {"role": role, "workspaceId": workspace_id, "expiresInMinutes": 30},
        )
        self.assertEqual(invitation.status, 200, invitation.body)
        token = str(invitation.body["token"])
        peer = Client(self.server.port, self.server.base_url)
        redemption = peer.request(
            "POST",
            "/api/v1/invites/redeem",
            {**identity(token, "QA Peer", "peer"), "role": "owner"},
            authenticate=False,
        )
        self.assertEqual(redemption.status, 201, redemption.body)
        peer.adopt_session(redemption)
        return peer, token

    def invite_role(self, workspace_id: str, role: str) -> Client:
        invitation = self.client.request(
            "POST",
            "/api/v1/invites",
            {"role": role, "workspaceId": workspace_id, "expiresInMinutes": 30},
        )
        self.assertEqual(invitation.status, 200, invitation.body)
        peer = Client(self.server.port, self.server.base_url)
        response = peer.request(
            "POST",
            "/api/v1/invites/redeem",
            identity(invitation.body["token"], f"QA {role}", f"peer-{role}"),
            authenticate=False,
        )
        self.assertEqual(response.status, 201, response.body)
        peer.adopt_session(response)
        return peer

    def test_public_status_private_default_and_claim_cookie_policy(self) -> None:
        status = self.client.request("GET", "/api/v1/status", authenticate=False)
        self.assertEqual(status.status, 200)
        self.assertFalse(status.body["claimed"])
        self.assertEqual(
            self.client.request("GET", "/api/v1/projects", authenticate=False).status, 401
        )
        invalid = self.client.request(
            "POST", "/api/v1/bootstrap/claim", identity("wrong-token", "Attacker", "bad")
        )
        self.assertEqual(invalid.status, 403)
        claimed = self.claim()
        cookies = "\n".join(claimed.set_cookies)
        session_cookie = next(value for value in claimed.set_cookies if value.startswith("c6_session="))
        self.assertIn("HttpOnly", session_cookie)
        self.assertIn("SameSite=Strict", cookies)
        self.assertNotIn("Secure", cookies, "HTTP localhost cookies must remain usable")
        self.assertNotIn(BOOTSTRAP_TOKEN, (self.data / "c6.sqlite3").read_bytes().decode(errors="ignore"))

    def test_field_bounds_and_request_body_limit(self) -> None:
        overlong = identity(BOOTSTRAP_TOKEN, "x" * 121, "bounded")
        self.assertEqual(
            self.client.request("POST", "/api/v1/bootstrap/claim", overlong).status, 400
        )
        self.claim()
        oversized = self.client.request(
            "POST",
            "/api/v1/workspaces",
            {"slug": "oversized", "name": "x" * (70 * 1024)},
        )
        self.assertEqual(oversized.status, 413)

    def test_exactly_one_concurrent_bootstrap_claim_wins_and_replay_fails(self) -> None:
        payload = identity(BOOTSTRAP_TOKEN, "Race Owner", "race")

        def attempt() -> Response:
            return Client(self.server.port).request(
                "POST", "/api/v1/bootstrap/claim", payload, authenticate=False
            )

        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            results = list(executor.map(lambda _: attempt(), range(2)))
        self.assertEqual(sorted(response.status for response in results), [201, 409])
        replay = attempt()
        self.assertEqual(replay.status, 409)
        peers = [response for response in results if response.status == 201]
        self.assertEqual(len(peers), 1)

    def test_state_and_existing_session_survive_restart(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        cookie = self.client.cookie
        csrf = self.client.csrf
        self.server.restart()
        restarted = Client(self.server.port)
        restarted.cookie, restarted.csrf = cookie, csrf
        status = restarted.request("GET", "/api/v1/status", authenticate=False)
        self.assertTrue(status.body["claimed"])
        workspaces = restarted.request("GET", "/api/v1/workspaces")
        self.assertEqual(workspaces.status, 200, workspaces.body)
        self.assertIn(workspace_id, [item["id"] for item in workspaces.body["workspaces"]])

    def test_cookie_mutations_require_csrf_and_reject_cross_origin(self) -> None:
        self.claim()
        no_csrf = self.client.request(
            "POST",
            "/api/v1/workspaces",
            {"slug": "no-csrf", "name": "No CSRF"},
            headers={"X-C6-CSRF": ""},
        )
        self.assertEqual(no_csrf.status, 403)
        wrong_csrf = self.client.request(
            "POST",
            "/api/v1/workspaces",
            {"slug": "wrong-csrf", "name": "Wrong CSRF"},
            headers={"X-C6-CSRF": "wrong"},
        )
        self.assertEqual(wrong_csrf.status, 403)
        cross_origin = self.client.request(
            "POST",
            "/api/v1/workspaces",
            {"slug": "cross-origin", "name": "Cross Origin"},
            headers={"Origin": "https://evil.invalid", "Sec-Fetch-Site": "cross-site"},
        )
        self.assertEqual(cross_origin.status, 403)
        self.assertEqual(self.client.request("GET", "/api/v1/workspaces").body["workspaces"], [])

    def test_invite_is_hashed_single_use_and_cannot_escalate_role(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        peer, token = self.invite_peer(workspace_id, "reader")
        database_text = (self.data / "c6.sqlite3").read_bytes().decode(errors="ignore")
        self.assertNotIn(token, database_text)
        replay = Client(self.server.port).request(
            "POST",
            "/api/v1/invites/redeem",
            identity(token, "Replay", "replay"),
            authenticate=False,
        )
        self.assertEqual(replay.status, 409)
        denied = peer.request(
            "POST",
            "/api/v1/projects",
            {"workspaceId": workspace_id, "slug": "escalated", "name": "Escalated"},
        )
        self.assertEqual(denied.status, 403, denied.body)
        visible = peer.request("GET", "/api/v1/workspaces")
        self.assertEqual(visible.body["workspaces"][0]["role"], "reader")

    def test_expired_invite_fails_closed(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        invitation = self.client.request(
            "POST", "/api/v1/invites", {"role": "reader", "workspaceId": workspace_id}
        )
        token = invitation.body["token"]
        self.server.stop()
        database = sqlite3.connect(self.data / "c6.sqlite3")
        database.execute("UPDATE invites SET expires_at='2000-01-01T00:00:00Z'")
        database.commit()
        database.close()
        self.server.start()
        expired = Client(self.server.port).request(
            "POST",
            "/api/v1/invites/redeem",
            identity(token, "Late Peer", "late"),
            authenticate=False,
        )
        self.assertEqual(expired.status, 403)

    def test_peer_revocation_invalidates_live_session(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        peer, _ = self.invite_peer(workspace_id)
        peers = self.client.request("GET", "/api/v1/peers")
        peer_id = next(item["id"] for item in peers.body["peers"] if item["displayName"] == "QA Peer")
        revoked = self.client.request("DELETE", f"/api/v1/peers/{peer_id}")
        self.assertEqual(revoked.status, 204)
        self.assertEqual(peer.request("GET", "/api/v1/session").status, 401)

    def test_device_revocation_invalidates_its_live_session(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        peer, _ = self.invite_peer(workspace_id)
        devices = peer.request("GET", "/api/v1/devices")
        self.assertEqual(devices.status, 200, devices.body)
        self.assertEqual(len(devices.body["devices"]), 1)
        revoked = peer.request("DELETE", f"/api/v1/devices/{devices.body['devices'][0]['id']}")
        self.assertEqual(revoked.status, 204, revoked.body)
        self.assertEqual(peer.request("GET", "/api/v1/session").status, 401)

    def test_workspace_role_action_matrix_is_server_enforced(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        project_id = self.project(workspace_id)
        peers = {role: self.invite_role(workspace_id, role) for role in [
            "reader", "runner", "contributor", "maintainer", "owner"
        ]}
        for role, peer in peers.items():
            self.assertEqual(peer.request("GET", f"/api/v1/projects/{project_id}").status, 200, role)

        # The MVP repository API is read-only, so its single seeded branch is
        # used for this authorization test; branch semantics live in c6-git.
        pr_body = {"title": "Role test", "sourceBranch": "main", "targetBranch": "main"}
        self.assertEqual(peers["reader"].request("POST", f"/api/v1/projects/{project_id}/pull-requests", pr_body).status, 403)
        self.assertEqual(peers["runner"].request("POST", f"/api/v1/projects/{project_id}/pull-requests", pr_body).status, 403)
        self.assertEqual(peers["contributor"].request("POST", f"/api/v1/projects/{project_id}/pull-requests", pr_body).status, 201)

        run_body = {"job": "role-check", "kind": "command"}
        self.assertEqual(peers["reader"].request("POST", f"/api/v1/projects/{project_id}/runs", run_body).status, 403)
        self.assertEqual(peers["runner"].request("POST", f"/api/v1/projects/{project_id}/runs", run_body).status, 201)

        update_body = {"workspaceId": workspace_id, "slug": "qa-app", "name": "Updated"}
        self.assertEqual(peers["contributor"].request("PUT", f"/api/v1/projects/{project_id}", update_body).status, 403)
        self.assertEqual(peers["maintainer"].request("PUT", f"/api/v1/projects/{project_id}", update_body).status, 200)
        for role in ["reader", "runner", "contributor", "maintainer", "owner"]:
            self.assertEqual(peers[role].request("GET", "/api/v1/audit").status, 403, role)
        self.assertEqual(self.client.request("GET", "/api/v1/audit").status, 200)

    def test_audit_records_security_and_data_mutations_without_credentials(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        self.project(workspace_id)
        invitation = self.client.request(
            "POST", "/api/v1/invites", {"role": "reader", "workspaceId": workspace_id}
        )
        audit = self.client.request("GET", "/api/v1/audit")
        self.assertEqual(audit.status, 200, audit.body)
        actions = {event["action"] for event in audit.body["events"]}
        self.assertTrue({"server.claim", "workspace.create", "project.create", "invite.create"} <= actions)
        serialized = json.dumps(audit.body)
        for credential in [
            BOOTSTRAP_TOKEN,
            invitation.body["token"],
            self.client.csrf,
            self.client.cookie,
        ]:
            self.assertNotIn(str(credential), serialized)

    def test_unknown_api_route_is_structured_404_not_spa_html(self) -> None:
        missing = self.client.request("GET", "/api/v1/definitely-not-real", authenticate=False)
        self.assertEqual(missing.status, 404)
        self.assertEqual(missing.body["error"]["code"], "not_found")

    def test_project_metadata_validates_inputs_and_never_accepts_secret_values(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        project_id = self.project(workspace_id)
        invalid_schedule = self.client.request(
            "POST",
            f"/api/v1/projects/{project_id}/schedules",
            {"job": "report", "cron": "not a cron", "timezone": "UTC"},
        )
        self.assertEqual(invalid_schedule.status, 400)
        secret = "qa-value-must-not-be-stored-or-reflected"
        unavailable = self.client.request(
            "PUT",
            f"/api/v1/projects/{project_id}/secrets/API_KEY/value",
            {"value": secret},
        )
        self.assertEqual(unavailable.status, 501)
        self.assertNotIn(secret, json.dumps(unavailable.body))
        self.assertNotIn(secret, (self.data / "c6.sqlite3").read_bytes().decode(errors="ignore"))

    def test_repository_api_rejects_invalid_refs_traversal_and_orphan_creation(self) -> None:
        self.claim()
        workspace_id = self.workspace()
        project_id = self.project(workspace_id)
        branches = self.client.request(
            "GET", f"/api/v1/projects/{project_id}/repository/branches"
        )
        self.assertEqual(branches.status, 200, branches.body)
        self.assertEqual([branch["name"] for branch in branches.body["branches"]], ["main"])
        commits = self.client.request(
            "GET", f"/api/v1/projects/{project_id}/repository/commits?revision=main&limit=10"
        )
        self.assertEqual(commits.status, 200, commits.body)
        self.assertEqual(len(commits.body["commits"]), 1)
        tree = self.client.request(
            "GET", f"/api/v1/projects/{project_id}/repository/tree?revision=main&recursive=true"
        )
        self.assertEqual(tree.status, 200, tree.body)
        paths = [entry["path"] for entry in tree.body["entries"]]
        self.assertEqual(paths, ["README.md", "c6.toml"])
        manifest = self.client.request(
            "GET", f"/api/v1/projects/{project_id}/repository/files/c6.toml?revision=main"
        )
        self.assertEqual(manifest.status, 200)
        self.assertIn("version = 1", manifest.body)

        invalid_ref = self.client.request(
            "GET", f"/api/v1/projects/{project_id}/repository/commits?revision=--help&limit=10"
        )
        self.assertEqual(invalid_ref.status, 400, invalid_ref.body)
        oversized_log = self.client.request(
            "GET", f"/api/v1/projects/{project_id}/repository/commits?revision=main&limit=999999"
        )
        self.assertEqual(oversized_log.status, 400, oversized_log.body)
        traversal = urllib.parse.quote("../private", safe="")
        escaped = self.client.request(
            "GET",
            f"/api/v1/projects/{project_id}/repository/files/{traversal}?revision=main",
        )
        self.assertIn(escaped.status, {400, 404}, escaped.body)

        repositories_before = sorted((self.data / "git").glob("*.git"))
        duplicate = self.client.request(
            "POST",
            "/api/v1/projects",
            {"workspaceId": workspace_id, "slug": "qa-app", "name": "Duplicate"},
        )
        self.assertEqual(duplicate.status, 409, duplicate.body)
        repositories_after = sorted((self.data / "git").glob("*.git"))
        self.assertEqual(
            repositories_after,
            repositories_before,
            "a rejected project must not leave an orphan bare repository",
        )

    def test_logout_revokes_cookie_immediately(self) -> None:
        self.claim()
        old_cookie = self.client.cookie
        logged_out = self.client.request("DELETE", "/api/v1/session")
        self.assertEqual(logged_out.status, 204)
        replay = Client(self.server.port)
        replay.cookie = old_cookie
        self.assertEqual(replay.request("GET", "/api/v1/session").status, 401)


class SecureCookieAcceptance(unittest.TestCase):
    def test_https_public_url_marks_cookie_secure(self) -> None:
        with tempfile.TemporaryDirectory(prefix="c6-cookie-qa-") as temporary:
            server = Server(pathlib.Path(temporary) / "data", secure_cookie=True)
            server.start()
            try:
                client = Client(server.port, server.base_url)
                claimed = client.request(
                    "POST",
                    "/api/v1/bootstrap/claim",
                    identity(BOOTSTRAP_TOKEN, "Secure Owner", "secure"),
                    authenticate=False,
                )
                self.assertEqual(claimed.status, 201, claimed.body)
                self.assertTrue(claimed.set_cookies)
                self.assertTrue(all("; Secure" in value for value in claimed.set_cookies))
            finally:
                server.stop()


class ExposurePolicy(unittest.TestCase):
    def test_plaintext_non_loopback_bind_requires_explicit_operator_override(self) -> None:
        with tempfile.TemporaryDirectory(prefix="c6-exposure-qa-") as temporary:
            result = subprocess.run(
                [str(SERVER)],
                cwd=ROOT,
                env={
                    **os.environ,
                    "C6_DATA_DIR": str(pathlib.Path(temporary) / "data"),
                    "C6_BIND": "0.0.0.0",
                    "C6_PORT": "0",
                    "C6_PUBLIC_BASE_URL": "http://c6.example.invalid",
                    "C6_BOOTSTRAP_TOKEN": BOOTSTRAP_TOKEN,
                },
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=5,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)

    def test_malformed_public_origins_refuse_startup(self) -> None:
        invalid_origins = [
            "HTTPS://c6.example",
            "https://user:password@c6.example",
            "https://c6.example/path",
            "https://c6.example?query=yes",
            "https://c6.example#fragment",
            "not-an-origin",
        ]
        for origin in invalid_origins:
            with self.subTest(origin=origin), tempfile.TemporaryDirectory(
                prefix="c6-origin-qa-"
            ) as temporary:
                result = subprocess.run(
                    [str(SERVER)],
                    cwd=ROOT,
                    env={
                        **os.environ,
                        "C6_DATA_DIR": str(pathlib.Path(temporary) / "data"),
                        "C6_BIND": "127.0.0.1",
                        "C6_PORT": "0",
                        "C6_PUBLIC_BASE_URL": origin,
                        "C6_BOOTSTRAP_TOKEN": BOOTSTRAP_TOKEN,
                    },
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    timeout=5,
                    check=False,
                )
                self.assertNotEqual(result.returncode, 0)


class GeneratedBootstrapFile(unittest.TestCase):
    def test_generated_token_file_is_owner_only_and_removed_after_claim(self) -> None:
        with tempfile.TemporaryDirectory(prefix="c6-bootstrap-file-qa-") as temporary:
            root = pathlib.Path(temporary)
            server = Server(root / "data", supplied_bootstrap=False)
            server.start()
            try:
                token_path = server.data_dir / "bootstrap-token"
                self.assertTrue(token_path.is_file())
                self.assertEqual(stat.S_IMODE(token_path.stat().st_mode), 0o600)
                token = token_path.read_text().strip()
                self.assertGreaterEqual(len(token), 32)
                client = Client(server.port, server.base_url)
                response = client.request(
                    "POST",
                    "/api/v1/bootstrap/claim",
                    identity(token, "Generated Token Owner", "generated"),
                    authenticate=False,
                )
                self.assertEqual(response.status, 201, response.body)
                self.assertFalse(token_path.exists())
                self.assertNotIn(
                    token,
                    (server.data_dir / "c6.sqlite3").read_bytes().decode(errors="ignore"),
                )
            finally:
                server.stop()


if __name__ == "__main__":
    unittest.main(verbosity=2)
