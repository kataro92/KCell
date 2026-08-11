//! Auto-bind — fill missing consumer requires from discovered providers.

use crate::binding::BindingSet;
use crate::discover::discover_providers;
use crate::manifest::{
    BindingProposal, ProposedBinding,
};
use crate::manifest::{ProposalMetadata, ProposalSpec};
use crate::registry::LocalRegistry;

/// Build a proposal that keeps existing bindings and adds missing required (and optional) edges.
pub fn propose_auto_bindings(
    registry: &LocalRegistry,
    current: &BindingSet,
    include_optional: bool,
) -> BindingProposal {
    let mut proposed: Vec<ProposedBinding> = current
        .bindings()
        .iter()
        .map(|b| ProposedBinding {
            consumer: b.consumer.clone(),
            provider: b.provider.clone(),
            capability: b.capability.clone(),
            required: b.required,
        })
        .collect();

    for rec in registry.iter() {
        if !rec.state().can_route() {
            continue;
        }
        let consumer = rec.manifest.metadata.name.as_str();
        for req in &rec.manifest.spec.requires {
            if req.optional && !include_optional {
                continue;
            }
            if proposed
                .iter()
                .any(|b| b.consumer == consumer && b.capability == req.name)
            {
                continue;
            }
            let mut providers = discover_providers(registry, Some(&req.name));
            providers.retain(|p| p.cell != consumer);
            // Prefer matching capability version when possible.
            providers.sort_by(|a, b| {
                let a_match = (a.capability_version == req.version) as u8;
                let b_match = (b.capability_version == req.version) as u8;
                b_match
                    .cmp(&a_match)
                    .then_with(|| a.cell.cmp(&b.cell))
            });
            if let Some(p) = providers.first() {
                proposed.push(ProposedBinding {
                    consumer: consumer.into(),
                    provider: p.cell.clone(),
                    capability: req.name.clone(),
                    required: !req.optional,
                });
            }
        }
    }

    BindingProposal {
        api_version: "kcell.dev/v1".into(),
        kind: "BindingProposal".into(),
        metadata: ProposalMetadata {
            proposer: "kcell:auto-bind".into(),
            generation: current.generation() + 1,
            reason: Some("auto-bind unmatched requires".into()),
        },
        spec: ProposalSpec {
            bindings: proposed,
            replace_generation: Some(current.generation()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Host;
    use crate::manifest::{
        Capability, CellManifest, CellMetadata, CellSpec, Communication, Permissions, Requirement,
        RestartPolicy, RuntimeKind, RuntimeSpec,
    };

    fn cell(name: &str, provides: &[&str], requires: &[&str]) -> CellManifest {
        CellManifest {
            api_version: "kcell.dev/v1".into(),
            kind: "Cell".into(),
            metadata: CellMetadata {
                name: name.into(),
                version: "0.1.0".into(),
                description: None,
            },
            spec: CellSpec {
                runtime: RuntimeSpec {
                    kind: RuntimeKind::Inprocess,
                    entrypoint: Some("main".into()),
                    artifact: None,
                },
                provides: provides
                    .iter()
                    .map(|n| Capability {
                        name: (*n).into(),
                        version: "1".into(),
                        contract: None,
                    })
                    .collect(),
                requires: requires
                    .iter()
                    .map(|n| Requirement {
                        name: (*n).into(),
                        version: "1".into(),
                        optional: false,
                        contract: None,
                    })
                    .collect(),
                communication: Communication {
                    active: true,
                    passive: true,
                },
                ports: vec![],
                resources: Default::default(),
                permissions: Permissions::default(),
                health: Default::default(),
                restart_policy: RestartPolicy::OnFailure,
            },
        }
    }

    #[test]
    fn fills_missing_require() {
        let mut host = Host::new();
        host.register_cell("1", cell("web", &[], &["llm"])).unwrap();
        host.register_cell("2", cell("brain", &["llm"], &[])).unwrap();
        host.activate_cell("web").unwrap();
        host.activate_cell("brain").unwrap();
        let proposal = propose_auto_bindings(host.registry(), host.bindings(), false);
        assert_eq!(proposal.spec.bindings.len(), 1);
        assert_eq!(proposal.spec.bindings[0].consumer, "web");
        assert_eq!(proposal.spec.bindings[0].provider, "brain");
        assert_eq!(proposal.spec.bindings[0].capability, "llm");
    }
}
