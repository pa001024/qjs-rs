# Phase 1: Semantic Core Closure - Research

**Researched:** 2026-02-25
**Domain:** Eval, lexical environment, completion values, descriptor invariants
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Implement direct vs indirect `eval` per target specification semantics; no project-specific simplification.
- Preserve strict vs non-strict `eval` behavior as defined by target semantics.
- Keep `indirect eval` visibility global-only; do not expose caller lexical bindings.
- Preserve original observable error categories and precedence (`SyntaxError`, `ReferenceError`, `TypeError`, etc.) rather than collapsing error types.
- Preserve target `this` behavior distinctions for direct/indirect and strict/non-strict execution.
- Implement lexical environment rules strictly by target semantics across nested blocks, functions, and loops.
- Preserve shadowing and capture timing behavior per specification; no convenience rewrites.
- Keep behavior deterministic when lexical bindings interact with existing runtime constructs (including currently supported `with`/eval paths) without introducing custom semantics.
- Implement `Object.defineProperty`, `Object.defineProperties`, and `Object.getOwnPropertyDescriptor` edge behavior according to target descriptor invariants.
- Illegal descriptor transitions must fail deterministically with appropriate semantic error behavior (no silent fallback).
- Keep property attribute consistency (`configurable`, `enumerable`, `writable`, accessor/data exclusivity) strict and spec-aligned.

### Claude's Discretion
- Test organization, fixture naming, and coverage layering (unit/integration/test262 subset split) are at Claude's discretion.
- Internal refactor granularity is at Claude's discretion as long as semantics stay aligned and scope does not expand.

### Deferred Ideas (OUT OF SCOPE)
None - discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SEM-01 | Engine executes direct and indirect `eval` with observable behavior aligned to current target semantics for scope, strict mode, and exception propagation. | Keep `EvalCallKind` split, enforce direct-call opcode path (`CallIdentifier*`), maintain strict injection and global-vs-local scope switching logic in VM. |
| SEM-02 | Engine preserves lexical environment correctness for closures, block scopes, and function boundaries under nested control flow. | Keep `scopes + var_scope_stack + with_objects + IdentifierReference` model; preserve per-iteration lexical scope lowering and TDZ (`Uninitialized`) behavior. |
| SEM-03 | Engine implements consistent completion-value behavior across `if/switch/label/try-finally/loop` control flow without panic paths. | Continue compiler-side completion temp strategy (`$__loop_completion_*`, static-empty completion checks) and VM-side handler unwind/rethrow path. Add regression grid for nested abrupt completion combos. |
| SEM-04 | Engine enforces object/property descriptor invariants for `Object.defineProperty/defineProperties/getOwnPropertyDescriptor` in edge cases. | Keep all descriptor transitions centralized in `execute_object_define_property`, verify non-configurable transitions and array-length invariants, and align `getOwnPropertyDescriptor(s)` synthesis rules. |
</phase_requirements>

## Summary

Phase 1 is not greenfield: the codebase already has working semantic machinery for all four requirements. Planning should focus on tightening determinism and proving invariants under cross-feature combinations, not introducing a new architecture.

The highest leverage plan is to treat Phase 1 as semantic hardening in four vertical tracks (eval, lexical scope, completion records, descriptors), each with targeted regression matrices and minimal refactors around existing central paths. The major risk is false confidence from broad passing subsets while strict-mode and edge-path intersections remain under-sampled.

**Primary recommendation:** Plan Phase 1 around focused regression expansion and small, centralized fixes in existing VM/bytecode hot paths, not broad subsystem rewrites.

## Standard Stack

### Core
| Library/Crate | Version | Purpose | Why Standard |
|---------------|---------|---------|--------------|
| `parser` | workspace `0.1.0` | Syntax + strict/early-error shaping | Already performs Annex B lowering and strict checks used by VM semantics. |
| `bytecode` | workspace `0.1.0` | Semantic lowering and completion orchestration | Owns completion temp strategy and per-iteration scope lowering. |
| `vm` | workspace `0.1.0` | Runtime semantics execution | Central implementation for eval call-kind split, env resolution, exception/completion, and descriptors. |
| `runtime` | workspace `0.1.0` | Value model (`JsValue`, `Realm`, native function enum) | Stable ABI for parser/bytecode/vm/builtins handshake. |
| `test-harness` | workspace `0.1.0` | Script-level and test262-lite validation | Existing semantic regression surface for this phase. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `fancy-regex` | `0.14` | RegExp execution backend in VM | Not core to SEM-01..04 but already part of runtime behavior affected by eval script execution. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Existing `IdentifierReference` + scope stack model | New environment abstraction layer | Higher churn and regression risk with little phase-local gain. |
| Current completion-temp bytecode strategy | VM-only completion reconstruction | Harder to reason/debug; increases abrupt-completion coupling in runtime loop. |

**Build/Test commands:**
```bash
cargo test
cargo test -p test-harness
```

## Architecture Patterns

### Recommended Project Structure
```text
crates/parser      # strict/early errors + Annex B parsing shape
crates/bytecode    # semantic lowering and completion temp emission
crates/vm          # eval/scope/descriptor runtime semantics
crates/test-harness# script and test262-lite behavioral verification
```

