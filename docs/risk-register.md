# Risk Register

| ID | Risk | Impact | Mitigation | Status |
| --- | --- | --- | --- | --- |
| R-001 | QuickJS C internals do not map 1:1 to Rust ownership/borrowing model. | High | Preserve semantics first, redesign internal representation with handles/arenas. | Open |
| R-002 | GC strategy blocks feature velocity if chosen too early. | High | Ship simple mark-sweep first, optimize after parity milestones. | Open |
| R-003 | Parser/VM scope creep delays runnable baseline. | Medium | Enforce feature-slice milestones and acceptance checks per phase. | Open |
| R-004 | Compatibility gaps become hard to trace late in project. | Medium | Maintain mapping/checklist docs and run frequent regression tests. | Open |
| R-005 | Function closure capture may diverge from JS lexical reference semantics in edge cases. | Medium | Continue hardening around hoisting/recursion edge cases after switching to reference-based lexical environments. | In Progress |
| R-006 | Function declaration hoisting in blocks currently uses simplified behavior vs full spec nuance. | Medium | Add strict-mode aware block-function tests and align runtime behavior incrementally. | Open |
