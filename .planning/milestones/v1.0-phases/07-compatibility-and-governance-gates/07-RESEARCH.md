# Phase 7: Compatibility and Governance Gates - Research

**Researched:** 2026-02-27  
**Domain:** MEM-03, TST-01, TST-02, TST-03, TST-04 governance/compatibility closure  
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Governance Gate Strictness
- Default-branch CI gates are hard-blocking: `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` must all pass to merge.
- Exceptions are not allowed by default and require an explicit exception workflow.
- Every approved exception must include an expiration date; expired exceptions are invalid automatically.
- Exception records must include: reason, impact scope, owner, and rollback/removal condition.

### GC Telemetry Baseline Strategy
- Maintain two independent threshold tracks: `baseline` and `stress`.
- Thresholds are fixed and adjusted only through versioned changes with documented rationale.
- Alert policy is two-tier: warning for mild drift, blocking for threshold breaches.
- Forced regression investigation is triggered on consecutive anomalies (not single-event noise).

### test262 Reporting Contract
- Report schema is fixed: `discovered`, `executed`, `failed`, and explicit `skipped` categories.
- Skip output must be category-granular (not aggregate-only).
- Produce dual outputs per tracked run: machine-readable JSON and human-readable Markdown.
- Archive compatibility snapshots by phase/milestone with reproducible diffability against previous snapshots.

### New Feature Test Compliance
- Compliance checks apply to runtime observable behavior changes.
- Minimum bar is strict `1 + 1`: at least one positive case and one boundary/error case per new feature.
- Exception path is limited to pure refactors with demonstrable no-semantic-change evidence.
- Enforce through mandatory PR checklist fields; incomplete checklist blocks merge.

### Claude's Discretion
- Exact file layout and command wiring for governance artifacts, as long as locked gate behavior is enforced.
- Naming and organization of report/telemetry outputs, provided fixed schema and comparability guarantees are preserved.
- Validation tooling implementation details, provided exception lifecycle and compliance policy remain deterministic.

### Deferred Ideas (OUT OF SCOPE)

None - discussion stayed within Phase 7 scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MEM-03 | GC telemetry reports stable baseline and stress profiles with documented thresholds and regression checks. | Existing `test262-run` GC stats + baseline thresholds already exist; planning must add deterministic profile artifacts, consecutive-anomaly policy, and drift governance. |
| TST-01 | CI keeps `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` green on default branch. | Current probe is not green; Phase 7 needs a first-wave CI restoration + hard gate enforcement workflow. |
| TST-02 | test262 reporting tracks discovered/executed/failed plus skip categories. | Current summary has aggregate `skipped` only; skip reason taxonomy and category counters must be added to harness/report schema. |
| TST-03 | Every new runtime feature lands with one positive and one boundary/error test. | Existing test style follows this pattern, but enforcement is policy-only; Phase 7 must add mandatory PR checklist + CI validation. |
| TST-04 | Repeatable compatibility snapshots + `docs/current-status.md` update after major convergence work. | Snapshot practice exists but is manual and target-dir-centric; plan needs reproducible snapshot contract, versioned manifests, and status update workflow. |
</phase_requirements>

## Summary

Phase 7 is a governance hardening phase, not a semantic expansion phase. The repository already has most technical primitives (CI workflow, GC guard thresholds, test262 runner, baseline docs), but enforcement is inconsistent and several phase outcomes are currently not satisfied in live runs.

Most important planning fact: the current baseline is not fully green. Local probes show `cargo fmt --check` fails on formatting drift, `cargo clippy --workspace --all-targets -- -D warnings` fails on `clippy::type_complexity` in parser, and `cargo test --workspace` fails because `runs_test262_lite_suite_in_stress_profile` currently reports 3 mismatches. This means TST-01 should be first execution wave, not a final polish step.

test262 reporting and snapshot governance also need contract upgrades. `test262-run` currently emits aggregate skip count and JSON only; Phase 7 requires explicit skip categories and parallel Markdown reports. Snapshot references are rich in docs, but process reproducibility and update discipline are still manual.

**Primary recommendation:** plan Phase 7 in this order: (1) restore and lock CI green baseline, (2) formalize GC baseline/stress telemetry governance with anomaly policy, (3) upgrade test262 reporting schema/output contracts, (4) enforce TST-03 via PR checklist + CI validator, (5) codify reproducible snapshot/update workflow.

## Current Evidence (2026-02-27)

### CI gate probes
- `cargo fmt --check`: failed (format drift in `crates/test-harness/tests/test262_lite.rs`, `crates/vm/src/lib.rs`).
- `cargo clippy --workspace --all-targets -- -D warnings`: failed (`clippy::type_complexity` in `crates/parser/src/lib.rs`).
- `cargo test --workspace`: failed because `crates/test-harness/tests/test262_lite.rs` stress profile test observed 3 mismatches.