### Pattern 1: Eval Call-Kind Split at Dispatch Boundary
**What:** Distinguish direct vs indirect `eval` by opcode form, not by string inspection.
**When to use:** Any change touching call lowering, eval semantics, or strict propagation.
**Example:**
```rust
// bytecode: identifier call -> direct eval candidate
Opcode::CallIdentifier { name: "eval", .. }
// vm: native call entry -> indirect eval
NativeFunction::Eval => execute_eval_argument(..., EvalCallKind::Indirect)
```
**Anchors:** `crates/bytecode/src/lib.rs:2032`, `crates/vm/src/lib.rs:2480`, `crates/vm/src/lib.rs:6529`, `crates/vm/src/lib.rs:7873`

### Pattern 2: Reference-First Identifier Semantics
**What:** Use `IdentifierReference` (`Binding`/`Property`/`Unresolvable`) for load/store and `with` interaction.
**When to use:** Changes to lexical lookup, sloppy global writes, strict missing-binding errors.
**Example:**
```rust
let reference = self.resolve_identifier_reference(name, realm, strict)?;
self.store_identifier_reference_value(reference, value, realm, strict)?;
```
**Anchors:** `crates/vm/src/lib.rs:13049`, `crates/vm/src/lib.rs:13140`

### Pattern 3: Compiler-Owned Completion Value Aggregation
**What:** Bytecode compiler precomputes completion candidate behavior and uses temp bindings for loop/switch/label/try paths.
**When to use:** Any SEM-03 work; do not bypass with ad-hoc VM fixes first.
**Example:**
```rust
if keep_value {
    let name = self.next_loop_completion_temp_name();
    code.push(Opcode::DefineVariable { name, mutable: true });
}
```
**Anchors:** `crates/bytecode/src/lib.rs:275`, `crates/bytecode/src/lib.rs:599`, `crates/bytecode/src/lib.rs:945`, `crates/bytecode/src/lib.rs:1308`

### Pattern 4: Centralized Descriptor Transition Validator
**What:** All `defineProperty/defineProperties` semantics funnel through `execute_object_define_property`.
**When to use:** Any descriptor edge-case, especially non-configurable transitions and array length writes.
**Example:**
```rust
if (has_get || has_set) && (has_value || desc_writable.is_some()) {
    return Err(VmError::TypeError("cannot have setter/getter and value or writable"));
}
```
**Anchors:** `crates/vm/src/lib.rs:10082`, `crates/vm/src/lib.rs:10147`, `crates/vm/src/lib.rs:10595`, `crates/vm/src/lib.rs:10994`

### Anti-Patterns to Avoid
- **Ad-hoc eval mode flags outside `EvalCallKind`:** leads to direct/indirect drift.
- **Direct scope map mutation bypassing reference path:** breaks `with`/strict/global-write semantics.
- **Descriptor writes through raw object maps in feature code:** bypasses invariant checks and creates silent divergence.
- **Fixing completion bugs only in VM control loop:** tends to regress other control-flow forms; fix lowering first.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Direct vs indirect eval detection | String/name heuristics during runtime call | Existing opcode + `EvalCallKind` dispatch | Already encodes spec-relevant distinction correctly. |
| Lexical lookup across scopes/with/global | New custom scope walk per feature | `resolve_identifier_reference` and `store/load_identifier_reference_value` | Keeps TDZ, sloppy/strict, and with-base semantics coherent. |
| Completion reconciliation for complex control flow | Per-opcode manual value juggling | Bytecode completion temps + existing `RethrowIfException` unwind path | Proven pattern, easier to audit in compile output. |
| Descriptor invariants | Feature-local property mutation helpers | `execute_object_define_property` / `execute_object_define_properties` | Single gate for invariant enforcement and deterministic errors. |

**Key insight:** SEM-01..04 success depends more on preserving semantic choke points than adding new surface area.

## Common Pitfalls

### Pitfall 1: Over-trusting current test262 pass rates
**What goes wrong:** Planning assumes strict-mode edge behavior is fully covered.
**Why it happens:** `test262` runner currently skips `onlyStrict/module/async` cases.
**How to avoid:** Add phase-local strict regression buckets in `test-harness` for each SEM requirement.
**Warning signs:** Fixes pass existing suites but regress strict-specific scripts.
**Anchor:** `crates/test-harness/src/test262.rs:124`

### Pitfall 2: Eval scope restoration regressions
**What goes wrong:** Eval mutates active scope state beyond call lifetime.
**Why it happens:** Direct/indirect eval rewires `scopes`, `var_scope_stack`, and `with_objects`.
**How to avoid:** Preserve save/restore envelope and add nested eval + with + strict tests.
**Warning signs:** Post-eval variable visibility changes unexpectedly.
**Anchor:** `crates/vm/src/lib.rs:7930`

