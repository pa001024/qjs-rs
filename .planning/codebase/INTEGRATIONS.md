# External Integrations

**Analysis Date:** 2026-02-25

## APIs & External Services

**Payment Processing:**
- None currently; no payment SDK/client usage in workspace manifests (`Cargo.toml`, `crates/*/Cargo.toml`).

**Email/SMS:**
- None currently; no mail/SMS provider integration code found (`crates/runtime/src/lib.rs`, `crates/builtins/src/lib.rs`, `crates/test-harness/src/lib.rs`).

**External APIs:**
- No runtime HTTP/API client integration in engine path (`crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `crates/builtins/src/lib.rs`).
- Compatibility input is local/offline test corpus, not remote API calls (`crates/test-harness/src/bin/test262-run.rs`, `crates/test-harness/src/test262.rs`, `docs/test262-baseline.md`).

## Data Storage

**Databases:**
- None; no SQL/NoSQL client dependencies in manifests (`Cargo.toml`, `crates/*/Cargo.toml`, `Cargo.lock`).

**File Storage:**
- Local filesystem only for test discovery and baseline I/O:
  - Suite traversal reads `*.js` recursively (`crates/test-harness/src/test262.rs`).
  - Optional JSON summary output via `--json <path>` (`crates/test-harness/src/bin/test262-run.rs`).
  - GC expectation baseline file loading via `--expect-gc-baseline <path>` (`crates/test-harness/src/bin/test262-run.rs`, `crates/test-harness/fixtures/test262-lite/gc-guard.baseline`).

**Caching:**
- CI dependency/build cache through GitHub Action `Swatinem/rust-cache@v2` (`.github/workflows/ci.yml`).
- No application-level Redis/memcached cache integration (`crates/runtime/src/lib.rs`, `crates/vm/src/lib.rs`).

## Authentication & Identity

**Auth Provider:**
- None; engine and harness run without user/account auth (`crates/test-harness/src/lib.rs`, `crates/test-harness/src/bin/test262-run.rs`).

**OAuth Integrations:**
- None; no OAuth client libraries or callback handlers present (`Cargo.lock`, `crates/*/Cargo.toml`).

## Monitoring & Observability

**Error Tracking:**
- No Sentry/Datadog/etc. integration in codebase (`Cargo.toml`, `crates/*/Cargo.toml`).

**Analytics:**
- None configured (`crates/runtime/src/lib.rs`, `crates/builtins/src/lib.rs`).

**Logs:**
- CLI and harness output to stdout/stderr only (`crates/test-harness/src/bin/test262-run.rs`, `crates/test-harness/src/test262.rs`).
- Optional verbose tracing toggled by env vars `QJS_TRACE_STAGES` and `QJS_TRACE_CASES` (`crates/test-harness/src/test262.rs`).

## CI/CD & Deployment

**Hosting:**
- No runtime hosting target defined; project is primarily library/workspace code (`Cargo.toml`, `docs/current-status.md`).

**CI Pipeline:**
- GitHub Actions workflow (`.github/workflows/ci.yml`) integrates:
  - `actions/checkout@v4` for source checkout.
  - `dtolnay/rust-toolchain@stable` for toolchain provisioning.
  - `Swatinem/rust-cache@v2` for Cargo cache.
  - Commands: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
  - GC guard gate using `cargo run -p test-harness --bin test262-run ... --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline`.

## Environment Configuration

**Development:**
- Required CLI argument: `--root <path>` for suite location (`crates/test-harness/src/bin/test262-run.rs`).
- Optional env vars: `QJS_TRACE_STAGES`, `QJS_TRACE_CASES` (`crates/test-harness/src/test262.rs`).
- Optional GC behavior flags: `--auto-gc`, `--runtime-gc`, thresholds/intervals (`crates/test-harness/src/bin/test262-run.rs`).

**Staging:**
- No dedicated staging profile/config files present (`Cargo.toml`, `.github/workflows/ci.yml`).

**Production:**
- No production secret manager or environment-specific deployment configs in repo (`Cargo.toml`, `.github/workflows/ci.yml`, `crates/*/Cargo.toml`).

## Webhooks & Callbacks

**Incoming:**
- None; no webhook endpoints/server components in current codebase (`crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `crates/test-harness/src/bin/test262-run.rs`).

**Outgoing:**
- None; no outbound webhook publisher or HTTP client integration (`Cargo.lock`, `crates/*/Cargo.toml`).

---

*Integration audit: 2026-02-25*
*Update when adding/removing external services*
