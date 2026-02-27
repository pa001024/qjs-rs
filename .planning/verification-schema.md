# Verification Artifact Frontmatter Schema

## Purpose

This document is the canonical machine contract for phase verification artifacts:

- `.planning/phases/*/*-VERIFICATION.md`

All verification files must use one normalized frontmatter shape so tooling can compute requirement coverage without parsing free-form body text.

## Canonical Frontmatter Contract

Every `*-VERIFICATION.md` file must begin with YAML frontmatter containing exactly these required machine keys:

```yaml
---
phase: 09-verification-traceability-normalization
phase_number: "09"
verified: 2026-02-27T12:00:00Z
status: passed
score: 3/3 tasks verified
requirements_checked:
  - SEM-01
  - SEM-02
---
```

For phases with no canonical requirement ownership mapping, the field is still required and must be an explicit empty list:

```yaml
requirements_checked: []
```

## Field Definitions

| Key | Type | Required | Format / Rule |
| --- | --- | --- | --- |
| `phase` | string | Yes | Phase slug directory name, e.g. `01-semantic-core-closure`. |
| `phase_number` | string | Yes | Two-digit phase number string (`01`..`99`). |
| `verified` | string | Yes | ISO-8601 timestamp. Example: `2026-02-27T09:12:00Z`. |
| `status` | string | Yes | Verification status token (for example `passed`, `failed`, `partial`). |
| `score` | string \| number | Yes | Human-readable numeric/ratio score preserved from verification outcome. |
| `requirements_checked` | array[string] | Yes | Requirement IDs in `AAA-00` format mapped to this phase by `.planning/REQUIREMENTS.md` Traceability table. |

## Deterministic Requirement Mapping Policy

`requirements_checked` is not free-form and is not derived from verification body prose.

Canonical source of truth:

1. Parse `.planning/REQUIREMENTS.md`.
2. Read the **Traceability** table (`Requirement | Phase | Status`).
3. Select only requirement IDs whose `Phase` equals this file's phase number.
4. Write that exact sorted list to `requirements_checked`.
5. If no requirement IDs map to the phase, set `requirements_checked: []`.

## Normalization Rules

When migrating or editing verification files:

- Use `verified` (canonical) and remove legacy `verified_at`.
- Ensure frontmatter exists for every phase verification file.
- Keep verification evidence body intact unless a field rename reference must be aligned.
- Keep `status` and `score` semantics unchanged.
- Do not add non-canonical machine keys unless future schema versioning explicitly defines them.
