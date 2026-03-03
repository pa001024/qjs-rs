# Phase 11 Target Closure Evidence (Packet-B)

_Date:_ 2026-02-28  
_Plan:_ 11-03 (`11-hot-path-optimization-and-target-closure`)

_Traceability note (11-08): this file preserves historical boa-gate transcripts for audit history. Active PERF-03 closure is `qjs-rs <= 1.25x quickjs-c` under `--require-qjs-lte-quickjs-ratio 1.25`._

## 1) Artifacts generated

- Baseline (locked reference): `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
- Packet-A candidate (existing): `target/benchmarks/engine-comparison.local-dev.packet-a.json`
- Packet-B candidate (this plan): `target/benchmarks/engine-comparison.local-dev.packet-b.json`
- Packet-B corroboration (ci-linux profile): `target/benchmarks/engine-comparison.ci-linux.packet-b.json`

Contract validation commands:

- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-b.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-b.json` ✅

## 2) PERF-04 packet deltas (local-dev)

### Aggregate (`qjs-rs`)

| Stage | qjs-rs mean ms | boa-engine mean ms | qjs/boa ratio |
|---|---:|---:|---:|
| Baseline | 1784.473 | 182.076 | 9.801x |
| Packet-A | 1294.700 | 201.816 | 6.415x |
| Packet-B | 1206.119 | 176.403 | 6.837x |

### Per-case (`qjs-rs` mean ms)

| Case | Baseline | Packet-A | Packet-B | Packet-A vs Baseline | Packet-B vs Packet-A |
|---|---:|---:|---:|---:|---:|
| `arith-loop` | 2153.732 | 1940.619 | 1844.848 | -9.90% | -4.94% |
| `fib-iterative` | 162.745 | 145.908 | 118.650 | -10.35% | -18.68% |
| `array-sum` | 4798.761 | 3071.256 | 2842.642 | -36.00% | -7.44% |
| `json-roundtrip` | 22.655 | 21.016 | 18.337 | -7.23% | -12.75% |

Packet-B hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`):

- `numeric_ops`: 31,417,400
- `identifier_resolution`: 136,257,800
- `array_indexed_property_get`: 2,801,400
- `array_indexed_property_set`: 2,800,000

## 3) PERF-03 closure gate result

Checker command:

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json \
  --require-qjs-lte-boa  # legacy historical gate
```

Result: ❌ **FAILED**

- `aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1206.119211 > boa-engine 176.403164`

Interpretation:

- Packet-B improved Phase 11 hotspot families versus baseline and versus packet-A in local-dev measurements.
- Historical legacy closure criterion (`qjs-rs <= boa-engine`) was unmet for this candidate.
- Active closure criterion is `qjs-rs <= 1.25x quickjs-c`; this packet section does not contain an authoritative quickjs-ratio pass.

## 4) PERF-05 maintainability boundary checklist

- [x] Runtime-core remains pure Rust (no C FFI added in VM/runtime core).
- [x] Packet-B logic is guarded with explicit fallback to canonical property semantics.
- [x] Dense-index fast path rejects non-dense arrays, accessor-backed indices, prototype-index hits, and exotic/proxy-like markers.
- [x] Changes remain layer-local (`crates/vm/src/*`, packet-specific tests, benchmark evidence/docs).

## 5) Quality/gate command outcomes (execution run)

