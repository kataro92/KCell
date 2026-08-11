//! Capability discovery views over the local registry.

use serde::{Deserialize, Serialize};

use crate::lifecycle::CellState;
use crate::manifest::RuntimeKind;
use crate::registry::LocalRegistry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub cell: String,
    pub cell_version: String,
    pub capability: String,
    pub capability_version: String,
    pub runtime: RuntimeKind,
    pub state: CellState,
}

/// List routable providers, optionally filtered by capability name.
pub fn discover_providers(registry: &LocalRegistry, capability: Option<&str>) -> Vec<ProviderInfo> {
    let mut out = Vec::new();
    for rec in registry.iter() {
        if !rec.state().can_route() {
            continue;
        }
        for cap in &rec.manifest.spec.provides {
            if let Some(want) = capability {
                if cap.name != want {
                    continue;
                }
            }
            out.push(ProviderInfo {
                cell: rec.manifest.metadata.name.clone(),
                cell_version: rec.manifest.metadata.version.clone(),
                capability: cap.name.clone(),
                capability_version: cap.version.clone(),
                runtime: rec.manifest.spec.runtime.kind,
                state: rec.state(),
            });
        }
    }
    out.sort_by(|a, b| (&a.capability, &a.cell).cmp(&(&b.capability, &b.cell)));
    out
}
