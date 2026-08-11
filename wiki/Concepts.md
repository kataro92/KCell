# Concepts

| Word | Meaning |
|------|---------|
| **Cell** | One unit with capabilities it provides or needs |
| **Stem Cell** | Template for `kcell specialize` / `kcell new` |
| **AI-man** | Cells + bindings + policy |
| **Host** | Loads, admits, binds, and invokes Cells |
| **Binding** | Maps a required capability to a provider Cell |
| **Policy** | Deny-by-default grants |

Active = call out. Passive = be called. Same Cell can do both.

Auto-config may **propose** bindings; only the Host applies them.
