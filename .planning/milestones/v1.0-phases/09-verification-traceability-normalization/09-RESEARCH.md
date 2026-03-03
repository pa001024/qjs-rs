# Phase 9: Verification Traceability Normalization - Research

**Researched:** 2026-02-27  
**Domain:** Planning artifact schema normalization, requirement traceability automation, audit reproducibility  
**Confidence:** HIGH

## User Constraints

- Phase target: **Phase 9 - Verification Traceability Normalization**.
- Goal: verification artifacts/tooling contracts become schema-consistent so requirement coverage auditing is automated + reproducible.
- Depends on: **Phase 8**.
- Requirement IDs to close in this phase: **None** (audit integration debt closure).
- Success criteria to satisfy:
  1. All phase verification files have consistent machine-parseable requirement mapping fields.
  2. Traceability checks no longer rely on manual fallback for frontmatter/schema mismatch.
  3. Milestone audit rerun computes requirement coverage directly from standardized verification artifacts.
- Project-specific discovery results:
  - `./CLAUDE.md`: not present.
  - `.agents/skills/`: not present.

## Summary

This phase is a **contract normalization phase**, not a runtime semantics phase. The core work is to remove schema drift across historical `*-VERIFICATION.md` files and make coverage extraction deterministic from machine fields instead of manual narrative parsing.

Current state is inconsistent enough that automation is brittle:
- Verification frontmatter schema currently has multiple variants (`verified` vs `verified_at`, with/without `phase_number`, with/without `requirements_checked`, and 2 files with no frontmatter at all).
- Existing verification schema validator (`gsd-tools frontmatter validate --schema verification`) passes only **3/8** phase verification files.
- Existing plan artifact/key-link verifier (`gsd-tools verify artifacts|key-links`) fails **24/24** plans due parser/shape mismatch.
- Requirement IDs recoverable from verification frontmatter are only **6** IDs, while body text contains all **20** v1 IDs.

**Primary recommendation:** treat Phase 9 as a two-part closure exactly matching roadmap intent:
1. **09-01:** define a canonical verification schema + migrate legacy phase verification artifacts (01-08) to that schema.
2. **09-02:** implement repo-local schema/traceability checks and wire them into CI so audit coverage is computed from frontmatter, not manual fallback.

## Standard Stack

### Core

| Component | Purpose | Why standard for this phase |
|---|---|---|
| Markdown files + YAML frontmatter in `.planning/phases/*/*-VERIFICATION.md` | Verification artifact source of truth | Already the established artifact format in this repo |
| `.planning/REQUIREMENTS.md` traceability table | Canonical requirement universe and owner mapping | Defines complete v1 requirement set and expected phase mapping |
| Repo-local Python script in `.github/scripts/` | Deterministic schema + traceability checker | CI already uses Python scripts and avoids workstation-specific tool paths |

### Supporting

| Component | Purpose | When to use |
|---|---|---|
| `gsd-tools frontmatter validate/get` (local Codex install) | Local diagnostics during development | Useful for iterative checks, but should not be sole CI dependency |
| GitHub Actions `ci.yml` | Enforce non-regression of artifact schema | Add a dedicated planning/traceability gate step in Phase 9 |

## Current State (Evidence You Should Plan Against)

### 1) Verification schema drift across Phase 01-08 files

| File | Frontmatter | Requirement mapping field in frontmatter |
|---|---|---|
| `01-VERIFICATION.md` | none | none |
| `02-VERIFICATION.md` | has frontmatter (`verified_at`) | `requirements_checked` present |
| `03-VERIFICATION.md` | none | none |
| `04-VERIFICATION.md` | has frontmatter (`verified_at`) | `requirements_checked` present |
| `05-VERIFICATION.md` | has frontmatter (`verified`) | none |
| `06-VERIFICATION.md` | has frontmatter (`verified`) | none |
| `07-VERIFICATION.md` | has frontmatter (`verified`) | none |
| `08-VERIFICATION.md` | has frontmatter (`verified_at`) | `requirements_checked` present |

Observed with:
- `node ... gsd-tools.cjs frontmatter get ...`
- `node ... gsd-tools.cjs frontmatter validate ... --schema verification`

### 2) Existing verification schema validator does not match actual artifacts

`frontmatter validate --schema verification` currently requires:
- `phase`
- `verified`
- `status`
- `score`

Result over 8 verification files:
- **valid: 3** (`05`, `06`, `07`)
- **invalid: 5** (`01`, `02`, `03`, `04`, `08`)

### 3) Requirement coverage cannot be derived from frontmatter today

- Requirement IDs visible via `requirements_checked` frontmatter only: **6 IDs** (`ASY-01`, `ASY-02`, `MEM-01`, `MEM-02`, `MOD-01`, `MOD-02`)
- Requirement IDs present somewhere in verification docs (mostly body tables/text): **20 IDs** (all v1 requirements)

