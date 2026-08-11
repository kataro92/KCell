# KCell

KCell is a small Host runtime for building AI systems from **Cells** — small pieces that talk to each other.

## Words

| Word | Meaning |
|------|---------|
| **Cell** | One unit (can call others, or be called) |
| **Stem Cell** | Starter template used to create a Cell |
| **AI-man** | A group of Cells wired to work together |
| **Host** | Loads Cells, checks permissions, connects them |

This repo is the **Host and contracts**. Apps and models live in separate Cell repos.

## Quick start

```bash
cargo build --release
cargo run -- run examples/echo-aiman/ai-man.yaml --root . \
  --invoke caller-cell --capability echo --json
cargo run -- new my-cell
```

## Pages

- [[Getting-Started]]
- [[Concepts]]
- [[CLI]]
- [[Adapters]]
- [[Compatibility]]

Canonical docs in the repo: [docs/](https://github.com/kataro92/KCell/tree/main/docs) · [README](https://github.com/kataro92/KCell#readme)
