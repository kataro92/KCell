# WASI Cells

Same JSON-line protocol as [stdio](stdio-protocol.md). Needs Cargo feature `wasi` (off by default).

```bash
cargo build -p kcell --features wasi
```

```yaml
spec:
  runtime:
    kind: wasi
    entrypoint: _start
    artifact: echo.wasm   # under the Cell dir
```

Demo: [`cells/echo-wasi-cell/`](../cells/echo-wasi-cell/).

```bash
rustup target add wasm32-wasip1
(cd cells/echo-wasi-cell && rustc --target wasm32-wasip1 -O -o echo.wasm main.rs)
cargo run -p kcell --features wasi -- run examples/echo-wasi-aiman/ai-man.yaml \
  --root . --invoke caller-wasi-cell --capability echo-wasi
```
