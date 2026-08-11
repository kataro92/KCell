# auto-config-cell

Proposes bindings only. The Host applies them.

```bash
cargo run -- serve examples/echo-autoconfig-aiman/ai-man.yaml --root .
cargo run -- call --json propose-from --cell auto-config-cell --apply
```

Provides `binding-propose@1` via `kcell worker-autoconfig`. See [docs/auto-config.md](../../docs/auto-config.md).
