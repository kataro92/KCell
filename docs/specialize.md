# Specialize

Create a Cell from the Stem template:

```bash
kcell specialize my-echo \
  --from templates/stem-cell \
  --dir cells/my-echo \
  --provide echo:1 \
  --runtime inprocess \
  --build \
  --json
```

| Flag | Notes |
|------|--------|
| `--from` | Stem dir (default `templates/stem-cell`) |
| `--provide` / `--require` | `name` or `name:version` (repeatable) |
| `--runtime` | `inprocess` \| `subprocess` \| `wasi` |
| `wasi` | Needs `--artifact` (`.wasm`) |
| `--build` | Writes package digest |

`kcell new NAME` is a short form: one provide, passive, in-process.

More: [Write a Cell](cell-authoring-for-agents.md), [`templates/stem-cell/`](../templates/stem-cell/).
