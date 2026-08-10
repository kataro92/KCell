# Performance budgets (MVP)

Baselines are recorded with `cargo bench` / release binary timing once executors land.
Until then, keep these soft budgets for the in-process Host path.

| Metric | Budget (local M-series / mid laptop) |
|--------|--------------------------------------|
| Cold start `kcell validate` (warm disk) | < 50 ms user time target after release build |
| Parse + validate 100 Cell manifests | < 20 ms |
| Activate 10 Cells (lifecycle only) | < 1 ms |
| Idle Host RSS (no adapters) | track; no broker/DB by default |
| Dependency count in `kcell_core` | keep minimal (serde + thiserror only) |

## Rules

- No new core dependency without an ADR note in `docs/`
- Default path must not start network brokers or telemetry collectors
- Add a bench when changing lifecycle, registry, or binding hot paths

## Current core dependencies

- `serde` / `serde_yaml` / `serde_json` — manifests
- `thiserror` — errors
