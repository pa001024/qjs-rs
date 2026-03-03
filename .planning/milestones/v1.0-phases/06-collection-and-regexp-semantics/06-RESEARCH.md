# Phase 6: Collection and RegExp Semantics - Research

**Researched:** 2026-02-26  
**Domain:** BUI-04 / BUI-05 (`Map`/`Set`/`WeakMap`/`WeakSet` + `RegExp`)  
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Collection Semantics Baseline
- Key equality for collections uses `SameValueZero` semantics (`NaN` equal to `NaN`, `+0` and `-0` treated as the same key).
- Insertion order follows spec-aligned behavior: `set` on an existing key does not move position; `delete` then re-`set` appends to the end.
- Iteration uses live-view semantics under in-loop mutation, not snapshot semantics.
- Phase 6 acceptance for collections prioritizes a core closure: constructor and baseline methods (`get/set/add/has/delete/clear/size/forEach/iterator`) plus boundary/error assertions.

### WeakMap / WeakSet Constraints
- Non-object keys are rejected immediately with deterministic `TypeError`.
- Iterable-constructor input with an invalid entry fails fast (throw on first invalid element, stop processing).
- API surface should stay shape-aligned with `Map`/`Set` where possible, while preserving strict weak-collection constraints.
- Weak-collection acceptance emphasizes object-key restrictions, core methods, and deterministic error behavior.

### RegExp Behavioral Baseline
- Phase 6 baseline supports flags `g/i/m/s/u/y`.
- Unsupported patterns or unsupported flags must fail deterministically at construction with `SyntaxError`.
- `exec` and `test` must share one matching core and consistent `lastIndex` semantics.
- `RegExp.prototype.toString` should return stable normalized `/source/flags` output.

### CI Regression Contract
- Use a three-layer gate for this phase: VM unit/integration tests, harness integration tests, and test262-lite subset gates.
- Newly introduced Phase 6 subset gates are expected to be fully green.
- Existing Phase 5 gates must not regress while Phase 6 changes land.
- Document fixed CI command contracts and expected baseline outputs in phase baseline docs.

### Claude's Discretion
- Internal helper decomposition and code organization as long as observable behavior and locked acceptance gates remain unchanged.
- Exact naming/layout of fixtures and test groupings, provided command contract stability is preserved.
- Sequencing of implementation across collections vs regexp internals, as long as dependency correctness and non-regression constraints are maintained.

### Deferred Ideas (OUT OF SCOPE)

None - discussion stayed within Phase 6 scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BUI-04 | `Map/Set/WeakMap/WeakSet` use dedicated semantics (no baseline constructor alias shortcuts). | Current `Map/Set` internals are usable; `WeakMap/WeakSet` remain constructor aliases and must be split to dedicated constructors/prototypes/internal slots. |
| BUI-05 | RegExp constructor and prototype methods (`exec/test/toString`) preserve flags and match behavior for supported patterns. | Current flag/property baseline exists; core gaps are `lastIndex` transitions, capture groups in `exec`, and deterministic SyntaxError/category behavior in failing test262 slices. |
</phase_requirements>

## Summary

Phase 6 should be planned as **closure and de-aliasing**, not greenfield implementation. `Map`/`Set` already have strong baseline behavior in `vm` (SameValueZero checks, tombstone/live iterator behavior, size brand checks). The major collection risk is that `WeakMap`/`WeakSet` are still wired as global aliases to `MapConstructor`/`SetConstructor`, which violates BUI-04 even if sampled test262 subsets currently pass.

For RegExp, constructor/prototype plumbing exists and supports `g/i/m/s/u/y`, but semantic closure is incomplete: sampled test262 `built-ins/RegExp` (40-case sample) shows 11 failures and larger slices timeout. Code evidence indicates missing/partial behavior around `exec` capture groups and `lastIndex` state updates.

**Primary recommendation:** Plan Phase 6 as four waves: (1) freeze Phase 5 gates and add dedicated phase gates, (2) split weak collections into dedicated constructors + strict key constraints, (3) close RegExp core (`exec/test/lastIndex/SyntaxError`), (4) expand and stabilize CI subsets for collections+RegExp.

## Local Evidence Snapshot (2026-02-26)

### Non-regression checks (Phase 5 gates)
- `cargo test -p vm core_builtins_object_array_boolean_function -- --exact`: passed (1/1).
- `cargo test -p vm core_builtins_string_number_math -- --exact`: passed (1/1).
- `cargo test -p test-harness --test core_builtins_baseline`: passed (3/3).
- `cargo test -p test-harness --test test262_lite core_builtins_subset -- --exact`: passed (1/1).

### Collection/RegExp current measurements
- `Map` sample: `discovered=204 executed=53 passed=53 failed=0`.
- `Set` sample: `discovered=383 executed=126 passed=126 failed=0`.
- `WeakMap` sample: `discovered=141 executed=37 passed=37 failed=0`.
- `WeakSet` sample: `discovered=85 executed=41 passed=41 failed=0`.
- `RegExp` sample (`max-cases 40`): `discovered=1879 executed=40 passed=29 failed=11`.
- `RegExp` larger slices (`built-ins/RegExp/prototype`, `.../prototype/exec`) timed out in this run (>180s), indicating immediate planning risk for execution-budget and semantic correctness.

