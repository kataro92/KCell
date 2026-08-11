# Getting started

```bash
git clone https://github.com/kataro92/KCell.git
cd KCell
cargo build --release
```

```bash
cargo run -- validate cells/echo-cell/cell.yaml --json
cargo run -- run examples/echo-aiman/ai-man.yaml --root . \
  --invoke caller-cell --capability echo --json
cargo run -- new my-cell
```

Long-running Host:

```bash
cargo run -- serve examples/echo-aiman/ai-man.yaml --root . --watch cells
cargo run -- call --json status
```

Before push: `scripts/check.sh`
