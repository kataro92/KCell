# Cell authoring for AI agents

Machine-readable source of truth: [`schemas/`](../schemas/).

## Canonical layout

```text
my-cell/
  cell.yaml     # required
  README.md     # optional short description
  src/          # optional implementation (runtime-specific)
```

AI-man composition:

```text
my-aiman/
  ai-man.yaml   # lists cells by relative path from --root
```

## Commands (non-interactive)

```bash
kcell new my-cell
kcell validate path/to/cell.yaml --json
kcell inspect path/to/cell.yaml
kcell validate path/to/ai-man.yaml --json
kcell run path/to/ai-man.yaml --root . --json
```

Exit code `0` = success; non-zero = failure. Prefer `--json` in automation.

## Checklist

1. `apiVersion: kcell.dev/v1` and correct `kind`
2. `metadata.name` is a dns-label; `version` is semver
3. At least one of `provides` / `requires`
4. `communication.active` and/or `passive` enabled
5. Request **minimum** `permissions` (default deny)
6. Do not put secrets in the package
7. Do not change `crates/kcell-core` for a single Cell need

## Anti-patterns

- Inventing a custom wire protocol instead of declaring ports/capabilities
- Granting `*` network/process in manifests “for convenience”
- Mutating another Cell’s package at runtime
- Putting AutoConfig apply logic inside a Cell (Host applies proposals)

## Golden path

See [`cells/echo-cell`](../cells/echo-cell) + [`examples/echo-aiman`](../examples/echo-aiman).
