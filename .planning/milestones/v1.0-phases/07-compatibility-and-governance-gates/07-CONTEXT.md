# Phase 7: Compatibility and Governance Gates - Context

**Gathered:** 2026-02-27
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers repeatable and enforceable compatibility/governance mechanisms for quality gates, GC telemetry thresholds, test262 reporting, and feature-level test compliance.

Out of scope: introducing new runtime language capabilities beyond governance and compatibility infrastructure already defined in roadmap Phase 7.

</domain>

<decisions>
## Implementation Decisions

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

</decisions>

<specifics>
## Specific Ideas

- Keep governance policy deterministic-first: explicit contracts and traceable exceptions over ad-hoc reviewer judgment.
- Prefer additive automation and report reproducibility so Phase 7 becomes a long-term release-quality control plane.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within Phase 7 scope.

</deferred>

---

*Phase: 07-compatibility-and-governance-gates*
*Context gathered: 2026-02-27*
