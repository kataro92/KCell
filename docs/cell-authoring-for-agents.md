# Write a Cell

Contracts: [`schemas/`](../schemas/).

## Layout

```text
my-cell/
  cell.yaml    # required
  README.md    # optional
  src/         # optional code
```

```text
my-aiman/
  ai-man.yaml  # lists Cells (paths relative to --root)
```

## Commands

```bash
kcell new my-cell
kcell specialize my-cell --from templates/stem-cell --provide my-cell:1
kcell validate path/to/cell.yaml --json
kcell build path/to/cell-dir --json
kcell run path/to/ai-man.yaml --root . --invoke consumer --capability some-cap --json
```

Exit `0` = ok. Prefer `--json` in scripts.

Subprocess Cells: one JSON line in, one out. See [stdio protocol](stdio-protocol.md). Demo worker: `kcell worker`.

## Checklist

1. `apiVersion: kcell.dev/v1`, `kind: Cell`
2. Valid `name` and semver `version`
3. At least one `provides` or `requires`
4. `communication` active and/or passive
5. Ask for only the permissions you need
6. No secrets in the package
7. Do not change core for one product Cell — use a child repo

## Example

[`cells/echo-cell`](../cells/echo-cell) + [`examples/echo-aiman`](../examples/echo-aiman).

Auto-config: [`docs/auto-config.md`](auto-config.md).
