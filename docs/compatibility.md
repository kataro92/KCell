# Compatibility

How to keep old Cells working when the Host upgrades. See also [nfr.md](nfr.md) (NFR-6).

## Meanings

| Term | Meaning |
|------|---------|
| **Host major** | SemVer major of `kcell_core` / `kcell` (e.g. 1.x → 2.x) |
| **Contract id** | Wire id such as `kcell.dev/v1`, `kcell.envelope.v1` |
| **Prior-major Cell** | Valid on Host major N−1 |

**Rule:** Host major **N** must still run Cells that were valid on major **N−1** (same grants; same documented envelope).

## Surfaces

| Surface | Id today | Breaking change |
|---------|----------|-----------------|
| Manifests | `kcell.dev/v1` | New `apiVersion` + dual-read |
| Envelope | `kcell.envelope.v1` | New schema + dual-accept |
| Control socket | `kcell.control.v1` | New schema + dual-accept |
| Host state | `kcell.host-state.v1` | New schema + dual-load |
| JSON Schema files | `schemas/*.v1.json` | New `*.v2.json` file |
| Rust API | crate SemVer | Major bump (does not excuse wire breaks) |

## Hard rules

1. Within one contract id: **additive only** (optional fields + defaults). No rename/remove/require-new or meaning change.  
2. Break ⇒ **new id**; Host dual-supports old + new for the prior-major window.  
3. Prefer `#[serde(default)]` so old files load.  
4. Fixtures in `tests/fixtures/compat/` must keep loading.

## Not covered

Undocumented bugs, operator grants, native ABI outside the envelope, or new ids on an **old** Host.

## Checklist before merge

- [ ] Which surface / contract id?  
- [ ] Additive in place, or new id + dual-support?  
- [ ] No new `required` / removed fields on existing schema files  
- [ ] Compat fixtures still pass  
- [ ] Docs updated if authors must change behavior  