### Code-level facts that matter for planning
- `WeakMap`/`WeakSet` globals are aliases today (`crates/builtins/src/lib.rs` defines both as `MapConstructor`/`SetConstructor`).
- `runtime::NativeFunction` has `MapConstructor` and `SetConstructor`, but no dedicated weak constructors (`crates/runtime/src/lib.rs`).
- `Map`/`Set` internals already use SameValueZero + tombstone arrays + live iterators (`crates/vm/src/lib.rs`).
- `RegExp` constructor validates flags and pattern and surfaces flag-derived properties (`crates/vm/src/lib.rs`).
- `RegExp.test`/`exec` both call shared `execute_regexp_match`, but current `exec` builds only `[match]` plus `index/input` (no capture groups) and does not apply full spec-grade `lastIndex` transition behavior across global/sticky paths (`crates/vm/src/lib.rs`).

## Standard Stack

### Core (Keep)
| Component | Version | Purpose | Why Standard Here |
|---|---|---|---|
| Workspace crates (`runtime`, `builtins`, `vm`, `test-harness`) | workspace `0.1.0` | Builtin/runtime semantics | Existing architecture already holds Phases 1-5 behavior. |
| `fancy-regex` | `0.14` | RegExp engine backend | Already integrated in `vm`; phase should close semantics around this backend first. |
| `regex` | `1.x` | Supporting regex utilities | Already in `vm` dependency set; avoid introducing new backend this phase. |

### Reference Implementations (Read-only anchors)
| Source | Role in planning |
|---|---|
| `D:/dev/QuickJS/quickjs.c` (`js_map_*`, `js_regexp_*`, `JS_SameValueZero`) | Semantic anchor for expected behavior and edge-case sequencing. |
| `D:/dev/boa/core/engine/src/builtins/*` | Rust-native decomposition patterns (ordered map/set, weak collection slot checks, regexp abstract exec structure). |

## Architecture Patterns to Plan For

### Pattern 1: Keep `Map`/`Set` storage model, split weak collections
- Reuse current `Map`/`Set` behavior core (`read/write_*_entries`, tombstones, live iterators).
- Introduce dedicated `NativeFunction` variants for `WeakMapConstructor` and `WeakSetConstructor`.
- Introduce dedicated weak markers and strict `this`-brand checks (`__weakMapTag`, `__weakSetTag` or equivalent slot model).
- Enforce object-key constraints in weak methods and constructor iterable ingestion.

### Pattern 2: Single RegExp match core with explicit state transitions
- Keep one matching core (`exec`/`test` call same internal function).
- Add explicit pre/post `lastIndex` logic by flags (`g`/`y` success/failure transitions).
- Return full `exec` result shape for phase baseline: matched string array with `index`, `input`, and capture slots needed by targeted subsets.
- Keep deterministic constructor-time failures as `SyntaxError` category.

### Pattern 3: Gate-first implementation sequence
1. Freeze existing Phase 5 gates and collection green subsets.
2. Implement weak de-aliasing and weak constraints.
3. Implement RegExp semantic closure.
4. Expand test262-lite fixtures and command contract for Phase 6.

## Don’t Hand-Roll

| Problem | Don’t Build | Use Instead | Why |
|---|---|---|---|
| Weak collection behavior | Reusing `Map`/`Set` constructor aliases | Dedicated weak constructors + slots + key guards | Alias path cannot satisfy BUI-04 and leaks semantic drift. |
| Iteration mutation semantics | Snapshot iterators for collections | Existing tombstone/live-view approach | Snapshot approach breaks locked decision for live mutation semantics. |
| RegExp dual behavior | Separate `exec` and `test` engines | Shared internal match core + explicit wrappers | Prevents drift in match/flag/lastIndex semantics. |
| CI coverage for this phase | Rely only on broad full-suite runs | Fixed phase-local command contract + smoke subsets | Faster feedback and non-regression discipline. |

## Common Pitfalls

### Pitfall 1: “Green sample means done” for weak collections
- Why it happens: sampled test262 weak suites execute only a small subset (many skipped by harness policy).
- Impact: BUI-04 can look green while alias design remains.
- Prevention: make constructor/prototype identity and non-object-key errors explicit phase gates.

### Pitfall 2: RegExp `lastIndex` drift
- Why it happens: shared match logic without explicit per-flag state transitions.
- Impact: `exec`/`test` divergence and subtle test262 failures.
- Prevention: centralize `lastIndex` transition table in one helper used by both methods.

### Pitfall 3: Missing capture group materialization
- Why it happens: current `exec` return array includes only full match.
- Impact: backreference/capture assertions fail (`arr[1]`, `arr[2]` clusters).
- Prevention: include capture extraction path in match core output contract before wiring `exec` return shape.

### Pitfall 4: RegExp performance regressions hide as hangs
- Why it happens: compile/match path currently recompiles matcher and may hit expensive patterns.
- Impact: long-running or timed-out compatibility sweeps.
- Prevention: add phase-level timeout/command budgeting and, if needed, instance-level compiled matcher caching plan.

