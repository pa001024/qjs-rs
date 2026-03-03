# TEST HARNESS KNOWLEDGE BASE

**Scope:** `crates/test-harness/` test262-lite suite engine, fixtures, and CLI runner.

## OVERVIEW
`test-harness` is the compatibility execution layer: it drives script/module evaluation, test262-lite discovery, suite accounting, and GC drift guardrails.

## STRUCTURE
```text
crates/test-harness/
├── src/lib.rs              # run_expression/run_script/run_module_entry APIs
├── src/test262.rs          # suite walker, frontmatter, summary/gc stats
├── src/bin/test262-run.rs  # CLI profiles + gc expectation checks
├── tests/                  # harness integration suites
└── fixtures/test262-lite/  # pass/fail cases + gc baseline
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Programmatic harness entrypoints | `crates/test-harness/src/lib.rs` | compile+execute helpers with globals/modules |
| Suite execution behavior | `crates/test-harness/src/test262.rs` | `SuiteOptions`, `SuiteSummary`, gc aggregates |
| CLI options and drift policy | `crates/test-harness/src/bin/test262-run.rs` | baseline/stress + expectation parsing |
| Compatibility fixtures | `crates/test-harness/fixtures/test262-lite` | behavior asset tree |
| End-to-end suite checks | `crates/test-harness/tests/test262_lite.rs` | subset and profile assertions |

## CONVENTIONS (HARNESS)
- Test names are scenario-driven (`*_subset`, `*_semantics`, `module_*`, `native_errors`).
- Fixture frontmatter drives expected phase (`parse`/`runtime`) more than folder naming.
- GC guard checks rely on explicit counters and optional baseline file thresholds.
- CLI and library flows should remain behaviorally aligned for suite reporting.

## ANTI-PATTERNS (HARNESS)
- Editing fixtures without corresponding expectation/test updates.
- Treating `--allow-failures` output as closure evidence.
- Skipping GC expectation balance checks (`collections_total` vs runtime+boundary).
- Diverging CLI-only behavior from `run_suite` library semantics.

## COMMANDS
```bash
cargo test -p test-harness
cargo test -p test-harness --test test262_lite

cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite
cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc
```

## NOTES
- `fixtures/test262-lite` is a high-signal compatibility boundary; preserve stable intent and provenance.
- Coordinate changes with `.github/workflows/ci.yml` phase gate commands.
