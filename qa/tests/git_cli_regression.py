#!/usr/bin/env python3
"""Real-process Phase 2.1 Git smart-HTTP and C6 CLI acceptance coverage."""

from __future__ import annotations

import base64
import json
import os
import pathlib
import stat
import subprocess
import tempfile
import unittest
from typing import Any

from api_regression import BOOTSTRAP_TOKEN, Client, Response, Server, identity


ROOT = pathlib.Path(__file__).resolve().parents[2]
C6 = ROOT / "target" / "debug" / "c6"
GIT_CREDENTIAL_C6 = ROOT / "target" / "debug" / "git-credential-c6"


class GitCliAcceptance(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="c6-git-cli-qa-")
        self.root = pathlib.Path(self.temporary.name)
        self.server = Server(
            self.root / "data",
            extra_environment={
                "C6_GIT_HTTP_ENABLED": "true",
                "RUST_LOG": "c6_server=trace,tower_http=trace",
            },
        )
        self.server.start()
        self.browser = Client(self.server.port, self.server.base_url)
        self.config_dir = self.root / "cli-state"
        self.transcript: list[str] = []
        self.secrets: list[str] = []
        self.command_environment = os.environ.copy()
        self.command_environment.update(
            {
                "C6_CONFIG_DIR": str(self.config_dir),
                "GIT_TERMINAL_PROMPT": "0",
                "PATH": f"{C6.parent}{os.pathsep}{os.environ.get('PATH', '')}",
            }
        )

    def tearDown(self) -> None:
        self.server.stop()
        self.temporary.cleanup()

    def redact(self, value: str) -> str:
        for secret in self.secrets:
            value = value.replace(secret, "[REDACTED]")
        return value

    def run_command(
        self,
        argv: list[str],
        *,
        cwd: pathlib.Path | None = None,
        stdin: str | None = None,
        expected: int = 0,
    ) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            argv,
            cwd=cwd or ROOT,
            env=self.command_environment,
            input=stdin,
            text=True,
            capture_output=True,
            timeout=30,
            check=False,
        )
        combined = result.stdout + result.stderr
        self.transcript.append(combined)
        self.assertEqual(
            result.returncode,
            expected,
            f"command failed ({result.returncode} != {expected}): {argv!r}\n{self.redact(combined)}",
        )
        return result

    def browser_post(self, path: str, body: Any) -> Response:
        response = self.browser.request("POST", path, body)
        self.assertIn(response.status, {200, 201}, response.body)
        return response

    def issue_credential(self, kind: str, scopes: list[str], label: str) -> tuple[str, str]:
        response = self.browser_post(
            "/api/v1/credentials",
            {"type": kind, "label": label, "scopes": scopes},
        )
        token = str(response.body["token"])
        credential_id = str(response.body["credential"]["id"])
        self.assertTrue(token.startswith("c6c_v1_" if kind == "cli" else "c6g_v1_"))
        self.secrets.append(token)
        return credential_id, token

    @staticmethod
    def basic(token: str) -> str:
        encoded = base64.b64encode(f"c6:{token}".encode()).decode()
        return f"Basic {encoded}"

    def git_advertisement(self, remote_path: str, *, headers: dict[str, str]) -> Response:
        return self.browser.request(
            "GET",
            f"{remote_path}/info/refs?service=git-upload-pack",
            headers=headers,
            authenticate=False,
        )

    def store_git_token(self, remote_url: str, token: str) -> None:
        self.run_command(
            [str(GIT_CREDENTIAL_C6), "store"],
            stdin=f"url={remote_url}\nusername=c6\npassword={token}\n\n",
        )

    def assert_secrets_absent(self, value: str | bytes, location: str) -> None:
        haystack = value if isinstance(value, bytes) else value.encode()
        for secret in self.secrets:
            self.assertNotIn(secret.encode(), haystack, f"credential leaked in {location}")

    def test_real_git_and_cli_survive_restart_then_fail_after_revocation(self) -> None:
        for binary in (C6, GIT_CREDENTIAL_C6):
            self.assertTrue(binary.is_file(), f"missing acceptance binary: {binary}")

        claim = self.browser_post(
            "/api/v1/bootstrap/claim",
            identity(BOOTSTRAP_TOKEN, "Git CLI Owner", "git-cli-owner"),
        )
        self.browser.adopt_session(claim)
        workspace = self.browser_post(
            "/api/v1/workspaces", {"slug": "qa-team", "name": "QA Team"}
        ).body
        project = self.browser_post(
            "/api/v1/projects",
            {
                "workspaceId": workspace["id"],
                "slug": "qa-app",
                "name": "QA App",
                "description": "Real Git and CLI acceptance",
            },
        ).body

        cli_id, cli_token = self.issue_credential("cli", ["api:read"], "QA CLI")
        git_id, git_token = self.issue_credential("git", ["git:read"], "QA Git")
        remote_path = "/git/qa-team/qa-app.git"
        remote_url = f"{self.server.base_url}{remote_path}"

        listed = self.browser.request("GET", "/api/v1/credentials")
        self.assertEqual(listed.status, 200, listed.body)
        self.assertEqual({item["id"] for item in listed.body["credentials"]}, {cli_id, git_id})
        self.assertNotIn("token", json.dumps(listed.body).lower())

        valid = self.git_advertisement(
            remote_path, headers={"Authorization": self.basic(git_token)}
        )
        self.assertEqual(valid.status, 200, valid.body)
        self.assertEqual(
            valid.headers.get("content-type"),
            "application/x-git-upload-pack-advertisement",
        )
        self.assertEqual(
            self.git_advertisement(remote_path, headers={}).status,
            401,
        )
        self.assertEqual(
            self.git_advertisement(
                remote_path, headers={"Cookie": self.browser.cookie or ""}
            ).status,
            401,
        )
        self.assertEqual(
            self.git_advertisement(
                remote_path,
                headers={
                    "Cookie": self.browser.cookie or "",
                    "Authorization": self.basic(git_token),
                },
            ).status,
            401,
        )
        self.assertEqual(
            self.git_advertisement(
                remote_path, headers={"Authorization": self.basic(cli_token)}
            ).status,
            401,
        )
        wrong_git_token = git_token[:-1] + ("A" if git_token[-1] != "A" else "B")
        self.secrets.append(wrong_git_token)
        self.assertEqual(
            self.git_advertisement(
                remote_path, headers={"Authorization": self.basic(wrong_git_token)}
            ).status,
            401,
        )
        rejected_query = self.browser.request(
            "GET",
            f"{remote_path}/info/refs?service=git-upload-pack&credential={wrong_git_token}",
            headers={"Authorization": self.basic(git_token)},
            authenticate=False,
        )
        self.assertEqual(rejected_query.status, 400, rejected_query.body)
        rejected_path = self.browser.request(
            "GET",
            f"/git/qa-team/{wrong_git_token}.git/info/refs?service=git-upload-pack",
            headers={"Authorization": self.basic(git_token)},
            authenticate=False,
        )
        self.assertEqual(rejected_path.status, 404, rejected_path.body)
        self.assert_secrets_absent(
            self.server.log.read_bytes(), "trace log after rejected credential-bearing URI"
        )
        wrong_class = self.browser.request(
            "GET",
            "/api/v1/cli/whoami",
            headers={"Authorization": f"Bearer {git_token}"},
            authenticate=False,
        )
        self.assertEqual(wrong_class.status, 401, wrong_class.body)
        dual_cli = self.browser.request(
            "GET",
            "/api/v1/cli/whoami",
            headers={
                "Authorization": f"Bearer {cli_token}",
                "Cookie": self.browser.cookie or "",
            },
            authenticate=False,
        )
        self.assertEqual(dual_cli.status, 401, dual_cli.body)

        receive_advertisement = self.browser.request(
            "GET",
            f"{remote_path}/info/refs?service=git-receive-pack",
            headers={"Authorization": self.basic(git_token)},
            authenticate=False,
        )
        self.assertNotEqual(receive_advertisement.status, 200)
        receive_post = self.browser.request(
            "POST",
            f"{remote_path}/git-receive-pack",
            body=None,
            headers={"Authorization": self.basic(git_token)},
            authenticate=False,
        )
        self.assertEqual(receive_post.status, 404, receive_post.body)

        self.run_command(
            [
                str(C6),
                "server",
                "add",
                self.server.base_url,
                "--name",
                "qa",
                "--allow-http-localhost",
            ]
        )
        self.run_command(
            [
                str(C6),
                "auth",
                "login",
                "--server",
                "qa",
                "--token-stdin",
                "--plaintext-store",
            ],
            stdin=f"{cli_token}\n",
        )
        status = self.run_command([str(C6), "--json", "auth", "status", "--server", "qa"])
        self.assertEqual(json.loads(status.stdout)["data"]["user"]["displayName"], "Git CLI Owner")
        projects = self.run_command(
            [str(C6), "--json", "project", "list", "--server", "qa", "--workspace", "qa-team"]
        )
        self.assertEqual(json.loads(projects.stdout)["data"]["projects"][0]["slug"], "qa-app")
        self.run_command([str(C6), "--json", "doctor", "--server", "qa"])
        self.store_git_token(remote_url, git_token)

        credentials_file = self.config_dir / "credentials.json"
        self.assertEqual(stat.S_IMODE(credentials_file.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(self.config_dir.stat().st_mode), 0o700)

        direct = self.run_command(
            [
                "git",
                "-c",
                "credential.helper=c6",
                "-c",
                "credential.useHttpPath=true",
                "ls-remote",
                remote_url,
            ]
        )
        self.assertIn("HEAD", direct.stdout)

        clone_dir = self.root / "clone"
        self.run_command(
            [str(C6), "clone", "qa-team/qa-app", str(clone_dir), "--server", "qa"]
        )
        self.assertTrue((clone_dir / ".git").is_dir())
        self.assertTrue((clone_dir / "README.md").is_file())
        clone_remote = self.run_command(
            ["git", "-C", str(clone_dir), "remote", "get-url", "origin"]
        ).stdout.strip()
        self.assertEqual(clone_remote, remote_url)
        self.assert_secrets_absent((clone_dir / ".git" / "config").read_bytes(), ".git/config")

        secondary = self.root / "secondary"
        self.run_command(["git", "init", str(secondary)])
        self.run_command(
            [str(C6), "remote", "add", "qa-team/qa-app", "--name", "c6", "--server", "qa"],
            cwd=secondary,
        )
        self.run_command(["git", "fetch", "c6"], cwd=secondary)
        self.assert_secrets_absent((secondary / ".git" / "config").read_bytes(), "remote config")

        push = self.run_command(
            ["git", "-C", str(clone_dir), "push", "origin", "HEAD:refs/heads/qa-copy"],
            expected=128,
        )
        push_output = (push.stdout + push.stderr).lower()
        self.assertTrue(
            "returned error: 400" in push_output or "returned error: 404" in push_output,
            self.redact(push_output),
        )

        self.assert_secrets_absent(self.server.log.read_bytes(), "server log before restart")
        self.assert_secrets_absent((self.server.data_dir / "c6.sqlite3").read_bytes(), "SQLite")
        audit = self.browser.request("GET", "/api/v1/audit")
        self.assertEqual(audit.status, 200, audit.body)
        self.assert_secrets_absent(json.dumps(audit.body), "audit response")

        self.server.restart()
        self.run_command([str(C6), "--json", "auth", "status", "--server", "qa"])
        self.run_command(["git", "-C", str(clone_dir), "fetch", "origin"])
        self.assert_secrets_absent(self.server.log.read_bytes(), "server log after restart")

        revoked = self.browser.request("DELETE", f"/api/v1/credentials/{git_id}")
        self.assertEqual(revoked.status, 204, revoked.body)
        denied = self.run_command(
            [
                "git",
                "-c",
                "credential.helper=c6",
                "-c",
                "credential.useHttpPath=true",
                "ls-remote",
                remote_url,
            ],
            expected=128,
        )
        self.assertIn("authentication", (denied.stdout + denied.stderr).lower())
        self.assertEqual(
            self.git_advertisement(
                remote_path, headers={"Authorization": self.basic(git_token)}
            ).status,
            401,
        )

        transcript = "".join(self.transcript)
        self.assert_secrets_absent(transcript, "CLI/Git output")
        self.assert_secrets_absent(self.server.log.read_bytes(), "final server log")
        self.assert_secrets_absent((self.server.data_dir / "c6.sqlite3").read_bytes(), "final SQLite")

    def test_git_http_transport_is_disabled_by_default(self) -> None:
        self.server.stop()
        default_server = Server(self.root / "default-off-data")
        default_server.start()
        self.server = default_server
        self.browser = Client(default_server.port, default_server.base_url)

        claim = self.browser_post(
            "/api/v1/bootstrap/claim",
            identity(BOOTSTRAP_TOKEN, "Default Off Owner", "default-off-owner"),
        )
        self.browser.adopt_session(claim)
        workspace = self.browser_post(
            "/api/v1/workspaces", {"slug": "default-off", "name": "Default Off"}
        ).body
        project = self.browser_post(
            "/api/v1/projects",
            {"workspaceId": workspace["id"], "slug": "private", "name": "Private"},
        ).body
        remote = self.browser.request("GET", f"/api/v1/projects/{project['id']}/remote")
        self.assertEqual(remote.status, 200, remote.body)
        self.assertEqual(remote.body["capabilities"], {"fetch": False, "push": False})

        _, git_token = self.issue_credential("git", ["git:read"], "Disabled Git")
        advertisement = self.git_advertisement(
            "/git/default-off/private.git",
            headers={"Authorization": self.basic(git_token)},
        )
        self.assertEqual(advertisement.status, 503, advertisement.body)


if __name__ == "__main__":
    unittest.main(verbosity=2)