- `cargo test -p vm packet_b_array_dense_index_fast_path_guarding -- --exact` ✅
- `cargo test -p vm perf_packet_b -- --nocapture` ✅
- `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-b.json --allow-missing-comparators` ✅
- `cargo run -p benchmarks --release -- --profile ci-linux --output target/benchmarks/engine-comparison.ci-linux.packet-b.json --allow-missing-comparators` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-b.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-b.json` ✅
- `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json --require-qjs-lte-boa` ❌ (legacy historical gate; see Section 3)
- `cargo fmt --check` ❌ (workspace formatting drift outside packet-B scope)
- `cargo clippy --all-targets -- -D warnings` ❌ (pre-existing benchmark target lint debt outside packet-B scope)
- `cargo test` ❌ (Windows host memory/pagefile exhaustion during full workspace test build)

## 6) Closure status

- PERF-04 packet evidence: ✅ complete (two optimization packets with measurable deltas)
- PERF-05 maintainability boundary: ✅ complete
- PERF-03 active closure (`qjs-rs <= 1.25x quickjs-c` on authoritative profile): ❌ not yet achieved (no authoritative quickjs-ratio green verdict recorded here)

## 7) Plan 11-05 governance + packet-c closure rerun (2026-02-28)

### 7.1 Packet-c artifact refresh

- Regenerated artifact: `target/benchmarks/engine-comparison.local-dev.packet-c.json`
- Aggregate means (`local-dev`):
  - `qjs-rs`: `1678.421964 ms`
  - `boa-engine`: `189.600068 ms`
  - `qjs-rs/boa-engine`: `8.8524x`

### 7.2 Final command transcript (single sequence)

1. `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-c.json --allow-missing-comparators` ✅
2. `cargo fmt --check` ❌
3. `cargo clippy --all-targets -- -D warnings` ✅
4. `cargo test` ✅
5. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-c.json` ✅
6. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-c.json --require-qjs-lte-boa` ❌ (legacy historical gate)

### 7.3 Gate verdict

- Governance gate bundle (`fmt` + `clippy` + `test`): ❌ **RED** (`cargo fmt --check` failed)
- PERF-03 authoritative closure gate: ❌ **FAILED**
  - `candidate qjs-rs 1678.421964 > boa-engine 189.600068`

### 7.4 Blocker summary

1. Workspace formatting drift remains in VM files (outside this plan's ownership set), so the governance bundle is not fully green.
2. Packet-c aggregate performance still fails the historical legacy PERF-03 gate (`qjs-rs <= boa-engine`); active quickjs-ratio closure remains open.

Closure remains **open**. No green governance-bundle verdict is recorded for this run.

## 8) Plan 11-07 authoritative governance + packet-d closure bundle (2026-02-28)

### 8.1 Single authoritative machine-readable provenance artifact

- Bundle path: `target/benchmarks/phase11-closure-bundle.json`
- `timestamp_utc`: `2026-02-28T13:30:03Z`
- Packet-D artifact path: `target/benchmarks/engine-comparison.local-dev.packet-d.json`
- Packet-D artifact hash/sha256: `5c86d5ad74fa925e2978be29489adfd4d2fe9d486685fbce9b8b52b595f41667`

### 8.2 Ordered command transcript from authoritative run

1. `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-d.json --allow-missing-comparators` (`rc=0`)
2. `cargo fmt --check` (`rc=0`)
3. `cargo clippy --all-targets -- -D warnings` (`rc=101`)
4. `cargo test` (`rc=0`)
5. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-d.json` (`rc=0`)
6. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json --require-qjs-lte-boa` (`rc=1`, legacy historical gate)

Exact command outputs captured from the run:

- Clippy failure:

  ```text
  error: this function has too many arguments (8/7)
     --> crates\benchmarks\src\main.rs:293:1
  ```

- Contract checker output:

  ```text
  benchmark contract check passed (target/benchmarks/engine-comparison.local-dev.packet-d.json)
  ```

- PERF-03 checker output:

  ```text
  perf target check failed
  - aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1390.811014 > boa-engine 181.287246
  ```

### 8.3 Packet-d aggregate means (`local-dev` candidate)

- `qjs-rs`: `1390.811014 ms`
- `boa-engine`: `181.287246 ms`
- `nodejs`: `2.211136 ms`
- `qjs-rs / boa-engine`: `7.6728x`

### 8.4 Final gate verdict (authoritative 11-07 bundle)

- Governance Gate Bundle (`fmt` + `clippy` + `test`): **RED**
  - `fmt=0`, `clippy=101`, `test=0`
- PERF-03 authoritative checker (`--require-qjs-lte-boa`): **FAILED** (legacy historical gate)
  - `qjs-rs 1390.811014 > boa-engine 181.287246`

Phase 11 closure remains **open** because governance and PERF-03 are not jointly green in the same authoritative run artifact.

## 9) Post-clippy-fix authoritative rerun (2026-02-28)

### 9.1 Single authoritative machine-readable provenance artifact

- Bundle path: `target/benchmarks/phase11-closure-bundle.json`
- `timestamp_utc`: `2026-02-28T16:27:50Z`
- Packet-D artifact path: `target/benchmarks/engine-comparison.local-dev.packet-d.json`
- Packet-D artifact hash/sha256: `02bcc45edebea604a98ac041cb0415f6aff71dde31fee49cb2f799b25c35ec16`

### 9.2 Ordered command transcript from authoritative rerun

1. `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-d.json --allow-missing-comparators` (`rc=0`)
2. `cargo fmt --check` (`rc=0`)
3. `cargo clippy --all-targets -- -D warnings` (`rc=0`)
4. `cargo test` (`rc=0`)
5. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-d.json` (`rc=0`)
6. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json --require-qjs-lte-boa` (`rc=1`, legacy historical gate)

PERF-03 checker output:

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1211.668632 > boa-engine 154.937264
```