## Code Anchors for Plan Tasks

### Collections (current reusable baseline)
- `crates/vm/src/lib.rs`: `strict_this_map`, `strict_this_set`, `read/write_map_entries`, `read/write_set_entries`.
- `crates/vm/src/lib.rs`: host functions `MapSetThis`/`MapDeleteThis`/`MapForEachThis` and `SetAddThis`/`SetDeleteThis`/`SetForEachThis`.
- `crates/vm/src/lib.rs`: iterator constructors and `next()` for map/set iterators.

### Weak collection de-aliasing targets
- `crates/builtins/src/lib.rs`: current `WeakMap`/`WeakSet` global alias wiring.
- `crates/runtime/src/lib.rs`: extend `NativeFunction` for dedicated weak constructors.
- `crates/vm/src/lib.rs`: add weak constructor dispatch + strict weak method checks.

### RegExp closure targets
- `crates/vm/src/lib.rs`: `create_regexp_value`, `validate_regexp_compile_inputs`, `execute_regexp_match`, `execute_regexp_compile_this`.
- `crates/vm/src/lib.rs`: host dispatch for `RegExpTestThis`, `RegExpExecThis`, `RegExpToStringThis`.

## Suggested Phase 6 Gate Contract

### Keep (must stay green)
- `cargo test -p vm core_builtins_object_array_boolean_function -- --exact`
- `cargo test -p vm core_builtins_string_number_math -- --exact`
- `cargo test -p test-harness --test core_builtins_baseline`
- `cargo test -p test-harness --test test262_lite core_builtins_subset -- --exact`

### Add (Phase 6 dedicated)
- VM targeted tests: weak non-object-key errors, weak iterable invalid-entry fail-fast, map/set live mutation iteration, regexp lastIndex matrix, regexp capture-group shape.
- Harness integration: new collection/regexp integration file(s) similar to Phase 5 style.
- test262 subset commands (fixed in docs):
  - `built-ins/Map --max-cases 200`
  - `built-ins/Set --max-cases 200`
  - `built-ins/WeakMap`
  - `built-ins/WeakSet`
  - `built-ins/RegExp --max-cases <phase-fixed-budget>` with explicit timeout budget.

## Open Questions (Need Decision Before PLAN.md)

1. Is true weak reachability behavior (GC-observable semantics) in scope for this phase, or only API/TypeError and non-enumerability semantics?
2. For RegExp phase gate, what is the fixed execution budget/timeout policy to prevent CI stalls on heavy patterns?
3. Should phase include compiled-regex caching on RegExp instances now, or defer to Phase 7 performance hardening?
4. What exact minimum test262 RegExp slice is the acceptance contract (to avoid moving target)?

## Sources

### Primary (HIGH confidence)
- `D:/dev/qjs-rs/.planning/phases/06-collection-and-regexp-semantics/06-CONTEXT.md`
- `D:/dev/qjs-rs/.planning/REQUIREMENTS.md`
- `D:/dev/qjs-rs/.planning/ROADMAP.md`
- `D:/dev/qjs-rs/.planning/STATE.md`
- `D:/dev/qjs-rs/docs/current-status.md`
- `D:/dev/qjs-rs/docs/test262-baseline.md`
- `D:/dev/qjs-rs/crates/builtins/src/lib.rs`
- `D:/dev/qjs-rs/crates/runtime/src/lib.rs`
- `D:/dev/qjs-rs/crates/vm/src/lib.rs`
- `D:/dev/qjs-rs/crates/test-harness/src/test262.rs`
- `D:/dev/qjs-rs/crates/test-harness/tests/test262_lite.rs`

### Reference implementations (HIGH confidence)
- `D:/dev/QuickJS/quickjs.c` (`js_map_*`, `js_regexp_*`, `JS_SameValueZero`)
- `D:/dev/boa/core/engine/src/builtins/map/*`
- `D:/dev/boa/core/engine/src/builtins/set/*`
- `D:/dev/boa/core/engine/src/builtins/weak_map/mod.rs`
- `D:/dev/boa/core/engine/src/builtins/weak_set/mod.rs`
- `D:/dev/boa/core/engine/src/builtins/regexp/mod.rs`

### Probe commands (HIGH confidence)
- `cargo test -p vm core_builtins_object_array_boolean_function -- --exact`
- `cargo test -p vm core_builtins_string_number_math -- --exact`
- `cargo test -p test-harness --test core_builtins_baseline`
- `cargo test -p test-harness --test test262_lite core_builtins_subset -- --exact`
- `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Map --max-cases 200 --allow-failures`
- `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Set --max-cases 200 --allow-failures`
- `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/WeakMap --allow-failures`
- `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/WeakSet --allow-failures`
- `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/RegExp --max-cases 40 --allow-failures --show-failures 20`

## Metadata

**Confidence breakdown**
- Collection architecture and gaps: HIGH (direct code + tests + command probes).
- RegExp baseline and risks: HIGH (direct code + command probes with failure list/timeouts).
- Execution strategy fit for planning: HIGH.

**Research date:** 2026-02-26  
**Valid until:** 2026-03-12
