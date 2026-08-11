# kcell-agui

AG-UI HTTP + SSE → Host `invoke`. Start `kcell serve` first.

```bash
cargo run -p kcell-agui -- \
  --socket .kcell/kcell.sock \
  --consumer caller-cell \
  --capability echo \
  --bind 127.0.0.1:3456
```

Details: [`docs/agui-adapter.md`](../../docs/agui-adapter.md).