Implication: current machine-path coverage is incomplete unless manual body parsing fallback is used.

### 4) Tooling mismatch already documented in milestone audit debt

From `.planning/v1.0-MILESTONE-AUDIT.md` and phase verification notes:
- Phase 03 verification lacked explicit ASY requirement mapping.
- Phase 06 verification required manual fallback due frontmatter/tool mismatch.
- `gsd-tools verify artifacts` and `verify key-links` currently fail against all existing plans (`24/24`) because `parseMustHavesBlock` expects a shape/indentation pattern that does not match current `must_haves` frontmatter representation.

## Architecture Patterns

### Pattern 1: Canonical Verification Frontmatter Contract (single source of machine truth)

Define one required frontmatter contract for all phase verification files (including legacy migrated files):

```yaml
phase: 06-collection-and-regexp-semantics
phase_number: "06"
verified: 2026-02-27T05:00:09Z
status: passed
score: "9/9 must-haves verified"
requirements_checked:
  - BUI-04
  - BUI-05
```

Guidance:
- Keep requirement mapping field simple (`requirements_checked: [REQ-ID...]`).
- Always include the field; allow empty list (`[]`) for phases with no requirement IDs.
- Prefer one timestamp key (`verified`) to avoid `verified`/`verified_at` dual schema.

### Pattern 2: Deterministic Traceability Pipeline

Build repo-local checker pipeline:
1. Parse all `*-VERIFICATION.md` frontmatter.
2. Validate required keys + ID format (`^[A-Z]{3}-\d{2}$`).
3. Parse `.planning/REQUIREMENTS.md` traceability table as canonical requirement set.
4. Compute coverage from `requirements_checked` only.
5. Fail if any required ID is unmapped/duplicate/orphaned.

Output both:
- machine JSON (`target/verification-traceability.json`), and
- human markdown summary (`target/verification-traceability.md`) for audits.

### Pattern 3: Idempotent Legacy Migration

Migration should be scriptable and repeat-safe:
- add missing frontmatter to 01/03,
- normalize key names (`verified_at` -> `verified`),
- add `phase_number` + `requirements_checked` where missing,
- do not rewrite evidence body unless necessary.

### Pattern 4: CI Contract Gate

Add CI step after governance and before/after Rust checks (either is fine) to run:
- schema conformance check,
- traceability coverage check,
- non-zero exit on mismatch.

This closes “manual fallback path” debt by making mismatch impossible to merge.

## Don't Hand-Roll

| Problem | Avoid | Use instead | Why |
|---|---|---|---|
| Coverage extraction | Regex scraping of free-form verification body text | Canonical frontmatter `requirements_checked` | Body sections vary by phase and are not contract-stable |
| Traceability ownership | Treat both ROADMAP and REQUIREMENTS as equal requirement owners | Use REQUIREMENTS traceability table as canonical owner map | REQUIREMENTS already enforces 1:1 requirement→phase mapping |
| CI dependency | Depending on local `C:/Users/.../gsd-tools` path | Repo-local `.github/scripts/*.py` checker | CI must be reproducible outside developer workstation |
| Migration | Manual editing 8 verification files ad hoc | Scripted idempotent migration | Prevents drift and review mistakes |

## Common Pitfalls

### Pitfall 1: Dual source-of-truth ambiguity (ROADMAP vs REQUIREMENTS)
- **What goes wrong:** coverage logic double-counts or conflicts when a requirement appears in multiple roadmap phase descriptions.
- **Avoid:** treat `.planning/REQUIREMENTS.md` traceability table as canonical mapping for coverage math.

### Pitfall 2: Schema “almost aligned” but still non-parseable
- **What goes wrong:** files look similar to humans but key names differ (`verified` vs `verified_at`), breaking automation.
- **Avoid:** strict required-key check in CI.

### Pitfall 3: Nested frontmatter objects that current tooling cannot parse robustly
- **What goes wrong:** parser flattens or misses list-of-object blocks (`plan_must_haves`, `must_haves.artifacts`).
- **Avoid:** keep machine-critical fields flat/simple, or upgrade parser and add regression tests before using nested shapes.

### Pitfall 4: Historical evidence loss during migration
- **What goes wrong:** body rewrites accidentally drop command/evidence context.
- **Avoid:** frontmatter-only migration whenever possible.

### Pitfall 5: “Passes locally” but not in CI
- **What goes wrong:** local Codex tooling available on Windows path, unavailable in CI runner.
- **Avoid:** use repo-local scripts and standard Python runtime only.

## Code Examples

### Example A: Canonical verification frontmatter

```markdown
---
phase: 03-promise-job-queue-semantics
phase_number: "03"
verified: 2026-02-26T00:00:00Z
status: passed
score: "6/6 must-haves verified"
requirements_checked:
  - ASY-01
  - ASY-02
---
```