### 9.3 Packet-d aggregate means (`local-dev` candidate)

- `qjs-rs`: `1211.668632 ms`
- `boa-engine`: `154.937264 ms`
- `nodejs`: `2.493782 ms`
- `qjs-rs / boa-engine`: `7.8204x`

### 9.4 Latest gate verdict

- Governance bundle (`fmt` + `clippy` + `test`): **PASS** (`fmt=0`, `clippy=0`, `test=0`)
- PERF-03 authoritative checker (`--require-qjs-lte-boa`): **FAILED** (legacy historical gate)
  - `qjs-rs 1211.668632 > boa-engine 154.937264`

Phase 11 closure remains **open** because PERF-03 is still red in the latest authoritative run artifact.

## 10) Latest authoritative rerun (2026-02-28)

### 10.1 Bundle provenance

- Bundle path: `target/benchmarks/phase11-closure-bundle.json`
- `timestamp_utc`: `2026-02-28T17:53:12Z`
- Packet-D artifact hash/sha256: `bde3e79d25d725cd07fc05f715cbb11e8c3df637a97c2acedcca1db08f7d01db`

### 10.2 Ordered command return codes

1. `bench_generate`: `rc=0`
2. `fmt`: `rc=0`
3. `clippy`: `rc=0`
4. `test`: `rc=0`
5. `contract`: `rc=0`
6. `perf_target`: `rc=1`

### 10.3 PERF-03 checker output

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1370.511975 > boa-engine 184.489346
```

### 10.4 Candidate aggregates

- `qjs-rs`: `1370.511975 ms`
- `boa-engine`: `184.489346 ms`
- `nodejs`: `1.997089 ms`
- `qjs-rs / boa-engine`: `7.4286x`

### 10.5 Verdict

- Governance bundle: **PASS**
- PERF-03 target: **FAIL**

Phase 11 remains **open**. Active closure still requires an authoritative `--require-qjs-lte-quickjs-ratio 1.25` pass.


## 11) Plan 11-09 packet-e authoritative quickjs-ratio attempt (2026-03-02)

### 11.1 VM packet-path optimization scope

- Implemented one guarded low-risk identifier-call dispatch optimization in packet-D path:
  - direct call-site binding resolution fast path for `CallIdentifier` / `CallIdentifierWithSpread` when slot metadata is available and `with` is absent.
  - fallback remains canonical `resolve_identifier_reference` path on guard miss.
- Added packet-D telemetry counters for direct call dispatch:
  - `identifier_call_direct_hits`
  - `identifier_call_direct_misses`
- Added parity coverage for direct call dispatch/fallback behavior:
  - `perf_packet_d_identifier_call_direct_dispatch_guarding`.

### 11.2 Governance/parity verification commands (Task 1)

1. `cargo fmt --check` ❌
   - failed with pre-existing and local formatting drift reports (including non-11-09 ownership paths such as `crates/bytecode/src/lib.rs`).
2. `cargo clippy -p vm -p benchmarks -- -D warnings` ✅
3. `cargo test -p vm perf_packet_d -- --nocapture` ✅
   - result: `5 passed; 0 failed`.
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture` ✅
   - result: `1 passed; 0 failed` for selected test filter.

### 11.3 Authoritative baseline/candidate artifacts and contract checks (Task 2)

