# PLANNING KNOWLEDGE BASE

**Scope:** `.planning/` roadmap, requirements, phase plans, and traceability artifacts.

## OVERVIEW
This directory is the planning contract for milestone execution (`PROJECT`, `ROADMAP`, `REQUIREMENTS`, `phases`, `milestones`, `research`, `codebase`).

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Active milestone requirements | `.planning/REQUIREMENTS.md` | source of PERF/TST/HOST requirement IDs |
| Milestone sequencing | `.planning/MILESTONES.md`, `.planning/ROADMAP.md` | phase progression and closure order |
| Execution plans by phase | `.planning/phases/*` | per-phase PLAN + verification intent |
| Project intent and status | `.planning/PROJECT.md`, `.planning/STATE.md` | current direction and handoff state |
| Verification schema linkage | `.planning/verification-schema.md` | traceability model used by CI checks |

## CONVENTIONS (PLANNING)
- Requirements IDs (`PERF-*`, `TST-*`, `HOST-*`) are traceability anchors; keep IDs stable.
- Phase directories are evidence-bearing artifacts, not scratch notes.
- Planning docs must stay consistent with CI traceability script inputs.
- Requirement status changes should reference concrete evidence artifacts.

## ANTI-PATTERNS (PLANNING)
- Renaming requirement IDs without coordinated traceability updates.
- Marking closure while `check_perf_target.py`/contract evidence still fails.
- Treating `.planning` as optional when CI verification gates consume it.
- Keeping stale phase plans that contradict `REQUIREMENTS.md`/`STATE.md`.

## COMMANDS
```bash
python .github/scripts/check_verification_traceability.py --requirements .planning/REQUIREMENTS.md --phases-dir .planning/phases --out-json target/verification-traceability.json --out-md target/verification-traceability.md
python .github/scripts/validate_governance.py --exceptions .github/governance/exceptions.json --check-template .github/PULL_REQUEST_TEMPLATE.md --repo-root . --self-test
```

## NOTES
- `.planning` is first-party governance input to CI; treat edits as policy changes.
- Keep milestone narrative aligned with `docs/current-status.md` and benchmark closure policy docs.
