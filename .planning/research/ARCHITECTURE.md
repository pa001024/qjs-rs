# Architecture Research

**Domain:** Pure Rust JavaScript runtime library aligned to QuickJS semantics
**Researched:** 2026-02-25
**Confidence:** HIGH

## Standard Architecture

### System Overview

```text
+------------------------------------------------------------------+
|                    Conformance and Host Layer                    |
| test-harness API, test262 runner, CI gates, embedder entrypoints |
+-------------------------------+----------------------------------+
                                |
+-------------------------------v----------------------------------+
|                 Frontend and Compilation Layer                   |
| lexer -> parser -> AST checks -> bytecode compiler              |
+-------------------------------+----------------------------------+
                                |
+-------------------------------v----------------------------------+
|                    Execution Semantics Layer                     |
| VM interpreter, call frames, lexical envs, completion records   |
| Promise reaction dispatch, module evaluate entry                 |
+-------------------------------+----------------------------------+
                                |
+-------------------------------v----------------------------------+
|                      Runtime State Layer                         |
| JsValue handles, object model, descriptors, realms, intrinsics  |
| module registry, job queue, host hooks                           |
+-------------------------------+----------------------------------+
                                |
+-------------------------------v----------------------------------+
|                  Memory and Observability Layer                  |
| mark-sweep GC, root manager, weak references, GC telemetry      |
+------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `crates/lexer` | Tokenize source text | Stateless scanner with context flags (regexp/template/strict hints) |
| `crates/parser` | Build AST and enforce early errors | Recursive descent parser + strict/early validation passes |
| `crates/bytecode` | Lower AST to VM contract | Opcode enum + chunk builder + control-flow patch tables |
| `crates/runtime` | Own canonical runtime data model | `JsValue` + object handles + realm/intrinsic descriptors |
| `crates/vm` | Execute opcodes and semantic operations | Interpreter loop + env records + property algorithm paths |
| `crates/builtins` | Install and version intrinsic objects | Declarative builtin tables + realm bootstrap installer |
| `jobs subsystem` | Promise job queue and host scheduling boundary | Queue abstraction + host callback interface + drain loop |
| `modules subsystem` | ESM parse/instantiate/evaluate lifecycle | Module graph cache + link/eval state machine |
| `crates/test-harness` | Compatibility and regression gating | Script runner + test262 adapter + GC/perf summary reporting |

## Recommended Project Structure

```text
crates/
├── lexer/                       # lexical analysis
├── parser/                      # AST + early error rules
├── ast/                         # syntax tree types
├── bytecode/                    # AST -> opcode contract
├── runtime/                     # value/object/realm core
│   └── src/
│       ├── value/               # JsValue and numeric/string primitives
│       ├── object/              # object slots, descriptors, prototype links
│       ├── realm/               # realm state and intrinsic registry
│       └── host/                # host hook traits (timers, jobs, modules)
├── vm/                          # semantic execution engine
│   └── src/
│       ├── exec/                # interpreter loop and opcode dispatch
│       ├── env/                 # lexical/variable/global environment records
│       ├── call/                # call/construct/this/super mechanics
│       ├── gc/                  # root marking and sweep integration
│       ├── jobs/                # microtask queue integration
│       ├── modules/             # ESM execution bridge
│       └── regexp/              # regexp compile/cache/exec path
├── builtins/                    # intrinsic constructors/prototypes
│   └── src/
│       ├── tables/              # declarative metadata per builtin
│       └── installers/          # realm bootstrap logic
└── test-harness/                # test262 + regression orchestration
```

### Structure Rationale

- **Keep crate boundaries stable (`lexer/parser/bytecode/runtime/vm/builtins/test-harness`):** avoids cross-cutting rewrites while compatibility grows.
- **Modularize inside `crates/vm` first, then extract crates only if needed:** current main risk is a single-file VM hotspot, not missing crates.
- **Move builtin metadata to declarative tables:** reduces descriptor drift and makes phased feature landing measurable.
- **Make `jobs` and `modules` explicit subdomains:** Promise and ESM are the main architectural gap to full compatibility.

## Architectural Patterns

### Pattern 1: Stable Bytecode Contract Boundary

**What:** Keep parser/AST evolution decoupled from VM internals through a stable opcode and chunk contract.
**When to use:** Always; especially when new syntax features are landing in parallel.
**Trade-offs:** Slightly more compiler work up front, much lower VM regression blast radius.

**Example:**
```rust
let ast = parser::parse_script(source)?;
let chunk = bytecode::compile_script(&ast)?;
let result = vm.execute_in_realm(&chunk, realm)?;
```

### Pattern 2: Declarative Intrinsics and Descriptor Tables

**What:** Define builtin methods/properties and attributes in data tables, then install into a realm.
**When to use:** For all builtins beyond minimal bootstrap, especially Object/Array/Function families.
**Trade-offs:** More schema discipline; easier audits and test262 correlation.

**Example:**
```rust
BuiltinSpec::new("Array.prototype.map")
    .arity(1)
    .attributes(Attrs::WRITABLE | Attrs::CONFIGURABLE)
    .native(NativeFunction::ArrayPrototypeMap);
```

### Pattern 3: Explicit Host Boundary for Async and Modules

**What:** VM schedules jobs and module resolution through host traits, not hidden globals.
**When to use:** Before shipping Promise microtasks, ESM loading, and top-level-await behavior.
**Trade-offs:** More interfaces to maintain; clear embedder behavior and deterministic tests.

**Example:**
```rust
trait HostHooks {
    fn enqueue_promise_job(&mut self, job: Job);
    fn resolve_module(&mut self, referrer: ModuleId, specifier: &str) -> Result<ModuleId, HostError>;
}
```

## Data Flow

### Request Flow

```text
[Source JS]
    ->
