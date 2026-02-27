# Architecture Research

**Domain:** qjs-rs v1.1 Performance Acceleration (instrumentation + optimization + gating)
**Researched:** 2026-02-27
**Confidence:** HIGH

## Standard Architecture

### System Overview

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                    Governance & Regression Gate Layer                        │
├──────────────────────────────────────────────────────────────────────────────┤
│  CI perf gate job  │ semantic non-regression check │ baseline policy docs  │
└───────────────┬───────────────────────────────┬─────────────────────────────┘
                │                               │
┌───────────────v───────────────────────────────v─────────────────────────────┐
│                 Benchmark & Instrumentation Control Plane                    │
├──────────────────────────────────────────────────────────────────────────────┤
│  benchmarks crate (runner)  │ perf schema/manifest  │ report renderer       │
│  engine adapters            │ threshold evaluator   │ hotspot summarizer    │
└───────────────┬───────────────────────────────┬─────────────────────────────┘
                │                               │
┌───────────────v───────────────────────────────v─────────────────────────────┐
│                 Engine Execution Plane (existing core pipeline)              │
├──────────────────────────────────────────────────────────────────────────────┤
│ parser -> bytecode -> vm -> runtime -> builtins                              │
│           (opt passes)    (fast paths + counters)                             │
└───────────────┬───────────────────────────────┬─────────────────────────────┘
                │                               │
