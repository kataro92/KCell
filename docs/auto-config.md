# Auto-config

An **auto-config Cell** suggests how to connect Cells. Only the **Host** applies the suggestion.

```bash
cargo run -- serve examples/echo-autoconfig-aiman/ai-man.yaml --root .
cargo run -- call --json propose-from --cell auto-config-cell --apply
cargo run -- call --json invoke --consumer caller-cell --capability echo
```

- Capability: `binding-propose@1`
- Package: [`cells/auto-config-cell/`](../cells/auto-config-cell/)
- Worker: `kcell worker-autoconfig`

Do not apply bindings inside the Cell.

The Host also has a built-in `auto_bind` op for a simple default match.
