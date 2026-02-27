# Governance Policy Artifacts

This directory stores policy-as-code assets used by CI and pull request governance checks.

## Files

- `exceptions.json`: approved and expiring exception records.
- `fixtures/`: deterministic pull request event payloads used by validator self-tests.

## Exception Contract

Each exception record must provide:

- `id`
- `reason`
- `impact_scope`
- `owner`
- `expires_at` (`YYYY-MM-DD`)
- `rollback_condition`

Expired exception records are rejected automatically by the validator.

## Validator Commands

Template and exception contract check:

```bash
python .github/scripts/validate_governance.py \
  --exceptions .github/governance/exceptions.json \
  --check-template .github/PULL_REQUEST_TEMPLATE.md
```

Validate a pull request event payload:

```bash
python .github/scripts/validate_governance.py \
  --exceptions .github/governance/exceptions.json \
  --check-template .github/PULL_REQUEST_TEMPLATE.md \
  --validate-pr-event .github/governance/fixtures/pr_event_runtime_change.json \
  --repo-root . \
  --require-test-reference-exists
```

Run deterministic self-tests:

```bash
python .github/scripts/validate_governance.py \
  --exceptions .github/governance/exceptions.json \
  --check-template .github/PULL_REQUEST_TEMPLATE.md \
  --validate-pr-event .github/governance/fixtures/pr_event_runtime_change.json \
  --repo-root . \
  --require-test-reference-exists \
  --self-test
```