### GC telemetry probes
- Default profile command (`test262-lite` root): `discovered=45, executed=45, passed=45, failed=0`, all GC stats zero.
- Stress profile command with baseline thresholds: `discovered=45, executed=45, passed=42, failed=3`, GC thresholds passed (`collections_total=46212`, `runtime_collections=46174`, `runtime_ratio~0.9992`, `reclaimed_objects=1750`) but semantic mismatches occurred.
- Current stress failures in this run:
  - `built-ins/JSON/parse-reviver-smoke.js`: `RuntimeFail("StaleHandle(6)")`
  - `built-ins/WeakMap/core-smoke.js`: assertion mismatch (`pulls` expected 2)
  - `built-ins/WeakSet/core-smoke.js`: assertion mismatch (`pulls` expected 2)

### test262 reporting probes
- Real test262 sample run (`--max-cases 1000`): `discovered=53162, executed=1000, skipped=555, passed=1000, failed=0`.
- Runner schema currently includes `discovered/executed/skipped/passed/failed` and GC block; no skip-category breakdown and no Markdown output writer.

### Governance artifact status
- No PR template exists under `.github/` to enforce `1+1` test compliance fields.
- Current Phase docs emphasize updates to `docs/current-status.md`, but enforcement is procedural rather than automated.
- `CLAUDE.md` absent; `.agents/skills/` absent (no additional project-local overrides from those locations).

## Standard Stack

### Core
| Component | Version/State | Purpose | Why Standard Here |
|---|---|---|---|
| GitHub Actions (`.github/workflows/ci.yml`) | existing | branch quality gates | Already hosts fmt/clippy/test + phase gates; extend rather than replace. |
| `test-harness` (`run_suite`, `test262-run`) | existing workspace crate | compatibility execution and summaries | Canonical place to add skip categories + JSON/Markdown report contract. |
| GC baseline file (`gc-guard.baseline`) | existing | threshold source of truth | Already versioned and parsed by CLI; build governance around this contract. |
| Docs (`docs/current-status.md`, `docs/test262-baseline.md`, `docs/gc-snapshot-report.md`) | existing | human-readable governance state | Keep as public status layer, but drive from reproducible artifacts. |

### Supporting
| Component | Purpose | When to Use |
|---|---|---|
| Baseline parser/merge logic in `test262-run` | enforce threshold contract + override rules | GC baseline/stress gate implementation and exception governance tooling. |
| Phase-local planning docs (`.planning/phases/...`) | lock requirement traceability and evidence | Map MEM-03/TST-* to explicit plan tasks and verification commands. |

## Architecture Patterns

### Pattern 1: Governance-as-Code Contract
Define one versioned governance spec (YAML/JSON/MD trio) that CI validates:
- required gates (`fmt`, `clippy`, `test`)
- GC profiles/thresholds (`baseline`, `stress`)
- exception records (`reason`, `owner`, `scope`, `expires_at`, `rollback_condition`)
- mandatory reporting outputs and artifact naming rules

### Pattern 2: Dual-profile GC Telemetry with Stateful Drift Policy
Keep existing profile split, but add explicit tracked-run lifecycle:
1. run baseline profile, persist JSON+MD snapshot
2. run stress profile, persist JSON+MD snapshot
3. compare against thresholds and previous snapshot
4. classify drift as warning/blocking
5. trigger mandatory investigation on consecutive anomalies

### Pattern 3: test262 Report Schema v2
Extend `SuiteSummary` with skip taxonomy and emit both output types per tracked run:
- machine JSON (stable fields, diff-friendly)
- human Markdown (table + category totals + top failures)

Recommended skip categories based on current skip logic:
- `fixture_file`
- `flag_module`
- `flag_only_strict`
- `flag_async`
- `requires_includes`
- `requires_feature`
- `requires_harness_global_262`

### Pattern 4: TST-03 Enforced at PR Boundary
Make `1+1` compliance non-optional:
- add PR template with mandatory fields:
  - feature behavior changed
  - positive test reference
  - boundary/error test reference
  - exception evidence (refactor-only) when applicable
- add CI checker that blocks merge when checklist is incomplete or missing

### Pattern 5: Reproducible Snapshot Pipeline
Move from ad-hoc command output references to reproducible artifacts:
- define canonical snapshot directory (phase/milestone scoped)
- include run metadata (`date`, `commit`, command, test root, rustc/toolchain)
- generate deterministic filenames and manifest index
- update `docs/current-status.md` from snapshot manifest, not manual copy/paste

## Don't Hand-Roll

| Problem | Don’t Build | Use Instead | Why |
|---|---|---|---|
| Skip categorization | Post-hoc grep on CLI text logs | Structured `SkipReason` enum + counters in summary | Deterministic and machine-verifiable TST-02 compliance. |
| Exception lifecycle | Reviewer memory/comments | Versioned exception records + expiry validator in CI | Required by locked governance strictness decisions. |
| Snapshot provenance | Free-form notes in status docs | Manifest-backed artifacts with metadata | Enables reproducibility and diffability required by TST-04. |
| 1+1 test policy | Convention-only review comments | PR checklist + CI enforcement | Makes TST-03 enforceable instead of aspirational. |

