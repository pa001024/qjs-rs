#!/usr/bin/env python3
"""Validate benchmark JSON artifacts against the Phase 10 benchmark contract."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "bench.v1"
ALLOWED_PROFILES = {"local-dev", "ci-linux"}
ALLOWED_TIMING_MODES = {"eval-per-iteration"}
ALLOWED_ENGINE_STATUS = {"available", "missing", "unsupported"}
REQUIRED_ENGINES = ["qjs-rs", "boa-engine", "nodejs", "quickjs-c"]
REQUIRED_CASE_IDS = ["arith-loop", "fib-iterative", "array-sum", "json-roundtrip"]
REQUIRED_TOP_LEVEL_FIELDS = [
    "schema_version",
    "generated_at_utc",
    "run_profile",
    "timing_mode",
    "config",
    "reproducibility",
    "environment",
    "cases",
    "aggregate",
]


class ContractError(Exception):
    """Raised for deterministic contract checker failures."""


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


def _require_positive_int(value: Any, path: str, errors: list[str]) -> int:
    if not isinstance(value, int) or value <= 0:
        _append_error(errors, path, "must be a positive integer")
        return 0
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


def _validate_config(config: dict[str, Any], errors: list[str]) -> None:
    _require_positive_int(config.get("iterations"), "config.iterations", errors)
    _require_positive_int(config.get("samples"), "config.samples", errors)
    _require_positive_int(config.get("warmup_iterations"), "config.warmup_iterations", errors)


def _validate_engine_status(
    reproducibility: dict[str, Any], errors: list[str]
) -> dict[str, str]:
    engine_status_entries = _require_array(
        reproducibility.get("engine_status"), "reproducibility.engine_status", errors
    )
    status_by_engine: dict[str, str] = {}

    for idx, entry_value in enumerate(engine_status_entries):
        entry = _require_object(
            entry_value, f"reproducibility.engine_status[{idx}]", errors
        )
        engine = _require_string(
            entry.get("engine"), f"reproducibility.engine_status[{idx}].engine", errors
        )
        status = _require_string(
            entry.get("status"), f"reproducibility.engine_status[{idx}].status", errors
        )
        _require_string(
            entry.get("command"), f"reproducibility.engine_status[{idx}].command", errors
        )

        if engine:
            if engine in status_by_engine:
                _append_error(
                    errors,
                    f"reproducibility.engine_status[{idx}].engine",
                    f"duplicate engine metadata for '{engine}'",
                )
            status_by_engine[engine] = status

        if status and status not in ALLOWED_ENGINE_STATUS:
            _append_error(
                errors,
                f"reproducibility.engine_status[{idx}].status",
                f"must be one of {sorted(ALLOWED_ENGINE_STATUS)}",
            )

        if status in {"missing", "unsupported"}:
            _require_string(
                entry.get("reason"), f"reproducibility.engine_status[{idx}].reason", errors
            )

    missing_metadata = sorted(set(REQUIRED_ENGINES) - set(status_by_engine))
    if missing_metadata:
        _append_error(
            errors,
            "reproducibility.engine_status",
            f"missing required engine metadata entries: {', '.join(missing_metadata)}",
        )

    return status_by_engine


def _validate_reproducibility(
    reproducibility: dict[str, Any], errors: list[str]
) -> dict[str, str]:
    required_engines = _require_array(
        reproducibility.get("required_engines"), "reproducibility.required_engines", errors
    )
    required_engines_values = [
        _require_string(value, "reproducibility.required_engines[]", errors)
        for value in required_engines
    ]
    if sorted(required_engines_values) != sorted(REQUIRED_ENGINES):
        _append_error(
            errors,
            "reproducibility.required_engines",
            f"must exactly match required engine IDs: {REQUIRED_ENGINES}",
        )

    required_case_ids = _require_array(
        reproducibility.get("required_case_ids"), "reproducibility.required_case_ids", errors
    )
    required_case_ids_values = [
        _require_string(value, "reproducibility.required_case_ids[]", errors)
        for value in required_case_ids
    ]
    if sorted(required_case_ids_values) != sorted(REQUIRED_CASE_IDS):
        _append_error(
            errors,
            "reproducibility.required_case_ids",
            f"must exactly match required case IDs: {REQUIRED_CASE_IDS}",
        )

    output_policy = _require_object(
        reproducibility.get("output_policy"), "reproducibility.output_policy", errors
    )
    _require_string(
        output_policy.get("default_path"), "reproducibility.output_policy.default_path", errors
    )
    _require_string(
        output_policy.get("effective_path"), "reproducibility.output_policy.effective_path", errors
    )
    _require_bool(
        reproducibility.get("comparator_strict_mode"),
        "reproducibility.comparator_strict_mode",
        errors,
    )

    return _validate_engine_status(reproducibility, errors)


def _validate_environment(environment: dict[str, Any], errors: list[str]) -> None:
    _require_string(environment.get("os"), "environment.os", errors)
    _require_string(environment.get("arch"), "environment.arch", errors)
    _require_positive_int(environment.get("cpu_parallelism"), "environment.cpu_parallelism", errors)
    _require_string(environment.get("rustc"), "environment.rustc", errors)
    _require_string(environment.get("node"), "environment.node", errors)
    _require_string(environment.get("quickjs_c"), "environment.quickjs_c", errors)


def _validate_case_engine_result(case_id: str, engine: str, payload: Any, errors: list[str]) -> None:
    path = f"cases[{case_id}].engines[{engine}]"
    engine_result = _require_object(payload, path, errors)
    _require_array(engine_result.get("sample_ms"), f"{path}.sample_ms", errors)
    _require_number(engine_result.get("mean_ms"), f"{path}.mean_ms", errors)
    _require_number(engine_result.get("median_ms"), f"{path}.median_ms", errors)
    _require_number(engine_result.get("min_ms"), f"{path}.min_ms", errors)
    _require_number(engine_result.get("max_ms"), f"{path}.max_ms", errors)
    _require_number(engine_result.get("stddev_ms"), f"{path}.stddev_ms", errors)
    _require_number(
        engine_result.get("throughput_ops_per_sec"), f"{path}.throughput_ops_per_sec", errors
    )
    _require_string(engine_result.get("guard_checksum_mode"), f"{path}.guard_checksum_mode", errors)
    _require_number(
        engine_result.get("warmup_guard_checksum"), f"{path}.warmup_guard_checksum", errors
    )
    _require_array(
        engine_result.get("sample_guard_checksums"), f"{path}.sample_guard_checksums", errors
    )
    _require_number(engine_result.get("mean_guard_checksum"), f"{path}.mean_guard_checksum", errors)
    _require_bool(
        engine_result.get("guard_checksum_consistent"), f"{path}.guard_checksum_consistent", errors
    )


def _validate_cases(cases: list[Any], status_by_engine: dict[str, str], errors: list[str]) -> None:
    seen_case_ids: list[str] = []
    for idx, case_value in enumerate(cases):
        case = _require_object(case_value, f"cases[{idx}]", errors)
        case_id = _require_string(case.get("id"), f"cases[{idx}].id", errors)
        _require_string(case.get("title"), f"cases[{idx}].title", errors)
        _require_string(case.get("description"), f"cases[{idx}].description", errors)
        engines = _require_object(case.get("engines"), f"cases[{idx}].engines", errors)
        seen_case_ids.append(case_id)

        unknown_case_engines = sorted(set(engines.keys()) - set(REQUIRED_ENGINES))
        if unknown_case_engines:
            _append_error(
                errors,
                f"cases[{idx}].engines",
                f"contains unknown engine IDs: {', '.join(unknown_case_engines)}",
            )

        for required_engine in REQUIRED_ENGINES:
            status = status_by_engine.get(required_engine)
            has_metrics = required_engine in engines
            if status == "available" and not has_metrics:
                _append_error(
                    errors,
                    f"cases[{idx}].engines",
                    f"missing metrics for available engine '{required_engine}'",
                )
            if has_metrics and status in {"missing", "unsupported"}:
                _append_error(
                    errors,
                    f"cases[{idx}].engines",
                    f"engine '{required_engine}' has metrics but status is '{status}'",
                )
            if has_metrics:
                _validate_case_engine_result(case_id or str(idx), required_engine, engines[required_engine], errors)

    missing_case_ids = sorted(set(REQUIRED_CASE_IDS) - set(seen_case_ids))
    unexpected_case_ids = sorted(set(seen_case_ids) - set(REQUIRED_CASE_IDS))
    duplicate_case_ids = sorted(
        {case_id for case_id in seen_case_ids if case_id and seen_case_ids.count(case_id) > 1}
    )

    if missing_case_ids:
        _append_error(
            errors,
            "cases",
            f"missing required case IDs: {', '.join(missing_case_ids)}",
        )
    if unexpected_case_ids:
        _append_error(
            errors,
            "cases",
            f"unexpected case IDs for bench.v1: {', '.join(unexpected_case_ids)}",
        )
    if duplicate_case_ids:
        _append_error(
            errors,
            "cases",
            f"duplicate case IDs detected: {', '.join(duplicate_case_ids)}",
        )


def _validate_aggregate(aggregate: dict[str, Any], errors: list[str]) -> None:
    mean_map = _require_object(aggregate.get("mean_ms_per_engine"), "aggregate.mean_ms_per_engine", errors)
    relative_map = _require_object(
        aggregate.get("relative_to_qjs_rs"), "aggregate.relative_to_qjs_rs", errors
    )

    for engine in mean_map:
        if engine not in REQUIRED_ENGINES:
            _append_error(
                errors,
                "aggregate.mean_ms_per_engine",
                f"contains unknown engine '{engine}'",
            )
    for engine in relative_map:
        if engine not in REQUIRED_ENGINES:
            _append_error(
                errors,
                "aggregate.relative_to_qjs_rs",
                f"contains unknown engine '{engine}'",
            )


def validate_report(report: dict[str, Any]) -> ValidationResult:
    errors: list[str] = []

    for field in REQUIRED_TOP_LEVEL_FIELDS:
        if field not in report:
            _append_error(errors, "root", f"missing required field '{field}'")

    schema_version = _require_string(report.get("schema_version"), "schema_version", errors)
    if schema_version and schema_version != SCHEMA_VERSION:
        _append_error(errors, "schema_version", f"must be '{SCHEMA_VERSION}'")

    run_profile = _require_string(report.get("run_profile"), "run_profile", errors)
    if run_profile and run_profile not in ALLOWED_PROFILES:
        _append_error(errors, "run_profile", f"must be one of {sorted(ALLOWED_PROFILES)}")

    timing_mode = _require_string(report.get("timing_mode"), "timing_mode", errors)
    if timing_mode and timing_mode not in ALLOWED_TIMING_MODES:
        _append_error(
            errors,
            "timing_mode",
            f"must be one of {sorted(ALLOWED_TIMING_MODES)}",
        )

    _require_string(report.get("generated_at_utc"), "generated_at_utc", errors)
    config = _require_object(report.get("config"), "config", errors)
    _validate_config(config, errors)

    reproducibility = _require_object(report.get("reproducibility"), "reproducibility", errors)
    status_by_engine = _validate_reproducibility(reproducibility, errors)

    environment = _require_object(report.get("environment"), "environment", errors)
    _validate_environment(environment, errors)

    cases = _require_array(report.get("cases"), "cases", errors)
    _validate_cases(cases, status_by_engine, errors)

    aggregate = _require_object(report.get("aggregate"), "aggregate", errors)
    _validate_aggregate(aggregate, errors)

    status = "passed" if not errors else "failed"
    return ValidationResult(status=status, errors=sorted(errors))


def run_check(input_path: Path) -> ValidationResult:
    if not input_path.is_file():
        raise ContractError(f"missing benchmark artifact: {input_path.as_posix()}")
    report = json.loads(input_path.read_text(encoding="utf-8"))
    if not isinstance(report, dict):
        raise ContractError(f"{input_path.as_posix()}: top-level JSON must be an object")
    return validate_report(report)


def _expect_failure(result: ValidationResult, expected_fragment: str, scenario: str) -> None:
    if result.status != "failed":
        raise ContractError(f"self-test '{scenario}' expected failure but checker passed")
    if not any(expected_fragment in error for error in result.errors):
        raise ContractError(
            f"self-test '{scenario}' failed for an unexpected reason: {result.errors}"
        )


def run_self_test(script_root: Path, repo_root: Path) -> None:
    fixture_root = script_root / "benchmark_contract" / "fixtures"
    valid_fixture = fixture_root / "benchmark-report-valid.json"
    missing_case_fixture = fixture_root / "benchmark-report-missing-case.json"

    missing_fixtures = [path for path in [valid_fixture, missing_case_fixture] if not path.is_file()]
    if missing_fixtures:
        joined = ", ".join(path.as_posix() for path in missing_fixtures)
        raise ContractError(f"self-test fixture(s) missing: {joined}")

    temp_root = repo_root / "target" / "benchmark-contract-self-test"
    if temp_root.exists():
        shutil.rmtree(temp_root)
    temp_root.mkdir(parents=True, exist_ok=True)

    # Positive fixture.
    positive_copy = temp_root / "benchmark-report-valid.json"
    shutil.copyfile(valid_fixture, positive_copy)
    positive_result = run_check(positive_copy)
    if positive_result.status != "passed":
        raise ContractError(
            f"self-test 'positive-valid-fixture' expected pass but failed: {positive_result.errors}"
        )

    # Negative fixture: missing required case.
    missing_case_copy = temp_root / "benchmark-report-missing-case.json"
    shutil.copyfile(missing_case_fixture, missing_case_copy)
    missing_case_result = run_check(missing_case_copy)
    _expect_failure(
        missing_case_result,
        "missing required case IDs",
        "missing-required-case",
    )

    # Negative in-memory scenario: required_engines drift.
    drifted_payload = json.loads(valid_fixture.read_text(encoding="utf-8"))
    reproducibility = drifted_payload.get("reproducibility", {})
    if isinstance(reproducibility, dict):
        reproducibility["required_engines"] = ["qjs-rs", "boa-engine", "nodejs"]
    drifted_result = validate_report(drifted_payload)
    _expect_failure(
        drifted_result,
        "must exactly match required engine IDs",
        "required-engine-drift",
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Validate engine benchmark JSON artifacts against the bench.v1 contract "
            "before publishing baseline evidence."
        )
    )
    parser.add_argument(
        "--input",
        type=Path,
        default=Path("target/benchmarks/engine-comparison.ci-linux.json"),
        help="Path to benchmark artifact JSON to validate",
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
            run_self_test(script_root=Path(__file__).resolve().parent, repo_root=Path(".").resolve())
        except ContractError as exc:
            print(f"benchmark contract self-test failed: {exc}", file=sys.stderr)
            return 1
        print("benchmark contract self-test passed")
        return 0

    try:
        result = run_check(args.input)
    except (ContractError, json.JSONDecodeError) as exc:
        print(f"benchmark contract check failed: {exc}", file=sys.stderr)
        return 1

    if result.status != "passed":
        print("benchmark contract check failed", file=sys.stderr)
        for error in result.errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"benchmark contract check passed ({args.input.as_posix()})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
