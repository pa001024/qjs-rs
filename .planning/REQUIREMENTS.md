# Requirements: qjs-rs (v1.1 milestone)

**Defined:** 2026-02-27
**Milestone:** v1.1 Performance Acceleration
**Core Value:** Deliver QuickJS-aligned JavaScript semantics in a pure Rust runtime without introducing C FFI into the runtime core.

## v1.1 Requirements

### Performance Baseline and Measurement

- [x] **PERF-01**: Project provides reproducible benchmark outputs comparing `qjs-rs`, `boa-engine`, `quickjs-c`, and `nodejs`, including machine-readable JSON and human-readable report artifacts.
- [x] **PERF-02**: Benchmark suite includes representative runtime hot paths (arithmetic loop, iterative function calls, array build/sum, JSON roundtrip) with configurable sample/iteration controls.

### Runtime Optimization

- [ ] **PERF-03**: `qjs-rs` aggregate mean latency on the tracked benchmark suite is **at most `1.25x quickjs-c`** on the same host and run configuration (equivalent to **>=80% of `quickjs-c` performance**). _(Open gap: latest authoritative bundle at `target/benchmarks/phase11-closure-bundle.json` (`2026-02-28T17:53:12Z`) remains below this threshold.)_
- [ ] **PERF-04**: At least two identified runtime hot paths receive targeted optimization backed by before/after benchmark evidence. _(Open closure state: packet evidence exists, but authoritative governance + PERF-03 bundle is still red.)_
- [ ] **PERF-05**: Optimization changes avoid large architectural regressions and preserve maintainability boundaries (no runtime-core C FFI). _(Open closure state: maintainability evidence exists, but phase closure remains blocked until the authoritative governance + PERF-03 bundle is jointly green.)_

### Correctness and Governance

- [ ] **TST-05**: Existing semantic/governance quality gates (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, test262-lite governance checks) remain green after optimization work.
- [ ] **TST-06**: Performance regression guardrails are documented and executable in CI/nightly workflows with deterministic artifact output paths.

## Future Requirements (post-v1.1)

- **LAN-01**: Expand full `Proxy` invariant coverage beyond minimal currently executable paths.
- **LAN-02**: Expand `Symbol` and `BigInt` edge behavior to larger conformance subsets.
- **LAN-03**: Broaden typed-array coverage beyond baseline `Uint8Array`-centric paths.
- **PERF-06**: Pursue additional QuickJS(C)-gap closure phases after crossing the >=80% milestone threshold.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Runtime core C FFI dependency | Violates project boundary (pure Rust runtime core) |
| Large feature-surface expansion in same milestone | v1.1 focus is performance closure to >=80% of quickjs-c |
| Unbounded benchmark scenarios without reproducibility contract | Prioritize deterministic and actionable benchmark signals |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PERF-01 | Phase 10 | Completed |
| PERF-02 | Phase 10 | Completed |
| PERF-03 | Phase 11 | Open (gap: latest authoritative bundle perf target still below >=80% quickjs-c threshold) |
| PERF-04 | Phase 11 | Open (packet evidence landed; authoritative bundle still red due governance+perf gap) |
| PERF-05 | Phase 11 | Open (maintainability evidence landed; authoritative governance+perf bundle still open) |
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