- Regenerated strict-comparator baseline (required for quickjs-ratio checker mode):
  - `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
  - `generated_at_utc`: `2026-03-02T07:30:27.870Z`
- Generated packet-e candidate:
  - `target/benchmarks/engine-comparison.local-dev.packet-e.json`
  - `generated_at_utc`: `2026-03-02T07:32:15.800Z`
  - `sha256`: `e2c83552ed5f89129b700885c8ec67476d26214fb96ec0fad94223723d465a9c`

Contract validation commands:

- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-e.json` ✅

### 11.4 PERF-03 quickjs-ratio checker verdict

Checker command:

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-e.json \
  --require-qjs-lte-quickjs-ratio 1.25
```

Result: ❌ **FAILED**

Exact checker output:

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-quickjs-ratio failed: candidate qjs-rs/quickjs-c 6.136312 > 1.250000 (qjs-rs=98.181000, quickjs-c=16.000000)
```

Aggregate means snapshot:

- Baseline: `qjs-rs=101.873618`, `quickjs-c=16.428571`
- Candidate packet-e: `qjs-rs=98.181000`, `quickjs-c=16.000000`
- Candidate ratio (`qjs-rs/quickjs-c`): `6.136312x`

Packet hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`, packet-e):

- `numeric_ops`: `157087`
- `identifier_resolution`: `349965`
- `array_indexed_property_get`: `14007`
- `array_indexed_property_set`: `14000`

### 11.5 Current closure status after 11-09

- PERF-04 packet evidence: ✅ retained and extended (new guarded call-dispatch optimization + parity test)
- PERF-05 maintainability boundary: ✅ retained (guarded fallback semantics, no runtime-core C FFI)
- PERF-03 active closure (`qjs-rs <= 1.25x quickjs-c`): ❌ still open (`6.136312x`)

## 12) Plan 11-10 packet-f authoritative quickjs-ratio attempt (2026-03-02)

### 12.1 Optimization scope

- Added guarded packet-D slot-cache revalidation path for generation-churn scenarios:
  - New packet-D counters: `slot_guard_revalidate_hits`, `slot_guard_revalidate_misses`.
  - Revalidation only accepts stale slot entries when cached binding remains valid in the current top lexical scope.
  - Guard rejection immediately falls back to canonical identifier resolution and clears stale slot entries.
- Added packet-D parity coverage:
  - `perf_packet_d_slot_revalidation_fallback_parity`.
- Extended benchmark packet toggle inference/tests for packet-f and packet-final output paths.

### 12.2 Governance + parity command transcript

1. `cargo fmt --check` ✅
2. `cargo clippy -p vm -p benchmarks -- -D warnings` ✅
3. `cargo test -p vm perf_packet_d -- --nocapture` ✅ (`6 passed; 0 failed`)
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture` ✅ (`1 passed; 0 failed`)

### 12.3 Authoritative packet-f artifact and checker outcomes

- Candidate artifact: `target/benchmarks/engine-comparison.local-dev.packet-f.json`
- `generated_at_utc`: `2026-03-02T16:51:43.510Z`
- `sha256`: `42aeb5097a68577c680589fd8a0bfe2cdc441a71841c535bf01320dfad4fe333`

Commands:

1. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-f.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators` ✅
2. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-f.json` ✅
3. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-f.json --require-qjs-lte-quickjs-ratio 1.25` ❌

Exact checker output:

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-quickjs-ratio failed: candidate qjs-rs/quickjs-c 6.085281 > 1.250000 (qjs-rs=83.020621, quickjs-c=13.642857)
```

Packet-f aggregate means:

- `qjs-rs`: `83.020621`
- `quickjs-c`: `13.642857`
- `qjs-rs/quickjs-c`: `6.085281x`

