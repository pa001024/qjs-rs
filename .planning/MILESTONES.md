# Milestones

## v1.0 milestone (Shipped: 2026-02-27)

**Phases completed:** 9 phases, 26 plans, 74 tasks

**Key accomplishments:**
- Closed semantic/runtime core gaps for eval/scope/control-flow/descriptor behavior and stale-handle safety.
- Delivered deterministic Promise job queue + host callback behavior with module lifecycle stability and cycle/cache handling.
- Completed core builtins + collection/RegExp conformance subsets and hardened CI/test262 governance gates.
- Integrated async/module builtins path parity and revalidated ASY-01/ASY-02 through module execution.
- Normalized verification schema and shipped a deterministic traceability checker with CI blocking gate.

**Known gaps at completion:**
- Initial `v1.0-MILESTONE-AUDIT.md` header remained `gaps_found`; follow-up traceability rerun passed (`20/20`) and evidence is archived in:
  - `.planning/milestones/v1.0-MILESTONE-AUDIT.md`
  - `target/verification-traceability.json`
  - `target/verification-traceability.md`

---

## v1.1 milestone (In Progress)

**Theme:** Performance Acceleration

**Updated target:** Continue hot-path optimization and reach **>=80% of `quickjs-c` performance** on the tracked suite (latency-equivalent: `qjs-rs <= 1.25x quickjs-c`) while keeping semantic/governance gates green.

**Current state:** Phase 11 remains open; governance is green in latest authoritative bundle, but performance closure has not yet met the updated target threshold.
