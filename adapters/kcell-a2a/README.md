# kcell-a2a

A2A Agent Card + JSON-RPC → Host `invoke`. Start `kcell serve` first.

```bash
cargo run -p kcell-a2a -- \
  --socket .kcell/kcell.sock \
  --consumer caller-cell \
  --capability echo \
  --bind 127.0.0.1:3457
```

Details: [`docs/a2a-adapter.md`](../../docs/a2a-adapter.md).
