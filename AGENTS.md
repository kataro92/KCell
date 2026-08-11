# Instructions for AI coding agents

KCell is a small Host plus stable contracts. Product Cells and adapters live outside core — do not fork core to ship a feature.

- Rules: [`docs/nfr.md`](docs/nfr.md)
- Compatibility: [`docs/compatibility.md`](docs/compatibility.md)

## Words

| Term | Meaning |
|------|---------|
| Stem Cell | Template used to create a Cell |
| Specialized Cell | Finished Cell package (version + digest); change ⇒ new revision |
| Active / Passive | Call out / be called |
| AI-man | Cells + bindings + policy |
| Host | Loads Cells, checks permissions, applies bindings |

Auto-config may **propose** bindings; only the Host applies them.

## Where code goes

- **Core** — only if every Cell needs it
- **CLI** — thin operator
- **Adapters** — MCP / AG-UI / A2A / transports
- **Cells / child repos** — product logic

## Must keep

1. Compact — fewer APIs/deps; delete dead code in the same PR  
2. Fast — no default brokers/DB/telemetry; honor `benches/BUDGET.md`  
3. Simple — one concept, one place; schemas win over prose  
4. Extensible — new Cell/protocol without rewriting core  
5. Compatible — Host major N runs Cells valid on N−1 (`docs/compatibility.md`)

## New Cell

1. `kcell specialize` or `kcell new`  
2. Fill `cell.yaml`  
3. Implement in a Cell/child repo  
4. `validate` → `build` → `inspect`  
5. Minimum permissions  

Prefer `--json` and schemas. Exit `0` = success.

## Hard rules

- No secrets in packages; packages are immutable  
- Deny-by-default permissions  
- No new core API/dep without NFR review  
- Do not replace AG-UI / A2A / MCP at the boundary  
- Do not force child repos to patch `kcell_core`  
- Contract changes follow [`docs/compatibility.md`](docs/compatibility.md); keep `tests/fixtures/compat/` green  
- **Before every push:** run `scripts/check.sh` (fmt + clippy `-D warnings` + `kcell_core` lib tests). Install the git hook once with `scripts/install-git-hooks.sh`. Do not push if check fails.  

## Docs

[`docs/README.md`](docs/README.md) · `schemas/` · `tests/fixtures/compat/` · `benches/BUDGET.md`

Optional features: `wasi`, `notify` (off by default).
