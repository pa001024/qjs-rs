# Phase 2: Runtime Safety and Root Integrity - Research

**Researched:** 2026-02-25
**Domain:** GC root completeness, handle lifecycle integrity, deterministic runtime failures
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
### Invalid/Stale Handle Error Contract
- Use deterministic typed runtime errors for all invalid/stale handle accesses; no silent recovery behavior.
- Keep error categories distinct for `InvalidHandle` vs `StaleHandle` to preserve diagnosis quality.
- Keep error type and message format stable enough for regression assertions.
- Fail fast at the first deterministic detection point.

### Root Coverage Boundary (Phase 2)
- Phase 2 must cover root scanning for stack frames, globals, module-cache candidates, and pending job-queue references.
- Do not defer any of these root categories to later phases.
- Root completeness in this phase is a hard gate, not a best-effort target.

### Safety Gate Pass Criteria
- Phase exit requires both functional correctness and stress-profile stability evidence.
- "No panic/undefined behavior under repeated allocation + collection" is mandatory.
- Deterministic typed failures are required for invalid-state paths.

### Exceptional Path Consistency
- Exceptional paths (GC checkpoints, stale-handle accesses, boundary failures) must be as deterministic and diagnosable as normal paths.
- Error contracts and observability are consistent across normal and exceptional control paths.

### Claude's Discretion
- Internal refactor layout and test-file decomposition are at Claude’s discretion.
- Specific naming of helper APIs and private runtime structs is at Claude’s discretion, as long as external semantic behavior matches locked decisions.

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MEM-01 | GC root management covers stack frames, globals, module cache candidates, and job queue references without stale-handle use. | Keep current root snapshot traversal as base (`collect_roots`), then add explicit Phase-2 root buckets for module-cache candidates and job-queue pending references, both included in root collection and tests. |
| MEM-02 | Runtime rejects invalid/stale object handles with deterministic typed errors instead of undefined behavior or panics. | Replace generic `UnknownObject`-style handle misses with deterministic classified errors (`InvalidHandle` vs `StaleHandle`) via a single handle-resolution gateway; route to stable runtime exception mapping and regression assertions. |
</phase_requirements>

## Summary

The repository already has a functional mark-sweep baseline, slot+generation object handles, runtime/boundary GC triggers, `gc_shadow_roots`, and stress guards. This means Phase 2 planning should be hardening and contract closure, not architecture replacement.

Two concrete gaps remain for this phase boundary. First, current root scanning does not yet expose explicit module-cache/job-queue root candidate containers in VM state; both categories are required by locked scope even if full module/job execution lands later. Second, stale/invalid-handle behavior is currently mostly surfaced as `UnknownObject` internals and is not yet a stable typed contract with separate `InvalidHandle`/`StaleHandle` categories.

**Primary recommendation:** Plan Phase 2 around four implementation tracks: (1) root-bucket completion for module/job candidates, (2) centralized handle classification, (3) deterministic typed error contract wiring, (4) dual-profile + failure-path regression gates.

## Standard Stack

### Core
| Library/Crate | Version | Purpose | Why Standard |
|---------------|---------|---------|--------------|
| `crates/vm` | workspace `0.1.0` | GC, object table, handle lifecycle, runtime errors | Existing implementation already owns `collect_roots`, mark/sweep, and slot+generation IDs; Phase 2 should stay here. |
| `crates/runtime` | workspace `0.1.0` | `JsValue` / `Realm` root ingress | `Realm.globals_values()` is already consumed by GC root collection. |
| `crates/test-harness` | workspace `0.1.0` | Stress/default profile verification + GC guard | Already includes suite GC aggregation and expectation gates. |
| `docs/*` GC docs | repo docs | Baseline contracts and risk framing | Existing memory/root/GC docs already define Phase-2-compatible intent; plan should align to these, not fork them. |

### Supporting
| Tool | Purpose | When to Use |
|------|---------|-------------|
| `cargo test` | Fast correctness gate | Per-task and per-plan check. |
| `test262-run --show-gc` | Default/stress observability | Gate profile-specific invariants and collection accounting. |
| `test262-run --expect-gc-baseline` | CI stability guard | Prevent silent GC metric regressions over time. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Explicit root buckets in VM | Implicit future roots when features land | Reintroduces omission risk for MEM-01 and causes phase-scope drift. |
| Classified handle errors | Keep `UnknownObject` as catch-all | Fails locked decision requiring separate `InvalidHandle` vs `StaleHandle`. |

## Architecture Patterns

### Recommended Project Structure
```text
crates/vm/src/lib.rs          # root collection, handle resolution, runtime error routing
crates/test-harness/src/*.rs  # gc profile execution + guard logic
crates/test-harness/tests/*.rs# suite-level profile assertions
docs/gc-*.md                  # threshold and risk documentation sync
```

