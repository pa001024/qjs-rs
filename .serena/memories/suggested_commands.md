常用命令（Windows PowerShell）：
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p vm`
- `cargo test -p test-harness --test test262_lite`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite`
- 基准：`cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators`