# Phase 13 Research: Boa host class / prototype alignment

**Phase:** 13  
**Date:** 2026-03-03  
**Status:** complete

## Objective

Identify the exact implementation anchors needed to align qjs-rs host class/prototype semantics with Boa-style behavior for:

- constructor `new` enforcement
- prototype fallback behavior
- `prototype.constructor` linkage invariants
- prototype mutation safety (`Object.setPrototypeOf`)

## Sources Read

- `.planning/phases/13-boa-host-class-prototype-d-dev-dna-builder-src-tauri-src-submodules-jsmat-rs/13-CONTEXT.md`
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `.planning/REQUIREMENTS.md`
- `crates/vm/src/external_host.rs`
- `crates/vm/src/lib.rs`
- `crates/vm/tests/collection_semantics.rs`
- `crates/vm/tests/native_errors.rs`
- `crates/test-harness/tests/rust_host_bindings.rs`
- `boa/core/engine/src/class.rs`
- `boa/core/engine/src/object/mod.rs`
- `boa/core/engine/src/context/intrinsics.rs`

## Key Findings (qjs-rs current state)

1. **Host callback registration and constructability already exist**
   - `register_host_callback_function` stores `constructable` in host metadata (`crates/vm/src/external_host.rs:38`).
   - `host_function_is_constructable` routes constructor capability checks (`crates/vm/src/external_host.rs:123`).

2. **Host function prototype creation already does constructor backlink with expected attributes**
   - `get_or_create_host_function_prototype_property` lazily creates `prototype` and sets `prototype.constructor = HostFunction(host_id)` with `writable=true, enumerable=false, configurable=true` (`crates/vm/src/external_host.rs:133`).

3. **Host construction path already uses prototype lookup/fallback hook**
   - In `execute_construct_value`, host function construction calls `get_or_create_host_function_prototype_property` before applying prototype components (`crates/vm/src/lib.rs:4376`).

4. **`Object.setPrototypeOf` safety checks are centralized and broad**
   - `execute_object_set_prototype_of` performs:
     - target validation (`null/undefined` => TypeError)
     - prototype parsing via `parse_prototype_value`
     - cycle prevention via `prototype_would_create_cycle`
     - extensibility checks across object/function/host/native targets
   - Anchors: `crates/vm/src/lib.rs:12747`, `crates/vm/src/lib.rs:12836`, `crates/vm/src/lib.rs:12863`.

5. **Current tests cover some invariants but not host-specific fallback mutation scenarios**
   - Constructor/new enforcement and constructor backlink exist for built-ins (`crates/vm/tests/collection_semantics.rs:15`, `crates/vm/tests/collection_semantics.rs:18`, `crates/vm/tests/native_errors.rs:16`).
   - Host constructor `this` binding is exercised (`crates/test-harness/tests/rust_host_bindings.rs:55`).
   - Missing: host callback prototype pollution/fallback and host-targeted `Object.setPrototypeOf` behavior matrix tests.

## Boa Reference Evidence (authoritative repo anchors)

1. **Native class constructors enforce `new`**
   - `Class::construct` rejects undefined `new_target` with TypeError (`boa/core/engine/src/class.rs:168`, `boa/core/engine/src/class.rs:171`).

2. **Prototype fallback behavior for class construction**
   - Boa reads `new_target.prototype`; when absent/non-object, falls back to registered class prototype from realm class map (`boa/core/engine/src/class.rs:179`).

3. **Constructor/prototype linkage is explicitly wired by constructor builder**
   - Constructor `prototype` property descriptor is created `writable=false, enumerable=false, configurable=false` and prototype gets `constructor` backlink with `writable=true, enumerable=false, configurable=true` (`boa/core/engine/src/object/mod.rs:927`, `boa/core/engine/src/object/mod.rs:943`).

4. **Prototype mutation safety respects extensibility/identity invariants**
   - `Object::set_prototype` only changes prototype when extensible; otherwise only allows SameValue no-op (`boa/core/engine/src/object/mod.rs:265`).

5. **Standard constructors/prototypes are explicitly represented and cached**
   - `StandardConstructor` model in intrinsics clarifies constructor/prototype pair ownership (`boa/core/engine/src/context/intrinsics.rs:74`).

## Risk Assessment

- **R1: semantic drift in fallback edge cases**
  - Trigger: host constructor `prototype` changed to non-object/null/function/object with unusual chain.
  - Mitigation: explicit matrix tests for each prototype value shape before/after mutation.

- **R2: hidden regressions in existing host callback behavior**
  - Trigger: adjusting construct path or property descriptors impacts existing host callback tests.
  - Mitigation: preserve existing test-harness coverage and add incremental tests instead of broad rewrites.

- **R3: overfitting to built-in constructor semantics, missing host-specific paths**
  - Trigger: relying only on `collection_semantics`/`native_errors` patterns.
  - Mitigation: add dedicated host callback prototype conformance test file in vm/test-harness.

## Gaps To Close In Phase 13 Plans

1. Add **host callback conformance tests** for:
   - call-without-new rejection for constructable-only semantics as required by design
   - prototype fallback when constructor `.prototype` is missing or non-object
   - `prototype.constructor` backlink integrity after refresh/recreation

2. Add **host-targeted `Object.setPrototypeOf` tests** for:
   - cycle detection
   - extensibility blocking
   - legal no-op SameValue mutation

3. Decide and lock **descriptor policy boundaries** between:
   - constructor object's `prototype` descriptor
   - prototype object's `constructor` descriptor
   while matching existing qjs-rs conventions and Boa behavior intent.

## Planning Guidance

- Prefer a 2-plan breakdown:
  1. **Semantics hardening + tests** (host construct/prototype invariants)
  2. **Mutation safety + integration case** (`Object.setPrototypeOf` on host entities + `jsmat.rs` usage path)
- Each plan should include:
  - explicit requirement IDs in frontmatter (if phase reqs are still TBD, define temporary phase-local IDs in plan text)
  - objective-level `must_haves`
  - verification commands focused on vm/test-harness target suites

## Proposed Verification Commands

- `cargo test -p vm collection_semantics -- --nocapture`
- `cargo test -p vm native_errors -- --nocapture`
- `cargo test -p test-harness rust_host_bindings -- --nocapture`
- `cargo test -p vm -- --nocapture` (final sweep after targeted checks)

---

Research concludes qjs-rs already contains most primitive hooks required by Phase 13. The highest-value work is targeted semantic hardening plus host-specific conformance tests for fallback and mutation edge cases.