### Pattern 1: Root Snapshot Is the Single Source of Reachability
**What:** Keep all reachability entry points in `collect_roots` and extend it with explicit Phase-2 categories.
**When to use:** Any module-cache/job-queue candidate addition; never root-scan ad hoc in sweep/mark call sites.
**Current anchor:** `crates/vm/src/lib.rs:783`
**Planned extension:** Add VM-owned candidate containers and append them in `collect_roots`.

### Pattern 2: One Handle Resolution Gateway
**What:** Route all object-handle reads/writes through one helper that classifies `InvalidHandle` vs `StaleHandle` before object access.
**When to use:** Every path that currently does `.objects.get(...).ok_or(VmError::UnknownObject(...))?`.
**Current anchors:** `crates/vm/src/lib.rs:764`, `crates/vm/src/lib.rs:1111`
**Planned extension:** `resolve_object_handle(object_id) -> Result<&JsObject, HandleError>`.

### Pattern 3: Deterministic Error Contract at Runtime Boundary
**What:** Preserve internal detail while mapping handle failures to stable typed runtime errors and stable messages.
**When to use:** `route_runtime_error_to_handler` / `runtime_error_exception_value`.
**Current anchor:** `crates/vm/src/lib.rs:11935`
**Planned extension:** Extend runtime error mapping to include classified handle errors with stable message format used in regression tests.

### Pattern 4: Dual-Profile Verification as a Phase Gate
**What:** Keep default profile invariants and stress profile reclamation invariants both required for merge.
**When to use:** Every change touching roots, handle lifecycle, GC trigger/checkpoint behavior.
**Current anchors:** `crates/vm/src/lib.rs:22539`, `crates/vm/src/lib.rs:22589`, `crates/test-harness/tests/test262_lite.rs:1`

### Anti-Patterns to Avoid
- **Adding roots only where feature code runs:** makes reachability nondeterministic and hard to audit.
- **Using generic `UnknownObject` for all handle failures:** loses stale-vs-invalid diagnostics required by locked decisions.
- **Relying only on stress GC runs:** misses default-profile lifecycle bugs.
- **Keeping `expect`/panic in caller-state restore paths for invariant enforcement:** violates Phase-2 no-panic objective on failure paths.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Module/job root coverage | Feature-local pin hacks | VM-level root candidate buckets included by `collect_roots` | Gives deterministic MEM-01 coverage and future-proof feature integration. |
| Handle validation | Per-call-site custom checks | Single `resolve_object_handle` helper + shared error classifier | Ensures consistent semantics and message stability. |
| Error shape | Free-form strings at call sites | Central runtime-error conversion contract | Locks deterministic typed behavior for MEM-02 regression assertions. |
| GC gate policy | One-off benchmark commands | Existing default+stress commands with baseline guard | Already integrated and measurable in CI/harness paths. |

## Common Pitfalls

### Pitfall 1: Root Coverage Drifts as New Containers Arrive
**What goes wrong:** New containers (module/job candidates) exist but are not in `collect_roots`.
**Why it happens:** Root logic is spread across feature code rather than centralized.
**How to avoid:** Enforce a rule that new reference-holding container changes must include `collect_roots` delta + root coverage tests.
**Warning signs:** Intermittent `UnknownObject` under runtime GC with new async/module scaffolding.

### Pitfall 2: `UnknownObject` Masks Handle Category
**What goes wrong:** Stale and invalid handles are indistinguishable and not regression-friendly.
**Why it happens:** Object lookup failure is currently treated as one bucket.
**How to avoid:** Add slot/generation-aware classifier and expose stable error tags/messages.
**Warning signs:** Tests can only assert "failed", not which contract branch fired.

### Pitfall 3: Exceptional Path Panic from Shadow Root Stack
**What goes wrong:** Caller-state restore can panic if shadow-root push/pop symmetry breaks.
**Why it happens:** `expect("caller state shadow roots should be present")` in restore path.
**How to avoid:** Convert to deterministic typed integrity error and add unwind-path regression tests.
**Warning signs:** Rare panic in nested calls + runtime GC + exception flow.

### Pitfall 4: Stress-Only Confidence
**What goes wrong:** Stress snapshots look healthy while default lifecycle invariants regress.
**Why it happens:** Throughput checks do not cover all deterministic failure contracts.
**How to avoid:** Keep both default and stress checks mandatory, including handle-error determinism tests.
**Warning signs:** Default profile behavior drifts while stress metrics remain stable.

## Code Examples

