# GC Snapshot Report

基线日期：2026-02-23

## Commands

### Default Profile

`cargo run -q -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --show-gc`

### Stress Profile

`cargo run -q -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc`

### Stress Guard Gate

`cargo run -q -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline`

### Phase 7 Snapshot Governance

`python .github/scripts/run_compat_snapshot.py --phase 07 --milestone v1.0 --manifest docs/compatibility/phase7-snapshots.json --output-dir target/compatibility --allow-dirty`

`python .github/scripts/sync_current_status.py --manifest docs/compatibility/phase7-snapshots.json --status-doc docs/current-status.md --mode check`

## Latest Snapshot

| Profile | discovered | passed | failed | collections_total | boundary_collections | runtime_collections | reclaimed_objects | mark_duration_ns | sweep_duration_ns |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | 26 | 26 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| Stress | 26 | 26 | 0 | 29283 | 22 | 29261 | 611 | 4350900 | 422000 |

## Checks

- `Default`: `collections_total == 0` 且 `runtime_collections == 0`。
- `Stress`: `runtime_collections > 0` 且 `collections_total == runtime_collections + boundary_collections`。
- `Stress`: test262-lite `26/26` 全通过。
- Drift policy: `status=warning` 提醒跟进，`status=blocking` 直接阻断，consecutive anomalies (`anomaly_streak >= 2`) 触发 `investigation_required=true` 并阻断。

## Notes

- 当前压力样例集合已扩展到 18 个 `gc-*` fixtures。
- `reclaimed_objects` 在当前压力样例下为 611，说明新增“显式释放引用”样例已能稳定观测到回收行为。