[lexer/parser] -> [AST validations] -> [bytecode compile] -> [VM execute]
    ->                                                |
[Result/Exception] <- [realm + builtins bootstrap] <-+
```

### State Management

```text
[Realm]
  |- Intrinsics table
  |- Global object
  |- Module registry
  |- Promise job queue
  |- Host hooks
        |
        v
[VM]
  |- Call stack
  |- Lexical env stack
  |- Object store handles
  |- GC roots and stats
```

### Key Data Flows

1. **Script execution flow:** source -> parse -> compile -> execute -> completion value/error; this is already stable and should remain the baseline path.
2. **Promise microtask flow:** runtime operation creates reactions -> enqueue job -> host drains microtask queue -> VM executes reaction closures until queue empty.
3. **ESM lifecycle flow:** module parse -> dependency graph resolution -> instantiate environments -> evaluate in dependency order -> expose live bindings and completion state.

## Build-Order Implications

| Build Order Step | Why It Must Precede | Roadmap Implication |
|------------------|---------------------|---------------------|
| 1. VM internal modularization (`exec/env/call/gc`) | Current VM concentration is the highest regression multiplier | Do this before major Promise/ESM feature waves |
| 2. Descriptor/object invariant hardening | Builtins correctness depends on object/descriptor semantics | Finish critical Object/Function edges before broad builtin expansion |
| 3. GC root and weak-reference correctness | WeakMap/WeakSet and promise reaction retention depend on GC semantics | Keep GC quality gates active while adding async features |
| 4. Promise jobs subsystem | Async semantics require deterministic microtask scheduling | Land queue API + host hooks before full Promise test262 enabling |
| 5. ESM subsystem (parse/instantiate/evaluate) | Module execution requires stable realm/jobs interfaces | Build module graph only after jobs + realm boundaries are explicit |
| 6. Compatibility expansion (`onlyStrict`, `module`, `includes`) | Coverage should open only after runtime paths exist | Expand harness skip policy in lockstep with feature readiness |

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Current -> ~5k passing subsets | Keep current crates; split VM internals into modules; preserve end-to-end harness as single truth path |
| ~5k -> ~20k broader subsets | Add declarative builtin specs, stronger invariant tests, and queue/module boundaries with feature flags |
| Full-compatibility target (language + built-ins + module/async) | Stabilize host interfaces, enable strict/module suites, and maintain CI snapshots for semantic and GC regressions |

### Scaling Priorities

1. **First bottleneck:** VM change coupling in a monolithic implementation. Fix by module boundaries and ownership per semantic domain.
2. **Second bottleneck:** Missing async/module execution boundaries. Fix by explicit job queue + module graph interfaces before expanding coverage gates.

## Anti-Patterns

### Anti-Pattern 1: Permanent Monolithic VM File

**What people do:** Keep all runtime semantics in one giant source file for short-term speed.
**Why it's wrong:** Merge conflicts, fragile invariants, and high regression probability across unrelated features.
**Do this instead:** Split by semantic ownership (`env`, `call`, `gc`, `jobs`, `modules`, `regexp`) and require focused tests per module.

### Anti-Pattern 2: Silent Semantic Fallbacks

**What people do:** Downgrade unsupported syntax/semantics into no-op behavior to keep tests running.
**Why it's wrong:** Produces false green signals and delays detection of spec gaps.
**Do this instead:** Emit explicit parse/runtime errors for unsupported paths until correct semantics are implemented.

### Anti-Pattern 3: Placeholder Builtins as Long-Term Design

**What people do:** Keep constructor aliases or stub behavior (for Promise/Weak collections/typed arrays) indefinitely.
**Why it's wrong:** Hidden incompatibility accumulates and makes later corrections disruptive.
**Do this instead:** Track placeholder surfaces explicitly and replace with dedicated internal slots + behavior tables in planned phases.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| test262 corpus | Local fixture + harness adapter | Keep skip policy explicit and shrink it per feature milestone |
| QuickJS reference engine | Behavioral diff and targeted fixture comparison | Use for edge-case confirmation, not source-level translation |
| Embedding host application | Trait-based runtime hooks | Core runtime stays pure Rust, no C FFI in runtime core |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `parser` <-> `bytecode` | Typed AST structures | Parser owns syntax/early errors; compiler owns lowering only |
| `bytecode` <-> `vm` | Opcode/chunk contract | Keep contract versioned and testable via disassembly snapshots |
| `runtime` <-> `vm` | Handle/object/realm APIs | Runtime owns data model; VM owns semantic operations |
| `builtins` <-> `vm` | Native function IDs + installer APIs | Builtins declare surfaces; VM executes native semantics |
| `vm` <-> `jobs/modules` | Queue and loader interfaces | Enables deterministic Promise/ESM behavior and host integration |
| `test-harness` <-> engine | Public run APIs + suite summaries | Single conformance gate for roadmap decisions |

## Sources

- `.planning/PROJECT.md`
- `.planning/codebase/ARCHITECTURE.md`
- `.planning/codebase/STRUCTURE.md`
- `.planning/codebase/CONCERNS.md`
- `docs/current-status.md`
- `docs/quickjs-mapping.md` (project reference map)
- `docs/semantics-checklist.md` (project semantic target checklist)

---
*Architecture research for: pure Rust JavaScript runtime aligned with QuickJS*
*Researched: 2026-02-25*
