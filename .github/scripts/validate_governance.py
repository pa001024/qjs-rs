#!/usr/bin/env python3
"""Validate repository governance contracts and pull-request checklist payloads."""

from __future__ import annotations

import argparse
import copy
import datetime as dt
import json
import re
import sys
from pathlib import Path
from typing import Any

REQUIRED_EXCEPTION_FIELDS = (
    "id",
    "reason",
    "impact_scope",
    "owner",
    "expires_at",
    "rollback_condition",
)

TEMPLATE_TOKENS = (
    "Runtime-observable behavior changed",
    "Refactor-only (no semantic change)",
    "Positive test reference:",
    "Boundary/error test reference:",
    "Refactor-only evidence:",
    "Exception record id:",
)

RUNTIME_LABEL = "Runtime-observable behavior changed"
REFACTOR_LABEL = "Refactor-only (no semantic change)"
POSITIVE_FIELD = "Positive test reference:"
BOUNDARY_FIELD = "Boundary/error test reference:"
EVIDENCE_FIELD = "Refactor-only evidence:"
EXCEPTION_FIELD = "Exception record id:"


class ValidationError(Exception):
    """Raised when governance checks fail."""


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValidationError(f"missing file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValidationError(f"invalid JSON in {path}: {exc}") from exc


def _parse_iso_date(raw: str, field_name: str) -> dt.date:
    try:
        return dt.date.fromisoformat(raw)
    except ValueError as exc:
        raise ValidationError(f"{field_name} must use YYYY-MM-DD format, got {raw!r}") from exc


def _normalize_markdown_value(raw: str) -> str:
    value = raw.strip()
    if value.startswith("`") and value.endswith("`") and len(value) >= 2:
        value = value[1:-1].strip()
    return value


def _extract_field(body: str, field_label: str) -> str | None:
    pattern = re.compile(rf"{re.escape(field_label)}\s*(.*)$", re.IGNORECASE | re.MULTILINE)
    match = pattern.search(body)
    if not match:
        return None
    value = _normalize_markdown_value(match.group(1))
    if value.lower() in {"", "n/a", "none", "-", "null"}:
        return None
    return value


def _checkbox_checked(body: str, label: str) -> bool:
    pattern = re.compile(
        rf"^\s*-\s*\[(?P<mark>[xX ])\]\s*{re.escape(label)}\s*$",
        re.MULTILINE,
    )
    match = pattern.search(body)
    if not match:
        raise ValidationError(f"missing checklist item in PR body: {label}")
    return match.group("mark").lower() == "x"


def _extract_reference_path(reference: str) -> str:
    token = reference.strip().strip("\"'").replace("\\", "/")
    if "::" in token:
        token = token.split("::", 1)[0]
    if "#" in token:
        token = token.split("#", 1)[0]
    line_ref = re.match(r"^(.+?):\d+(?::\d+)?$", token)
    if line_ref:
        token = line_ref.group(1)
    return token.lstrip("./")


def _validate_reference_exists(reference: str, repo_root: Path, field_name: str) -> None:
    relative_path = _extract_reference_path(reference)
    candidate = repo_root / relative_path
    if not candidate.is_file():
        raise ValidationError(
            f"{field_name} points to missing file: {relative_path!r} (resolved from {reference!r})"
        )


def validate_exception_file(path: Path, today: dt.date) -> dict[str, dict[str, Any]]:
    payload = _read_json(path)
    if not isinstance(payload, dict):
        raise ValidationError(f"{path} must be a JSON object with an 'exceptions' array")
    records = payload.get("exceptions")
    if not isinstance(records, list):
        raise ValidationError(f"{path} must define an 'exceptions' array")

    by_id: dict[str, dict[str, Any]] = {}
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            raise ValidationError(f"exceptions[{index}] must be an object")
        for field in REQUIRED_EXCEPTION_FIELDS:
            value = record.get(field)
            if not isinstance(value, str) or not value.strip():
                raise ValidationError(f"exceptions[{index}].{field} must be a non-empty string")
        record_id = record["id"].strip()
        if record_id in by_id:
            raise ValidationError(f"duplicate exception id: {record_id}")
        expires = _parse_iso_date(record["expires_at"].strip(), f"exceptions[{index}].expires_at")
        if expires < today:
            raise ValidationError(f"exception {record_id} is expired on {expires.isoformat()}")
        by_id[record_id] = record
    return by_id


def validate_template(path: Path) -> None:
    content = path.read_text(encoding="utf-8")
    missing = [token for token in TEMPLATE_TOKENS if token not in content]
    if missing:
        joined = ", ".join(missing)
        raise ValidationError(f"template {path} missing required governance fields: {joined}")


def _extract_pr_body(event_payload: Any, event_path: Path) -> str:
    if not isinstance(event_payload, dict):
        raise ValidationError(f"{event_path} must be a JSON object")
    pull_request = event_payload.get("pull_request")
    if not isinstance(pull_request, dict):
        raise ValidationError(f"{event_path} missing 'pull_request' object")
    body = pull_request.get("body")
    if not isinstance(body, str) or not body.strip():
        raise ValidationError(f"{event_path} pull_request.body is required")
    return body


def validate_pr_event_payload(
    event_payload: Any,
    event_path: Path,
    repo_root: Path,
    require_reference_exists: bool,
    exceptions_by_id: dict[str, dict[str, Any]],
) -> None:
    body = _extract_pr_body(event_payload, event_path)
    runtime_checked = _checkbox_checked(body, RUNTIME_LABEL)
    refactor_checked = _checkbox_checked(body, REFACTOR_LABEL)

    if runtime_checked == refactor_checked:
        raise ValidationError(
            "exactly one checklist mode must be selected: runtime-observable OR refactor-only"
        )

    positive_reference = _extract_field(body, POSITIVE_FIELD)
    boundary_reference = _extract_field(body, BOUNDARY_FIELD)
    evidence = _extract_field(body, EVIDENCE_FIELD)
    exception_id = _extract_field(body, EXCEPTION_FIELD)

    if runtime_checked:
        if not positive_reference:
            raise ValidationError("runtime-observable PRs require a positive test reference")
        if not boundary_reference:
            raise ValidationError("runtime-observable PRs require a boundary/error test reference")
        if require_reference_exists:
            _validate_reference_exists(positive_reference, repo_root, POSITIVE_FIELD.rstrip(":"))
            _validate_reference_exists(boundary_reference, repo_root, BOUNDARY_FIELD.rstrip(":"))
    else:
        if not exception_id:
            raise ValidationError("refactor-only PRs require an exception record id")
        if exception_id not in exceptions_by_id:
            raise ValidationError(f"refactor-only exception id not found: {exception_id}")
        if not evidence:
            raise ValidationError("refactor-only PRs require no-semantic-change evidence")


def _self_test_runtime_missing_boundary(
    runtime_fixture: Any,
    runtime_path: Path,
    repo_root: Path,
    exceptions_by_id: dict[str, dict[str, Any]],
) -> None:
    mutated = copy.deepcopy(runtime_fixture)
    body = _extract_pr_body(mutated, runtime_path)
    body = re.sub(
        rf"{re.escape(BOUNDARY_FIELD)}.*",
        f"{BOUNDARY_FIELD} N/A",
        body,
        count=1,
        flags=re.IGNORECASE,
    )
    mutated["pull_request"]["body"] = body
    try:
        validate_pr_event_payload(
            mutated,
            runtime_path,
            repo_root,
            require_reference_exists=True,
            exceptions_by_id=exceptions_by_id,
        )
    except ValidationError:
        return
    raise ValidationError("self-test expected missing boundary reference to fail")


def run_self_test(
    script_root: Path,
    repo_root: Path,
    exceptions_by_id: dict[str, dict[str, Any]],
) -> None:
    fixture_dir = script_root / "governance" / "fixtures"
    runtime_path = fixture_dir / "pr_event_runtime_change.json"
    refactor_path = fixture_dir / "pr_event_refactor_only.json"

    runtime_fixture = _read_json(runtime_path)
    refactor_fixture = _read_json(refactor_path)

    validate_pr_event_payload(
        runtime_fixture,
        runtime_path,
        repo_root,
        require_reference_exists=True,
        exceptions_by_id=exceptions_by_id,
    )
    validate_pr_event_payload(
        refactor_fixture,
        refactor_path,
        repo_root,
        require_reference_exists=False,
        exceptions_by_id=exceptions_by_id,
    )
    _self_test_runtime_missing_boundary(runtime_fixture, runtime_path, repo_root, exceptions_by_id)

    today = dt.date.today()
    expired_payload = {
        "exceptions": [
            {
                "id": "expired-self-test",
                "reason": "self-test",
                "impact_scope": "self-test",
                "owner": "self-test",
                "expires_at": "2000-01-01",
                "rollback_condition": "self-test",
            }
        ]
    }
    temp_path = repo_root / "target" / "governance-expired-self-test.json"
    temp_path.parent.mkdir(parents=True, exist_ok=True)
    temp_path.write_text(json.dumps(expired_payload), encoding="utf-8")
    try:
        try:
            validate_exception_file(temp_path, today)
        except ValidationError:
            return
        raise ValidationError("self-test expected expired exception validation failure")
    finally:
        temp_path.unlink(missing_ok=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate governance policy contracts.")
    parser.add_argument(
        "--exceptions",
        type=Path,
        required=True,
        help="Path to governance exception records JSON.",
    )
    parser.add_argument(
        "--check-template",
        type=Path,
        help="Path to PR template that must expose governance checklist fields.",
    )
    parser.add_argument(
        "--validate-pr-event",
        type=Path,
        help="Path to a GitHub pull_request event JSON payload.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path("."),
        help="Repository root used for test reference existence checks.",
    )
    parser.add_argument(
        "--require-test-reference-exists",
        action="store_true",
        help="Require positive and boundary references to point to real files under repo root.",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run deterministic validator self-tests using governance fixtures.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    today = dt.date.today()
    repo_root = args.repo_root.resolve()
    script_root = Path(__file__).resolve().parent.parent

    try:
        exceptions_by_id = validate_exception_file(args.exceptions, today)
        if args.check_template:
            validate_template(args.check_template)

        if args.validate_pr_event:
            event_payload = _read_json(args.validate_pr_event)
            validate_pr_event_payload(
                event_payload,
                args.validate_pr_event,
                repo_root=repo_root,
                require_reference_exists=args.require_test_reference_exists,
                exceptions_by_id=exceptions_by_id,
            )

        if args.self_test:
            run_self_test(script_root, repo_root, exceptions_by_id)
    except ValidationError as exc:
        print(f"governance validation failed: {exc}", file=sys.stderr)
        return 1

    print("governance validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
