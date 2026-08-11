# A2A adapter

Optional bridge: Agent Card + JSON-RPC ↔ `kcell serve`. Not part of `kcell_core`.

```bash
cargo run -p kcell-cli -- serve examples/echo-aiman/ai-man.yaml --root .
cargo run -p kcell-a2a -- \
  --socket .kcell/kcell.sock \
  --consumer caller-cell \
  --capability echo \
  --bind 127.0.0.1:3457
```

- `GET /.well-known/agent-card.json`
- `POST /` → `message/send` → Host `invoke`

Package: [`adapters/kcell-a2a/`](../adapters/kcell-a2a/).
