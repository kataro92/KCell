# KCell

Composable AI cells. Build specialized agents from a **Stem Cell**, compose them into an **AI-man**, and let them discover and connect at runtime.

## Idea

- **Cell** — one AI unit with *active* (outbound) and *passive* (inbound) communication
- **Stem Cell** — generic base that absorbs config, libraries, and code to become any specialized Cell
- **AI-man** — a folder of Cells composed together (like an organism made of many cells)

Example: drop an Ollama brain Cell, a web chatbot Cell, and an auto-config Cell into one AI-man. Activate the host and you get a local chat UI backed by Ollama. Later, drop in a Cursor CLI brain Cell — the web UI can pick up Composer as another reply path without rewriting the other Cells.

## Design principles

- **Core only** — small runtime: manifest, lifecycle, registry, binding, policy, execution interfaces
- **Adapters outside core** — AG-UI, A2A, MCP, transports, and model integrations stay optional packages
- **Local-first** — one machine for MVP; contracts stay portable to multi-host later
- **Untrusted by default** — Cells run under deny-by-default capabilities / sandbox
- **Agent-authored** — schemas, CLI, and docs are written so coding agents can create Cells safely

## Repository layout

```text
core/           # Host runtime (mechanisms only)
schemas/        # cell.yaml, ai-man.yaml, binding contracts
adapters/       # Protocol / transport adapters (out of core)
cells/          # Reference / specialized Cells
examples/       # Minimal AI-man compositions
docs/           # Specs and agent authoring guides
benches/        # Performance budgets and regression gates
AGENTS.md       # Instructions for AI coding agents
```

## Status

Early scaffolding. Spec and core implementation are in progress. See `docs/` and `AGENTS.md` before contributing Cells or touching the core.

## License

TBD
