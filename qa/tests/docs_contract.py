#!/usr/bin/env python3
"""Validate the local documentation structure without network access."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"

REQUIRED_PRODUCT = {
    "README.md",
    "VISION.md",
    "PRINCIPLES.md",
    "USERS_AND_USE_CASES.md",
    "PRODUCT_MODEL.md",
    "CAPABILITIES.md",
    "WORKFLOWS.md",
    "COLLABORATION_AND_SHARING.md",
    "AGENT_CENTRIC.md",
    "C6R.md",
    "DEPLOYMENT_MODES.md",
    "NON_GOALS.md",
    "ROADMAP.md",
    "OPEN_QUESTIONS.md",
}

REQUIRED_ARCHITECTURE = {
    "README.md",
    "SYSTEM_CONTEXT.md",
    "COMPONENTS.md",
    "DOMAIN_AND_DATA.md",
    "TRUST_AND_AUTHORIZATION.md",
    "DEPLOYMENT_TOPOLOGIES.md",
    "CONNECTOR_AND_RELAY.md",
    "STORAGE_AND_CONSISTENCY.md",
    "API_AND_PROTOCOLS.md",
    "FAILURE_MODES_AND_OPERABILITY.md",
    "EXTENSION_POINTS.md",
    "C6R_ARCHITECTURE.md",
    "AGENT_AND_RUNTIME_ARCHITECTURE.md",
    "ROADMAP.md",
}

LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
HEADING = re.compile(r"^#{1,6}\s+(.+?)\s*#*\s*$", re.MULTILINE)
ALLOWED_SPEC_STATUS = (
    "draft",
    "accepted design",
    "partially implemented",
    "implemented",
    "superseded",
)
LEGACY_DESTINATIONS = {
    "PRODUCT.md": "product/README.md",
    "ARCHITECTURE.md": "architecture/README.md",
    "ROADMAP.md": "product/CAPABILITIES.md",
}


def markdown_files() -> list[Path]:
    files = [ROOT / "README.md", ROOT / "CONTRIBUTING.md", ROOT / "SECURITY.md"]
    files.extend(sorted(DOCS.rglob("*.md")))
    files.extend(sorted((ROOT / "qa").rglob("*.md")))
    files.extend(sorted((ROOT / "examples").rglob("*.md")))
    files.extend(sorted((ROOT / "teams" / "c6-build-team").rglob("*.md")))
    return [path for path in files if path.is_file()]


def local_target(raw_target: str) -> tuple[str, str] | None:
    target = raw_target.strip()
    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1]
    if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", target):
        return None
    path, _, anchor = target.partition("#")
    return unquote(path), unquote(anchor).lower()


def heading_anchors(path: Path) -> set[str]:
    anchors: set[str] = set()
    counts: dict[str, int] = {}
    for heading in HEADING.findall(path.read_text(encoding="utf-8")):
        plain = re.sub(r"<[^>]+>", "", heading)
        plain = re.sub(r"[`*_~]", "", plain).lower().strip()
        base = re.sub(r"[^\w\- ]", "", plain, flags=re.UNICODE)
        base = re.sub(r"\s+", "-", base)
        count = counts.get(base, 0)
        counts[base] = count + 1
        anchors.add(base if count == 0 else f"{base}-{count}")
    return anchors


def main() -> int:
    errors: list[str] = []

    product_names = {path.name for path in (DOCS / "product").glob("*.md")}
    architecture_names = {
        path.name for path in (DOCS / "architecture").glob("*.md")
    }
    for missing in sorted(REQUIRED_PRODUCT - product_names):
        errors.append(f"missing product handbook page: docs/product/{missing}")
    for missing in sorted(REQUIRED_ARCHITECTURE - architecture_names):
        errors.append(
            f"missing architecture handbook page: docs/architecture/{missing}"
        )

    for path in markdown_files():
        text = path.read_text(encoding="utf-8")
        for match in LINK.finditer(text):
            parsed_target = local_target(match.group(1))
            if parsed_target is None:
                continue
            target, anchor = parsed_target
            resolved = (path.parent / target).resolve() if target else path.resolve()
            if not resolved.is_relative_to(ROOT):
                errors.append(
                    f"link escapes repository in {path.relative_to(ROOT)}: "
                    f"{match.group(1)}"
                )
                continue
            if not resolved.exists():
                errors.append(
                    f"broken link in {path.relative_to(ROOT)}: {match.group(1)}"
                )
                continue
            if anchor and resolved.suffix.lower() == ".md":
                if anchor not in heading_anchors(resolved):
                    errors.append(
                        f"broken anchor in {path.relative_to(ROOT)}: "
                        f"{match.group(1)}"
                    )

    for path in sorted((DOCS / "specs").glob("*.md")):
        opening = "\n".join(path.read_text(encoding="utf-8").splitlines()[:12])
        status_match = re.search(r"^Status:\s*(.+)$", opening, re.MULTILINE)
        if status_match is None:
            errors.append(f"spec lacks opening status metadata: {path.relative_to(ROOT)}")
        elif not status_match.group(1).lower().startswith(ALLOWED_SPEC_STATUS):
            errors.append(
                f"spec has unsupported status vocabulary: {path.relative_to(ROOT)}: "
                f"{status_match.group(1)}"
            )
        if "Capability status:" not in opening:
            errors.append(
                f"spec lacks capability-ledger link: {path.relative_to(ROOT)}"
            )

    for legacy_name, destination in LEGACY_DESTINATIONS.items():
        legacy = DOCS / legacy_name
        legacy_text = legacy.read_text(encoding="utf-8")
        line_count = len(legacy.read_text(encoding="utf-8").splitlines())
        if line_count > 40:
            errors.append(
                f"legacy overview became a second source of truth: "
                f"{legacy.relative_to(ROOT)} has {line_count} lines"
            )
        if f"]({destination})" not in legacy_text:
            errors.append(
                f"legacy overview does not point to canonical destination: "
                f"{legacy.relative_to(ROOT)} -> {destination}"
            )

    if errors:
        for error in errors:
            print(f"docs contract: {error}", file=sys.stderr)
        return 1

    print("docs contract: handbook structure, metadata, and links passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
