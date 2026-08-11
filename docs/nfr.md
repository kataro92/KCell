# Non-functional requirements (NFR)

Hard rules so the Host stays small and usable as a shared kernel. Details for upgrades: [compatibility.md](compatibility.md).

## NFR-1 — Extreme code compactness

| ID | Requirement |
|----|-------------|
| NFR-1.1 | `kcell_core` MUST contain only universal mechanisms: manifest, lifecycle, registry, binding, policy, execution/transport **interfaces**. |
| NFR-1.2 | Product logic (models, UI, routing policy, vendor SDKs) MUST NOT enter core; it lives in Cells, adapters, or child repos. |
| NFR-1.3 | Prefer fewer types, fewer modules, and fewer public APIs over “complete” frameworks. |
| NFR-1.4 | Delete dead code and unused abstractions in the same change that makes them unused. |
| NFR-1.5 | New core dependency requires justification against this document and an update to [`benches/BUDGET.md`](../benches/BUDGET.md). |

**Test:** If a feature is useful to only one Cell or one child repo, it does not belong in KCell core.

## NFR-2 — Extreme performance

| ID | Requirement |
|----|-------------|
| NFR-2.1 | Default Host path MUST be allocation-light on hot paths (register, transition, bind, admit, lookup). |
| NFR-2.2 | Default path MUST NOT start brokers, databases, telemetry collectors, or protocol servers unless explicitly enabled. |
| NFR-2.3 | Cold start and manifest validation MUST stay within budgets in [`benches/BUDGET.md`](../benches/BUDGET.md). |
| NFR-2.4 | Measure before “optimizing”; budget changes need recorded benchmarks, not intuition. |
| NFR-2.5 | Zero-cost abstractions preferred; avoid reflection, runtime codegen, and heavyweight async runtimes in core unless proven necessary. |

## NFR-3 — Extreme structural simplicity

| ID | Requirement |
|----|-------------|
| NFR-3.1 | Directory and crate layout MUST stay shallow: core, cli, schemas, adapters, cells, examples, docs. |
| NFR-3.2 | One concept → one place. Do not duplicate lifecycle/policy/binding logic across crates. |
| NFR-3.3 | Schemas (`schemas/*.json`) are the contract source of truth; prose docs MUST NOT invent parallel fields. |
| NFR-3.4 | CLI stays a thin operator over core; no business orchestration hidden in CLI. |
| NFR-3.5 | Avoid pluggable plugin frameworks inside core; extension happens *outside* via stable interfaces and manifests. |

**Mental model for contributors:** mechanisms, not policies.

## NFR-4 — Extreme extensibility

| ID | Requirement |
|----|-------------|
| NFR-4.1 | New Cell kinds MUST be addable without modifying `kcell_core` (manifest + package + optional adapter). |
| NFR-4.2 | New protocols (AG-UI / A2A / MCP / custom) MUST plug in as adapters, not core rewrites. |
| NFR-4.3 | Execution backends (WASI, subprocess, later OCI) MUST implement a narrow execution interface. |
| NFR-4.4 | Binding and capability discovery MUST allow runtime growth (add Cell → appear in registry → new binding generation). |
| NFR-4.5 | Public contracts (YAML schemas, Host APIs, CLI `--json`) MUST remain stable and versioned (`kcell.dev/v1`, …). |

## NFR-5 — Kernel for child repositories

KCell is designed as the **shared core** that other repos consume:

| Child repo role (examples) | Depends on KCell for | Owns itself |
|----------------------------|----------------------|-------------|
| Specialized Cell repos (brain, web UI, tools) | `cell.yaml` contract, Host lifecycle/admission | Cell implementation, vendor SDKs |
| AI-man / product repos | `ai-man.yaml`, activate/run semantics | Composition, UX, deployment |
| Adapter repos | Host interfaces + envelopes | Protocol codecs, transports |
| Platform / ops repos | CLI, schemas, policy gate | CI, packaging, signing |

| ID | Requirement |
|----|-------------|
| NFR-5.1 | Child repos MUST be able to depend on released `kcell_core` / schemas / CLI without vendoring Host internals. |
| NFR-5.2 | Breaking changes to schemas or Host public API REQUIRE a version bump and migration note. |
| NFR-5.3 | Core MUST remain language-agnostic at the **contract** layer (YAML/JSON); Rust is the first Host implementation, not the only possible Cell language. |
| NFR-5.4 | Documentation for agents (`AGENTS.md`, authoring guide) MUST stay accurate so child repos can be authored by AI agents against this kernel. |
| NFR-5.5 | Features that only serve one child product MUST land in that child repo, never as core special cases. |

## NFR-6 — Backward compatibility

Downstream Cells authored against Host major **N−1** MUST keep running on Host major **N**. Full rules: [`compatibility.md`](compatibility.md).

| ID | Requirement |
|----|-------------|
| NFR-6.1 | Host major N MUST admit/activate/bind/invoke Cells whose manifests and envelopes were valid on Host major N−1 (same operator grants). |
| NFR-6.2 | Within a contract id (`kcell.dev/v1`, `kcell.envelope.v1`, …), changes MUST be additive only (optional fields + defaults; no rename/remove/require-new or semantic change). |
| NFR-6.3 | Breaking wire/schema changes MUST introduce a new contract id and dual-support the prior id for the prior-major window. |
| NFR-6.4 | Existing `schemas/*.vN.json` MUST NOT gain new `required` fields or remove/rename properties; breaking shape ⇒ new schema file. |
| NFR-6.5 | Compat fixtures under `tests/fixtures/compat/` MUST continue to load/validate on every Host change in this repository. |

**Test:** A minimal `apiVersion: kcell.dev/v1` Cell from the prior major still validates and runs on the current Host without rewriting the Cell package.

## Acceptance summary

A change is **rejected** if it:

- grows core for a single consumer,
- adds default runtime weight (network/DB/telemetry) without opt-in,
- complicates layout without removing an equivalent concept elsewhere,
- forces child repos to patch core to extend behavior,
- breaks schema/CLI contracts without a versioned migration,
- or drops prior-major contract support without dual-read for the required window ([`compatibility.md`](compatibility.md)).
