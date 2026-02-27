# Current Status Snapshot

Generated from `docs/compatibility/phase7-snapshots.json`.

## Compatibility Governance

| Field | Value |
| --- | --- |
| phase | 07 |
| milestone | v1.0 |

## Profile Drift Status

| Profile | status | anomaly_streak | investigation_required | discovered | executed | failed |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| baseline | ok | 0 | False | 45 | 45 | 0 |
| stress | ok | 0 | False | 45 | 45 | 3 |

## Policy

- `status=blocking` is CI-blocking.
- `anomaly_streak >= 2` sets `investigation_required=true` and is CI-blocking.
- Regenerate this file with:
  - `python .github/scripts/sync_current_status.py --manifest docs/compatibility/phase7-snapshots.json --status-doc docs/current-status.md --mode write`
