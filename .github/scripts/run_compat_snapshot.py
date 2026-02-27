#!/usr/bin/env python3
"""Run baseline/stress compatibility snapshots and append a manifest entry."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


def run(cmd: list[str], cwd: Path) -> str:
    completed = subprocess.run(
        cmd,
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )
    output = completed.stdout.strip()
    if output:
        print(output)
    return output


def repo_relative(path: Path, repo_root: Path) -> str:
    try:
        return path.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError:
        return str(path.resolve())


def load_manifest(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"schema_version": 1, "runs": []}
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"manifest must be an object: {path}")
    runs = payload.get("runs")
    if not isinstance(runs, list):
        raise RuntimeError(f"manifest must define a runs array: {path}")
    return payload


def latest_profile_json_path(
    manifest: dict[str, Any], profile: str, repo_root: Path
) -> Path | None:
    runs = manifest.get("runs", [])
    if not isinstance(runs, list):
        return None
    for entry in reversed(runs):
        if not isinstance(entry, dict):
            continue
        profiles = entry.get("profiles")
        if not isinstance(profiles, dict):
            continue
        profile_payload = profiles.get(profile)
        if not isinstance(profile_payload, dict):
            continue
        json_path = profile_payload.get("json")
        if not isinstance(json_path, str):
            continue
        candidate = (repo_root / json_path).resolve()
        if candidate.exists():
            return candidate
    return None


def is_repo_dirty(repo_root: Path) -> bool:
    completed = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return bool(completed.stdout.strip())


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run compatibility snapshots.")
    parser.add_argument("--phase", required=True, help="Phase identifier, e.g. 07.")
    parser.add_argument("--milestone", required=True, help="Milestone identifier, e.g. v1.0.")
    parser.add_argument("--manifest", type=Path, required=True, help="Manifest JSON path.")
    parser.add_argument(
        "--output-dir",
        type=Path,
        required=True,
        help="Output directory for per-run snapshot artifacts.",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path("crates/test-harness/fixtures/test262-lite"),
        help="test262 root for snapshot runs.",
    )
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="Allow dirty git state when running snapshots.",
    )
    return parser.parse_args()


def profile_summary(payload: dict[str, Any]) -> dict[str, Any]:
    return {
        "discovered": payload.get("discovered", 0),
        "executed": payload.get("executed", 0),
        "skipped": payload.get("skipped", 0),
        "passed": payload.get("passed", 0),
        "failed": payload.get("failed", 0),
        "skipped_categories": payload.get("skipped_categories", {}),
        "gc": payload.get("gc", {}),
        "gc_drift": payload.get("gc_drift", {}),
    }


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parents[2]

    if not args.allow_dirty and is_repo_dirty(repo_root):
        print(
            "compat snapshot failed: repository has uncommitted changes (pass --allow-dirty to override)",
            file=sys.stderr,
        )
        return 1

    manifest_path = (repo_root / args.manifest).resolve()
    output_dir = (repo_root / args.output_dir).resolve()
    root_path = (repo_root / args.root).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest_path.parent.mkdir(parents=True, exist_ok=True)

    manifest = load_manifest(manifest_path)
    previous_baseline = latest_profile_json_path(manifest, "baseline", repo_root)
    previous_stress = latest_profile_json_path(manifest, "stress", repo_root)

    short_commit = run(["git", "rev-parse", "--short", "HEAD"], repo_root).strip()
    timestamp = dt.datetime.now(dt.UTC).replace(microsecond=0)
    timestamp_compact = timestamp.strftime("%Y%m%dT%H%M%SZ")
    timestamp_iso = timestamp.isoformat().replace("+00:00", "Z")
    run_id = f"phase{args.phase}-{args.milestone}-{timestamp_compact}-{short_commit}"

    run_dir = output_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=True)

    baseline_json = run_dir / "baseline.json"
    baseline_md = run_dir / "baseline.md"
    stress_json = run_dir / "stress.json"
    stress_md = run_dir / "stress.md"

    baseline_cmd = [
        "cargo",
        "run",
        "-p",
        "test-harness",
        "--bin",
        "test262-run",
        "--",
        "--root",
        str(root_path),
        "--profile",
        "baseline",
        "--allow-failures",
        "--show-gc",
        "--json",
        str(baseline_json),
        "--markdown",
        str(baseline_md),
    ]
    if previous_baseline is not None:
        baseline_cmd.extend(["--previous-summary", str(previous_baseline)])

    stress_cmd = [
        "cargo",
        "run",
        "-p",
        "test-harness",
        "--bin",
        "test262-run",
        "--",
        "--root",
        str(root_path),
        "--profile",
        "stress",
        "--allow-failures",
        "--auto-gc",
        "--auto-gc-threshold",
        "1",
        "--runtime-gc",
        "--runtime-gc-interval",
        "1",
        "--show-gc",
        "--expect-gc-baseline",
        str(repo_root / "crates/test-harness/fixtures/test262-lite/gc-guard.baseline"),
        "--json",
        str(stress_json),
        "--markdown",
        str(stress_md),
    ]
    if previous_stress is not None:
        stress_cmd.extend(["--previous-summary", str(previous_stress)])

    print(f"running compatibility snapshot: {run_id}")
    run(baseline_cmd, repo_root)
    run(stress_cmd, repo_root)

    baseline_payload = json.loads(baseline_json.read_text(encoding="utf-8"))
    stress_payload = json.loads(stress_json.read_text(encoding="utf-8"))

    entry = {
        "run_id": run_id,
        "phase": str(args.phase),
        "milestone": str(args.milestone),
        "commit": short_commit,
        "timestamp_utc": timestamp_iso,
        "profiles": {
            "baseline": {
                "command": " ".join(baseline_cmd),
                "json": repo_relative(baseline_json, repo_root),
                "markdown": repo_relative(baseline_md, repo_root),
                "summary": profile_summary(baseline_payload),
            },
            "stress": {
                "command": " ".join(stress_cmd),
                "json": repo_relative(stress_json, repo_root),
                "markdown": repo_relative(stress_md, repo_root),
                "summary": profile_summary(stress_payload),
            },
        },
    }

    manifest.setdefault("schema_version", 1)
    runs = manifest.setdefault("runs", [])
    if not isinstance(runs, list):
        raise RuntimeError("manifest runs field must be a list")
    runs.append(entry)
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    profile_failures: list[str] = []
    for profile_name, payload in (
        ("baseline", baseline_payload),
        ("stress", stress_payload),
    ):
        drift = payload.get("gc_drift", {})
        status = drift.get("status")
        anomaly_streak = drift.get("anomaly_streak")
        investigation_required = bool(drift.get("investigation_required", False))
        print(
            f"{profile_name}: status={status}, anomaly_streak={anomaly_streak}, investigation_required={investigation_required}"
        )
        if status == "blocking" or investigation_required:
            profile_failures.append(profile_name)

    if profile_failures:
        print(
            "compat snapshot failed due to blocking drift policy in profiles: "
            + ", ".join(profile_failures),
            file=sys.stderr,
        )
        return 1

    print(f"compat snapshot manifest updated: {repo_relative(manifest_path, repo_root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
