# Engine Benchmark Report

- Generated at: `2026-02-27T13:55:32.421Z`
- Host: `windows/x86_64`, logical CPUs: `16`
- Rust: `rustc 1.93.1 (01f6ddf75 2026-02-11)`
- Node.js: `v24.13.0`
- QuickJS(C): `QuickJS version 2025-09-13`
- Config: `5` samples/case, `120` iterations/sample

## Mean Latency by Case (Lower is Better)

![engine benchmark chart](engine-benchmark-chart.svg)

## Per-case Mean Latency

| Case | qjs-rs mean(ms) | boa-engine mean(ms) | quickjs-c mean(ms) | nodejs mean(ms) |
|------|-----------------:|--------------------:|-------------------:|---------------:|
| arith-loop | 1046.119 | 103.279 | 14.200 | 1.416 |
| fib-iterative | 73.923 | 13.128 | 2.800 | 0.502 |
| array-sum | 1520.806 | 235.808 | 17.600 | 3.924 |
| json-roundtrip | 7.242 | 11.542 | 2.400 | 0.356 |

## Aggregate Comparison

| Engine | Avg mean(ms) across cases | Relative vs qjs-rs (higher=faster) |
|--------|---------------------------:|------------------------------------:|
| qjs-rs | 662.022 | 1.000x |
| boa-engine | 90.939 | 7.280x |
| quickjs-c | 9.250 | 71.570x |
| nodejs | 1.550 | 427.234x |