### Handle Classification Gateway (planned)
```rust
enum HandleErrorKind {
    InvalidHandle, // malformed / never-allocated / impossible generation
    StaleHandle,   // reclaimed or generation-mismatched old handle
}

fn classify_object_handle(&self, object_id: u64) -> HandleErrorKind {
    let slot = (object_id & OBJECT_ID_SLOT_MASK) as usize;
    let generation = (object_id >> OBJECT_ID_SLOT_BITS) as u32;
    if slot >= self.object_generations.len() {
        return HandleErrorKind::InvalidHandle;
    }
    let current = self.object_generations[slot];
    if generation > current {
        return HandleErrorKind::InvalidHandle;
    }
    if generation < current || !self.objects.contains_key(&object_id) {
        return HandleErrorKind::StaleHandle;
    }
    unreachable!("only called for invalid lookup path");
}
```

### Root Candidate Buckets for MEM-01 (planned)
```rust
struct Vm {
    // existing fields...
    module_root_candidates: BTreeMap<u64, JsValue>,
    job_queue_root_candidates: BTreeMap<u64, JsValue>,
}

fn collect_roots(&self, realm: &Realm) -> Vec<JsValue> {
    let mut roots = /* existing roots */;
    roots.extend(self.module_root_candidates.values().cloned());
    roots.extend(self.job_queue_root_candidates.values().cloned());
    roots
}
```

## State of the Art

| Old Approach | Current Approach | Needed for Phase 2 | Impact |
|--------------|------------------|--------------------|--------|
| No GC lifecycle hardening | Mark-sweep + runtime/boundary triggers + stress guard | Keep and tighten with explicit root-category completeness | Reuse existing architecture, avoid rewrite risk. |
| Flat object IDs | `slot+generation` object IDs | Keep and build error-classification on top | Enables deterministic stale-handle detection. |
| Generic unknown-handle surface | `UnknownObject` internal error paths | Add typed `InvalidHandle`/`StaleHandle` contract | Satisfies MEM-02 diagnosability and determinism requirements. |

## Open Questions

1. **Error surfacing level for handle failures**
   - What we know: runtime error conversion currently handles only select VM errors.
   - What's unclear: whether handle integrity errors must always become JS exceptions or sometimes host-only typed errors.
   - Recommendation: define one contract for script-observable paths and one for embedding API paths, both using the same error kind taxonomy.

2. **Root candidate API shape for not-yet-landed features**
   - What we know: module/job execution is planned later, but root categories are mandatory now.
   - What's unclear: whether to expose token-based registration APIs now or keep internal VM-only staging containers.
   - Recommendation: start VM-internal with deterministic tests; expose host-facing API only when embedding use cases require it.

3. **Panic policy on integrity invariants**
   - What we know: some paths still use `expect(...)` for internal assumptions.
   - What's unclear: whether all runtime-path `expect` should be eliminated in this phase or only handle/GC-adjacent paths.
   - Recommendation: require no panic for any path reachable by script execution under GC/checkpoint activity.

## Sources

### Primary (HIGH confidence)
- `.planning/phases/02-runtime-safety-and-root-integrity/02-CONTEXT.md` - locked decisions and scope.
- `.planning/REQUIREMENTS.md` - MEM-01 and MEM-02 definitions.
- `.planning/ROADMAP.md` - phase goal and success criteria.
- `.planning/STATE.md` - current phase transition context.
- `crates/vm/src/lib.rs` - current GC/root/handle/runtime-error implementation.
- `crates/test-harness/src/test262.rs` - suite execution options and GC summary wiring.
- `crates/test-harness/src/bin/test262-run.rs` - GC guard thresholds and profile gating.
- `crates/test-harness/tests/test262_lite.rs` - stress-mode invariants currently enforced.
- `docs/root-strategy.md` - root taxonomy baseline and phase intent.
- `docs/gc-design.md` - GC algorithm + slot/generation handle model.
- `docs/gc-test-plan.md` - regression matrix and profile gates.
- `docs/current-status.md` - latest implementation baseline and GC/stress evidence.

### Secondary (MEDIUM confidence)
- `docs/risk-register.md` - current lifecycle risk states and monitoring assumptions.
- `docs/quickjs-mapping.md` - roadmap-level module/job planned status.
- `.planning/research/PITFALLS.md` - known failure patterns for GC lifecycle regressions.

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - derived from current repository implementation and active CI/test harness usage.
- Architecture: HIGH - directly mapped to current VM root collection and handle lifecycle paths.
- Pitfalls: MEDIUM - strongly supported by repo history/docs, but some future module/job integration details remain to be finalized.

**Research date:** 2026-02-25
**Valid until:** 2026-03-27