### Pitfall 3: Completion value corruption in nested abrupt flow
**What goes wrong:** `break/continue/return/throw` interactions lose prior completion value or trigger runtime underflow.
**Why it happens:** Missing temp resets or incorrect finally unwind composition.
**How to avoid:** Validate compile output for nested `if/switch/label/try-finally/loop` combinations before VM changes.
**Warning signs:** `StackUnderflow` or wrong expression result after control transfer.
**Anchors:** `crates/bytecode/src/lib.rs:539`, `crates/bytecode/src/lib.rs:1308`, `crates/vm/src/lib.rs:11926`

### Pitfall 4: Descriptor synthesis drift between define/getOwnPropertyDescriptor
**What goes wrong:** `defineProperty` accepts states that `getOwnPropertyDescriptor` does not reflect consistently.
**Why it happens:** Multiple descriptor paths (objects/functions/native/host, synthetic defaults).
**How to avoid:** Pair every transition test with descriptor readback assertions.
**Warning signs:** Passes mutation test but fails descriptor shape/invariant checks.
**Anchors:** `crates/vm/src/lib.rs:10082`, `crates/vm/src/lib.rs:10994`

## Code Examples

### Direct eval dispatch boundary
```rust
if name == "eval" && matches!(callee, JsValue::NativeFunction(NativeFunction::Eval)) {
    return self.execute_eval_argument(args.first(), realm, caller_strict, EvalCallKind::Direct);
}
```
Source: `crates/vm/src/lib.rs:2493`

### Strict-forced direct eval parse
```rust
let force_strict = matches!(call_kind, EvalCallKind::Direct) && caller_strict;
let parse_source = if force_strict {
    Cow::Owned(format!("\"use strict\";\n{source}"))
} else {
    Cow::Borrowed(source)
};
```
Source: `crates/vm/src/lib.rs:7896`

### Non-configurable descriptor transition rejection
```rust
if (current_is_data || current_is_accessor) && !current_attributes.configurable {
    if desc_configurable == Some(true) {
        return Err(VmError::TypeError("cannot redefine non-configurable property"));
    }
}
```
Source: `crates/vm/src/lib.rs:10278`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Mixed eval behavior with limited separation | Explicit `EvalCallKind` + direct/indirect dispatch split | In-place by current VM baseline | Clear planning seam for SEM-01 without structural rewrite. |
| Completion handling prone to empty-statement pollution | Compiler static-empty completion filtering + dedicated completion temps | Current bytecode baseline | Deterministic value propagation in complex control flow. |
| Descriptor behavior fragmented | Centralized `defineProperty/defineProperties` invariants + descriptor readback APIs | Current VM baseline | SEM-04 can focus on edge matrices instead of API scaffolding. |

**Deprecated/outdated for planning**
- Broad "rewrite env model" proposals: high risk, low phase-local value.
- Assuming test262 strict coverage is sufficient: currently false due skip rules.

## Open Questions

1. **Should Phase 1 include enabling strict test262 slices, or stay in targeted custom strict tests?**
   - What we know: `onlyStrict` is skipped in current harness path.
   - What's unclear: Whether roadmap intends harness capability expansion in this phase.
   - Recommendation: Keep scope by adding targeted strict regression files now; defer harness expansion unless explicitly approved.

2. **How far should no-panic guarantees go for SEM-03?**
   - What we know: Runtime paths convert many failures to typed errors, but compiler has internal `expect` invariants.
   - What's unclear: Whether phase gate includes compiler panic hardening or runtime-only guarantee.
   - Recommendation: Treat runtime no-panic as must-have; add a separate backlog item for compiler invariant hardening.

3. **Descriptor synthesis policy consistency across object/function/native/host targets**
   - What we know: `getOwnPropertyDescriptor` has target-specific synthesis logic.
   - What's unclear: Remaining divergence from target semantics in rarely used properties.
   - Recommendation: Add cross-target descriptor parity tests for representative keys before feature expansion.

## Sources

### Primary (HIGH confidence)
- `.planning/phases/01-semantic-core-closure/01-CONTEXT.md` - locked decisions and scope boundaries
- `.planning/REQUIREMENTS.md` - SEM-01..SEM-04 requirement definitions
- `.planning/ROADMAP.md` - phase goal and success criteria
- `.planning/STATE.md` - current milestone position
- `crates/bytecode/src/lib.rs` - completion lowering and lexical scope compilation
- `crates/vm/src/lib.rs` - eval semantics, identifier resolution, completion unwind, descriptor invariants
- `crates/parser/src/lib.rs` - strict/lexical early errors and Annex B lowering tests
- `crates/test-harness/src/lib.rs` - script-level semantic regression coverage
- `crates/test-harness/src/test262.rs` - skip policy showing strict/module/async blind spots
- `docs/current-status.md` - latest convergence notes and known semantic gap areas
- `docs/semantics-checklist.md` - current semantic status tracking

### Secondary (MEDIUM confidence)
- `docs/quickjs-mapping.md` - alignment intent and implementation map
- `docs/risk-register.md` - ongoing risk framing for eval/with/strict and completion regressions

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - directly observed in workspace crates and wiring.
- Architecture: HIGH - validated against current parser/bytecode/vm control paths.
- Pitfalls: MEDIUM - based on code + docs, but strict-only matrix still partially inferred from skip policy.

**Research date:** 2026-02-25
**Valid until:** 2026-03-27