┌───────────────v───────────────────────────────v─────────────────────────────┐
│                         Artifact & History Layer                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ target/benchmarks/*.json │ docs/reports/*.md,*.svg │ baseline snapshots     │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Change Type (v1.1) | Responsibility | Typical Implementation |
|-----------|---------------------|----------------|------------------------|
| `crates/benchmarks` | **Modified** | Own reproducible benchmark execution across `qjs-rs`, `boa-engine`, `quickjs-c`, `nodejs` | Deterministic case catalog + engine adapters + JSON output schema |
| `crates/vm` instrumentation module (`src/perf/*`) | **New** | Capture opcode/call/property/alloc counters with near-zero semantic impact | Feature-gated counters, monotonic snapshots, no behavioral branching |
| `crates/vm` hot-path execution | **Modified** | Add guarded fast paths for arithmetic loops, array access, call-heavy paths | Guard -> fast path -> fallback to existing slow/spec path |
| `crates/bytecode` optimization pass layer (`src/opt/*`) | **New** | Run semantics-preserving bytecode-level optimizations before execution | Small pass manager (`Pass` trait) + pass ordering + opt-debug dump |
| `crates/test-harness` perf-semantic bridge | **Modified** | Provide differential checks proving optimized path == baseline semantics | Shared script matrix executed with optimization on/off |
| `.github/scripts/perf_gate.py` | **New** | Enforce threshold-based non-regression in CI using benchmark JSON | Compare latest JSON vs checked-in baseline profile + fail on drift |
| `.github/workflows/ci.yml` perf jobs | **Modified** | Run benchmark-gate and semantic safety gate in strict order | Dedicated job stage with artifacts upload |
| `docs/reports/*` + `docs/engine-benchmarks.md` | **Modified** | Human-visible evidence and governance policy | Rendered report + threshold rationale + update process |

## Recommended Project Structure

```text
crates/
├── bytecode/
│   └── src/
│       ├── lib.rs                  # existing compiler entry
│       └── opt/                    # NEW: optimization pass pipeline
│           ├── mod.rs
│           ├── pass.rs             # pass trait + context
│           ├── peephole.rs         # local instruction rewrites
│           └── const_fold.rs       # guarded constant folding
├── vm/
│   └── src/
│       ├── lib.rs                  # existing VM
│       ├── perf/                   # NEW: instrumentation data model
│       │   ├── mod.rs
│       │   ├── counters.rs
│       │   └── snapshot.rs
│       └── fast_path/              # NEW: guarded hot-path helpers
│           ├── mod.rs
│           ├── arithmetic.rs
│           ├── array_ops.rs
│           └── calls.rs
├── benchmarks/
│   └── src/
│       ├── main.rs                 # existing CLI runner
│       ├── cases.rs                # NEW: benchmark case registry
│       ├── adapters/               # NEW: engine-specific runners
│       └── schema.rs               # NEW: stable JSON report model
└── test-harness/
    └── src/
        └── perf_semantic.rs        # NEW: optimization parity helpers

.github/
├── scripts/
│   └── perf_gate.py                # NEW: threshold comparator
└── workflows/
    └── ci.yml                      # MODIFIED: perf-gate stage

docs/
├── engine-benchmarks.md            # MODIFIED: runbook + policy
└── reports/
    ├── engine-benchmark-report.md  # generated
    └── engine-benchmark-chart.svg  # generated

benchmarks/
└── baselines/
    ├── local-dev.json              # NEW: developer reference baseline
    ├── ci-linux.json               # NEW: CI-target baseline
    └── policy.toml                 # NEW: thresholds, variance bounds, opt-in cases
```

### Structure Rationale

- **Keep optimization code close to owners (`bytecode`, `vm`)**: avoids creating cross-crate optimization glue that erodes existing architecture.
- **Introduce a control plane, not a semantic plane**: benchmarks/instrumentation/reporting sit *around* engine core and do not redefine runtime behavior.
- **Separate baseline artifacts by environment**: avoids invalid CI failures due to host variance while keeping regression tracking deterministic.
- **Pair perf changes with semantic parity helpers**: each optimization must ship with a direct parity check target.

## Architectural Patterns

### Pattern 1: Guarded Fast Path + Canonical Slow Path

**What:** Implement optimization as a strict guard around existing semantics implementation.
**When to use:** Arithmetic opcodes, array indexing, common function-call patterns.
**Trade-offs:** Slightly more code paths; much safer semantic behavior under edge cases.

**Example:**
```rust
fn add(lhs: JsValue, rhs: JsValue) -> Result<JsValue, VmError> {
    if let (JsValue::Number(a), JsValue::Number(b)) = (&lhs, &rhs) {
        return Ok(JsValue::Number(a + b));
    }
    self.add_slow_path(lhs, rhs) // existing spec-aligned logic
}
```

### Pattern 2: Two-Stage Measurement Pipeline (Collect -> Evaluate)

**What:** VM/test runner emits raw benchmark JSON; gate logic evaluates thresholds in a separate script.
**When to use:** CI/perf governance where threshold policy changes faster than runtime code.
**Trade-offs:** Extra artifact step; clean policy evolution without touching engine execution.

**Example:**
```bash
cargo run -p benchmarks --release -- --output target/benchmarks/engine-comparison.json
python .github/scripts/perf_gate.py \
  --input target/benchmarks/engine-comparison.json \
  --baseline benchmarks/baselines/ci-linux.json \
  --policy benchmarks/baselines/policy.toml
```

### Pattern 3: Differential Optimization Verification

**What:** Run identical scripts with optimization disabled/enabled and assert identical observable outputs/errors.
**When to use:** Every new optimization pass or VM fast path.
**Trade-offs:** More test time; strongest protection against semantic drift.

**Example:**
```rust
let baseline = run_script_with_opts(script, VmPerfOptions::disabled())?;
let optimized = run_script_with_opts(script, VmPerfOptions::enabled())?;
assert_eq!(baseline.observable_result, optimized.observable_result);
assert_eq!(baseline.error_shape, optimized.error_shape);
```

### Pattern 4: Pass Budgeting and Feature Flags

**What:** Keep optimization passes individually toggleable and measurable.
**When to use:** During milestone bring-up and regression triage.
**Trade-offs:** Slight config complexity; dramatically better bisectability.

## Data Flow

### Request Flow

```text
[Benchmark Case Catalog]
    ↓
[benchmarks runner] → [parser] → [bytecode + opt passes] → [vm fast path + counters] → [runtime/builtins]
    ↓                                                                                  ↓
[raw perf JSON artifact] ← [result normalizer] ← [engine adapters (boa/node/quickjs-c)]
    ↓
[perf gate evaluator] → [CI pass/fail + report links]
```

### State Management

```text
[Perf Config]
    ↓
[Pass Manager + VM Perf Options] → [Execution]
    ↓                                   ↓
[Perf Snapshot Buffer]             [Semantic Result]
    ↓                                   ↓
[Benchmark JSON] ----------------> [Differential Semantic Check]
```

### Key Data Flows

1. **Instrumentation flow:** VM emits counters/snapshots keyed by case + sample; artifacts are persisted without mutating runtime semantics.
2. **Optimization flow:** bytecode and VM fast paths run under explicit flags; parity harness compares outputs against non-optimized execution.
3. **Regression-gate flow:** CI evaluates benchmark aggregate and per-case thresholds against baseline policy, then only marks success if semantic suite stays green.

## Build/Order Rationale (v1.1)

| Order | Step | Why this order is required |
|------:|------|----------------------------|
| 1 | Stabilize benchmark schema + artifacts | Needed before any optimization so before/after is comparable. |
| 2 | Add VM/bytecode instrumentation hooks | Hotspot evidence must come from real execution paths. |
| 3 | Add differential semantic harness (opt off/on) | Prevents introducing optimization debt without safety net. |
| 4 | Land guarded optimization passes (one family at a time) | Enables focused rollback and attribution per pass. |
| 5 | Freeze CI baseline profile + policy thresholds | Baselines are meaningful only after first stable optimized run. |
| 6 | Enable CI perf gate (non-blocking soak -> blocking) | Reduces flake risk and false failures during initial stabilization. |

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 4-8 microbench cases (current) | Keep single benchmark binary, strict deterministic case scripts, full cross-engine run on demand. |
| 10-30 cases (milestone growth) | Split suite into tiers (`core-hotpath`, `json`, `collections`, `calls`) and run subset on PR, full suite nightly. |
| 30+ cases / multi-host | Add manifest-driven suite sharding and baseline per host class; keep gate metric on normalized ratios, not absolute time only. |

### Scaling Priorities

1. **First bottleneck:** benchmark runtime in PR CI. Fix with tiered suites and cached engine setup.
2. **Second bottleneck:** noisy thresholds across hosts. Fix with environment-tagged baselines + rolling variance windows.

## Anti-Patterns

### Anti-Pattern 1: Semantic Shortcuts as “Optimization”

**What people do:** Replace spec path logic with simplified assumptions (e.g., treating all adds as numeric).
**Why it's wrong:** Breaks language behavior and masks drift behind throughput gains.
**Do this instead:** Always guard fast paths and fall back to canonical slow path.

### Anti-Pattern 2: Benchmark Harness Owning Semantics

**What people do:** Put ad-hoc parser/vm behavior toggles directly in benchmark runner.
**Why it's wrong:** Creates a second execution semantics plane and invalid comparisons.
**Do this instead:** Benchmarks call the same public engine path as tests; only control flags may differ.

### Anti-Pattern 3: Hard-Coding One-Machine Thresholds

**What people do:** Use absolute millisecond limits copied from a single developer machine.
**Why it's wrong:** CI flakiness and false negatives/positives.
**Do this instead:** Gate with ratio-based thresholds against environment-specific baselines.

### Anti-Pattern 4: Overfitting Optimizations to Four Cases

**What people do:** Tune exactly for current microbench scripts and regress broader workloads.
**Why it's wrong:** Non-representative wins do not improve real engine quality.
**Do this instead:** Scale benchmark suite by workload family and keep per-family non-regression metrics.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| `boa-engine` (Rust crate) | In-process adapter from `crates/benchmarks` | Pin version in Cargo.lock for reproducibility. |
| `nodejs` binary | Subprocess adapter (`node -e`) | Capture version in report; isolate process startup from inner-loop timing policy. |
| `quickjs-c` binary | WSL subprocess adapter | Keep path/config explicit and fail with actionable diagnostics when unavailable. |
| GitHub Actions artifacts | Upload benchmark JSON/report on gate runs | Required for auditability and trend debugging. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `bytecode(opt)` ↔ `vm` | Existing `Chunk` contract + optional pass metadata | Do not expose VM internals to bytecode passes. |
| `vm(perf)` ↔ `test-harness` | Snapshot DTOs + execution result | Perf counters are observational; harness remains semantic source of truth. |
| `benchmarks` ↔ engine crates | Public parse/compile/execute APIs | No benchmark-only semantic API forks. |
| `perf_gate.py` ↔ benchmark artifacts | Versioned JSON schema | Add schema version to prevent silent parser drift. |
| CI perf gate ↔ semantic gates | Job dependency ordering | Perf pass is valid only if semantic gates pass first. |

## Sources

- `.planning/PROJECT.md`
- `.planning/REQUIREMENTS.md`
- `.planning/STATE.md`
- `.planning/codebase/ARCHITECTURE.md`
- `docs/engine-benchmarks.md`
- `docs/reports/engine-benchmark-report.md`
- `crates/benchmarks/src/main.rs`
- `.github/workflows/ci.yml`

---
*Architecture research for: qjs-rs v1.1 performance acceleration*
*Researched: 2026-02-27*
