---
phase: 06-collection-and-regexp-semantics
verified: 2026-02-27T05:00:09Z
status: passed
score: 9/9 must-haves verified
---

# Phase 6: Collection and RegExp Semantics Verification Report

**Phase Goal:** Collections and regular expressions use dedicated semantics aligned with targeted runtime behavior.
**Verified:** 2026-02-27T05:00:09Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | `WeakMap` and `WeakSet` are dedicated constructors/prototypes, not aliases of `Map` and `Set`. | ✓ VERIFIED | Dedicated runtime variants exist in `crates/runtime/src/lib.rs:46`, `crates/runtime/src/lib.rs:49`; globals wire to dedicated constructors in `crates/builtins/src/lib.rs:99`, `crates/builtins/src/lib.rs:105`; VM dispatches dedicated constructors in `crates/vm/src/lib.rs:7798`, `crates/vm/src/lib.rs:7802`; dedicated prototype builders in `crates/vm/src/lib.rs:16958`, `crates/vm/src/lib.rs:17007`. |
| 2 | `Map/Set` preserve SameValueZero key semantics and live iteration behavior under in-loop mutations. | ✓ VERIFIED | SameValueZero comparator implemented in `crates/vm/src/lib.rs:22117`; used in Map/Set operations in `crates/vm/src/lib.rs:6838`, `crates/vm/src/lib.rs:7007`; live iteration behavior via per-step fresh entry reads in `crates/vm/src/lib.rs:6916`, `crates/vm/src/lib.rs:7066`, iterator next in `crates/vm/src/lib.rs:9876`, `crates/vm/src/lib.rs:9998`; integration assertions in `crates/test-harness/tests/collection_semantics.rs:30`. |
| 3 | `WeakMap/WeakSet` reject non-object keys with deterministic `TypeError` behavior in constructor and method paths. | ✓ VERIFIED | Weak key guard in `crates/vm/src/lib.rs:4521`; method-path enforcement in `crates/vm/src/lib.rs:6943`, `crates/vm/src/lib.rs:7125`; constructor iterable enforcement/fail-fast in `crates/vm/src/lib.rs:10539`, `crates/vm/src/lib.rs:10557`, `crates/vm/src/lib.rs:10569`; integration assertions in `crates/test-harness/tests/collection_semantics.rs:60`, `crates/test-harness/tests/collection_semantics.rs:96`. |
| 4 | `RegExp` constructor accepts supported flags (`g/i/m/s/u/y`) and rejects unsupported patterns/flags with deterministic `SyntaxError`. | ✓ VERIFIED | Constructor path in `crates/vm/src/lib.rs:8162`; supported-flag validation in `crates/vm/src/lib.rs:8448`; compile input validation and pattern failure as `SyntaxError` in `crates/vm/src/lib.rs:8474`, `crates/vm/src/lib.rs:8485`; integration assertions in `crates/test-harness/tests/regexp_semantics.rs:72`. |
| 5 | `RegExp.prototype.exec` and `RegExp.prototype.test` share one match core and apply consistent `lastIndex` transitions. | ✓ VERIFIED | Both dispatch paths call the same core `execute_regexp_match` in `crates/vm/src/lib.rs:7358`, `crates/vm/src/lib.rs:7368`; `lastIndex` transitions centralized in `crates/vm/src/lib.rs:8555`, `crates/vm/src/lib.rs:8590`; integration assertions in `crates/test-harness/tests/regexp_semantics.rs:28`. |
| 6 | `RegExp.prototype.toString` emits stable normalized `/source/flags` output while preserving observable constructor state. | ✓ VERIFIED | Flag normalization in `crates/vm/src/lib.rs:8462`; normalized flags applied on construction in `crates/vm/src/lib.rs:8229`, `crates/vm/src/lib.rs:8254`; toString output format in `crates/vm/src/lib.rs:8550`; integration assertions in `crates/test-harness/tests/regexp_semantics.rs:11`. |
| 7 | Phase 6 introduces explicit collection and RegExp test262-lite gates that run green with existing Phase 5 gates unchanged. | ✓ VERIFIED | Phase 6 gate test exists in `crates/test-harness/tests/test262_lite.rs:168`; Phase 5 gate remains in same file `crates/test-harness/tests/test262_lite.rs:149`; CI runs workspace tests plus explicit Phase 6 step in `.github/workflows/ci.yml:28`, `.github/workflows/ci.yml:31`; direct gate chain execution passed (see verification run results). |
| 8 | CI command contracts for Phase 6 are fixed and reproducible for VM tests, harness integration tests, and test262-lite subset tests. | ✓ VERIFIED | CI command contract is explicit in `.github/workflows/ci.yml:33`, `.github/workflows/ci.yml:39`; commands execute successfully in this verification run (`weak_collection_constructor_identity`, `collection_semantics_same_value_zero_and_live_iteration`, `regexp_last_index_transition_matrix`, `regexp_exec_capture_and_constructor_errors`, harness suites, and `collection_and_regexp_subset`). |
| 9 | Baseline documentation records Phase 6 gate commands and measured outcomes for repeatable regression tracking. | ✓ VERIFIED | Contract and outcomes documented in `docs/test262-baseline.md:22`, `docs/test262-baseline.md:43`, with explicit non-regression clause at `docs/test262-baseline.md:45`. |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/runtime/src/lib.rs` | Dedicated constructor variants for collections/regexp | ✓ VERIFIED | Exists; substantive enum variants for `Map/Set/WeakMap/WeakSet/RegExp` in `crates/runtime/src/lib.rs:46`, `crates/runtime/src/lib.rs:60`; wired via builtins + VM dispatch. |
| `crates/builtins/src/lib.rs` | Baseline globals map to dedicated constructors | ✓ VERIFIED | Exists; dedicated global wiring for `WeakMap/WeakSet/RegExp` in `crates/builtins/src/lib.rs:99`, `crates/builtins/src/lib.rs:117`; wired to VM native dispatch. |
| `crates/vm/src/lib.rs` | Dedicated collection + regexp semantics implementation | ✓ VERIFIED | Exists; substantive constructor/method/validation/iteration logic across `crates/vm/src/lib.rs:6830`, `crates/vm/src/lib.rs:10350`, `crates/vm/src/lib.rs:8224`, `crates/vm/src/lib.rs:8590`; wired by native and host dispatch. |
| `crates/test-harness/tests/collection_semantics.rs` | Script-level collection semantic regression coverage | ✓ VERIFIED | Exists; 4 semantic tests with positive + error edges in `crates/test-harness/tests/collection_semantics.rs:11`, `crates/test-harness/tests/collection_semantics.rs:96`; wired via CI step. |
| `crates/test-harness/tests/regexp_semantics.rs` | Script-level regexp semantic regression coverage | ✓ VERIFIED | Exists; 4 semantic tests for clone/flags/exec-test/constructor errors in `crates/test-harness/tests/regexp_semantics.rs:11`, `crates/test-harness/tests/regexp_semantics.rs:72`; wired via CI step. |
| `crates/test-harness/tests/test262_lite.rs` | Exact-name subset gate for phase 6 families | ✓ VERIFIED | Exists; exact-name gate `collection_and_regexp_subset` in `crates/test-harness/tests/test262_lite.rs:168`; wired in CI and executed in this verification run. |
| `crates/test-harness/fixtures/test262-lite/pass/built-ins/Map/core-smoke.js` | Map smoke fixture with positive + boundary assertions | ✓ VERIFIED | Exists; SameValueZero/live iteration assertions and TypeError edges in `crates/test-harness/fixtures/test262-lite/pass/built-ins/Map/core-smoke.js:4`, `crates/test-harness/fixtures/test262-lite/pass/built-ins/Map/core-smoke.js:31`; consumed by `collection_and_regexp_subset`. |
| `crates/test-harness/fixtures/test262-lite/pass/built-ins/Set/core-smoke.js` | Set smoke fixture with positive + boundary assertions | ✓ VERIFIED | Exists; SameValueZero/live iteration and TypeError edges in `crates/test-harness/fixtures/test262-lite/pass/built-ins/Set/core-smoke.js:4`, `crates/test-harness/fixtures/test262-lite/pass/built-ins/Set/core-smoke.js:19`; consumed by `collection_and_regexp_subset`. |
| `crates/test-harness/fixtures/test262-lite/pass/built-ins/WeakMap/core-smoke.js` | WeakMap smoke fixture with object-key + fail-fast boundaries | ✓ VERIFIED | Exists; object-key semantics and fail-fast iterable edge in `crates/test-harness/fixtures/test262-lite/pass/built-ins/WeakMap/core-smoke.js:4`, `crates/test-harness/fixtures/test262-lite/pass/built-ins/WeakMap/core-smoke.js:45`; consumed by gate. |
| `crates/test-harness/fixtures/test262-lite/pass/built-ins/WeakSet/core-smoke.js` | WeakSet smoke fixture with object-value + fail-fast boundaries | ✓ VERIFIED | Exists; object-value semantics and fail-fast iterable edge in `crates/test-harness/fixtures/test262-lite/pass/built-ins/WeakSet/core-smoke.js:4`, `crates/test-harness/fixtures/test262-lite/pass/built-ins/WeakSet/core-smoke.js:41`; consumed by gate. |
| `crates/test-harness/fixtures/test262-lite/pass/built-ins/RegExp/core-smoke.js` | RegExp smoke fixture with flags/exec/captures/error boundaries | ✓ VERIFIED | Exists; normalized flags, lastIndex transitions, capture materialization, SyntaxError edges in `crates/test-harness/fixtures/test262-lite/pass/built-ins/RegExp/core-smoke.js:15`, `crates/test-harness/fixtures/test262-lite/pass/built-ins/RegExp/core-smoke.js:34`; consumed by gate. |
| `.github/workflows/ci.yml` | Additive Phase 6 gate chain in CI | ✓ VERIFIED | Exists; explicit phase step in `.github/workflows/ci.yml:31` with all Phase 6 commands in `.github/workflows/ci.yml:33`, `.github/workflows/ci.yml:39`. |
| `docs/test262-baseline.md` | Documented command contract + measured outcomes | ✓ VERIFIED | Exists; Phase 6 section and outcomes in `docs/test262-baseline.md:22`, `docs/test262-baseline.md:43`, additive non-regression requirement in `docs/test262-baseline.md:45`. |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `builtins::install_baseline` | Runtime native constructor variants | `WeakMap`/`WeakSet` globals map to dedicated `NativeFunction` values | ✓ WIRED | `crates/builtins/src/lib.rs:99` and `crates/builtins/src/lib.rs:105` map to dedicated variants declared in `crates/runtime/src/lib.rs:48` and `crates/runtime/src/lib.rs:49`; dispatched in `crates/vm/src/lib.rs:7798`, `crates/vm/src/lib.rs:7802`. |
| Collection host dispatch | Weak-key/object constraints | Method dispatch + helper validation | ✓ WIRED | Method paths enforce key checks in `crates/vm/src/lib.rs:6943`, `crates/vm/src/lib.rs:7093`; helper implemented in `crates/vm/src/lib.rs:4521`. |
| Constructor iterable ingestion | Fail-fast weak collection behavior | `add_collection_entries_from_iterable` error close-and-return | ✓ WIRED | WeakMap/WeakSet constructor ingestion calls helper in `crates/vm/src/lib.rs:10439`, `crates/vm/src/lib.rs:10488`; fail-fast path at `crates/vm/src/lib.rs:10569`. |
| `RegExpTestThis` dispatch | Shared regexp match core | `execute_regexp_match` | ✓ WIRED | `crates/vm/src/lib.rs:7358` and `crates/vm/src/lib.rs:7368` both route through shared matcher `crates/vm/src/lib.rs:8585`. |
| Match core output | `exec` return shape | Array + `index`/`input` property materialization | ✓ WIRED | Result array and properties built in `crates/vm/src/lib.rs:7372`, `crates/vm/src/lib.rs:7394`. |
| Match core transitions | `lastIndex` writable/transition semantics | `set_regexp_last_index` inside shared matcher | ✓ WIRED | Transition helper in `crates/vm/src/lib.rs:8555`; used in matcher failure/success paths at `crates/vm/src/lib.rs:8600`, `crates/vm/src/lib.rs:8654`. |
| `test262_lite` phase gate | Phase 6 fixture families | `collection_and_regexp_subset` loop | ✓ WIRED | Gate enumerates `Map/Set/WeakMap/WeakSet/RegExp` in `crates/test-harness/tests/test262_lite.rs:169`. |
| CI workflow | Phase 6 gate execution | Explicit gate step commands | ✓ WIRED | CI executes full chain in `.github/workflows/ci.yml:31`, `.github/workflows/ci.yml:39`. |
| Baseline docs | CI contract parity | Mirrored command list + outcomes | ✓ WIRED | Docs match CI command set in `docs/test262-baseline.md:27`, `docs/test262-baseline.md:34` and outcomes at `docs/test262-baseline.md:37`, `docs/test262-baseline.md:43`. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| BUI-04 | `06-01-PLAN.md`, `06-03-PLAN.md` | `Map/Set/WeakMap/WeakSet` use dedicated semantics (no baseline constructor alias shortcuts). | ✓ SATISFIED | Dedicated constructors/prototypes and dispatch in `crates/runtime/src/lib.rs:46`, `crates/builtins/src/lib.rs:99`, `crates/vm/src/lib.rs:7798`, `crates/vm/src/lib.rs:16958`; SameValueZero + live iteration + weak key constraints in `crates/vm/src/lib.rs:6838`, `crates/vm/src/lib.rs:6916`, `crates/vm/src/lib.rs:4521`; regression coverage in `crates/test-harness/tests/collection_semantics.rs:11` and test262-lite phase gate `crates/test-harness/tests/test262_lite.rs:168`. |
| BUI-05 | `06-02-PLAN.md`, `06-03-PLAN.md` | RegExp constructor and prototype methods (`exec/test/toString`) preserve flags and match behavior for supported patterns. | ✓ SATISFIED | RegExp constructor + validation/normalization in `crates/vm/src/lib.rs:8162`, `crates/vm/src/lib.rs:8448`, `crates/vm/src/lib.rs:8462`; shared exec/test core and lastIndex transitions in `crates/vm/src/lib.rs:7358`, `crates/vm/src/lib.rs:8585`; toString in `crates/vm/src/lib.rs:8550`; regression coverage in `crates/test-harness/tests/regexp_semantics.rs:11` and phase gate `crates/test-harness/tests/test262_lite.rs:168`. |

Phase 6 orphaned-requirement check: none. REQUIREMENTS maps Phase 6 to only BUI-04/BUI-05, and both are declared in Phase 6 plan frontmatter.

### Anti-Patterns Found

No blocker/warning anti-patterns were found in phase key files (`TODO/FIXME/placeholder stubs`, empty implementations, console-log-only handlers).

### Human Verification Required

None for phase-goal acceptance. The goal is runtime semantic behavior and is covered by direct code-path inspection plus automated gate execution.

### Verification Notes

- `gsd-tools verify artifacts` and `gsd-tools verify key-links` could not parse current frontmatter shape (`must_haves.artifacts` and `must_haves.key_links` are string arrays, not structured objects). Manual artifact and wiring verification was performed with line-level evidence instead.
- Executed verification command chain succeeded:
  - `cargo test -p vm weak_collection_constructor_identity -- --exact`
  - `cargo test -p vm collection_semantics_same_value_zero_and_live_iteration -- --exact`
  - `cargo test -p vm regexp_last_index_transition_matrix -- --exact`
  - `cargo test -p vm regexp_exec_capture_and_constructor_errors -- --exact`
  - `cargo test -p test-harness --test collection_semantics`
  - `cargo test -p test-harness --test regexp_semantics`
  - `cargo test -p test-harness --test test262_lite collection_and_regexp_subset -- --exact`

---

_Verified: 2026-02-27T05:00:09Z_
_Verifier: Claude (gsd-verifier)_
