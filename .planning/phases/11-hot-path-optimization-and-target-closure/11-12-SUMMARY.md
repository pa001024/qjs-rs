---
phase: 11-hot-path-optimization-and-target-closure
plan: 12
subsystem: runtime
tags: [perf, vm, benchmarks, packet-g, verification]
requires:
  - phase: 11-hot-path-optimization-and-target-closure
    provides: packet-final baseline/evidence state and active quickjs-ratio closure gate
provides:
  - packet-g guarded identifier-resolution fallback-reduction path
  - packet-g parity/hotspot counter coverage
  - authoritative packet-g benchmark artifact and closure verdict sync
affects: [phase-11-verification, requirements-traceability, roadmap-progress, phase-12-gates]
tech-stack:
  added: []
  patterns: [guarded-fast-path-with-canonical-fallback, authoritative-benchmark-evidence-sync]
key-files:
  created:
    - .planning/phases/11-hot-path-optimization-and-target-closure/11-12-SUMMARY.md
  modified:
    - crates/vm/src/lib.rs
    - crates/vm/src/fast_path.rs
    - crates/vm/tests/perf_hotspot_attribution.rs
    - crates/vm/tests/perf_packet_d.rs
    - crates/benchmarks/src/contract.rs
    - crates/benchmarks/src/main.rs
    - crates/benchmarks/tests/hot_path_contract.rs
    - docs/engine-benchmarks.md
    - .planning/phases/11-hot-path-optimization-and-target-closure/11-TARGET-CLOSURE-EVIDENCE.md
    - .planning/phases/11-hot-path-optimization-and-target-closure/11-VERIFICATION.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
key-decisions:
  - "Packet-g path uses scope-generation guarded name-cache + revalidate + canonical slow fallback, with no semantic bypass."
  - "Benchmark timed samples keep packet metrics disabled; packet taxonomy counters are enabled only in the attribution rerun."
  - "Phase 11 remains open because packet-g authoritative ratio failed gate (6.236987x > 1.25x)."
patterns-established:
  - "Packet evidence updates must include artifact hash, checker transcript, aggregate means, and explicit PASS/FAIL gate wording."
  - "Hotspot taxonomy fields can expand contract payload without changing bench.v1 required envelope checks."
requirements-completed: [PERF-03, PERF-04, PERF-05]
duration: 2h 09m
completed: 2026-03-03
---

# Phase 11 Plan 12: Packet-G Closure Attempt Summary

**Packet-g identifier-resolution guard path landed with deterministic parity/taxonomy coverage, but authoritative quickjs-ratio gate remained red (`6.236987x > 1.25x`).**

## Performance

- **Duration:** 2h 09m
- **Started:** 2026-03-03T18:52:00+08:00
- **Completed:** 2026-03-03T21:01:00+08:00
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments

- Added packet hotspot taxonomy assertions and fallback-scan attribution coverage in VM hotspot tests.
- Implemented packet-g guarded identifier name-cache path with generation-based revalidation and canonical fallback.
- Executed authoritative packet-g benchmark workflow (`local-dev`, strict comparators), then synchronized evidence/verification/requirements/roadmap to the exact checker outcome.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add packet-g hotspot miss taxonomy before code-path changes** - `029ed5c` (`perf`)
2. **Task 2: Implement packet-g guarded identifier fallback-reduction path** - `9aec2c0` (`perf`)
3. **Task 3: Run authoritative packet-g closure workflow and publish verdict** - `36c1b52` (`perf`)

## Files Created/Modified

- `crates/vm/src/lib.rs` - packet-g guard/revalidate/fallback integration and hotspot taxonomy recording
- `crates/vm/src/fast_path.rs` - remove unused packet-g helper to keep clippy clean
- `crates/vm/tests/perf_hotspot_attribution.rs` - deterministic packet taxonomy + workload-shape hotspot tests
- `crates/vm/tests/perf_packet_d.rs` - packet-g toggle parity/counter/invalidation coverage
- `crates/benchmarks/src/contract.rs` - extend hotspot counter payload for packet-d/packet-g taxonomy
- `crates/benchmarks/src/main.rs` - packet-g runtime toggle inference + hotspot metrics capture wiring
- `crates/benchmarks/tests/hot_path_contract.rs` - update hotspot contract test payload for new fields
- `docs/engine-benchmarks.md` - document packet-g candidate workflow
- `.planning/phases/11-hot-path-optimization-and-target-closure/11-TARGET-CLOSURE-EVIDENCE.md` - append plan 11-12 transcript, artifact hash, ratio verdict
- `.planning/phases/11-hot-path-optimization-and-target-closure/11-VERIFICATION.md` - refresh authoritative phase verdict to packet-g run
- `.planning/REQUIREMENTS.md` - point PERF-03 open-gap wording to packet-g artifact
- `.planning/ROADMAP.md` - mark 11-12 executed and keep Phase 11 open

## Decisions Made

- Chose packet-g cache invalidation via shared scope-generation checks rather than unconditional name-cache flush, preserving revalidate observability.
- Kept packet metrics default-off during timed benchmark loops; enabled only for the extra hotspot attribution pass to avoid timing distortion.
- Retained explicit open-gap wording across verification/requirements/roadmap because authoritative ratio gate failed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Benchmark contract tests broke after hotspot schema expansion**
- **Found during:** Task 3 (governance preflight)
- **Issue:** `HotspotAttributionCounters` initializers in `crates/benchmarks/tests/hot_path_contract.rs` missed new fields.
- **Fix:** Updated both initializers with packet-d/packet-g taxonomy fields.
- **Files modified:** `crates/benchmarks/tests/hot_path_contract.rs`
- **Verification:** `cargo test -p benchmarks` passed.
- **Committed in:** `36c1b52` (Task 3)

**2. [Rule 3 - Blocking] Workspace formatting drift blocked `cargo fmt --check`**
- **Found during:** Task 3 (governance command chain)
- **Issue:** fmt check failed on touched and pre-existing drifted files.
- **Fix:** Ran `cargo fmt` and carried resulting deterministic formatting updates.
- **Files modified:** `crates/bytecode/src/lib.rs`, `crates/test-harness/tests/rust_host_bindings.rs`, plus touched VM test files.
- **Verification:** `cargo fmt --check` passed.
- **Committed in:** `36c1b52` (Task 3)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes were required to complete governance gates; no architectural scope creep.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 11 plan queue is now fully executed through `11-12`.
- Phase 11 milestone closure is still blocked by PERF-03 (`qjs-rs/quickjs-c = 6.236987x` > `1.25x`).
- Next work requires a new Phase 11 closure attempt or milestone-level decision on follow-up optimization strategy.

---
*Phase: 11-hot-path-optimization-and-target-closure*
*Completed: 2026-03-03*