### Example B: Minimal repo-local checker behavior

```python
# .github/scripts/check_verification_traceability.py (shape)
# 1) load requirements IDs from REQUIREMENTS.md traceability table
# 2) load each phase verification frontmatter
# 3) assert required keys exist
# 4) collect requirements_checked union
# 5) fail if required IDs missing/orphaned
```

### Example C: CI wiring

```yaml
- name: Verification Traceability Gate
  run: |
    python .github/scripts/check_verification_traceability.py \
      --requirements .planning/REQUIREMENTS.md \
      --phases-dir .planning/phases \
      --out-json target/verification-traceability.json
```

## State of the Art (for this repo)

| Old approach | Current pain | Target approach |
|---|---|---|
| Mixed verification schemas and body-dependent parsing | Coverage requires manual fallback and ad hoc interpretation | Single schema + frontmatter-driven coverage computation |
| External tool assumptions (local Codex paths/parsers) | Not CI-portable and parser mismatch-prone | Repo-local CI checker using deterministic contract |
| Human audit synthesis from mixed formats | Non-reproducible cross-run variance | Scripted audit inputs + stable JSON/Markdown outputs |

## Open Questions (Resolve During Planning)

1. **Canonical timestamp key:** keep `verified` only, or allow dual-read (`verified` + `verified_at`) with normalized output?
   - Recommendation: normalize to `verified` and support dual-read only during migration.

2. **Requirement mapping for historical closure phases:** when requirement ownership moved to a closure phase (e.g., ASY moved to Phase 8 in REQUIREMENTS traceability), should earlier implementation phase verification also list it?
   - Recommendation: coverage computation should follow REQUIREMENTS ownership; legacy docs can mention IDs in body but frontmatter contract should align with canonical mapping policy.

3. **Parser strategy for must_haves tooling drift:** patch parser to support existing nested shapes, or simplify contract to flat machine fields only?
   - Recommendation: for Phase 9 scope, prioritize requirement traceability fields first; parser upgrades can be incremental but must include tests.

## Planning Guidance by Plan Slice

### 09-01 should produce
- A written canonical schema contract for verification artifacts (where to store: `.planning/` docs).
- Migration of legacy verification files (`01`..`08`) to canonical frontmatter.
- Backfilled `requirements_checked` for every phase verification artifact.
- Optional: migration report listing changed files and field-level changes.

### 09-02 should produce
- Repo-local checker script(s) for schema + requirement coverage.
- CI gate in `.github/workflows/ci.yml` enforcing checker pass.
- Checker regression tests or self-test fixtures.
- Milestone audit rerun using standardized artifacts, demonstrating direct coverage computation (no manual fallback path).

## Sources

Primary sources used:
- `C:/Users/Administrator/.codex/agents/gsd-phase-researcher.md`
- `.planning/REQUIREMENTS.md`
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `.planning/v1.0-MILESTONE-AUDIT.md`
- `.planning/phases/01-semantic-core-closure/01-VERIFICATION.md`
- `.planning/phases/02-runtime-safety-and-root-integrity/02-VERIFICATION.md`
- `.planning/phases/03-promise-job-queue-semantics/03-VERIFICATION.md`
- `.planning/phases/04-es-module-lifecycle/04-VERIFICATION.md`
- `.planning/phases/05-core-builtins-baseline/05-VERIFICATION.md`
- `.planning/phases/06-collection-and-regexp-semantics/06-VERIFICATION.md`
- `.planning/phases/07-compatibility-and-governance-gates/07-VERIFICATION.md`
- `.planning/phases/08-async-and-module-builtins-integration-closure/08-VERIFICATION.md`
- `.github/workflows/ci.yml`
- `C:/Users/Administrator/.codex/get-shit-done/bin/lib/frontmatter.cjs`
- `C:/Users/Administrator/.codex/get-shit-done/bin/lib/verify.cjs`
- `C:/Users/Administrator/.codex/get-shit-done/bin/lib/template.cjs`
- `C:/Users/Administrator/.codex/get-shit-done/bin/lib/commands.cjs`
- `C:/Users/Administrator/.codex/get-shit-done/workflows/audit-milestone.md`
- `C:/Users/Administrator/.codex/get-shit-done/workflows/execute-phase.md`

## Metadata

**Confidence breakdown:**
- Schema drift diagnosis: **HIGH** (direct file/tool outputs)
- Tooling mismatch diagnosis: **HIGH** (direct command evidence)
- Plan decomposition fit to roadmap (`09-01`, `09-02`): **HIGH**
- Requirement ownership policy recommendation: **MEDIUM** (needs explicit team decision)

**Research date:** 2026-02-27  
**Valid until:** 2026-03-13

## RESEARCH COMPLETE
