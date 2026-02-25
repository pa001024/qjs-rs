# Codebase Concerns

**Analysis Date:** 2026-02-25

## Tech Debt

**Monolithic VM implementation (`crates/vm/src/lib.rs`):**
- Issue: Core execution, builtins, GC, object model, and a very large in-file test module are colocated in one file (`21,960` lines).
- Why: Rapid semantic convergence work prioritized speed of iteration over modularization.
- Impact: High change coupling and difficult review/debug isolation; regression risk rises when touching unrelated runtime areas.
- Fix approach: Split `crates/vm/src/lib.rs` into domain modules (`call`, `objects`, `gc`, `regexp`, `builtins/*`) with focused unit tests per module.

**Baseline constructor aliasing in builtins bootstrap (`crates/builtins/src/lib.rs`):**
- Issue: `WeakMap`/`WeakSet` and most typed-array globals are currently wired to baseline constructors (`Map`/`Set`/`Uint8Array`) instead of dedicated semantics.
- Why: Phase-5 incremental delivery chose minimum runnable surface first.
- Impact: Hidden semantic drift for GC behavior, key constraints, and element coercion; future fixes become migration-heavy.
- Fix approach: Replace aliases with dedicated constructors and internal slots; keep aliases only behind explicit feature flags if needed.

**Parser fallback for unsupported `for-in`/`for-of` shapes (`crates/parser/src/lib.rs`):**
- Issue: Unsupported loop heads are lowered to `for (; false; )` no-op loops.
- Why: Temporary compatibility shortcut to avoid parser hard-fail during phase rollout.
- Impact: Silent semantic mismatch (invalid or unsupported forms do not fail loudly).
- Fix approach: Replace fallback with explicit `ParseError`/early error, or complete lowering support for all allowed head shapes.

**Manual function metadata tables in VM (`crates/vm/src/lib.rs`):**
- Issue: Large hard-coded match tables (e.g., native property exposure/attributes) are manually maintained.
- Why: Feature-by-feature additions were appended inline.
- Impact: Easy to miss descriptor attributes or method wiring when adding new builtins.
- Fix approach: Centralize builtin metadata as declarative tables and generate accessor/attribute lookup paths.

## Known Bugs

**Unsupported `for-in`/`for-of` forms can degrade to zero-iteration loops (`crates/parser/src/lib.rs`):**
- Symptoms: Source parses/executed path may silently skip loop body instead of surfacing unsupported syntax/semantics.
- Trigger: `for-in`/`for-of` heads that fail `supports_for_in_lowering` / `supports_for_of_lowering`.
- Workaround: Restrict loop heads to currently supported forms (single declaration, no unsupported initializer patterns).
- Root cause: Explicit fallback branch to `condition: false` in parser lowering.

**`GeneratorFunction` constructor accepts only narrow body shape (`crates/vm/src/lib.rs`):**
- Symptoms: Constructor throws `SyntaxError: unsupported GeneratorFunction body` for many valid bodies.
- Trigger: `GeneratorFunction(...)` body containing statements outside the parser’s current `yield`-segment shortcut.
- Workaround: Avoid dynamic generator construction for non-trivial bodies.
- Root cause: `parse_generator_constructor_yield_expressions` is intentionally minimal and rejects general grammar.

**`Promise` constructor is placeholder-level (`crates/vm/src/lib.rs`):**
- Symptoms: Resolve/reject functions passed to executor do not implement real Promise state transitions/queueing.
- Trigger: `new Promise((resolve, reject) => ...)` relying on standard settlement/then chaining semantics.
- Workaround: Treat Promise support as partial; avoid behavior that depends on spec-complete job queue semantics.
- Root cause: Constructor currently injects host placeholders (`HostFunction::FunctionPrototype`) and lacks full promise internals.

## Security Considerations

**Panic-driven CLI argument handling (`crates/test-harness/src/bin/test262-run.rs`):**
- Risk: Invalid input causes process panic (hard abort), which is unsafe for service-style or untrusted input contexts.
- Current mitigation: Tool is mainly used in controlled CI/dev contexts.
- Recommendations: Switch to structured argument parsing + graceful error returns (`Result`/exit code `2`) instead of `panic!`.

