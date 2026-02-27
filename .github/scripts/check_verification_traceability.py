#!/usr/bin/env python3
"""Validate verification frontmatter traceability coverage against REQUIREMENTS mapping."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

REQUIREMENT_ID_RE = re.compile(r"^[A-Z]{3}-\d{2}$")
PHASE_NUMBER_RE = re.compile(r"^\d{2}$")
REQUIRED_FIELDS = (
    "phase",
    "phase_number",
    "verified",
    "status",
    "score",
    "requirements_checked",
)
TABLE_ROW_RE = re.compile(r"^\|\s*([A-Z]{3}-\d{2})\s*\|\s*Phase\s+(\d+)\s*\|\s*([^|]+)\|")
FRONTMATTER_KEY_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$")


class TraceabilityError(Exception):
    """Raised for deterministic checker failures."""


@dataclass
class VerificationEntry:
    path: Path
    frontmatter: dict[str, Any]
    requirements_checked: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def phase(self) -> str:
        value = self.frontmatter.get("phase")
        return str(value) if value is not None else "<missing>"

    @property
    def phase_number(self) -> str:
        value = self.frontmatter.get("phase_number")
        return str(value) if value is not None else "<missing>"


def _strip_quotes(raw: str) -> str:
    value = raw.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def _parse_inline_list(value: str) -> list[str]:
    inner = value.strip()[1:-1].strip()
    if not inner:
        return []
    parts = [part.strip() for part in inner.split(",")]
    return [_strip_quotes(part) for part in parts]


def _extract_frontmatter(text: str, path: Path) -> str:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        raise TraceabilityError(f"{path}: missing YAML frontmatter opening delimiter")

    for index in range(1, len(lines)):
        if lines[index].strip() == "---":
            return "\n".join(lines[1:index])

    raise TraceabilityError(f"{path}: missing YAML frontmatter closing delimiter")


def _parse_frontmatter(block: str, path: Path) -> dict[str, Any]:
    data: dict[str, Any] = {}
    lines = block.splitlines()
    idx = 0

    while idx < len(lines):
        line = lines[idx]
        if not line.strip():
            idx += 1
            continue

        match = FRONTMATTER_KEY_RE.match(line.strip())
        if not match:
            raise TraceabilityError(f"{path}: unsupported frontmatter line {idx + 1}: {line!r}")

        key, raw_value = match.group(1), match.group(2)
        if raw_value is None:
            raw_value = ""

        value = raw_value.strip()
        if value == "":
            items: list[str] = []
            idx += 1
            while idx < len(lines):
                item_line = lines[idx]
                if not item_line.strip():
                    idx += 1
                    continue
                item_match = re.match(r"^\s*-\s+(.*)$", item_line)
                if item_match:
                    items.append(_strip_quotes(item_match.group(1).strip()))
                    idx += 1
                    continue
                break
            data[key] = items
            continue

        if value == "[]":
            data[key] = []
        elif value.startswith("[") and value.endswith("]"):
            data[key] = _parse_inline_list(value)
        else:
            data[key] = _strip_quotes(value)
        idx += 1

    return data


def parse_traceability_requirements(requirements_path: Path) -> dict[str, str]:
    try:
        content = requirements_path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise TraceabilityError(f"missing requirements file: {requirements_path}") from exc

    in_traceability = False
    mapping: dict[str, str] = {}

    for raw_line in content.splitlines():
        line = raw_line.rstrip()
        if line.startswith("## "):
            in_traceability = line.strip().lower() == "## traceability"
            continue

        if not in_traceability:
            continue

        row = TABLE_ROW_RE.match(line)
        if not row:
            continue

        req_id, phase_number_raw, _status = row.groups()
        phase_number = f"{int(phase_number_raw):02d}"

        if req_id in mapping:
            raise TraceabilityError(f"duplicate requirement id in traceability table: {req_id}")
        mapping[req_id] = phase_number

    if not mapping:
        raise TraceabilityError(
            f"{requirements_path}: no requirement mappings found in Traceability table"
        )

    return mapping


def discover_verification_files(phases_dir: Path) -> list[Path]:
    if not phases_dir.is_dir():
        raise TraceabilityError(f"missing phases directory: {phases_dir}")

    files = sorted(phases_dir.glob("*/*-VERIFICATION.md"))
    if not files:
        raise TraceabilityError(f"{phases_dir}: no verification files found")
    return files


def load_verification_entry(path: Path) -> VerificationEntry:
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise TraceabilityError(f"missing verification file: {path}") from exc

    block = _extract_frontmatter(text, path)
    frontmatter = _parse_frontmatter(block, path)

    entry = VerificationEntry(path=path, frontmatter=frontmatter)

    for key in REQUIRED_FIELDS:
        if key not in frontmatter:
            entry.errors.append(f"{path}: missing required frontmatter field '{key}'")

    phase_number = frontmatter.get("phase_number")
    if phase_number is not None and not PHASE_NUMBER_RE.match(str(phase_number)):
        entry.errors.append(
            f"{path}: phase_number must be a two-digit string, got {phase_number!r}"
        )

    requirements_raw = frontmatter.get("requirements_checked")
    if requirements_raw is None:
        requirements_list: list[str] = []
    elif isinstance(requirements_raw, list):
        requirements_list = [str(item) for item in requirements_raw]
    else:
        entry.errors.append(
            f"{path}: requirements_checked must be a list of requirement IDs"
        )
        requirements_list = []

    seen_in_file: set[str] = set()
    for req_id in requirements_list:
        if not REQUIREMENT_ID_RE.match(req_id):
            entry.errors.append(
                f"{path}: invalid requirement id format in requirements_checked: {req_id!r}"
            )
        if req_id in seen_in_file:
            entry.errors.append(f"{path}: duplicate requirement id in requirements_checked: {req_id}")
        seen_in_file.add(req_id)

    entry.requirements_checked = requirements_list
    return entry


def build_report(
    requirements_map: dict[str, str],
    entries: list[VerificationEntry],
    requirements_path: Path,
    phases_dir: Path,
) -> dict[str, Any]:
    errors: list[str] = []

    for entry in entries:
        errors.extend(entry.errors)

    canonical_ids = sorted(requirements_map)
    reported_by_requirement: dict[str, list[dict[str, str]]] = {}
    orphaned_set: set[str] = set()
    ownership_mismatches: list[dict[str, str]] = []

    for entry in entries:
        for req_id in entry.requirements_checked:
            if req_id not in requirements_map:
                orphaned_set.add(req_id)
                continue

            location = {
                "phase_number": entry.phase_number,
                "phase": entry.phase,
                "file": entry.path.as_posix(),
            }
            reported_by_requirement.setdefault(req_id, []).append(location)

            expected_phase = requirements_map[req_id]
            if entry.phase_number != expected_phase:
                ownership_mismatches.append(
                    {
                        "requirement": req_id,
                        "expected_phase_number": expected_phase,
                        "actual_phase_number": entry.phase_number,
                        "file": entry.path.as_posix(),
                    }
                )

    missing = [req_id for req_id in canonical_ids if req_id not in reported_by_requirement]

    duplicate_map: dict[str, list[dict[str, str]]] = {}
    for req_id, locations in sorted(reported_by_requirement.items()):
        if len(locations) > 1:
            unique_locations: list[dict[str, str]] = []
            seen: set[tuple[str, str, str]] = set()
            for location in locations:
                token = (
                    location["phase_number"],
                    location["phase"],
                    location["file"],
                )
                if token in seen:
                    continue
                seen.add(token)
                unique_locations.append(location)
            if len(unique_locations) > 1:
                duplicate_map[req_id] = unique_locations

    orphaned = sorted(orphaned_set)
    ownership_mismatches = sorted(
        ownership_mismatches,
        key=lambda item: (item["requirement"], item["file"]),
    )

    if missing:
        errors.append(
            "missing canonical requirement coverage: " + ", ".join(missing)
        )
    if orphaned:
        errors.append(
            "orphaned requirement mappings (not in REQUIREMENTS traceability): "
            + ", ".join(orphaned)
        )
    if duplicate_map:
        errors.append(
            "duplicate requirement mappings detected: "
            + ", ".join(sorted(duplicate_map.keys()))
        )
    if ownership_mismatches:
        mismatch_ids = sorted({item["requirement"] for item in ownership_mismatches})
        errors.append(
            "requirement mapped to non-canonical phase: " + ", ".join(mismatch_ids)
        )

    files_section = [
        {
            "file": entry.path.as_posix(),
            "phase": entry.phase,
            "phase_number": entry.phase_number,
            "requirements_checked": sorted(entry.requirements_checked),
        }
        for entry in sorted(entries, key=lambda item: item.path.as_posix())
    ]

    covered_count = len({req_id for req_id in reported_by_requirement if req_id in requirements_map})
    report = {
        "status": "failed" if errors else "passed",
        "inputs": {
            "requirements": requirements_path.as_posix(),
            "phases_dir": phases_dir.as_posix(),
        },
        "summary": {
            "verification_files": len(entries),
            "canonical_requirements": len(canonical_ids),
            "covered_requirements": covered_count,
            "missing_count": len(missing),
            "orphaned_count": len(orphaned),
            "duplicate_count": len(duplicate_map),
            "ownership_mismatch_count": len(ownership_mismatches),
            "error_count": len(errors),
        },
        "requirements": {
            "canonical": canonical_ids,
            "missing": missing,
            "orphaned": orphaned,
            "duplicates": duplicate_map,
            "ownership_mismatches": ownership_mismatches,
        },
        "files": files_section,
        "errors": errors,
    }
    return report


def write_markdown_report(report: dict[str, Any], output_path: Path) -> None:
    summary = report["summary"]
    requirements = report["requirements"]

    lines = [
        "# Verification Traceability Report",
        "",
        f"- Status: **{report['status']}**",
        f"- Verification files scanned: `{summary['verification_files']}`",
        f"- Canonical requirements: `{summary['canonical_requirements']}`",
        f"- Covered requirements: `{summary['covered_requirements']}`",
        f"- Missing mappings: `{summary['missing_count']}`",
        f"- Orphaned mappings: `{summary['orphaned_count']}`",
        f"- Duplicate mappings: `{summary['duplicate_count']}`",
        f"- Ownership mismatches: `{summary['ownership_mismatch_count']}`",
        "",
        "## Missing Canonical Requirement Coverage",
    ]

    missing = requirements["missing"]
    if missing:
        lines.extend([f"- `{req_id}`" for req_id in missing])
    else:
        lines.append("- None")

    lines.extend(["", "## Orphaned Requirement Mappings"])
    orphaned = requirements["orphaned"]
    if orphaned:
        lines.extend([f"- `{req_id}`" for req_id in orphaned])
    else:
        lines.append("- None")

    lines.extend(["", "## Duplicate Requirement Mappings"])
    duplicates: dict[str, list[dict[str, str]]] = requirements["duplicates"]
    if duplicates:
        for req_id, locations in duplicates.items():
            lines.append(f"- `{req_id}`")
            for location in locations:
                lines.append(
                    "  - "
                    f"phase `{location['phase_number']}` ({location['phase']}) -> `{location['file']}`"
                )
    else:
        lines.append("- None")

    lines.extend(["", "## Ownership Mismatches"])
    mismatches: list[dict[str, str]] = requirements["ownership_mismatches"]
    if mismatches:
        for mismatch in mismatches:
            lines.append(
                "- "
                f"`{mismatch['requirement']}` expected phase `{mismatch['expected_phase_number']}` "
                f"but found in phase `{mismatch['actual_phase_number']}` ({mismatch['file']})"
            )
    else:
        lines.append("- None")

    lines.extend(["", "## Errors"])
    if report["errors"]:
        lines.extend([f"- {message}" for message in report["errors"]])
    else:
        lines.append("- None")

    lines.extend(["", "## File Inventory", ""])
    lines.append("| File | Phase | Phase Number | Requirements Checked |")
    lines.append("| --- | --- | --- | --- |")
    for item in report["files"]:
        reqs = ", ".join(item["requirements_checked"]) if item["requirements_checked"] else "(none)"
        lines.append(
            f"| `{item['file']}` | `{item['phase']}` | `{item['phase_number']}` | `{reqs}` |"
        )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_outputs(report: dict[str, Any], json_path: Path, markdown_path: Path) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    write_markdown_report(report, markdown_path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Validate verification frontmatter schema and requirement traceability "
            "coverage against .planning/REQUIREMENTS.md"
        )
    )
    parser.add_argument(
        "--requirements",
        type=Path,
        help="Path to .planning/REQUIREMENTS.md",
    )
    parser.add_argument(
        "--phases-dir",
        type=Path,
        help="Path to .planning/phases directory",
    )
    parser.add_argument(
        "--out-json",
        type=Path,
        default=Path("target/verification-traceability.json"),
        help="Output path for machine-readable report JSON",
    )
    parser.add_argument(
        "--out-md",
        type=Path,
        default=Path("target/verification-traceability.md"),
        help="Output path for human-readable report markdown",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help=(
            "Run deterministic checker self-tests against repository fixtures without "
            "reading live .planning phase artifacts"
        ),
    )
    return parser.parse_args()


def run_check(requirements_path: Path, phases_dir: Path) -> dict[str, Any]:
    requirements_map = parse_traceability_requirements(requirements_path)
    verification_files = discover_verification_files(phases_dir)
    entries = [load_verification_entry(path) for path in verification_files]
    return build_report(requirements_map, entries, requirements_path, phases_dir)


def _copy_fixture_file(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(src, dst)


def _expect_failure(report: dict[str, Any], expected_error_fragment: str, scenario: str) -> None:
    if report["status"] != "failed":
        raise TraceabilityError(f"self-test '{scenario}' expected failure but checker passed")
    if not any(expected_error_fragment in error for error in report["errors"]):
        raise TraceabilityError(
            f"self-test '{scenario}' failed for an unexpected reason: {report['errors']}"
        )


def run_self_test(script_root: Path, repo_root: Path) -> None:
    fixture_root = script_root / "verification_traceability" / "fixtures"
    requirements_fixture = fixture_root / "requirements_traceability_sample.md"
    phase01_fixture = fixture_root / "phase01-verification-valid.md"
    phase03_fixture = fixture_root / "phase03-verification-missing-reqs.md"
    phase08_fixture = fixture_root / "phase08-verification-valid.md"

    required_fixtures = (
        requirements_fixture,
        phase01_fixture,
        phase03_fixture,
        phase08_fixture,
    )
    missing_fixtures = [path for path in required_fixtures if not path.is_file()]
    if missing_fixtures:
        joined = ", ".join(path.as_posix() for path in missing_fixtures)
        raise TraceabilityError(f"self-test fixture(s) missing: {joined}")

    temp_root = repo_root / "target" / "verification-traceability-self-test"
    if temp_root.exists():
        shutil.rmtree(temp_root)
    temp_root.mkdir(parents=True, exist_ok=True)

    # Positive scenario: valid canonical coverage.
    positive_phases = temp_root / "positive" / "phases"
    _copy_fixture_file(
        phase01_fixture,
        positive_phases / "01-semantic-core-closure" / "01-VERIFICATION.md",
    )
    _copy_fixture_file(
        phase08_fixture,
        positive_phases / "08-async-and-module-builtins-integration-closure" / "08-VERIFICATION.md",
    )
    positive_report = run_check(requirements_fixture, positive_phases)
    if positive_report["status"] != "passed":
        raise TraceabilityError(
            f"self-test 'positive' expected pass but failed: {positive_report['errors']}"
        )

    # Negative scenario: schema regression must catch missing requirements_checked.
    missing_field_phases = temp_root / "missing-field" / "phases"
    _copy_fixture_file(
        phase01_fixture,
        missing_field_phases / "01-semantic-core-closure" / "01-VERIFICATION.md",
    )
    _copy_fixture_file(
        phase03_fixture,
        missing_field_phases / "03-promise-job-queue-semantics" / "03-VERIFICATION.md",
    )
    _copy_fixture_file(
        phase08_fixture,
        missing_field_phases / "08-async-and-module-builtins-integration-closure" / "08-VERIFICATION.md",
    )
    missing_field_report = run_check(requirements_fixture, missing_field_phases)
    _expect_failure(
        missing_field_report,
        "missing required frontmatter field 'requirements_checked'",
        "missing-field",
    )

    # Negative scenario: canonical requirement coverage must fail when one requirement is unclaimed.
    missing_coverage_phases = temp_root / "missing-coverage" / "phases"
    _copy_fixture_file(
        phase01_fixture,
        missing_coverage_phases / "01-semantic-core-closure" / "01-VERIFICATION.md",
    )
    missing_coverage_report = run_check(requirements_fixture, missing_coverage_phases)
    _expect_failure(
        missing_coverage_report,
        "missing canonical requirement coverage",
        "missing-coverage",
    )


def main() -> int:
    args = parse_args()

    if args.self_test:
        try:
            run_self_test(script_root=Path(__file__).resolve().parent, repo_root=Path(".").resolve())
        except TraceabilityError as exc:
            print(f"verification traceability self-test failed: {exc}", file=sys.stderr)
            return 1
        print("verification traceability self-test passed")
        return 0

    if args.requirements is None or args.phases_dir is None:
        print(
            "verification traceability check failed: --requirements and --phases-dir are required "
            "unless --self-test is used",
            file=sys.stderr,
        )
        return 1

    try:
        report = run_check(args.requirements, args.phases_dir)
    except TraceabilityError as exc:
        print(f"verification traceability check failed: {exc}", file=sys.stderr)
        return 1

    write_outputs(report, args.out_json, args.out_md)

    if report["status"] != "passed":
        print("verification traceability check failed", file=sys.stderr)
        for message in report["errors"]:
            print(f"- {message}", file=sys.stderr)
        return 1

    print("verification traceability check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
