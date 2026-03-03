# CI SCRIPT KNOWLEDGE BASE

**Scope:** `.github/scripts/` governance, traceability, compatibility, and benchmark policy automation.

## OVERVIEW
These Python scripts are CI gatekeepers; they validate policy artifacts before or alongside Rust build/test steps.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Governance checklist/exception validation | `.github/scripts/validate_governance.py` | checks PR template + exception contract |
| Requirement traceability verification | `.github/scripts/check_verification_traceability.py` | validates `.planning` mapping outputs |
| Benchmark schema contract validation | `.github/scripts/check_engine_benchmark_contract.py` | validates `bench.v1` artifacts |
| Performance closure ratio policy | `.github/scripts/check_perf_target.py` | authoritative PERF-03 ratio gate |
| Compatibility snapshot generation | `.github/scripts/run_compat_snapshot.py` | writes manifest-backed snapshot artifacts |
| Current status drift sync/check | `.github/scripts/sync_current_status.py` | `docs/current-status.md` consistency guard |

## CONVENTIONS (CI SCRIPTS)
- Script outputs are part of CI evidence (`target/*.json`, `target/*.md`).
- Validation scripts support deterministic self-tests and fixture paths.
- Script argument names and defaults are policy surface; changes require doc + workflow sync.

## ANTI-PATTERNS (CI SCRIPTS)
- Skipping contract/traceability/governance scripts and relying only on `cargo test`.
- Softening benchmark comparator requirements without policy updates.
- Updating workflow calls without updating script docs/fixtures.
- Treating script failures as non-blocking when workflow defines them as hard gates.

## COMMANDS
```bash
python .github/scripts/validate_governance.py --exceptions .github/governance/exceptions.json --check-template .github/PULL_REQUEST_TEMPLATE.md --repo-root . --self-test
python .github/scripts/check_verification_traceability.py --requirements .planning/REQUIREMENTS.md --phases-dir .planning/phases --out-json target/verification-traceability.json --out-md target/verification-traceability.md
python .github/scripts/check_engine_benchmark_contract.py --self-test
python .github/scripts/check_perf_target.py --self-test
python .github/scripts/run_compat_snapshot.py --phase 07 --milestone v1.0 --manifest docs/compatibility/phase7-snapshots.json --output-dir target/compatibility --allow-dirty
python .github/scripts/sync_current_status.py --manifest docs/compatibility/phase7-snapshots.json --status-doc docs/current-status.md --mode check
```

## NOTES
- Keep fixture directories under `benchmark_contract/fixtures` and `verification_traceability/fixtures` in sync with script behavior.
- Workflow wiring lives in `.github/workflows/ci.yml`; script changes should be validated there.
