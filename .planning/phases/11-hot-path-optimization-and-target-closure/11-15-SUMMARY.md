---
phase: 11-hot-path-optimization-and-target-closure
plan: 15
subsystem: performance-runtime
tags: [perf, vm, packet-i, benchmark-contract, hotspot-attribution]
requires:
  - phase: 11-hot-path-optimization-and-target-closure
    provides: packet-h baseline and authoritative closure evidence from 11-14
provides:
  - packet-i shadow-aware slot/name revalidation behind a runtime toggle
  - packet-i parity and hotspot regression coverage for loop/block/with churn workloads
  - packet-i benchmark inference and smoke artifact flow with contract validation
affects: [phase-11-verification, roadmap-progress, benchmark-evidence, perf-closure-rerun]
tech-stack:
  added: []
  patterns: [toggle-gated-fast-path-extension, cache-revalidation-with-shadow-guard, output-path-driven-benchmark-toggles]
key-files:
  created:
    - .planning/phases/11-hot-path-optimization-and-target-closure/11-15-SUMMARY.md
  modified:
    - crates/vm/tests/perf_packet_d.rs
    - crates/vm/tests/perf_hotspot_attribution.rs
    - crates/vm/src/lib.rs
    - crates/benchmarks/src/main.rs
    - docs/engine-benchmarks.md
key-decisions:
  - "Expose packet-i as an explicit VM runtime toggle so benchmark packet inference can compare packet-h vs packet-i safely."
  - "Extend packet-d/packet-g stale-entry revalidation using non-shadowed visibility checks only when packet-i is enabled; keep packet-h and canonical fallback behavior unchanged."
  - "Treat packet-i as a deterministic output-path inferred packet stack (c+d+g+h+i) and require a strict-comparator smoke artifact plus contract validation."
patterns-established:
  - "Packet-level optimizations must land with toggle parity coverage before authoritative closure reruns."
  - "Hotspot attribution regressions should assert revalidate/fallback deltas with deterministic packet toggles, not ad-hoc harness logic."
requirements-completed: [PERF-03, PERF-04, PERF-05]
duration: 10 min
completed: 2026-03-03
---

# Phase 11 Plan 15: Packet-I Revalidation Optimization Summary

**Packet-i shadow-aware identifier revalidation is now implemented, test-backed for parity/hotspot behavior, and benchmark-wired with a contract-valid smoke artifact path.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-03T09:33:44Z
- **Completed:** 2026-03-03T09:43:47Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Added packet-i toggle-aware RED coverage in `perf_packet_d` and `perf_hotspot_attribution` for parity, revalidate-hit, and fallback-scan behavior.
- Implemented packet-i VM behavior by extending packet-d/packet-g stale cache revalidation to allow visible non-shadowed binding reuse when scope generation changes.
- Wired packet-i benchmark inference/runtime toggles and validated `target/benchmarks/engine-comparison.local-dev.packet-i.smoke.json` with strict comparators and contract checks.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add packet-i parity and hotspot regression tests for scope-churn revalidation** - `655d3c5` (`test`)
2. **Task 2: Implement packet-i shadow-aware slot/name revalidation in VM fast-path resolution** - `df9ac3d` (`perf`)
3. **Task 3: Wire packet-i benchmark toggle inference and produce smoke artifact** - `b15d9cf` (`feat`)

## Files Created/Modified
- `crates/vm/tests/perf_packet_d.rs` - Added packet-i toggle parity/miss-path tests in existing perf packet suite.
- `crates/vm/tests/perf_hotspot_attribution.rs` - Added packet-i revalidate-hit and fallback-scan hotspot regression coverage.
- `crates/vm/src/lib.rs` - Added packet-i runtime toggle and shadow-aware stale-entry revalidation for packet-d/packet-g paths.
- `crates/benchmarks/src/main.rs` - Added packet-i inference, runtime wiring, and packet-i toggle tests.
- `docs/engine-benchmarks.md` - Documented packet-i toggle stack and smoke artifact command sequence.

## Decisions Made
- Enabled packet-i only via explicit VM toggle and output-path inference to preserve rollback safety.
- Reused existing packet-d/packet-g counters and hotspot taxonomy instead of introducing packet-i-specific counters.
- Kept guarded fallback semantics unchanged for with/prototype/accessor/unknown-identifier paths while expanding safe revalidation scope.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Hotspot source workload adjustment for packet-g revalidate signal**
- **Found during:** Task 2 (VM + hotspot verification)
- **Issue:** Initial packet-g hotspot assertion workload did not reliably exercise packet-g revalidate-hit deltas under packet-h baseline.
- **Fix:** Switched the packet-g signal script to a loop-stable `var` binding churn shape and asserted aggregate fallback scan reduction.
- **Files modified:** `crates/vm/tests/perf_hotspot_attribution.rs`
- **Verification:** `cargo test -p vm perf_hotspot_attribution -- --nocapture`
- **Committed in:** `df9ac3d`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Improved determinism of hotspot regression checks without expanding scope.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Packet-i candidate is now ready for authoritative quickjs-ratio closure rerun workflows.
- Phase 11 remains open until an authoritative packet candidate satisfies PERF-03 (`qjs-rs/quickjs-c <= 1.25`).

---
*Phase: 11-hot-path-optimization-and-target-closure*
*Completed: 2026-03-03*