**Invariant panics in production VM/compiler paths (`crates/vm/src/lib.rs`, `crates/bytecode/src/lib.rs`):**
- Risk: `expect`/`unreachable!` in runtime code can become denial-of-service if invariants are violated by unexpected input integration paths.
- Current mitigation: Parser + internal pipelines attempt to preserve invariants.
- Recommendations: Convert invariant failures to typed runtime/compile errors where reachable from public APIs; add invariant fuzzing.

**Regex backtracking risk (`crates/vm/src/lib.rs`, `crates/vm/Cargo.toml`):**
- Risk: `fancy-regex` can exhibit catastrophic backtracking on adversarial patterns/inputs.
- Current mitigation: Pattern normalization exists, but no explicit timeout/budget guard is present.
- Recommendations: Add execution budget/timeout strategy, cache compiled patterns, and gate high-risk patterns in untrusted environments.

## Performance Bottlenecks

**GC stress profile overhead in harness (`crates/test-harness/src/test262.rs`, `crates/vm/src/lib.rs`):**
- Problem: Very high collection frequency under stress mode.
- Measurement: Local run on 2026-02-25: `test262-lite` default `~159ms`; stress (`--auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1`) `~9260ms` with `collections_total=44020` for `26` executed cases.
- Cause: Threshold/interval configuration triggers near-continuous collection at runtime checkpoints.
- Improvement path: Add adaptive GC heuristics and profile-specific thresholds; keep current stress mode as guard only.

**Regex compilation on match path (`crates/vm/src/lib.rs`):**
- Problem: `normalize_regexp_pattern` + `RegexBuilder::new(...).build()` occurs during matching path.
- Measurement: No dedicated regex microbenchmark committed in repo; compile-on-exec is visible in current code path.
- Cause: No compiled-regex cache keyed by `(pattern, flags)`.
- Improvement path: Add cached compiled regex objects on RegExp instances and invalidate on `compile`/slot changes.

**Per-case thread spawn in test262 harness (`crates/test-harness/src/test262.rs`):**
- Problem: Each case runs in a dedicated thread with `32MB` configured stack.
- Measurement: `execute_case_with_options_and_stats` spawns a thread per executed case.
- Cause: Isolation strategy favors crash containment over throughput.
- Improvement path: Use worker pool or shared executor with panic boundaries to reduce thread creation overhead.

## Fragile Areas

**Caller-state shadow-root restoration (`crates/vm/src/lib.rs`):**
- Why fragile: State restoration depends on balanced push/pop across many error/return paths (`gc_shadow_roots` + caller stacks).
- Common failures: Mismatched restoration can trigger `expect("caller state shadow roots should be present")` panic.
- Safe modification: Any call-stack lifecycle change should add regression tests covering success/error/async/exception branches.
- Test coverage: Covered by VM tests, but concentrated in one large file and easy to miss during refactors.

**Loop/break/label compilation contexts (`crates/bytecode/src/lib.rs`):**
- Why fragile: Multiple parallel stacks (`loops`, `break_contexts`, `label_contexts`, completion targets) must remain consistent.
- Common failures: Context underflow currently guarded by `expect(...)`; malformed or newly introduced AST patterns can panic.
- Safe modification: Modify loop/labeled lowering with paired parser + compiler tests for nested `try/finally` control-flow.
- Test coverage: Good regression volume exists, but no dedicated invariants/fuzz layer for compiler context stacks.

**Proxy implementation strategy (`crates/vm/src/lib.rs`):**
- Why fragile: Current proxy path mirrors keys/values from target and partial trap handling mixes emulation with materialized props.
- Common failures: Spec invariants around traps/target consistency can drift as more traps are added.
- Safe modification: Move to explicit internal slot + trap-dispatch model, then phase out mirroring shortcuts.
- Test coverage: Basic `Proxy.revocable` and minimal paths exist; advanced invariant coverage remains limited.

## Scaling Limits

