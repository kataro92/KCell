# KCell

<p align="center">
  <img src="docs/images/kcell-hero.png" alt="KCell" width="100%" />
</p>

KCell is a small runtime for building AI systems from **Cells** — small pieces that talk to each other.

## Simple words

| Word | Meaning |
|------|---------|
| **Cell** | One unit (can call others, or be called) |
| **Stem Cell** | Starter template used to create a Cell |
| **AI-man** | A group of Cells wired to work together |
| **Host** | The program that loads Cells, checks permissions, and connects them |

This repo is the **Host and contracts**. Apps, UIs, and models live in separate Cell repos.

## Try it

```bash
cargo build --release
cargo run -- validate cells/echo-cell/cell.yaml --json
cargo run -- run examples/echo-aiman/ai-man.yaml --root . \
  --invoke caller-cell --capability echo --json
cargo run -- new my-cell
```

Long-running Host:

```bash
cargo run -- serve examples/echo-aiman/ai-man.yaml --root . --watch cells
# other terminal:
cargo run -- call --json status
```

## Layout

```text
crates/     # Host (kcell-core) and CLI (kcell)
schemas/    # Contracts for cell.yaml and ai-man.yaml
templates/  # Stem Cell for `kcell specialize` / `kcell new`
cells/      # Example Cells
examples/   # Example AI-men
adapters/   # Optional bridges (MCP, AG-UI, A2A)
docs/       # Guides
```

## Learn more

- [Docs index](docs/README.md)
- [Write a Cell](docs/cell-authoring-for-agents.md)
- [Rules for coding agents](AGENTS.md)
- [Keep things small & fast](docs/nfr.md)
- [Compatibility when upgrading](docs/compatibility.md)

Before pushing code: `scripts/check.sh` (or install `scripts/install-git-hooks.sh`).

Install CLI from crates.io: `cargo install kcell-cli` (binary name: `kcell`).

Wiki pages (source in [`wiki/`](wiki/)): publish with `scripts/publish-wiki.sh` after the GitHub Wiki tab has its first page.

## License

[MIT](LICENSE)
