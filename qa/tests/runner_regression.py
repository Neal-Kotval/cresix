#!/usr/bin/env python3
"""Black-box abuse and lifecycle tests for the C6 runner Unix protocol."""

from __future__ import annotations

import base64
import concurrent.futures
import hashlib
import hmac
import json
import os
import pathlib
import socket
import stat
import subprocess
import tempfile
import time
import unittest
import uuid
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
RUNNER = ROOT / "target" / "debug" / "c6-runner"
AUTH_KEY = b"qa-runner-auth-key-is-synthetic-and-long-enough"
MAX_FRAME_BYTES = 256 * 1024


def b64(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).decode().rstrip("=")


def signed(command: dict[str, Any], *, issued_at_ms: int | None = None) -> dict[str, Any]:
    request_id = str(uuid.uuid4())
    timestamp = issued_at_ms if issued_at_ms is not None else int(time.time() * 1000)
    nonce = b64(os.urandom(16))
    payload_bytes = json.dumps(command, separators=(",", ":")).encode()
    payload = b64(payload_bytes)
    signing_input = f"c6-runner-v1\n{request_id}\n{timestamp}\n{nonce}\n{payload}".encode()
    mac = b64(hmac.new(AUTH_KEY, signing_input, hashlib.sha256).digest())
    return {
        "version": 1,
        "request_id": request_id,
        "issued_at_ms": timestamp,
        "nonce": nonce,
        "payload": payload,
        "mac": mac,
    }


def execution(run_id: str | None = None, **simulation: Any) -> dict[str, Any]:
    plan = {"delay_ms": 0, "stdout": "complete", "stderr": "", "exit_code": 0}
    plan.update(simulation)
    return {
        "type": "execute",
        "execution": {
            "run_id": run_id or str(uuid.uuid4()),
            "workspace_id": str(uuid.uuid4()),
            "project_id": str(uuid.uuid4()),
            "revision_sha": "a" * 40,
            "manifest_digest": "sha256:" + "b" * 64,
            "resources": {
                "cpu_millis": 100,
                "memory_bytes": 16 * 1024 * 1024,
                "disk_bytes": 1024 * 1024,
                "process_limit": 8,
                "timeout_seconds": 10,
                "log_bytes": 1024,
            },
            "network": {"mode": "deny_all"},
            "repository_write": "none",
            "simulation": plan,
        },
    }


class Runner:
    def __init__(self, root: pathlib.Path) -> None:
        self.root = root
        self.socket_path = root / "runner.sock"
        self.state = root / "state"
        self.log = root / "runner.log"
        self.process: subprocess.Popen[bytes] | None = None

    def start(self) -> None:
        if not RUNNER.is_file():
            raise AssertionError(f"missing runner binary: {RUNNER}; qa/run.sh builds it first")
        environment = os.environ.copy()
        environment.update(
            {
                "C6_RUNNER_SOCKET": str(self.socket_path),
                "C6_RUNNER_STATE_DIR": str(self.state),
                "C6_RUNNER_AUTH_KEY": AUTH_KEY.decode(),
                "RUST_LOG": "c6_runner=warn",
            }
        )
        log_handle = self.log.open("ab")
        self.process = subprocess.Popen(
            [str(RUNNER)], cwd=ROOT, env=environment, stdout=log_handle, stderr=subprocess.STDOUT
        )
        log_handle.close()
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AssertionError("runner exited during startup (process output suppressed)")
            if self.socket_path.exists():
                try:
                    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as probe:
                        probe.connect(str(self.socket_path))
                    return
                except (ConnectionRefusedError, FileNotFoundError):
                    # A previous daemon may have left a stale socket pathname;
                    # the new daemon replaces it safely before accepting.
                    pass
            time.sleep(0.03)
        raise AssertionError("runner socket did not appear (process output suppressed)")

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

    def raw(self, frame: bytes) -> bytes:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(8)
            connection.connect(str(self.socket_path))
            try:
                connection.sendall(frame)
            except BrokenPipeError:
                return b""
            try:
                connection.shutdown(socket.SHUT_WR)
            except OSError:
                pass
            chunks: list[bytes] = []
            while True:
                try:
                    chunk = connection.recv(64 * 1024)
                except ConnectionResetError:
                    break
                if not chunk:
                    break
                chunks.append(chunk)
            return b"".join(chunks)

    def request(self, envelope: dict[str, Any]) -> dict[str, Any]:
        response = self.raw(json.dumps(envelope, separators=(",", ":")).encode() + b"\n")
        if not response:
            raise AssertionError("runner closed without a structured response")
        return json.loads(response)


