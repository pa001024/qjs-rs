# Phase 5: Core Builtins Baseline - Research

**Researched:** 2026-02-26  
**Domain:** BUI-01 / BUI-02 / BUI-03 core builtins, native errors, and JSON interop  
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Builtin Coverage Prioritization
- Use layered rollout: stabilize must-have semantics first, then expand secondary edges.
- `Object`/`Function` must reach core semantic closure in this phase.
- Priority order for implementation and stabilization: `Array -> String -> Number/Math -> Date -> Boolean`.
- Acceptance uses behavior-truth gates, not API-count gates: each builtin area requires positive and boundary/error assertions.

### Error Hierarchy External Behavior
- Phase 5 includes: `Error`, `TypeError`, `ReferenceError`, `SyntaxError`, `RangeError`, `EvalError`, `URIError`.
- `name`/`message` follow standard constructor defaults and observable override behavior.
- `Error.prototype.toString` must follow standard name/message combination semantics.
- `instanceof` checks are strict acceptance criteria: subclass instances must satisfy both subclass and `Error` chains.

### JSON Behavioral Surface
- `JSON.parse` scope includes baseline parse plus `reviver` semantics.
- `JSON.stringify` scope includes baseline stringify plus `replacer` and `space` semantics.
- Invalid input/cycle paths must produce deterministic `TypeError`/`SyntaxError` category behavior.
- Object serialization output ordering must remain stable and testable under current engine guarantees.

### Date/Math/Number Determinism Baseline
- `Date` baseline closure for this phase includes constructor behavior, `Date.parse`, `Date.UTC`, `getTime`, `toString`, and `toUTCString`.
- Date tests should bias to UTC/timestamp assertions; avoid over-constraining locale-dependent text outputs.
- `Math`/`Number` acceptance prioritizes edge consistency (`NaN`, `Infinity`, `-0`, rounding/format paths).
- `Boolean`/`Number` boxing must be explicitly differentiated (`call` vs `new`) with observable `valueOf`/`toString` behavior.

### Claude's Discretion
- Internal task partitioning and exact sequencing inside each prioritized builtin block.
- Exact fixture shape and assertion decomposition, as long as the locked acceptance gates above are preserved.
- Internal helper/API refactor strategy used to reduce duplication while preserving current external behavior.

