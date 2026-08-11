# AG-UI adapter

Optional bridge: HTTP + SSE ↔ `kcell serve`. Not part of `kcell_core`.

```bash
cargo run -p kcell -- serve examples/echo-aiman/ai-man.yaml --root .
cargo run -p kcell-agui -- \
  --socket .kcell/kcell.sock \
  --consumer caller-cell \
  --capability echo \
  --bind 127.0.0.1:3456
```

`POST /agent` with AG-UI run input → Host `invoke` → SSE events (`RUN_STARTED` … `RUN_FINISHED`).

Package: [`adapters/kcell-agui/`](../adapters/kcell-agui/).
