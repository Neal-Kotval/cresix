#!/usr/bin/env python3
"""One durable, end-to-end C6-on-C6 dogfood journey.

This intentionally crosses both real process boundaries in one scenario. It is
not a replacement for the focused abuse suites.
"""

from __future__ import annotations

import pathlib
import sys
import tempfile
import uuid


TESTS = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(TESTS))

from api_regression import BOOTSTRAP_TOKEN, Client, Server, identity  # noqa: E402
from runner_regression import Runner, execution, signed  # noqa: E402


def require_status(response, expected: int, operation: str):
    if response.status != expected:
        raise AssertionError(f"{operation}: expected {expected}, got {response.status}: {response.body}")
    return response.body


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="c6-dogfood-", dir="/tmp") as temporary:
        root = pathlib.Path(temporary)
        server = Server(root / "control-data")
        runner = Runner(root / "runner")
        runner.root.mkdir()
        server.start()
        runner.start()
        try:
            owner = Client(server.port, server.base_url)
            claimed = owner.request(
                "POST",
                "/api/v1/bootstrap/claim",
                identity(BOOTSTRAP_TOKEN, "C6 Dogfood Owner", "dogfood-owner"),
            )
            require_status(claimed, 201, "claim server")
            owner.adopt_session(claimed)

            workspace = require_status(
                owner.request(
                    "POST", "/api/v1/workspaces", {"slug": "c6", "name": "C6 Builders"}
                ),
                201,
                "create workspace",
            )
            project = require_status(
                owner.request(
                    "POST",
                    "/api/v1/projects",
                    {
                        "workspaceId": workspace["id"],
                        "slug": "cresix",
                        "name": "Cresix",
                        "description": "C6 hosts its own product record during acceptance QA.",
                    },
                ),
                201,
                "create cresix project",
            )
            branches = require_status(
                owner.request(
                    "GET", f"/api/v1/projects/{project['id']}/repository/branches"
                ),
                200,
                "inspect seeded branches",
            )
            if [branch["name"] for branch in branches["branches"]] != ["main"]:
                raise AssertionError(f"unexpected seed branches: {branches}")
            tree = require_status(
                owner.request(
                    "GET",
                    f"/api/v1/projects/{project['id']}/repository/tree?revision=main&recursive=true",
                ),
                200,
                "inspect seeded tree",
            )
            if {entry["path"] for entry in tree["entries"]} != {"README.md", "c6.toml"}:
                raise AssertionError(f"unexpected seed tree: {tree}")

            invitation = require_status(
                owner.request(
                    "POST",
                    "/api/v1/invites",
                    {"workspaceId": workspace["id"], "role": "runner", "expiresInMinutes": 30},
                ),
                200,
                "invite runner peer",
            )
            peer = Client(server.port, server.base_url)
            redeemed = peer.request(
                "POST",
                "/api/v1/invites/redeem",
                identity(invitation["token"], "C6 Dogfood Peer", "dogfood-peer"),
                authenticate=False,
            )
            require_status(redeemed, 201, "redeem peer invitation")
            peer.adopt_session(redeemed)
            peer_session = require_status(peer.request("GET", "/api/v1/session"), 200, "peer session")
            if peer_session["workspaces"][0]["role"] != "runner":
                raise AssertionError(f"invited role was not preserved: {peer_session}")

            schedule = require_status(
                owner.request(
                    "POST",
                    f"/api/v1/projects/{project['id']}/schedules",
                    {
                        "job": "nightly-regression",
                        "cron": "0 2 * * *",
                        "timezone": "UTC",
                        "concurrency": "forbid",
                        "enabled": True,
                    },
                ),
                201,
                "create schedule",
            )
            deployment = require_status(
                owner.request(
                    "POST",
                    f"/api/v1/projects/{project['id']}/deployments",
                    {"revisionSha": project["headSha"], "environment": "preview"},
                ),
                201,
                "create deployment record",
            )
            run_record = require_status(
                peer.request(
                    "POST",
                    f"/api/v1/projects/{project['id']}/runs",
                    {
                        "job": "nightly-regression",
                        "kind": "cron",
                        "revisionSha": project["headSha"],
                    },
                ),
                201,
                "queue control-plane run",
            )

            command = execution(run_id=run_record["id"], stdout="C6 dogfood run complete")
            command["execution"]["workspace_id"] = workspace["id"]
            command["execution"]["project_id"] = project["id"]
            command["execution"]["revision_sha"] = project["headSha"]
            executed = runner.request(signed(command))
            if executed["type"] != "finished" or executed["record"]["status"] != "succeeded":
                raise AssertionError(f"runner did not finish dogfood run: {executed}")

            owner_cookie, owner_csrf = owner.cookie, owner.csrf
            server.restart()
            runner.restart()
            owner = Client(server.port, server.base_url)
            owner.cookie, owner.csrf = owner_cookie, owner_csrf
            require_status(owner.request("GET", "/api/v1/session"), 200, "session after restart")
            for endpoint, key, expected_id in [
                ("schedules", "schedules", schedule["id"]),
                ("deployments", "deployments", deployment["id"]),
                ("runs", "runs", run_record["id"]),
            ]:
                listing = require_status(
                    owner.request("GET", f"/api/v1/projects/{project['id']}/{endpoint}"),
                    200,
                    f"list durable {endpoint}",
                )
                if expected_id not in [item["id"] for item in listing[key]]:
                    raise AssertionError(f"{endpoint} record did not survive restart: {listing}")
            persisted = runner.request(
                signed({"type": "inspect", "run_id": str(uuid.UUID(run_record["id"]))})
            )
            if persisted["record"]["status"] != "succeeded":
                raise AssertionError(f"runner result did not survive restart: {persisted}")
        finally:
            server.stop()
            runner.stop()

    print("C6 dogfood journey passed: claim -> cresix -> peer -> schedule/run/deploy -> restart")


if __name__ == "__main__":
    main()
