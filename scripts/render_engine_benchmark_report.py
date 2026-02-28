#!/usr/bin/env python3
"""Render benchmark JSON into SVG + markdown with contract metadata context."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ENGINE_ORDER = ["qjs-rs", "boa-engine", "quickjs-c", "nodejs"]
ENGINE_COLORS = {
    "qjs-rs": "#2563eb",
    "boa-engine": "#16a34a",
    "quickjs-c": "#d97706",
    "nodejs": "#dc2626",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--input",
        default="target/benchmarks/engine-comparison.json",
        help="path to benchmark json",
    )
    parser.add_argument(
        "--chart",
        default="docs/reports/engine-benchmark-chart.svg",
        help="output svg chart path",
    )
    parser.add_argument(
        "--report",
        default="docs/reports/engine-benchmark-report.md",
        help="output markdown report path",
    )
    return parser.parse_args()


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def engine_status_map(report: dict) -> dict[str, dict[str, Any]]:
    reproducibility = report.get("reproducibility", {})
    statuses = reproducibility.get("engine_status", [])
    mapping: dict[str, dict[str, Any]] = {}
    if isinstance(statuses, list):
        for entry in statuses:
            if isinstance(entry, dict):
                engine = entry.get("engine")
                if isinstance(engine, str):
                    mapping[engine] = entry
    return mapping


def draw_svg(report: dict, output_path: Path) -> None:
    cases = report["cases"]
    width = 1200
    height = 620
    margin_left = 70
    margin_right = 30
    margin_top = 50
    margin_bottom = 120

    inner_w = width - margin_left - margin_right
    inner_h = height - margin_top - margin_bottom

    max_ms = 0.0
    for case in cases:
        engines = case.get("engines", {})
        if not isinstance(engines, dict):
            continue
        for engine in ENGINE_ORDER:
            result = engines.get(engine)
            if isinstance(result, dict) and isinstance(result.get("mean_ms"), (int, float)):
                max_ms = max(max_ms, float(result["mean_ms"]))
    max_ms = max(max_ms, 1.0)

    group_count = len(cases)
    group_w = inner_w / max(group_count, 1)
    bar_w = group_w / (len(ENGINE_ORDER) + 1.5)

    def y_scale(value_ms: float) -> float:
        return margin_top + inner_h - (value_ms / max_ms) * inner_h

    parts: list[str] = []
    parts.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" font-family="Inter,Segoe UI,Arial,sans-serif">'
    )
    parts.append(f'<rect width="{width}" height="{height}" fill="#ffffff"/>')

    # title
    parts.append(
        '<text x="70" y="28" font-size="20" font-weight="700" fill="#111827">'
        "JS Engine Benchmark (mean ms, lower is better)"
        "</text>"
    )

    # grid + y ticks
    ticks = 5
    for i in range(ticks + 1):
        value = max_ms * i / ticks
        y = y_scale(value)
        parts.append(
            f'<line x1="{margin_left}" y1="{y:.2f}" x2="{width - margin_right}" y2="{y:.2f}" '
            'stroke="#e5e7eb" stroke-width="1"/>'
        )
        parts.append(
            f'<text x="{margin_left - 8}" y="{y + 4:.2f}" text-anchor="end" '
            'font-size="11" fill="#6b7280">'
            f"{value:.1f}ms</text>"
        )

    # bars
    for idx, case in enumerate(cases):
        group_x = margin_left + idx * group_w
        engines = case.get("engines", {})
        if not isinstance(engines, dict):
            engines = {}
        for j, engine in enumerate(ENGINE_ORDER):
            result = engines.get(engine)
            if not isinstance(result, dict):
                continue
            mean_ms = result.get("mean_ms")
            if not isinstance(mean_ms, (int, float)):
                continue
            x = group_x + (j + 0.4) * bar_w
            y = y_scale(mean_ms)
            h = margin_top + inner_h - y
            color = ENGINE_COLORS[engine]
            parts.append(
                f'<rect x="{x:.2f}" y="{y:.2f}" width="{bar_w * 0.9:.2f}" height="{h:.2f}" '
                f'fill="{color}" rx="3"/>'
            )
        center = group_x + group_w / 2
        parts.append(
            f'<text x="{center:.2f}" y="{height - margin_bottom + 24}" text-anchor="middle" '
            'font-size="11" fill="#111827">'
            f'{case["id"]}</text>'
        )

    # axes
    parts.append(
        f'<line x1="{margin_left}" y1="{margin_top + inner_h}" x2="{width - margin_right}" '
        f'y2="{margin_top + inner_h}" stroke="#111827" stroke-width="1.2"/>'
    )
    parts.append(
        f'<line x1="{margin_left}" y1="{margin_top}" x2="{margin_left}" y2="{margin_top + inner_h}" '
        'stroke="#111827" stroke-width="1.2"/>'
    )

    # legend
    lx = width - 300
    ly = 22
    for i, engine in enumerate(ENGINE_ORDER):
        x = lx + i * 95
        parts.append(
            f'<rect x="{x}" y="{ly - 10}" width="12" height="12" fill="{ENGINE_COLORS[engine]}" rx="2"/>'
        )
        parts.append(
            f'<text x="{x + 17}" y="{ly}" font-size="12" fill="#374151">{engine}</text>'
        )

    parts.append("</svg>")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(parts), encoding="utf-8")


def render_table(report: dict) -> str:
    statuses = engine_status_map(report)

    def render_cell(case: dict, engine: str) -> str:
        engines = case.get("engines", {})
        if isinstance(engines, dict):
            result = engines.get(engine)
            if isinstance(result, dict):
                mean_ms = result.get("mean_ms")
                if isinstance(mean_ms, (int, float)):
                    return f"{mean_ms:.3f}"
        status = statuses.get(engine, {}).get("status")
        if isinstance(status, str):
            return f"N/A ({status})"
        return "N/A"

    rows = [
        "| Case | qjs-rs mean(ms) | boa-engine mean(ms) | quickjs-c mean(ms) | nodejs mean(ms) |",
        "|------|-----------------:|--------------------:|-------------------:|---------------:|",
    ]
    for case in report["cases"]:
        rows.append(
            f'| {case["id"]} | {render_cell(case, "qjs-rs")} | '
            f'{render_cell(case, "boa-engine")} | {render_cell(case, "quickjs-c")} | '
            f'{render_cell(case, "nodejs")} |'
        )
    return "\n".join(rows)


def render_aggregate(report: dict) -> str:
    statuses = engine_status_map(report)
    mean_map = report["aggregate"]["mean_ms_per_engine"]
    rel = report["aggregate"]["relative_to_qjs_rs"]
    lines = [
        "| Engine | Avg mean(ms) across cases | Relative vs qjs-rs (higher=faster) |",
        "|--------|---------------------------:|------------------------------------:|",
    ]
    for engine in ENGINE_ORDER:
        if engine in mean_map and engine in rel:
            lines.append(f"| {engine} | {mean_map[engine]:.3f} | {rel[engine]:.3f}x |")
            continue
        status = statuses.get(engine, {}).get("status")
        if isinstance(status, str):
            lines.append(f"| {engine} | N/A ({status}) | N/A |")
        else:
            lines.append(f"| {engine} | N/A | N/A |")
    return "\n".join(lines)


def render_contract_metadata(report: dict) -> str:
    reproducibility = report.get("reproducibility", {})
    output_policy = reproducibility.get("output_policy", {})
    config = report.get("config", {})
    required_engines = reproducibility.get("required_engines", [])
    required_cases = reproducibility.get("required_case_ids", [])

    required_engines_text = ", ".join(required_engines) if required_engines else "n/a"
    required_cases_text = ", ".join(required_cases) if required_cases else "n/a"

    return "\n".join(
        [
            f'- schema version: `{report.get("schema_version", "unknown")}`',
            f'- run profile: `{report.get("run_profile", "unknown")}`',
            f'- timing mode: `{report.get("timing_mode", "unknown")}`',
            f'- comparator strict mode: `{reproducibility.get("comparator_strict_mode", "unknown")}`',
            f'- output default path: `{output_policy.get("default_path", "unknown")}`',
            f'- output effective path: `{output_policy.get("effective_path", "unknown")}`',
            f'- run controls: `iterations={config.get("iterations", "n/a")}`, '
            f'`samples={config.get("samples", "n/a")}`, '
            f'`warmup_iterations={config.get("warmup_iterations", "n/a")}`',
            f"- required engines: `{required_engines_text}`",
            f"- required cases: `{required_cases_text}`",
        ]
    )


def render_engine_status_table(report: dict) -> str:
    statuses = engine_status_map(report)
    lines = [
        "| Engine | Status | Command | Path | Workdir | Version | Reason |",
        "|--------|--------|---------|------|---------|---------|--------|",
    ]
    for engine in ENGINE_ORDER:
        entry = statuses.get(engine, {})
        lines.append(
            "| {engine} | {status} | `{command}` | `{path}` | `{workdir}` | `{version}` | {reason} |".format(
                engine=engine,
                status=entry.get("status", "missing-metadata"),
                command=entry.get("command", "unknown"),
                path=entry.get("path", "n/a"),
                workdir=entry.get("workdir", "n/a"),
                version=entry.get("version", "n/a"),
                reason=entry.get("reason", "—"),
            )
        )
    return "\n".join(lines)


def write_markdown(report: dict, report_path: Path, chart_path: Path) -> None:
    env = report["environment"]
    relative_chart = chart_path.name
    content = f"""# Engine Benchmark Report

- Generated at: `{report["generated_at_utc"]}`
- Host: `{env["os"]}/{env["arch"]}`, logical CPUs: `{env["cpu_parallelism"]}`
- Rust: `{env["rustc"]}`
- Node.js: `{env["node"]}`
- QuickJS(C): `{env.get("quickjs_c", "unknown")}`

## Contract Metadata

{render_contract_metadata(report)}

## Comparator Availability + Version Status

{render_engine_status_table(report)}

Unsupported/missing comparators are flagged above and shown as `N/A (status)` in the latency tables below.

## Mean Latency by Case (Lower is Better)

![engine benchmark chart]({relative_chart})

## Per-case Mean Latency

{render_table(report)}

## Aggregate Comparison

{render_aggregate(report)}
"""
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(content, encoding="utf-8")


def main() -> None:
    args = parse_args()
    input_path = Path(args.input)
    chart_path = Path(args.chart)
    report_path = Path(args.report)

    report = load_json(input_path)
    draw_svg(report, chart_path)
    write_markdown(report, report_path, chart_path)
    print(f"Wrote chart: {chart_path}")
    print(f"Wrote report: {report_path}")


if __name__ == "__main__":
    main()
