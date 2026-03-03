---
phase: 05-core-builtins-baseline
phase_number: "05"
verified: 2026-02-26T18:23:13.8214985Z
status: passed
score: 9/9 must-haves verified
requirements_checked:
  - BUI-01
  - BUI-02
  - BUI-03
---

# Phase 5: Core Builtins Baseline Verification Report

**Phase Goal:** Core builtin objects, error hierarchy, and JSON interop satisfy targeted conformance scenarios.
**Verified:** 2026-02-26T18:23:13.8214985Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Native error subclasses expose dedicated constructor/prototype chains instead of aliasing `Error.prototype`. | ✓ VERIFIED | `crates/vm/src/lib.rs:16018` sets subclass prototype `__proto__` to `Error.prototype`; `crates/vm/src/lib.rs:20160` routes each constructor `.prototype` to dedicated subclass prototypes; `cargo test -p vm native_error_constructor_prototype_chain -- --exact` passed (1/1). |
| 2 | `name`/`message` defaults+overrides and `Error.prototype.toString` are deterministic across `Error` and standard subclasses. | ✓ VERIFIED | `crates/vm/src/lib.rs:7985` normalizes constructor message defaults; `crates/vm/src/lib.rs:7289` implements `Error.prototype.toString` composition and receiver guard; `cargo test -p test-harness --test native_errors` passed (4/4). |
| 3 | `instanceof` checks pass for subclass and `Error` ancestors in targeted regressions. | ✓ VERIFIED | `crates/test-harness/tests/native_errors.rs:44` asserts subclass+ancestor `instanceof`; `crates/vm/tests/native_errors.rs:8` checks constructor/prototype chain end-to-end; both test commands passed. |
| 4 | `JSON.parse` accepts baseline nested JSON and applies reviver walk semantics deterministically. | ✓ VERIFIED | `crates/vm/src/lib.rs:20762` parses JSON then applies reviver via `json_internalize_property`; `crates/vm/src/lib.rs:20834` recursively walks arrays/objects; `cargo test -p vm json_parse_reviver_semantics -- --exact` and `cargo test -p test-harness --test json_interop` passed. |
| 5 | Malformed JSON input throws deterministic `SyntaxError`-category failures. | ✓ VERIFIED | `crates/vm/src/lib.rs:20785` maps malformed parse to `SyntaxError`; `crates/test-harness/tests/json_interop.rs:31` validates malformed input throws `SyntaxError`; targeted tests passed. |
| 6 | `JSON.stringify` supports replacer/space behavior and throws deterministic `TypeError` on cycles. | ✓ VERIFIED | `crates/vm/src/lib.rs:20728` builds stringify context with replacer/property-list/gap; `crates/vm/src/lib.rs:21041` detects cycles and calls `json_stringify_cycle_error`; `crates/vm/src/lib.rs:20899` constructs `TypeError`; `cargo test -p vm json_stringify_replacer_space_cycle -- --exact` passed. |
| 7 | Core builtin subset (`Object`,`Function`,`Array`,`String`,`Number`,`Boolean`,`Math`,`Date`) executes deterministically in targeted phase CI scenarios. | ✓ VERIFIED | `crates/vm/tests/core_builtins_baseline.rs:8` and `crates/test-harness/tests/core_builtins_baseline.rs:11` cover these families; `cargo test -p test-harness --test test262_lite core_builtins_subset -- --exact` passed (1/1). |
| 8 | Baseline green areas (`Object`,`Array`,`Boolean`) remain regression-locked while subset gates stay stable. | ✓ VERIFIED | `crates/test-harness/tests/test262_lite.rs:145` enforces per-family subset execution with zero failures; `docs/test262-baseline.md:5` records fixed Phase-5 command contract and green results. |
| 9 | `Function`/`String`/`Number`/`Math`/`Date` behavior includes positive and boundary/error assertions. | ✓ VERIFIED | `crates/test-harness/tests/core_builtins_baseline.rs:19` (Function coercion + SyntaxError), `crates/test-harness/tests/core_builtins_baseline.rs:47` (String coercion throw), `crates/test-harness/tests/core_builtins_baseline.rs:57` (Date parse/UTC and error path); all targeted tests passed. |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/vm/src/lib.rs` | Native error/JSON/core builtins semantics implementation | ✓ VERIFIED | Exists; substantive (>25k LOC with concrete implementations); wired via VM tests and harness test execution. |
| `crates/test-harness/tests/native_errors.rs` | Native error regression coverage | ✓ VERIFIED | Exists with deterministic assertions (`name/message/toString/instanceof`); executed and passed (4/4). |
| `crates/test-harness/tests/json_interop.rs` | JSON interop regression coverage | ✓ VERIFIED | Exists with reviver/replacer/space/malformed/cycle assertions; executed and passed (4/4). |
| `crates/test-harness/tests/core_builtins_baseline.rs` | Core builtin family integration regression coverage | ✓ VERIFIED | Exists with positive and boundary/error checks across Phase-5 families; executed and passed (3/3). |
| `crates/test-harness/tests/test262_lite.rs` | test262-lite subset gates for NativeErrors/JSON/CoreBuiltins | ✓ VERIFIED | Exists with dedicated subset runners `native_errors_subset`, `json_subset`, `core_builtins_subset`; each exact gate command passed. |
| `docs/test262-baseline.md` | Phase-5 CI subset contract + measured baseline | ✓ VERIFIED | Exists with explicit command contract and result snapshots for core builtins (`core_builtins_*` and `core_builtins_subset`). |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `crates/vm/src/lib.rs` | Native error constructor `.prototype` | Per-subclass prototype getter routing | ✓ WIRED | `crates/vm/src/lib.rs:20160`-`crates/vm/src/lib.rs:20177` routes each native error constructor to its dedicated prototype accessor. |
| `crates/vm/src/lib.rs` | Subclass prototype chain | Shared native-error factory setting `__proto__` to `Error.prototype` | ✓ WIRED | `crates/vm/src/lib.rs:16020`-`crates/vm/src/lib.rs:16026` links subclass prototypes to `Error.prototype`. |
| `crates/test-harness/tests/native_errors.rs` | Runtime constructor behavior | Harness scripts asserting chain + stringification | ✓ WIRED | Assertions in `crates/test-harness/tests/native_errors.rs:18`, `crates/test-harness/tests/native_errors.rs:44`, `crates/test-harness/tests/native_errors.rs:58`; suite passed. |
| `crates/vm/src/lib.rs` | JSON parse output | Reviver post-walk recursion path | ✓ WIRED | `crates/vm/src/lib.rs:20770`-`crates/vm/src/lib.rs:20780` and `crates/vm/src/lib.rs:20834`-`crates/vm/src/lib.rs:20879`. |
| `crates/vm/src/lib.rs` | JSON stringify traversal | Deterministic object/array key traversal with cycle guard | ✓ WIRED | `crates/vm/src/lib.rs:21075`-`crates/vm/src/lib.rs:21079` uses `collect_own_property_keys`; `crates/vm/src/lib.rs:21041` cycle detection. |
| `crates/test-harness/tests/json_interop.rs` + `crates/test-harness/tests/test262_lite.rs` | JSON behavior gate | Direct harness assertions + `json_subset` smoke gate | ✓ WIRED | `crates/test-harness/tests/test262_lite.rs:130` defines `json_subset`; command passed (1/1). |
| `crates/vm/src/lib.rs` + `crates/runtime/src/lib.rs` | Core builtin constructor/method surface | NativeFunction enum + VM dispatch/property mappings | ✓ WIRED | NativeFunction surface includes Number static + Math + Date entries (`crates/runtime/src/lib.rs:39`-`crates/runtime/src/lib.rs:93`); VM dispatch/property wiring at `crates/vm/src/lib.rs:7620`, `crates/vm/src/lib.rs:7877`, `crates/vm/src/lib.rs:15096`, `crates/vm/src/lib.rs:20040`. |
| `crates/test-harness/tests/test262_lite.rs` | Phase-5 builtin roots | Family-scoped subset runner over 8 target families | ✓ WIRED | `crates/test-harness/tests/test262_lite.rs:145` iterates Object/Array/Boolean/Function/String/Number/Math/Date and enforces zero failures. |
| `docs/test262-baseline.md` | BUI-01 acceptance gate | Documented fixed commands/results for CI contract | ✓ WIRED | `docs/test262-baseline.md:5`-`docs/test262-baseline.md:20` lists exact command contract and passing baseline results. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| BUI-02 | `05-01-PLAN.md` | Error hierarchy constructor/prototype behavior and stringification | ✓ SATISFIED | Requirement exists in `REQUIREMENTS.md` (`BUI-02`); implementation+tests verified via `crates/vm/src/lib.rs:16018`, `crates/vm/src/lib.rs:7289`, `crates/test-harness/tests/native_errors.rs`, `crates/vm/tests/native_errors.rs`; all targeted commands passed. |
| BUI-03 | `05-02-PLAN.md` | JSON parse/stringify baseline interop scenarios | ✓ SATISFIED | Requirement exists in `REQUIREMENTS.md` (`BUI-03`); parse/stringify/reviver/replacer/cycle code at `crates/vm/src/lib.rs:20728`-`crates/vm/src/lib.rs:21113`; coverage in `crates/test-harness/tests/json_interop.rs` and `json_subset`; all targeted commands passed. |
| BUI-01 | `05-03-PLAN.md` | Core builtins conformance subset for CI | ✓ SATISFIED | Requirement exists in `REQUIREMENTS.md` (`BUI-01`); builtins surface/wiring in `crates/runtime/src/lib.rs:39`-`crates/runtime/src/lib.rs:93` and `crates/vm/src/lib.rs:15096`, `crates/vm/src/lib.rs:20040`; validated by VM/harness/test262-lite subset gates and documented in `docs/test262-baseline.md:5`. |

Phase-5 orphaned requirement IDs: **None**. `BUI-01`, `BUI-02`, and `BUI-03` are all declared in plan frontmatter and present in `REQUIREMENTS.md` traceability.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| --- | --- | --- | --- | --- |
| `crates/vm/src/lib.rs` | 66 | `SURROGATE_PLACEHOLDER_*` identifier match in heuristic scan | ℹ️ Info | Naming match only; not a stub/TODO signal. |
| `crates/vm/src/lib.rs` | 23759 | `return null` inside test fixture script literal | ℹ️ Info | Intentional test input, not implementation placeholder. |

No blocker or warning anti-patterns found in Phase-5 implementation artifacts.

### Human Verification Required

None for this phase scope. All must-have truths are covered by deterministic runtime/harness/test262-lite automated checks.

### Gaps Summary

No gaps found. Phase 5 goal is achieved for the scoped BUI-01/BUI-02/BUI-03 contract.

---

_Verified: 2026-02-26T18:23:13.8214985Z_
_Verifier: Claude (gsd-verifier)_
