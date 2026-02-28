# Requirements: qjs-rs (v1.1 milestone)

**Defined:** 2026-02-27
**Milestone:** v1.1 Performance Acceleration
**Core Value:** Deliver QuickJS-aligned JavaScript semantics in a pure Rust runtime without introducing C FFI into the runtime core.

## v1.1 Requirements

### Performance Baseline and Measurement

- [x] **PERF-01**: Project provides reproducible benchmark outputs comparing `qjs-rs`, `boa-engine`, `quickjs-c`, and `nodejs`, including machine-readable JSON and human-readable report artifacts.
- [x] **PERF-02**: Benchmark suite includes representative runtime hot paths (arithmetic loop, iterative function calls, array build/sum, JSON roundtrip) with configurable sample/iteration controls.

### Runtime Optimization

- [ ] **PERF-03**: `qjs-rs` aggregate mean latency on the tracked benchmark suite is **no worse than** `boa-engine` on the same host and run configuration. _(Open gap: packet-c candidate still fails authoritative checker as of 2026-02-28.)_
- [ ] **PERF-04**: At least two identified runtime hot paths receive targeted optimization backed by before/after benchmark evidence. _(Open closure state: packet evidence exists, but Phase 11 closure bundle is not fully green.)_
- [ ] **PERF-05**: Optimization changes avoid large architectural regressions and preserve maintainability boundaries (no runtime-core C FFI). _(Open closure state: maintainability checks exist, but governance + PERF-03 bundle remains open.)_

### Correctness and Governance

- [ ] **TST-05**: Existing semantic/governance quality gates (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, test262-lite governance checks) remain green after optimization work.
- [ ] **TST-06**: Performance regression guardrails are documented and executable in CI/nightly workflows with deterministic artifact output paths.

## Future Requirements (post-v1.1)

- **LAN-01**: Expand full `Proxy` invariant coverage beyond minimal currently executable paths.
- **LAN-02**: Expand `Symbol` and `BigInt` edge behavior to larger conformance subsets.
- **LAN-03**: Broaden typed-array coverage beyond baseline `Uint8Array`-centric paths.
- **PERF-06**: Pursue QuickJS(C)-gap closure phases after surpassing boa-engine baseline.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Runtime core C FFI dependency | Violates project boundary (pure Rust runtime core) |
| Large feature-surface expansion in same milestone | v1.1 focus is performance closure against boa-engine |
| Unbounded benchmark scenarios without reproducibility contract | Prioritize deterministic and actionable benchmark signals |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PERF-01 | Phase 10 | Completed |
| PERF-02 | Phase 10 | Completed |
| PERF-03 | Phase 11 | Open (gap: packet-c perf target + governance bundle not jointly green) |
| PERF-04 | Phase 11 | Open (packet evidence landed; awaiting closed bundle) |
| PERF-05 | Phase 11 | Open (maintainability evidence landed; awaiting closed bundle) |
| TST-05 | Phase 12 | Planned |
| TST-06 | Phase 12 | Planned |

**Coverage:**
- v1.1 requirements: 7 total
- Mapped to phases (exactly once): 7/7 (100%)
- Unmapped: 0 ✓
- Multi-mapped: 0 ✓
- Checked off: 2/7

---
*Requirements defined: 2026-02-27 for milestone v1.1*
*Roadmap alignment updated: 2026-02-28*
