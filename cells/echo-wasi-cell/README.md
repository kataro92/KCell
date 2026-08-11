# echo-wasi-cell

WASI demo Cell (same JSON-line protocol as subprocess).

```bash
rustup target add wasm32-wasip1
rustc --target wasm32-wasip1 -O -o echo.wasm main.rs
cargo run -p kcell --features wasi -- run examples/echo-wasi-aiman/ai-man.yaml \
  --root . --invoke caller-wasi-cell --capability echo-wasi
```
