#!/usr/bin/env python3
"""Validate Phase 11 perf-target closure policy and baseline/candidate deltas."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "bench.v1"
AUTHORITATIVE_PROFILE = "local-dev"
AUTHORITATIVE_TIMING_MODE = "eval-per-iteration"
DEFAULT_REQUIRED_COMPARATORS = ["qjs-rs", "boa-engine"]
DEFAULT_OPTIONAL_COMPARATORS = ["quickjs-c", "nodejs"]


class PerfTargetError(Exception):
    """Raised when the perf-target checker encounters deterministic failures."""


@dataclass
class ValidationResult:
    status: str
    errors: list[str]


def _append_error(errors: list[str], path: str, message: str) -> None:
    errors.append(f"{path}: {message}")


def _require_object(value: Any, path: str, errors: list[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        _append_error(errors, path, "must be an object")
        return {}
    return value


def _require_array(value: Any, path: str, errors: list[str]) -> list[Any]:
    if not isinstance(value, list):
        _append_error(errors, path, "must be an array")
        return []
    return value


def _require_string(value: Any, path: str, errors: list[str]) -> str:
    if not isinstance(value, str) or not value.strip():
        _append_error(errors, path, "must be a non-empty string")
        return ""
    return value


def _require_number(value: Any, path: str, errors: list[str]) -> float:
    if not isinstance(value, (int, float)):
        _append_error(errors, path, "must be a number")
        return 0.0
    return float(value)


def _require_bool(value: Any, path: str, errors: list[str]) -> bool:
    if not isinstance(value, bool):
        _append_error(errors, path, "must be a boolean")
        return False
    return value


def _parse_case_limits(raw_items: list[str]) -> dict[str, float]:
    limits: dict[str, float] = {}
    for item in raw_items:
        if "=" not in item:
            raise PerfTargetError(
                f"--max-case-regression expects CASE=RATIO entries, got '{item}'"
            )
        case_id, ratio_text = item.split("=", 1)
        case_id = case_id.strip()
        if not case_id:
            raise PerfTargetError(f"invalid --max-case-regression entry '{item}' (empty case id)")
        try:
            ratio = float(ratio_text)
        except ValueError as exc:  # pragma: no cover - defensive parse branch
            raise PerfTargetError(
                f"invalid --max-case-regression ratio in '{item}'"
            ) from exc
        if ratio <= 0:
            raise PerfTargetError(
                f"invalid --max-case-regression ratio in '{item}' (must be > 0)"
            )
        limits[case_id] = ratio
    return limits


def _collect_engine_status(report: dict[str, Any], errors: list[str], prefix: str) -> dict[str, dict[str, Any]]:
    reproducibility = _require_object(
        report.get("reproducibility"), f"{prefix}.reproducibility", errors
    )
    status_entries = _require_array(
        reproducibility.get("engine_status"),
        f"{prefix}.reproducibility.engine_status",
        errors,
    )
    status_by_engine: dict[str, dict[str, Any]] = {}
    for idx, entry_value in enumerate(status_entries):
        entry = _require_object(
            entry_value, f"{prefix}.reproducibility.engine_status[{idx}]", errors
        )
        engine = _require_string(
            entry.get("engine"),
            f"{prefix}.reproducibility.engine_status[{idx}].engine",
            errors,
        )
        status = _require_string(
            entry.get("status"),
            f"{prefix}.reproducibility.engine_status[{idx}].status",
            errors,
        )
        if status in {"missing", "unsupported"}:
            _require_string(
                entry.get("reason"),
                f"{prefix}.reproducibility.engine_status[{idx}].reason",
                errors,
            )
        if engine:
            status_by_engine[engine] = entry
    return status_by_engine


def _collect_case_qjs_means(report: dict[str, Any], errors: list[str], prefix: str) -> dict[str, float]:
    cases = _require_array(report.get("cases"), f"{prefix}.cases", errors)
    means: dict[str, float] = {}
    for idx, case_value in enumerate(cases):
        case = _require_object(case_value, f"{prefix}.cases[{idx}]", errors)
        case_id = _require_string(case.get("id"), f"{prefix}.cases[{idx}].id", errors)
        engines = _require_object(case.get("engines"), f"{prefix}.cases[{idx}].engines", errors)
        qjs = _require_object(engines.get("qjs-rs"), f"{prefix}.cases[{idx}].engines.qjs-rs", errors)
        mean_ms = _require_number(qjs.get("mean_ms"), f"{prefix}.cases[{idx}].engines.qjs-rs.mean_ms", errors)
        if case_id:
            means[case_id] = mean_ms
    return means


def _require_aggregate_mean(report: dict[str, Any], engine: str, errors: list[str], prefix: str) -> float:
    aggregate = _require_object(report.get("aggregate"), f"{prefix}.aggregate", errors)
    mean_map = _require_object(
        aggregate.get("mean_ms_per_engine"), f"{prefix}.aggregate.mean_ms_per_engine", errors
    )
    return _require_number(
        mean_map.get(engine),
        f"{prefix}.aggregate.mean_ms_per_engine.{engine}",
        errors,
    )


def _validate_perf_target_metadata(report: dict[str, Any], errors: list[str], prefix: str) -> dict[str, Any]:
    metadata = _require_object(report.get("perf_target"), f"{prefix}.perf_target", errors)
    policy_id = _require_string(metadata.get("policy_id"), f"{prefix}.perf_target.policy_id", errors)
    profile = _require_string(
        metadata.get("authoritative_run_profile"),
        f"{prefix}.perf_target.authoritative_run_profile",
        errors,
    )
    timing = _require_string(
        metadata.get("authoritative_timing_mode"),
        f"{prefix}.perf_target.authoritative_timing_mode",
        errors,
    )
    _require_string(
        metadata.get("optimization_mode"),
        f"{prefix}.perf_target.optimization_mode",
        errors,
    )
    _require_string(
        metadata.get("optimization_tag"),
        f"{prefix}.perf_target.optimization_tag",
        errors,
    )
    _require_bool(
        metadata.get("same_host_required"),
        f"{prefix}.perf_target.same_host_required",
        errors,
    )
    _require_string(
        metadata.get("host_fingerprint"),
        f"{prefix}.perf_target.host_fingerprint",
        errors,
    )
    required_comparators = _require_array(
        metadata.get("required_comparators"),
        f"{prefix}.perf_target.required_comparators",
        errors,
    )
    optional_comparators = _require_array(
        metadata.get("optional_comparators"),
        f"{prefix}.perf_target.optional_comparators",
        errors,
    )
    required_values = [
        _require_string(value, f"{prefix}.perf_target.required_comparators[]", errors)
        for value in required_comparators
    ]
    optional_values = [
        _require_string(value, f"{prefix}.perf_target.optional_comparators[]", errors)
        for value in optional_comparators
    ]

    schema_version = _require_string(report.get("schema_version"), f"{prefix}.schema_version", errors)
    if schema_version and schema_version != SCHEMA_VERSION:
        _append_error(errors, f"{prefix}.schema_version", f"must be '{SCHEMA_VERSION}'")

    run_profile = _require_string(report.get("run_profile"), f"{prefix}.run_profile", errors)
    timing_mode = _require_string(report.get("timing_mode"), f"{prefix}.timing_mode", errors)

    if run_profile and run_profile != AUTHORITATIVE_PROFILE:
        _append_error(
            errors,
            f"{prefix}.run_profile",
            f"must be '{AUTHORITATIVE_PROFILE}' for PERF-03 closure checks",
        )
    if timing_mode and timing_mode != AUTHORITATIVE_TIMING_MODE:
        _append_error(
            errors,
            f"{prefix}.timing_mode",
            f"must be '{AUTHORITATIVE_TIMING_MODE}' for PERF-03 closure checks",
        )
    if profile and profile != AUTHORITATIVE_PROFILE:
        _append_error(
            errors,
            f"{prefix}.perf_target.authoritative_run_profile",
            f"must be '{AUTHORITATIVE_PROFILE}'",
        )
    if timing and timing != AUTHORITATIVE_TIMING_MODE:
        _append_error(
            errors,
            f"{prefix}.perf_target.authoritative_timing_mode",
            f"must be '{AUTHORITATIVE_TIMING_MODE}'",
        )
    if policy_id and not policy_id.startswith("phase11"):
        _append_error(
            errors,
            f"{prefix}.perf_target.policy_id",
            "must identify the phase11 closure policy",
        )

    required_sorted = sorted(set(required_values))
    if required_sorted != sorted(DEFAULT_REQUIRED_COMPARATORS):
        _append_error(
            errors,
            f"{prefix}.perf_target.required_comparators",
            f"must equal {DEFAULT_REQUIRED_COMPARATORS}",
        )
    optional_sorted = sorted(set(optional_values))
    if optional_sorted != sorted(DEFAULT_OPTIONAL_COMPARATORS):
        _append_error(
            errors,
            f"{prefix}.perf_target.optional_comparators",
            f"must equal {DEFAULT_OPTIONAL_COMPARATORS}",
        )

    return metadata


def _validate_comparator_policy(
    status_by_engine: dict[str, dict[str, Any]],
    required_comparators: list[str],
    optional_comparators: list[str],
    errors: list[str],
    prefix: str,
) -> None:
    for engine in required_comparators:
        entry = status_by_engine.get(engine)
        if entry is None:
            _append_error(
                errors,
                f"{prefix}.reproducibility.engine_status",
                f"missing metadata entry for required comparator '{engine}'",
            )
            continue
        status = entry.get("status")
        if status != "available":
            _append_error(
                errors,
                f"{prefix}.reproducibility.engine_status[{engine}]",
                f"required comparator must be available (found '{status}')",
            )

    for engine in optional_comparators:
        entry = status_by_engine.get(engine)
        if entry is None:
            _append_error(
                errors,
                f"{prefix}.reproducibility.engine_status",
                f"missing metadata entry for optional comparator '{engine}'",
            )
            continue
        status = entry.get("status")
        if status in {"missing", "unsupported"}:
            reason = entry.get("reason")
            if not isinstance(reason, str) or not reason.strip():
                _append_error(
                    errors,
                    f"{prefix}.reproducibility.engine_status[{engine}].reason",
                    "must be a non-empty string when comparator is unavailable",
                )


def validate_reports(
    baseline: dict[str, Any],
    candidate: dict[str, Any],
    *,
    require_qjs_lte_boa: bool,
    expect_case_improvement: list[str],
    max_case_regression: dict[str, float],
) -> ValidationResult:
    errors: list[str] = []

    baseline_metadata = _validate_perf_target_metadata(baseline, errors, "baseline")
    candidate_metadata = _validate_perf_target_metadata(candidate, errors, "candidate")

    baseline_status = _collect_engine_status(baseline, errors, "baseline")
    candidate_status = _collect_engine_status(candidate, errors, "candidate")

    required = baseline_metadata.get("required_comparators", DEFAULT_REQUIRED_COMPARATORS)
    optional = baseline_metadata.get("optional_comparators", DEFAULT_OPTIONAL_COMPARATORS)
    if not isinstance(required, list):
        required = DEFAULT_REQUIRED_COMPARATORS
    if not isinstance(optional, list):
        optional = DEFAULT_OPTIONAL_COMPARATORS
    required_values = [str(value) for value in required]
    optional_values = [str(value) for value in optional]

    _validate_comparator_policy(
        baseline_status,
        required_values,
        optional_values,
        errors,
        "baseline",
    )
    _validate_comparator_policy(
        candidate_status,
        required_values,
        optional_values,
        errors,
        "candidate",
    )

    baseline_policy_id = baseline_metadata.get("policy_id")
    candidate_policy_id = candidate_metadata.get("policy_id")
    if baseline_policy_id != candidate_policy_id:
        _append_error(
            errors,
            "perf_target.policy_id",
            "baseline/candidate policy IDs must match",
        )

    baseline_host = baseline_metadata.get("host_fingerprint")
    candidate_host = candidate_metadata.get("host_fingerprint")
    same_host_required = bool(
        baseline_metadata.get("same_host_required") or candidate_metadata.get("same_host_required")
    )
    if same_host_required and baseline_host != candidate_host:
        _append_error(
            errors,
            "perf_target.host_fingerprint",
            "baseline/candidate must be produced on the same host for closure claims",
        )

    baseline_qjs_agg = _require_aggregate_mean(baseline, "qjs-rs", errors, "baseline")
    candidate_qjs_agg = _require_aggregate_mean(candidate, "qjs-rs", errors, "candidate")
    candidate_boa_agg = _require_aggregate_mean(candidate, "boa-engine", errors, "candidate")

    baseline_case_means = _collect_case_qjs_means(baseline, errors, "baseline")
    candidate_case_means = _collect_case_qjs_means(candidate, errors, "candidate")

    if sorted(baseline_case_means.keys()) != sorted(candidate_case_means.keys()):
        _append_error(
            errors,
            "cases",
            "baseline/candidate case IDs must match for perf-target comparisons",
        )

    for case_id in expect_case_improvement:
        baseline_mean = baseline_case_means.get(case_id)
        candidate_mean = candidate_case_means.get(case_id)
        if baseline_mean is None or candidate_mean is None:
            _append_error(
                errors,
                f"cases[{case_id}]",
                "case missing in baseline or candidate for improvement check",
            )
            continue
        if candidate_mean >= baseline_mean:
            _append_error(
                errors,
                f"cases[{case_id}]",
                (
                    "expected qjs-rs improvement but candidate mean_ms "
                    f"{candidate_mean:.6f} >= baseline {baseline_mean:.6f}"
                ),
            )

    for case_id, ratio in max_case_regression.items():
        baseline_mean = baseline_case_means.get(case_id)
        candidate_mean = candidate_case_means.get(case_id)
        if baseline_mean is None or candidate_mean is None:
            _append_error(
                errors,
                f"cases[{case_id}]",
                "case missing in baseline or candidate for max regression check",
            )
            continue
        if baseline_mean <= 0:
            _append_error(
                errors,
                f"cases[{case_id}]",
                "baseline mean must be > 0 for regression ratio checks",
            )
            continue
        observed_ratio = candidate_mean / baseline_mean
        if observed_ratio > ratio:
            _append_error(
                errors,
                f"cases[{case_id}]",
                (
                    f"regression ratio {observed_ratio:.6f} exceeds allowed {ratio:.6f} "
                    f"(candidate={candidate_mean:.6f}, baseline={baseline_mean:.6f})"
                ),
            )

    if require_qjs_lte_boa and candidate_qjs_agg > candidate_boa_agg:
        _append_error(
            errors,
            "aggregate.mean_ms_per_engine",
            (
                "require-qjs-lte-boa failed: "
                f"candidate qjs-rs {candidate_qjs_agg:.6f} > boa-engine {candidate_boa_agg:.6f}"
            ),
        )

    # Always include this sanity condition: baseline must include qjs-rs aggregate.
    if baseline_qjs_agg <= 0:
        _append_error(
            errors,
            "baseline.aggregate.mean_ms_per_engine.qjs-rs",
            "must be > 0 for perf-target comparisons",
        )

    status = "passed" if not errors else "failed"
    return ValidationResult(status=status, errors=sorted(errors))


def _read_report(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise PerfTargetError(f"missing benchmark artifact: {path.as_posix()}")
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise PerfTargetError(f"{path.as_posix()}: top-level JSON must be an object")
    return payload


def run_check(
    *,
    baseline_path: Path,
    candidate_path: Path,
    require_qjs_lte_boa: bool,
    expect_case_improvement: list[str],
    max_case_regression: dict[str, float],
) -> ValidationResult:
    baseline = _read_report(baseline_path)
    candidate = _read_report(candidate_path)
    return validate_reports(
        baseline,
        candidate,
        require_qjs_lte_boa=require_qjs_lte_boa,
        expect_case_improvement=expect_case_improvement,
        max_case_regression=max_case_regression,
    )


def _expect_failure(result: ValidationResult, expected_fragment: str, scenario: str) -> None:
    if result.status != "failed":
        raise PerfTargetError(f"self-test '{scenario}' expected failure but checker passed")
    if not any(expected_fragment in error for error in result.errors):
        raise PerfTargetError(
            f"self-test '{scenario}' failed for an unexpected reason: {result.errors}"
        )


def _fixture_report(
    *,
    host_fingerprint: str,
    optimization_mode: str,
    optimization_tag: str,
    packet_id: str | None,
    qjs_agg: float,
    boa_agg: float,
    case_means: dict[str, float],
    quickjs_status: str = "missing",
    quickjs_reason: str = "quickjs-c not installed",
    node_status: str = "available",
    node_reason: str | None = None,
) -> dict[str, Any]:
    case_ids = ["arith-loop", "fib-iterative", "array-sum", "json-roundtrip"]
    cases = []
    for case_id in case_ids:
        qjs_mean = case_means.get(case_id, 1.0)
        cases.append(
            {
                "id": case_id,
                "title": case_id,
                "description": case_id,
                "engines": {
                    "qjs-rs": {"mean_ms": qjs_mean},
                    "boa-engine": {"mean_ms": max(qjs_mean / 2.0, 0.001)},
                },
            }
        )

    quickjs_entry = {
        "engine": "quickjs-c",
        "status": quickjs_status,
        "command": "qjs",
        "reason": quickjs_reason if quickjs_status in {"missing", "unsupported"} else None,
    }
    node_entry = {
        "engine": "nodejs",
        "status": node_status,
        "command": "node",
        "reason": node_reason if node_status in {"missing", "unsupported"} else None,
    }
    engine_status = [
        {
            "engine": "qjs-rs",
            "status": "available",
            "command": "in-process",
        },
        {
            "engine": "boa-engine",
            "status": "available",
            "command": "in-process",
        },
        node_entry,
        quickjs_entry,
    ]

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at_utc": "2026-02-28T00:00:00Z",
        "run_profile": AUTHORITATIVE_PROFILE,
        "timing_mode": AUTHORITATIVE_TIMING_MODE,
        "perf_target": {
            "policy_id": "phase11-perf03-local-dev-eval-per-iteration",
            "authoritative_run_profile": AUTHORITATIVE_PROFILE,
            "authoritative_timing_mode": AUTHORITATIVE_TIMING_MODE,
            "same_host_required": True,
            "host_fingerprint": host_fingerprint,
            "optimization_mode": optimization_mode,
            "optimization_tag": optimization_tag,
            "packet_id": packet_id,
            "required_comparators": DEFAULT_REQUIRED_COMPARATORS,
            "optional_comparators": DEFAULT_OPTIONAL_COMPARATORS,
        },
        "reproducibility": {
            "engine_status": engine_status,
        },
        "cases": cases,
        "aggregate": {
            "mean_ms_per_engine": {
                "qjs-rs": qjs_agg,
                "boa-engine": boa_agg,
            }
        },
    }


def run_self_test(repo_root: Path) -> None:
    temp_root = repo_root / "target" / "perf-target-self-test"
    if temp_root.exists():
        shutil.rmtree(temp_root)
    temp_root.mkdir(parents=True, exist_ok=True)

    baseline_fixture = _fixture_report(
        host_fingerprint="host-a",
        optimization_mode="baseline",
        optimization_tag="phase11-baseline",
        packet_id=None,
        qjs_agg=120.0,
        boa_agg=110.0,
        case_means={
            "arith-loop": 200.0,
            "fib-iterative": 100.0,
            "array-sum": 300.0,
            "json-roundtrip": 20.0,
        },
    )
    candidate_fixture = _fixture_report(
        host_fingerprint="host-a",
        optimization_mode="packet",
        optimization_tag="packet-a",
        packet_id="packet-a",
        qjs_agg=95.0,
        boa_agg=110.0,
        case_means={
            "arith-loop": 140.0,
            "fib-iterative": 70.0,
            "array-sum": 240.0,
            "json-roundtrip": 21.0,
        },
    )

    baseline_path = temp_root / "baseline.json"
    candidate_path = temp_root / "candidate.json"
    baseline_path.write_text(json.dumps(baseline_fixture, indent=2), encoding="utf-8")
    candidate_path.write_text(json.dumps(candidate_fixture, indent=2), encoding="utf-8")

    positive = run_check(
        baseline_path=baseline_path,
        candidate_path=candidate_path,
        require_qjs_lte_boa=True,
        expect_case_improvement=["arith-loop", "fib-iterative"],
        max_case_regression={"json-roundtrip": 1.10},
    )
    if positive.status != "passed":
        raise PerfTargetError(
            f"self-test 'positive' expected pass but failed: {positive.errors}"
        )

    # Negative scenario: same-host rule violated.
    different_host = dict(candidate_fixture)
    different_host["perf_target"] = dict(candidate_fixture["perf_target"])
    different_host["perf_target"]["host_fingerprint"] = "host-b"
    different_host_path = temp_root / "candidate-different-host.json"
    different_host_path.write_text(json.dumps(different_host, indent=2), encoding="utf-8")
    different_host_result = run_check(
        baseline_path=baseline_path,
        candidate_path=different_host_path,
        require_qjs_lte_boa=False,
        expect_case_improvement=[],
        max_case_regression={},
    )
    _expect_failure(
        different_host_result,
        "must be produced on the same host",
        "same-host-policy",
    )

    # Negative scenario: qjs-rs slower than boa when closure flag required.
    slower_candidate = dict(candidate_fixture)
    slower_candidate["aggregate"] = {
        "mean_ms_per_engine": {"qjs-rs": 150.0, "boa-engine": 110.0}
    }
    slower_path = temp_root / "candidate-slower-than-boa.json"
    slower_path.write_text(json.dumps(slower_candidate, indent=2), encoding="utf-8")
    slower_result = run_check(
        baseline_path=baseline_path,
        candidate_path=slower_path,
        require_qjs_lte_boa=True,
        expect_case_improvement=[],
        max_case_regression={},
    )
    _expect_failure(
        slower_result,
        "require-qjs-lte-boa failed",
        "qjs-vs-boa-closure",
    )

    # Negative scenario: unavailable optional comparator missing reason metadata.
    missing_reason = _fixture_report(
        host_fingerprint="host-a",
        optimization_mode="packet",
        optimization_tag="packet-a",
        packet_id="packet-a",
        qjs_agg=95.0,
        boa_agg=110.0,
        case_means={"arith-loop": 140.0},
        quickjs_status="missing",
        quickjs_reason="",
    )
    missing_reason_path = temp_root / "candidate-missing-reason.json"
    missing_reason_path.write_text(json.dumps(missing_reason, indent=2), encoding="utf-8")
    missing_reason_result = run_check(
        baseline_path=baseline_path,
        candidate_path=missing_reason_path,
        require_qjs_lte_boa=False,
        expect_case_improvement=[],
        max_case_regression={},
    )
    _expect_failure(
        missing_reason_result,
        "reason: must be a non-empty string",
        "missing-comparator-reason",
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Validate Phase 11 perf-target baseline/candidate artifacts against the "
            "authoritative closure policy."
        )
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=Path("target/benchmarks/engine-comparison.local-dev.phase11-baseline.json"),
        help="Path to baseline benchmark artifact JSON",
    )
    parser.add_argument(
        "--candidate",
        type=Path,
        default=Path("target/benchmarks/engine-comparison.local-dev.packet-b.json"),
        help="Path to candidate benchmark artifact JSON",
    )
    parser.add_argument(
        "--require-qjs-lte-boa",
        action="store_true",
        help="Fail if candidate aggregate qjs-rs mean_ms is greater than boa-engine",
    )
    parser.add_argument(
        "--expect-case-improvement",
        action="append",
        default=[],
        help="Require candidate qjs-rs mean_ms to improve over baseline for CASE_ID",
    )
    parser.add_argument(
        "--max-case-regression",
        action="append",
        default=[],
        help="Allow at most CASE_ID=RATIO regression (candidate/baseline) for listed cases",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run deterministic fixture-backed self-tests",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    if args.self_test:
        try:
            run_self_test(repo_root=Path(".").resolve())
        except (PerfTargetError, json.JSONDecodeError) as exc:
            print(f"perf target self-test failed: {exc}", file=sys.stderr)
            return 1
        print("perf target self-test passed")
        return 0

    try:
        max_case_regression = _parse_case_limits(args.max_case_regression)
        result = run_check(
            baseline_path=args.baseline,
            candidate_path=args.candidate,
            require_qjs_lte_boa=args.require_qjs_lte_boa,
            expect_case_improvement=args.expect_case_improvement,
            max_case_regression=max_case_regression,
        )
    except (PerfTargetError, json.JSONDecodeError) as exc:
        print(f"perf target check failed: {exc}", file=sys.stderr)
        return 1

    if result.status != "passed":
        print("perf target check failed", file=sys.stderr)
        for error in result.errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        "perf target check passed "
        f"(baseline={args.baseline.as_posix()}, candidate={args.candidate.as_posix()})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
