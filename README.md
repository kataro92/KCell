# KCell

Composable AI cells. Build specialized agents from a **Stem Cell**, compose them into an **AI-man**, and let them discover and connect at runtime.

## Idea

- **Cell** — one AI unit with *active* (outbound) and *passive* (inbound) communication
- **Stem Cell** — generic base that absorbs config, libraries, and code to become any specialized Cell
- **AI-man** — a folder of Cells composed together (like an organism made of many cells)

## Quick start

```bash
cargo build --release
cargo run -- validate cells/echo-cell/cell.yaml --json
cargo run -- run examples/echo-aiman/ai-man.yaml --root . --json
cargo run -- new my-cell
```

## Design principles

- **Core only** — small runtime: manifest, lifecycle, registry, binding, policy, execution interfaces
- **Adapters outside core** — AG-UI, A2A, MCP, transports, and model integrations stay optional packages
- **Local-first** — one machine for MVP; contracts stay portable to multi-host later
- **Untrusted by default** — Cells run under deny-by-default capabilities / sandbox
- **Agent-authored** — schemas, CLI, and docs are written so coding agents can create Cells safely

## Repository layout

```text
crates/kcell-core/   # Host runtime (mechanisms only)
crates/kcell-cli/    # kcell CLI (new, validate, inspect, run)
schemas/             # cell.yaml, ai-man.yaml, binding contracts
adapters/            # Protocol / transport adapters (out of core)
cells/               # Reference / specialized Cells
examples/            # Minimal AI-man compositions
docs/                # Specs and agent authoring guides
benches/             # Performance budgets
AGENTS.md            # Instructions for AI coding agents
```

## Status

Core MVP in progress: manifests, lifecycle, local registry, binding proposals, policy gate, CLI.

## License

MIT OR Apache-2.0
