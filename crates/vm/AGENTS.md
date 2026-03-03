# VM KNOWLEDGE BASE

**Scope:** `crates/vm/` execution engine internals and VM-facing tests.

## OVERVIEW
`vm` is the runtime core: bytecode execution, object model, module lifecycle, promise jobs, GC, and fast-path toggles.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Full execution reset + run entry | `crates/vm/src/lib.rs` | `Vm::execute_in_realm` |
| Host-facing runtime wrapper | `crates/vm/src/script_runtime.rs` | `ScriptRuntime` + callback registration |
| Promise static/combinator behavior | `crates/vm/src/promise_builtins.rs` | resolve/reject/all/any/race/allSettled |
| Fast-path guards and counters | `crates/vm/src/fast_path.rs` | packet A/B/C/D/G/H states + metrics |
| Perf attribution toggles | `crates/vm/src/perf.rs` | env flag + attribution snapshot |
| Runtime limit checks | `crates/vm/src/runtime_limits.rs` | memory/stack/interrupt enforcement |

## TEST MAP
| Area | Location |
|------|----------|
| Module lifecycle | `crates/vm/tests/module_lifecycle.rs` |
| Collection semantics | `crates/vm/tests/collection_semantics.rs` |
| Native error behavior | `crates/vm/tests/native_errors.rs` |
| Packet perf guards | `crates/vm/tests/perf_packet_*.rs` |
| Hotspot attribution | `crates/vm/tests/perf_hotspot_attribution.rs` |

## CONVENTIONS (VM)
- Keep semantics-first behavior alignment; optimization packets are gated and measurable.
- Preserve explicit lifecycle handling for module states and promise job queue transitions.
- Use typed runtime errors (`VmError`) and stable error identifiers for policy-sensitive flows.
- Keep unsafe disabled; low-level speedups still route through safe Rust structures.

## ANTI-PATTERNS (VM)
- Shortcutting state reset paths inside `execute_in_realm`.
- Adding hidden long-lived host references instead of explicit register/unregister flow.
- Enabling packet metrics/toggles without associated tests and benchmark evidence.
- Changing strict-mode/object/property semantics without test262-lite/harness validation.

## COMMANDS
```bash
cargo test -p vm
cargo test -p vm module_lifecycle -- --exact
cargo test -p vm regexp_last_index_transition_matrix -- --exact
```

## NOTES
- This crate is the primary hotspot in the workspace; small edits can affect multiple semantic domains.
- Coordinate behavior changes with `crates/test-harness` tests and benchmark contract checks.