class RunnerAcceptance(unittest.TestCase):
    def setUp(self) -> None:
        # Unix-domain socket paths are short on macOS (104 bytes), while its
        # default temporary root is unusually long.
        self.temporary = tempfile.TemporaryDirectory(prefix="c6-runner-qa-", dir="/tmp")
        self.runner = Runner(pathlib.Path(self.temporary.name))
        self.runner.start()

    def tearDown(self) -> None:
        self.runner.stop()
        self.temporary.cleanup()

    def test_socket_is_owner_only_and_authenticated_ping_works(self) -> None:
        mode = stat.S_IMODE(self.runner.socket_path.stat().st_mode)
        self.assertEqual(mode, 0o600)
        response = self.runner.request(signed({"type": "ping"}))
        self.assertEqual(response["version"], 1)
        self.assertEqual(response["type"], "pong")

    def test_tampered_mac_stale_request_and_exact_replay_are_rejected(self) -> None:
        envelope = signed({"type": "ping"})
        self.assertEqual(self.runner.request(envelope)["type"], "pong")
        replay = self.runner.request(envelope)
        self.assertEqual((replay["type"], replay["code"]), ("rejected", "replay_detected"))

        tampered = signed({"type": "ping"})
        tampered["mac"] = b64(b"0" * 32)
        rejected = self.runner.request(tampered)
        self.assertEqual(rejected["code"], "authentication_failed")

        stale = signed({"type": "ping"}, issued_at_ms=int(time.time() * 1000) - 120_000)
        rejected = self.runner.request(stale)
        self.assertEqual(rejected["code"], "stale_request")

    def test_malformed_oversized_and_multiple_frames_close_without_crashing_daemon(self) -> None:
        self.assertEqual(self.runner.raw(b"not-json\n"), b"")
        self.assertEqual(self.runner.raw(b"x" * (MAX_FRAME_BYTES + 1) + b"\n"), b"")
        first = json.dumps(signed({"type": "ping"}), separators=(",", ":")).encode()
        self.assertEqual(self.runner.raw(first + b"\n" + first + b"\n"), b"")
        self.assertEqual(self.runner.request(signed({"type": "ping"}))["type"], "pong")

    def test_invalid_execution_and_blocked_network_destination_fail_closed(self) -> None:
        command = execution()
        command["execution"]["revision_sha"] = "HEAD"
        invalid = self.runner.request(signed(command))
        self.assertEqual((invalid["type"], invalid["code"]), ("rejected", "invalid_execution"))

        command = execution()
        command["execution"]["network"] = {
            "mode": "allow_list",
            "destinations": [{"host": "169.254.169.254", "port": 80}],
        }
        blocked = self.runner.request(signed(command))
        self.assertEqual((blocked["type"], blocked["code"]), ("rejected", "invalid_execution"))

    def test_success_failure_and_log_bounds_are_terminal(self) -> None:
        succeeded = self.runner.request(signed(execution(stdout="x" * 1500)))
        self.assertEqual(succeeded["type"], "finished")
        self.assertEqual(succeeded["record"]["status"], "succeeded")
        self.assertEqual(len(succeeded["record"]["stdout"]["content"]), 1024)
        self.assertTrue(succeeded["record"]["stdout"]["truncated"])

        failed = self.runner.request(signed(execution(stderr="failure", exit_code=7)))
        self.assertEqual(failed["record"]["status"], "failed")
        self.assertEqual(failed["record"]["exit_code"], 7)

    def test_timeout_produces_persisted_terminal_result(self) -> None:
        command = execution(delay_ms=1300)
        command["execution"]["resources"]["timeout_seconds"] = 1
        run_id = command["execution"]["run_id"]
        timed_out = self.runner.request(signed(command))
        self.assertEqual(timed_out["record"]["status"], "timed_out")
        inspected = self.runner.request(signed({"type": "inspect", "run_id": run_id}))
        self.assertEqual(inspected["record"]["status"], "timed_out")

    def test_duplicate_run_is_idempotent_but_conflicting_reuse_is_rejected(self) -> None:
        command = execution()
        first = self.runner.request(signed(command))
        second = self.runner.request(signed(command))
        self.assertEqual(first["record"], second["record"])
        conflict = json.loads(json.dumps(command))
        conflict["execution"]["simulation"]["stdout"] = "different"
        rejected = self.runner.request(signed(conflict))
        self.assertEqual(rejected["code"], "idempotency_conflict")

    def test_cancellation_is_terminal_and_repeated_cancel_is_safe(self) -> None:
        command = execution(delay_ms=3000)
        run_id = command["execution"]["run_id"]
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            running = executor.submit(self.runner.request, signed(command))
            time.sleep(0.15)
            cancellation = self.runner.request(signed({"type": "cancel", "run_id": run_id}))
            finished = running.result(timeout=8)
        self.assertEqual(cancellation["type"], "cancel_acknowledged")
        self.assertFalse(cancellation["already_terminal"])
        self.assertEqual(finished["record"]["status"], "cancelled")
        repeated = self.runner.request(signed({"type": "cancel", "run_id": run_id}))
        self.assertTrue(repeated["already_terminal"])

    def test_terminal_result_survives_runner_restart(self) -> None:
        command = execution()
        run_id = command["execution"]["run_id"]
        result = self.runner.request(signed(command))["record"]
        self.runner.restart()
        inspected = self.runner.request(signed({"type": "inspect", "run_id": run_id}))
        self.assertEqual(inspected["record"], result)


class StartupPolicy(unittest.TestCase):
    def test_weak_auth_key_refuses_to_start(self) -> None:
        if not RUNNER.is_file():
            self.fail(f"missing runner binary: {RUNNER}")
        with tempfile.TemporaryDirectory(prefix="c6-runner-key-qa-", dir="/tmp") as temporary:
            root = pathlib.Path(temporary)
            result = subprocess.run(
                [str(RUNNER)],
                cwd=ROOT,
                env={
                    **os.environ,
                    "C6_RUNNER_SOCKET": str(root / "runner.sock"),
                    "C6_RUNNER_STATE_DIR": str(root / "state"),
                    "C6_RUNNER_AUTH_KEY": "too-short",
                },
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=5,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "runner.sock").exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