### Deferred Ideas (OUT OF SCOPE)
None - discussion stayed within Phase 5 scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BUI-01 | Core builtins (`Object`, `Function`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Date`) satisfy targeted conformance subsets used in CI. | Use current green Object/Array/Boolean baseline as fixed gates; plan focused closure work on Function/String/Number/Math/Date based on measured failing clusters. |
| BUI-02 | Error hierarchy (`Error` plus standard subclasses) exposes expected constructor/prototype behavior and stringification. | Replace shared-prototype shortcuts for native errors with per-subclass prototype chain wiring and strict `instanceof` behavior; preserve deterministic `name/message/toString` semantics. |
| BUI-03 | `JSON.parse` and `JSON.stringify` support baseline interoperability scenarios used by harness and integration tests. | Replace current placeholder JSON host behavior with algorithmic parse/stringify paths including `reviver`/`replacer`/`space`, deterministic SyntaxError/TypeError categories, and stable key ordering rules. |
</phase_requirements>

## Summary

Phase 5 is not greenfield. The runtime already contains extensive builtin infrastructure and many high-value wins (notably `Object` and large `Array` subsets), but closure risk is concentrated in three areas: JSON is still placeholder-level, native error subclass prototype chains are incomplete, and `Date`/`Number`/`Math` still have substantial conformance holes.

The planning mistake to avoid is treating this phase as broad API expansion. The right plan is targeted semantic hardening around existing architecture seams: constructor/prototype factories, native/host dispatch, and deterministic error conversion. Most work should happen in `crates/vm/src/lib.rs` with test leverage in `crates/test-harness`.

**Primary recommendation:** Plan Phase 5 as four waves: baseline freeze and gates -> Error hierarchy closure -> JSON closure -> Function/String/Number/Math/Date conformance closure, with explicit per-wave deterministic regression suites.

## Current Baseline (Repo + Probe Data)

### What is already strong
- Global builtin exposure path exists and is stable (`crates/builtins/src/lib.rs`).
- VM has mature object/prototype plumbing and many core methods (`crates/vm/src/lib.rs`).
- `Object` and large `Array` subsets are already reported green in docs (`docs/current-status.md`, `docs/test262-baseline.md`).
- `Boolean` subset is green in current snapshot (`docs/current-status.md`).

### High-risk gaps confirmed by live probes (2026-02-26)

| Area (test262 root) | Executed | Passed | Failed | Planning implication |
|---|---:|---:|---:|---|
| `built-ins/JSON` | 97 | 12 | 85 | BUI-03 is a primary critical path. |
| `built-ins/Date` | 200 | 101 | 99 | Date baseline semantics need focused closure. |
| `built-ins/Number` | 200 | 115 | 85 | Number coercion and static methods are missing/incomplete. |
| `built-ins/Math` | 162 | 117 | 45 | Missing method surface causes many `NotCallable` failures. |
| `built-ins/String` | 200 | 145 | 55 | Remaining constructor/prototype edge behavior gaps. |
| `built-ins/Function` | 200 | 158 | 42 | Dynamic constructor edge cases remain. |
| `built-ins/NativeErrors` | 37 | 25 | 12 | Confirms BUI-02 prototype-chain defects. |
| `built-ins/Error` | 26 | 22 | 4 | Core Error path mostly present but not fully closed. |

### Confirmed root causes in code
- JSON is intentionally minimal today: `execute_json_parse` accepts only primitive literals/numbers and returns `Undefined` otherwise; `execute_json_stringify` collapses objects to `"{}"`/`"[]"` (`crates/vm/src/lib.rs:20282`, `crates/vm/src/lib.rs:20318`).
- Native error prototypes are not fully distinct: `ReferenceError/SyntaxError/EvalError/RangeError/URIError` currently resolve to `Error.prototype` in constructor prototype lookup (`crates/vm/src/lib.rs:19728`, `crates/vm/src/lib.rs:7672`).
- `Date` text behavior is currently custom (`Date(...)`) and not object-tag/spec-like output for several tested paths (`crates/vm/src/lib.rs:7104`, `crates/vm/src/lib.rs:7196`, `crates/vm/src/lib.rs:7412`).
- `Math` currently exposes only a subset of methods (many ES methods absent, creating `NotCallable` failures) (`crates/vm/src/lib.rs:14798`).
- `Number` static function surface is incomplete (for example only `isNaN` is wired on constructor among common statics) (`crates/vm/src/lib.rs:19596`).

## Standard Stack

### Core
| Component | Version | Purpose | Why Standard for this phase |
|---|---|---|---|
| Rust workspace crates (`vm`, `runtime`, `builtins`, `test-harness`) | workspace `0.1.0` | Builtin semantics + execution + harness | Existing architecture already solved most adjacent phases; avoid redesign. |
| `test-harness` + `test262-run` | workspace `0.1.0` | Deterministic subset measurement and regression | Already integrated and supports repeatable subset runs with summaries. |

### Supporting
| Component | Version | Purpose | When to use |
|---|---|---|---|
| `fancy-regex` / `regex` | current in `vm` | Existing regex-dependent paths | Keep as-is; not a Phase 5 focus except compatibility safety. |
| `serde_json` (recommended) | `1.x` | JSON grammar/tokenization baseline | Use for parse/token correctness if you choose dependency over custom parser logic. |

## Architecture Patterns (Plan Around These)

### Pattern 1: Three-layer builtin architecture
- Layer A: symbol exposure in `install_baseline` (`crates/builtins/src/lib.rs`).
- Layer B: lazy prototype/object factories in VM (`*_prototype_value`, `math_object_value`, `json_object_value` in `crates/vm/src/lib.rs`).
- Layer C: behavior dispatch in `execute_native_call` / `execute_host_function_call` (`crates/vm/src/lib.rs`).
- Planning use: keep new semantics in Layer B/C; avoid pushing behavior into `builtins` crate.

### Pattern 2: Deterministic runtime errors as objects
- Runtime already maps many failures into typed error-like objects via `create_error_exception` (`crates/vm/src/lib.rs:13232`).
- Planning use: BUI-02/BUI-03 should preserve deterministic error category and constructor chain while removing current prototype shortcuts.

### Pattern 3: Property metadata helpers as single choke points
- Helpers like `set_builtin_function_length`, `set_builtin_function_name`, and property-attribute setters are already central (`crates/vm/src/lib.rs:17165`).
- Planning use: all new builtin methods and constructors should be attached through these helpers to avoid metadata drift.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| JSON lexical grammar and escaping | New ad-hoc character parser/serializer in one pass | `serde_json` parse/emit core or an equivalent proven parser + VM conversion walkers | Reduces parser bug surface; focus effort on JS-specific reviver/replacer semantics. |
| CI subset execution harness | New custom test runner | Existing `test262-run` + `run_suite` flow (`crates/test-harness/src/test262.rs`) | Already deterministic and integrated with existing gates. |
| Builtin function metadata wiring | Scattered manual property writes | Existing VM helper patterns for name/length/attributes | Prevents regressions in `length/name/enumerable/configurable` behavior. |

## Requirement-Focused Planning Guidance

### BUI-01: Core builtins targeted subsets

Recommended plan split:
1. Lock current green gates first (`Object`, `Array`, `Boolean`) as non-regression suite.
2. Close Function/String gaps with targeted constructor/prototype edge tests.
3. Close Number/Math method-surface + conversion-edge gaps.
4. Close Date constructor/static/prototype deterministic baseline.

Planning note: keep task order aligned to locked priority (`Array -> String -> Number/Math -> Date -> Boolean`) while treating Object/Function closure as acceptance-critical.

### BUI-02: Error hierarchy closure

Must change:
- Create dedicated prototype objects for `ReferenceError`, `SyntaxError`, `RangeError`, `EvalError`, `URIError` (not only `TypeError`).
- Ensure each `XError.prototype.__proto__ === Error.prototype`.
- Ensure each constructor `.prototype` points to its own prototype object.
- Keep `name/message` defaults and `Error.prototype.toString` behavior deterministic.

Known failing cluster to target first:
- `built-ins/NativeErrors/*/prototype/proto.js`
- `built-ins/NativeErrors/*/prototype/not-error-object.js`

### BUI-03: JSON parse/stringify closure

Must change:
- `JSON.parse(text, reviver)`:
  - Full baseline object/array/string/number/bool/null parsing.
  - Deterministic `SyntaxError` on malformed input.
  - Post-parse reviver walk semantics (holder/key recursion).
- `JSON.stringify(value, replacer, space)`:
  - Baseline object/array nested serialization.
  - Replacer function and replacer array semantics.
  - `space` handling (number/string clamp behavior).
  - Deterministic cycle detection and `TypeError`.
  - Stable property output order consistent with engine guarantees.

Known failing cluster to target first:
- `built-ins/JSON/parse/*` malformed-input throw expectations.
- `built-ins/JSON/stringify/*` baseline structure and function filtering.

## Common Pitfalls

### Pitfall 1: False confidence from existing builtins wins
- What goes wrong: planning assumes Phase 5 is near-done because Object/Array are strong.
- Why: large wins in one cluster hide JSON/Error/Date debt.
- Avoidance: enforce per-requirement gate metrics (BUI-01/02/03 tracked separately).

### Pitfall 2: Prototype alias shortcuts for native errors
- What goes wrong: subclasses share `Error.prototype`, breaking constructor/proto invariants.
- Why: shortcut currently used for several native errors.
- Avoidance: dedicated prototype factories for each native error subclass.

### Pitfall 3: JSON placeholder behavior leaked into acceptance gates
- What goes wrong: parse/stringify return permissive values instead of throwing typed errors.
- Why: current implementation is intentionally minimal.
- Avoidance: mark JSON as wave-critical and gate with malformed/cycle tests first.

### Pitfall 4: Conformance signal skew from harness skip rules
- What goes wrong: planning over-trusts pass rates while many flagged cases are skipped.
- Why: `module`, `onlyStrict`, `async`, `includes`, and `features` are skipped by harness policy (`crates/test-harness/src/test262.rs`).
- Avoidance: supplement with explicit phase-local regression suites in `test-harness` for required semantics.

## Suggested Validation Gates for Planning

### Wave Gate A: Baseline freeze
- `cargo test -p vm`
- `cargo test -p test-harness`
- Keep existing green builtins subsets from docs as non-regression targets.

### Wave Gate B: Error hierarchy
- Add dedicated harness/VM tests for each subclass constructor/prototype chain and `toString`.
- Target zero failures for `built-ins/NativeErrors` sampled run used by phase.

### Wave Gate C: JSON
- Add deterministic integration suite covering parse/stringify + reviver/replacer/space + malformed/cycle errors.
- Target meaningful reduction then closure in `built-ins/JSON` sampled run used by phase.

### Wave Gate D: Core builtin closure
- Add/update focused suites for Function/String/Number/Math/Date failing clusters.
- Track via stable sampled commands and phase-local deterministic tests.

## Open Questions (Need Decisions Before Detailed PLAN.md)

1. Should JSON parser implementation use `serde_json` as a dependency or stay fully in-house?
2. For Date string outputs in this phase, what exact deterministic format is accepted for `toString`/`toUTCString` while staying within locked constraints?
3. Is the current string-based `instanceof` fallback for error-like strings intentionally retained, or should BUI-02 harden toward object-only semantics?
4. Which exact sampled test262 commands are declared as the Phase 5 CI subset contract (to avoid moving target during execution)?

## Sources

### Primary (HIGH confidence)
- `.planning/phases/05-core-builtins-baseline/05-CONTEXT.md`
- `.planning/REQUIREMENTS.md`
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `crates/vm/src/lib.rs`
- `crates/builtins/src/lib.rs`
- `crates/test-harness/src/test262.rs`
- `docs/current-status.md`
- `docs/test262-baseline.md`

### Secondary (HIGH confidence, live local probes)
- Local command probes on 2026-02-26:
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/JSON --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Date --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Number --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Math --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/String --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Function --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/Error --max-cases 200 --allow-failures`
  - `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test/built-ins/NativeErrors --max-cases 200 --allow-failures`

## Metadata

**Confidence breakdown**
- Baseline diagnosis: HIGH (direct code + live probe evidence)
- Architecture guidance: HIGH (aligned to existing code seams)
- Execution risk forecast: MEDIUM-HIGH (JSON and Date closure breadth still large)

**Research date:** 2026-02-26  
**Valid until:** 2026-03-12