Packet-f hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`):

- `numeric_ops`: `157087`
- `identifier_resolution`: `349965`
- `array_indexed_property_get`: `14007`
- `array_indexed_property_set`: `14000`

### 12.4 Verdict

- Governance bundle (fmt + clippy + targeted packet tests): ✅ PASS
- PERF-03 quickjs-ratio gate: ❌ FAIL (`6.085281x > 1.25x`)

## 13) Plan 11-11 packet-final authoritative quickjs-ratio attempt (2026-03-02)

### 13.1 Final optimization scope

- Added two-scope specialized fast path in `resolve_binding_id_slow` for common lexical lookup shape (`[global, local]`) while preserving canonical fallback scanning for larger scope stacks.
- Kept packet-D guarded slot revalidation/fallback semantics from 11-10 unchanged.

### 13.2 Governance + parity command transcript

1. `cargo fmt --check` ✅
2. `cargo clippy -p vm -p benchmarks -- -D warnings` ✅
3. `cargo test -p vm perf_packet_d -- --nocapture` ✅ (`6 passed; 0 failed`)
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture` ✅ (`1 passed; 0 failed`)

### 13.3 Authoritative packet-final artifact and checker outcomes

- Candidate artifact: `target/benchmarks/engine-comparison.local-dev.packet-final.json`
- `generated_at_utc`: `2026-03-02T16:52:08.444Z`
- `sha256`: `b351b97e14c70018f3b0f2837fec738e15f4d2dd6543e049f36472bb2a87d60c`

Commands:

1. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-final.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators` ✅
2. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-final.json` ✅
3. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-final.json --require-qjs-lte-quickjs-ratio 1.25` ❌

Exact checker output:

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-quickjs-ratio failed: candidate qjs-rs/quickjs-c 5.755257 > 1.250000 (qjs-rs=76.668243, quickjs-c=13.321429)
```

Packet-final aggregate means:

- `qjs-rs`: `76.668243`
- `quickjs-c`: `13.321429`
- `qjs-rs/quickjs-c`: `5.755257x`

Packet-final hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`):

- `numeric_ops`: `157087`
- `identifier_resolution`: `349965`
- `array_indexed_property_get`: `14007`
- `array_indexed_property_set`: `14000`

### 13.4 Final gate verdict after 11-11

- Governance bundle (fmt + clippy + targeted packet tests): ✅ PASS
- PERF-03 quickjs-ratio gate: ❌ FAIL (`5.755257x > 1.25x`)

Phase 11 closure remains **open**. No authoritative quickjs-ratio-green candidate is recorded in plans 11-10/11-11.

## 14) Plan 11-12 packet-g authoritative quickjs-ratio attempt (2026-03-02)

### 14.1 Optimization scope

- Added packet-g guarded identifier-resolution fallback-reduction path:
  - name-cache guard keyed by identifier name with shared scope-generation invalidation.
  - stale cache entries revalidate against current top lexical scope before reuse.
  - guard miss/revalidate-miss paths always fall back to canonical `resolve_binding_id_slow`.
- Added packet-g parity/counter coverage in `perf_packet_d` suite:
  - packet-g on/off parity over packet-d script families.
  - hit/miss/revalidate coverage across lexical loops, with-scope, prototype/accessor, and unknown identifiers.
- Extended benchmark hotspot payload schema to carry packet-d + packet-g guard taxonomy counters.

### 14.2 Governance + parity command transcript

1. `cargo fmt --check` ✅
2. `cargo clippy -p vm -p benchmarks -- -D warnings` ✅
3. `cargo test -p vm perf_packet_d -- --nocapture` ✅ (`9 passed; 0 failed`)
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture` ✅ (`3 passed; 0 failed`)

### 14.3 Authoritative packet-g artifact and checker outcomes

- Baseline artifact (locked): `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
  - `generated_at_utc`: `2026-03-02T07:30:27.870Z`
- Candidate artifact: `target/benchmarks/engine-comparison.local-dev.packet-g.json`
  - `generated_at_utc`: `2026-03-02T21:00:07.571Z`
  - `sha256`: `8574932d7325779b1e4376e8a8d722e503ccb3c1ac4390d9fdd8ccaadb7d2d1c`

Commands:

1. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-g.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators` ✅
2. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-g.json` ✅
3. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-g.json --require-qjs-lte-quickjs-ratio 1.25` ❌

