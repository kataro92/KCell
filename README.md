# KCell

<p align="center">
  <img src="docs/images/kcell-hero.png" alt="KCell" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/kataro92/KCell/releases/tag/v0.0.1"><img src="https://img.shields.io/badge/release-v0.0.1-blue" alt="v0.0.1" /></a>
  <a href="https://crates.io/crates/kcell-cli"><img src="https://img.shields.io/crates/v/kcell-cli.svg" alt="kcell-cli on crates.io" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT" /></a>
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

## Install

```bash
cargo install kcell-cli
kcell --help
```

Current version: **0.0.1** · [Release notes](https://github.com/kataro92/KCell/releases/tag/v0.0.1) · [Wiki](https://github.com/kataro92/KCell/wiki)

Also on crates.io: [`kcell_core`](https://crates.io/crates/kcell_core), [`kcell-mcp`](https://crates.io/crates/kcell-mcp), [`kcell-agui`](https://crates.io/crates/kcell-agui), [`kcell-a2a`](https://crates.io/crates/kcell-a2a).

## Try it (from this repo)

```bash
cargo build --release
cargo run -p kcell-cli -- validate cells/echo-cell/cell.yaml --json
cargo run -p kcell-cli -- run examples/echo-aiman/ai-man.yaml --root . \
  --invoke caller-cell --capability echo --json
cargo run -p kcell-cli -- new my-cell
```

Long-running Host:

```bash
cargo run -p kcell-cli -- serve examples/echo-aiman/ai-man.yaml --root . --watch cells
# other terminal:
cargo run -p kcell-cli -- call --json status
```

## Layout

```text
crates/kcell-core/   # Host library
crates/kcell-cli/    # CLI (binary: kcell)
schemas/             # cell.yaml / ai-man.yaml contracts
templates/           # Stem Cell for specialize / new
cells/               # Example Cells
examples/            # Example AI-men
adapters/            # MCP, AG-UI, A2A
docs/                # Guides
wiki/                # GitHub Wiki source
```

## Learn more

- [Docs](docs/README.md) · [Write a Cell](docs/cell-authoring-for-agents.md) · [Wiki](https://github.com/kataro92/KCell/wiki)
- [AGENTS.md](AGENTS.md) · [Compatibility](docs/compatibility.md) · [NFRs](docs/nfr.md)

Before push: `scripts/check.sh` (or `scripts/install-git-hooks.sh` once).

## License

[MIT](LICENSE)
