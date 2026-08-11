# kcell-mcp

MCP tools over stdio → Host control socket. Start `kcell serve` first.

```bash
cargo run -p kcell-mcp -- --socket .kcell/kcell.sock --consumer caller-cell
```

Details: [`docs/mcp-adapter.md`](../../docs/mcp-adapter.md).