Exact checker output:

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-quickjs-ratio failed: candidate qjs-rs/quickjs-c 6.236987 > 1.250000 (qjs-rs=79.521582, quickjs-c=12.750000)
```

Aggregate means (packet-g candidate):

- `qjs-rs`: `79.521582`
- `quickjs-c`: `12.750000`
- `qjs-rs/quickjs-c`: `6.236987x`

### 14.4 PERF-04 packet delta + hotspot attribution note

- Aggregate quickjs-ratio delta:
  - `packet-final`: `5.755257x`
  - `packet-g`: `6.236987x`
  - Result: packet-g regressed ratio in this authoritative rerun (`+0.481730x`), so no closure claim.
- packet-g hotspot taxonomy snapshot (`qjs_rs_hotspot_attribution.total`):
  - `identifier_resolution_fallback_scans`: `6265`
  - `packet_g_name_guard_hits`: `4655`
  - `packet_g_name_guard_misses`: `6265`
  - `packet_g_name_guard_revalidate_hits`: `1561`
  - `packet_g_name_guard_revalidate_misses`: `4620`
- Interpretation: packet-g taxonomy is now visible and contract-serialized; this run still leaves the PERF-03 ratio gap open.

### 14.5 PERF-05 boundary evidence

- Runtime-core C FFI boundary scan command:
  - `rg --line-number 'extern\\s+\"C\"|\\bunsafe\\b' crates/vm crates/runtime crates/bytecode crates/builtins`
  - Result: no matches (`rc=1` means “not found” for `rg`).
- Boundary conclusion:
  - No runtime-core C FFI introduction observed.
  - Packet-g changes remain within VM/benchmark/doc/test layers.

### 14.6 Verdict after 11-12

- Governance bundle (fmt + clippy + targeted packet suites): ✅ PASS
- PERF-03 quickjs-ratio gate: ❌ FAIL (`6.236987x > 1.25x`)

Phase 11 closure remains **open**.

## 15) Plan 11-14 authoritative packet-h closure rerun (2026-03-03)

### 15.1 Machine-checkable closure bundle

- Bundle path: `target/benchmarks/phase11-closure-bundle.packet-h.json`
- `timestamp_utc`: `2026-03-03T06:53:53.161609Z`
- Authoritative run timestamp: `2026-03-03T06:51:55.625857Z`
- Baseline path: `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
- Candidate path: `target/benchmarks/engine-comparison.local-dev.packet-h.json`
- Candidate hash/sha256: `91a2559fdf1264f7bb1fb29f8cabde4733a277a8ca4ab848c9a96257bf251e94`

### 15.2 Ordered governance + checker outcomes (from bundle)

1. `cargo fmt --check` (`exit_code=1`) ❌
2. `cargo clippy -p vm -p benchmarks -- -D warnings` (`exit_code=0`) ✅
3. `cargo test -p vm perf_packet_d -- --nocapture` (`exit_code=0`) ✅
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture` (`exit_code=0`) ✅
5. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-h.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators` (`exit_code=0`) ✅
6. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-h.json` (`exit_code=0`) ✅
7. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-h.json --require-qjs-lte-quickjs-ratio 1.25` (`exit_code=1`, `status=threshold_fail_expected`) ❌

Checker log paths (same run provenance):

- `target/benchmarks/perf-target.packet-h.stdout.log`
- `target/benchmarks/perf-target.packet-h.stderr.log`
- `target/benchmarks/perf-target.packet-h.verdict.json`

### 15.3 Aggregate means and PERF-03 verdict

Aggregate means (`target/benchmarks/phase11-closure-bundle.packet-h.json`):

- `qjs-rs`: `81.827593`
- `quickjs-c`: `13.071429`
- `qjs-rs/quickjs-c`: `6.260034x`

Exact checker failure output (`target/benchmarks/perf-target.packet-h.stderr.log`):

```text
perf target check failed
- aggregate.mean_ms_per_engine: require-qjs-lte-quickjs-ratio failed: candidate qjs-rs/quickjs-c 6.260034 > 1.250000 (qjs-rs=81.827593, quickjs-c=13.071429)
```

Verdict:

- PERF-03 quickjs-ratio gate: ❌ FAIL (`6.260034x > 1.25x`)
- Closure status: **OPEN** (packet-h bundle is authoritative + machine-checkable, but target remains unmet)