## Common Pitfalls

### Pitfall 1: “Telemetry green” while semantics are red
Stress GC thresholds can pass while actual test cases fail (observed 3 failures in current stress run). Telemetry gates and semantic pass/fail gates must both be explicit.

### Pitfall 2: Aggregate `skipped` hides compatibility ceiling
Current aggregate skipped count masks where coverage is excluded (module/strict/includes/features/harness globals). Without category reporting, convergence claims are hard to trust.

### Pitfall 3: Manual status updates drift from live state
Current docs contain stale profile totals (`26` vs live `45` test262-lite cases). Phase 7 must shift to artifact-driven status updates.

### Pitfall 4: Hard-blocking policy without exception protocol
Locked decision requires hard blocking plus explicit exceptions. Enforcing one without the other creates either deadlock or uncontrolled bypasses.

## Code Examples

### Proposed JSON report shape (test262-run)
```json
{
  "discovered": 53162,
  "executed": 1000,
  "failed": 0,
  "skipped": 555,
  "skipped_categories": {
    "flag_module": 120,
    "flag_only_strict": 210,
    "requires_includes": 180,
    "requires_feature": 40,
    "fixture_file": 5
  },
  "gc": {
    "collections_total": 0,
    "boundary_collections": 0,
    "runtime_collections": 0,
    "reclaimed_objects": 0
  }
}
```

### Proposed tracked run contract (command pair)
```powershell
# baseline profile
cargo run -p test-harness --bin test262-run -- `
  --root crates/test-harness/fixtures/test262-lite `
  --show-gc `
  --json artifacts/compat/phase7/<run-id>/baseline.json `
  --markdown artifacts/compat/phase7/<run-id>/baseline.md

# stress profile
cargo run -p test-harness --bin test262-run -- `
  --root crates/test-harness/fixtures/test262-lite `
  --auto-gc --auto-gc-threshold 1 `
  --runtime-gc --runtime-gc-interval 1 `
  --show-gc `
  --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline `
  --json artifacts/compat/phase7/<run-id>/stress.json `
  --markdown artifacts/compat/phase7/<run-id>/stress.md
```

## State of the Art (Project-local)

| Area | Current | Needed for Phase 7 |
|---|---|---|
| CI gates | Defined, but currently not green in local probe | Restore green baseline, then enforce hard-block with controlled exceptions |
| GC telemetry | Baseline/stress metrics + threshold parser exist | Add two-tier alert policy + consecutive anomaly workflow + reproducible artifacts |
| test262 reports | JSON + console text, aggregate skip count | Add skip-category taxonomy + Markdown output + tracked run manifest |
| TST-03 compliance | Strong testing culture, no formal PR gate | Mandatory PR checklist + CI checker + exception evidence path |
| Snapshot governance | Rich docs, manual updates | Artifact-first reproducible pipeline + automated `current-status` refresh |

## Open Questions

1. Where should reproducible compatibility artifacts live long-term (tracked repo path vs CI artifacts + pinned manifest in repo)?
2. For warning-tier GC drift, should PRs pass with annotation while default-branch/nightly blocks only on repeated anomalies?
3. Should stress telemetry command require zero semantic mismatches, or split into separate telemetry-only and semantic-stress gates?
4. What exact “major convergence work” trigger updates `docs/current-status.md` (per merged PR, per milestone, or per snapshot batch)?

## Sources

### Primary (HIGH confidence)
- `.planning/phases/07-compatibility-and-governance-gates/07-CONTEXT.md`
- `.planning/REQUIREMENTS.md`
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `.github/workflows/ci.yml`
- `crates/test-harness/src/test262.rs`
- `crates/test-harness/src/bin/test262-run.rs`
- `crates/test-harness/tests/test262_lite.rs`
- `crates/test-harness/fixtures/test262-lite/gc-guard.baseline`
- `docs/current-status.md`
- `docs/test262-lite.md`
- `docs/test262-baseline.md`
- `docs/gc-snapshot-report.md`
- `docs/gc-test-plan.md`
- `docs/risk-register.md`

### Command probes (HIGH confidence)
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -q -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --show-gc --json target/test262-summary-default-phase7.json`
- `cargo run -q -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline --allow-failures --show-failures 20 --json target/test262-summary-stress-phase7-allow.json`
- `cargo run -q -p test-harness --bin test262-run -- --root D:/dev/test262/test --max-cases 1000 --allow-failures --json target/test262-real-1000-phase7.json`

## Metadata

**Confidence breakdown**
- Requirement mapping and gap diagnosis: HIGH (direct code/docs + live command evidence)
- Governance architecture recommendations: HIGH (constrained by locked decisions + existing project patterns)
- Future policy tuning (warning vs blocking thresholds): MEDIUM (needs project preference decision)

**Research date:** 2026-02-27  
**Valid until:** 2026-03-13