**test262 coverage ceiling due skip policy (`crates/test-harness/src/test262.rs`, `docs/current-status.md`):**
- Current capacity: `language` sample reports `discovered=23882`, `executed=5265` (snapshot in `docs/current-status.md`).
- Limit: Module/strict/includes/feature-gated tests are intentionally skipped (`should_skip` logic).
- Symptoms at limit: Compatibility metrics can look stable while unsupported feature surfaces remain untested.
- Scaling path: Gradually enable `module`, `onlyStrict`, harness `includes`, and feature flags with host support expansion.

**VM maintainability at current file concentration (`crates/vm/src/lib.rs`):**
- Current capacity: Single-file runtime currently `21,960` lines.
- Limit: Parallel development and safe refactor throughput degrade as merge/conflict density rises.
- Symptoms at limit: Cross-feature regressions and long review cycles for seemingly local changes.
- Scaling path: Modularize runtime into crates/modules with ownership boundaries and independent tests.

## Dependencies at Risk

**`fancy-regex` (`crates/vm/Cargo.toml`):**
- Risk: Backtracking engine characteristics can create unpredictable worst-case runtime.
- Impact: Regex-heavy workloads can become latency spikes or timeouts.
- Migration plan: Keep `regex` for safe subsets and restrict `fancy-regex` usage to patterns requiring advanced features; add cache/time budget.

## Missing Critical Features

**ES Module pipeline (`docs/semantics-checklist.md`, `docs/current-status.md`, `crates/parser/src/lib.rs`, `crates/vm/src/lib.rs`):**
- Problem: Module parse/instantiate/evaluate flow is still planned, not implemented end-to-end.
- Current workaround: Script-only execution and test262 module skip.
- Blocks: Standards-level module compatibility and realistic host integration scenarios.
- Implementation complexity: High (loader graph, linkage, environment records, error semantics).

**Promise job queue / microtask semantics (`docs/semantics-checklist.md`, `crates/vm/src/lib.rs`):**
- Problem: Promise behavior exists only as minimal constructor/async settlement surface.
- Current workaround: Limited async paths; no full microtask queue contract.
- Blocks: Spec-conformant async orchestration and many Promise-related test262 suites.
- Implementation complexity: High (queue semantics, host hooks, reentrancy ordering).

**Spec-complete collection/typed-array semantics (`crates/builtins/src/lib.rs`, `crates/vm/src/lib.rs`):**
- Problem: Several globals still depend on alias/minimal implementations.
- Current workaround: Baseline constructor wiring for compatibility smoke tests.
- Blocks: Correct WeakMap/WeakSet constraints, typed-array family behavior, and edge-case interoperability.
- Implementation complexity: Medium-High.

## Test Coverage Gaps

**Skipped strict/module/include/feature test262 paths (`crates/test-harness/src/test262.rs`):**
- What's not tested: `module`, `onlyStrict`, `async` flag behavior, harness include-dependent cases, and feature-flagged suites.
- Risk: Large semantic surfaces can regress or remain unimplemented without visibility.
- Priority: High.
- Difficulty to test: Requires host harness expansion and module/microtask runtime support.

**Compiler/VM invariant hardening around panic sites (`crates/bytecode/src/lib.rs`, `crates/vm/src/lib.rs`):**
- What's not tested: Property-based/fuzz validation for malformed AST/invariant edge cases across parser->bytecode->vm.
- Risk: Process-aborting panics can escape into consumers if invariants are broken.
- Priority: High.
- Difficulty to test: Needs fuzz harness and explicit negative corpus for internal invariants.

**Performance regression guardrails (`crates/test-harness/src/test262.rs`, `crates/vm/src/lib.rs`):**
- What's not tested: Stable latency/throughput budgets for regex-heavy and runtime-GC-heavy workloads.
- Risk: Semantic fixes can introduce large runtime cost without CI detection.
- Priority: Medium.
- Difficulty to test: Requires reproducible benchmark harness and threshold management in CI.

---

*Concerns audit: 2026-02-25*
*Update as issues are fixed or new ones discovered*
