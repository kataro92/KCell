# MCP adapter

Optional bridge: MCP client ↔ `kcell serve`. Not part of `kcell_core`.

```bash
cargo run -p kcell-cli -- serve examples/echo-aiman/ai-man.yaml --root .
cargo run -p kcell-mcp -- --socket .kcell/kcell.sock --consumer caller-cell
```

| MCP | Host |
|-----|------|
| `tools/list` | `discover` |
| `tools/call` | `invoke` |

Tool name: `{cell}__{capability}`. Args: `{ "payload": { … } }`.

Package: [`adapters/kcell-mcp/`](../adapters/kcell-mcp/).
