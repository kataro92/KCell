# Instructions for AI coding agents

KCell is a **small core** plus optional adapters and Cells. Prefer the smallest change that preserves contracts.

## Mental model

| Term | Meaning |
|------|---------|
| Stem Cell | Factory / runtime contract used to specialize a Cell |
| Specialized Cell | Immutable package (version + digest); change ⇒ new revision |
| Active / Passive | Outbound vs inbound communication *capabilities* on the same Cell |
| AI-man | Composition of Cells + bindings + policy |
| Host | Authority for lifecycle, admission, sandbox, and binding apply |

Auto-config Cells may **propose** bindings; only the Host validates and applies them.

## Where to put code

- **Core** (`core/`): only if every Cell needs it (manifest, lifecycle, registry, binding, policy, execution interfaces)
- **Adapters** (`adapters/`): AG-UI, A2A, MCP, transports, observability backends
- **Cells** (`cells/`): domain logic (brains, UIs, tools, auto-config)
- **Do not** expand core to fix a single Cell’s needs

## Workflow for a new Cell

1. Scaffold from Stem Cell template / CLI (`new`)
2. Declare capabilities and ports in `cell.yaml`
3. Implement behind those contracts
4. `validate` → `test` → `build` → `inspect`
5. Request **minimum** permissions; treat tool descriptions as untrusted

Use non-interactive CLI with stable exit codes when available. Prefer machine-readable schema over prose.

## Hard rules

- Package artifacts are immutable; secrets stay out of packages
- Deny-by-default: no ambient network/fs/process without grant
- Keep hot paths allocation-bounded; add benches when touching core hot path
- No new public API or dependency in core without a clear, universal need
- Do not invent a replacement for AG-UI / A2A / MCP at the boundary

## Docs

- Spec and MVP scope: `docs/`
- Research notes (non-normative): `nghien_cuu_kien_truc_agent.md`
