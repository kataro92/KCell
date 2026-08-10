# Performance budgets (MVP)

Tied to [NFR-2](../docs/nfr.md#nfr-2--extreme-performance). Baselines use `cargo bench` / release timing as executors land.

| Metric | Budget (local M-series / mid laptop) |
|--------|--------------------------------------|
| Cold start `kcell validate` (warm disk) | < 50 ms user time after release build |
| Parse + validate 100 Cell manifests | < 20 ms |
| Activate 10 Cells (lifecycle only) | < 1 ms |
| Idle Host RSS (no adapters) | track; no broker/DB by default |
| `kcell_core` dependency set | minimal (serde family + thiserror only unless NFR exception) |

## Rules

- No new core dependency without NFR review + note here or in `docs/`
- Default path must not start network brokers or telemetry collectors
- Add a bench when changing lifecycle, registry, or binding hot paths
- Budget changes require measured evidence, not intuition

## Current core dependencies

- `serde` / `serde_yaml` / `serde_json` — manifests
- `thiserror` — errors
