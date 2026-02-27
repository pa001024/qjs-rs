#!/usr/bin/env python3
"""Synchronize docs/current-status.md from compatibility snapshot manifest."""

from __future__ import annotations

import argparse
import difflib
import json
import sys
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Sync current-status.md from snapshot manifest.")
    parser.add_argument("--manifest", type=Path, required=True, help="Snapshot manifest path.")
    parser.add_argument("--status-doc", type=Path, required=True, help="Status doc path.")
    parser.add_argument(
        "--mode",
        choices=("write", "check"),
        required=True,
        help="write regenerates file, check validates deterministic sync.",
    )
    return parser.parse_args()


def load_manifest(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"manifest must be a JSON object: {path}")
    runs = payload.get("runs")
    if not isinstance(runs, list):
        raise RuntimeError(f"manifest must include a runs array: {path}")
    return payload


def render_status(manifest: dict[str, Any], manifest_path: Path) -> str:
    runs = manifest.get("runs", [])
    if not runs:
        return (
            "# Current Status Snapshot\n\n"
            f"Generated from `{manifest_path.as_posix()}`.\n\n"
            "No compatibility snapshots recorded yet.\n"
        )

    latest = runs[-1]
    phase = str(latest.get("phase", "unknown"))
    milestone = str(latest.get("milestone", "unknown"))
    profiles = latest.get("profiles", {})

    baseline = profiles.get("baseline", {}).get("summary", {})
    stress = profiles.get("stress", {}).get("summary", {})

    def row(profile_name: str, payload: dict[str, Any]) -> str:
        drift = payload.get("gc_drift", {})
        return (
            f"| {profile_name} | {drift.get('status', 'unknown')} | "
            f"{drift.get('anomaly_streak', 0)} | {bool(drift.get('investigation_required', False))} | "
            f"{payload.get('discovered', 0)} | {payload.get('executed', 0)} | {payload.get('failed', 0)} |"
        )

    lines = [
        "# Current Status Snapshot",
        "",
        f"Generated from `{manifest_path.as_posix()}`.",
        "",
        "## Compatibility Governance",
        "",
        "| Field | Value |",
        "| --- | --- |",
        f"| phase | {phase} |",
        f"| milestone | {milestone} |",
        "",
        "## Profile Drift Status",
        "",
        "| Profile | status | anomaly_streak | investigation_required | discovered | executed | failed |",
        "| --- | --- | ---: | --- | ---: | ---: | ---: |",
        row("baseline", baseline if isinstance(baseline, dict) else {}),
        row("stress", stress if isinstance(stress, dict) else {}),
        "",
        "## Policy",
        "",
        "- `status=blocking` is CI-blocking.",
        "- `anomaly_streak >= 2` sets `investigation_required=true` and is CI-blocking.",
        "- Regenerate this file with:",
        f"  - `python .github/scripts/sync_current_status.py --manifest {manifest_path.as_posix()} --status-doc docs/current-status.md --mode write`",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parents[2]
    manifest_path = (repo_root / args.manifest).resolve()
    status_path = (repo_root / args.status_doc).resolve()

    manifest = load_manifest(manifest_path)
    expected = render_status(manifest, args.manifest)

    if args.mode == "write":
        status_path.parent.mkdir(parents=True, exist_ok=True)
        status_path.write_text(expected, encoding="utf-8")
        print(f"wrote {args.status_doc.as_posix()}")
        return 0

    actual = status_path.read_text(encoding="utf-8") if status_path.exists() else ""
    if actual != expected:
        diff = "\n".join(
            difflib.unified_diff(
                actual.splitlines(),
                expected.splitlines(),
                fromfile=f"{args.status_doc.as_posix()} (actual)",
                tofile=f"{args.status_doc.as_posix()} (expected)",
                lineterm="",
            )
        )
        if diff:
            print(diff)
        print(
            "current-status drift detected; run sync_current_status.py --mode write",
            file=sys.stderr,
        )
        return 1

    print("current-status is synchronized")
    return 0


if __name__ == "__main__":
    sys.exit(main())
