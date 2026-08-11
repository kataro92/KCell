# Control socket

`kcell serve` keeps an AI-man running. Other tools talk to it with **one JSON line in → one JSON line out** on a Unix socket (default `.kcell/kcell.sock`).

## Request / response

```json
{
  "schema": "kcell.control.v1",
  "id": "ctrl-1",
  "op": "status",
  "capability": null,
  "consumer": null,
  "payload": {},
  "path": null,
  "cell": null,
  "replace": false,
  "apply": false,
  "limit": 20
}
```

```json
{
  "schema": "kcell.control.v1",
  "id": "ctrl-1",
  "ok": true,
  "error": null,
  "result": {}
}
```

## Ops

| op | Need | Result |
|----|------|--------|
| `ping` | — | `{ "pong": true }` |
| `status` | — | cells and counts |
| `discover` | `capability?` | providers |
| `invoke` | `consumer`, `capability`, `payload?` | reply envelope |
| `audit` | `limit?` | recent events |
| `load` | `path`, `replace?` | load Cell dir |
| `unload` | `cell` | stop and remove |
| `apply_bindings` | `path` | apply BindingProposal YAML |
| `auto_bind` | `apply?` | fill missing bindings |
| `propose_from` | `cell`, `apply?` | ask a Cell for a proposal |
| `shutdown` | — | stop server |

## Examples

```bash
cargo run -- serve examples/echo-aiman/ai-man.yaml --root . --watch cells
cargo run -- call --json status
cargo run -- call --json invoke --consumer caller-cell --capability echo
cargo run -- call --json load --path cells/echo-sub-cell
cargo run -- call shutdown
```

Watch: first scan is baseline only; later scans load new/changed Cells and unload removed ones. Use `--auto-bind` to rebind after loads.

```bash
cargo run -p kcell --features notify -- serve … --watch cells --watch-notify
```

`--watch-interval-ms` is the poll period (or notify debounce).

## Saved state

`serve` writes hot-loaded Cells, bindings, and policy grants to `.kcell/host-state.json`.

| Flag | Meaning |
|------|---------|
| `--state PATH` | State file |
| `--no-persist` | Do not write |
| `--no-restore` | Do not restore on start |

Unload also clears that Cell’s executors and related bindings.
