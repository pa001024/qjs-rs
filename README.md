# qjs-rs

纯 Rust 实现的 JavaScript 运行时，目标是语义优先并与 QuickJS 行为对齐。

## 目录概览

- `crates/`：核心实现（parser / bytecode / vm / runtime / builtins / test-harness）
- `docs/`：规范、性能策略、报告
- `scripts/`：辅助脚本（如基准报告渲染）
- `.planning/`：里程碑与阶段计划

## 快速开始

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 运行基准

```bash
cargo run -p benchmarks --bin benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --strict-comparators
```

## 生成基准报告

```bash
python scripts/render_engine_benchmark_report.py \
  --input target/benchmarks/engine-comparison.local-dev.json \
  --chart docs/reports/engine-benchmark-chart.svg \
  --report docs/reports/engine-benchmark-report.md
```

当前报告文件：

- `docs/reports/engine-benchmark-report.md`
- `docs/reports/engine-benchmark-chart.svg`
