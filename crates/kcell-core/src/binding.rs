use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::manifest::{BindingProposal, ProposedBinding};
use crate::registry::LocalRegistry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveBinding {
    pub consumer: String,
    pub provider: String,
    pub capability: String,
    pub required: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BindingSet {
    generation: u64,
    bindings: Vec<ActiveBinding>,
}

impl BindingSet {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn bindings(&self) -> &[ActiveBinding] {
        &self.bindings
    }

    pub fn provider_for(&self, consumer: &str, capability: &str) -> Option<&ActiveBinding> {
        self.bindings
            .iter()
            .find(|b| b.consumer == consumer && b.capability == capability)
    }

    /// Drop bindings where `name` is consumer or provider. Bumps generation if anything removed.
    /// Returns `true` if the set changed.
    pub fn retain_without_cell(&mut self, name: &str) -> bool {
        let before = self.bindings.len();
        self.bindings
            .retain(|b| b.consumer != name && b.provider != name);
        if self.bindings.len() != before {
            self.generation = self.generation.saturating_add(1).max(1);
            true
        } else {
            false
        }
    }

    /// Replace bindings wholesale (used by durable state restore helpers).
    pub fn replace(&mut self, generation: u64, bindings: Vec<ActiveBinding>) {
        self.generation = generation;
        self.bindings = bindings;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BindingApplyResult {
    Applied { generation: u64 },
    Rejected { reason: String },
}

/// Validate a proposal against the live registry (capability match + health).
pub fn validate_proposal(registry: &LocalRegistry, proposal: &BindingProposal) -> Result<()> {
    crate::manifest::validate_binding_proposal(proposal)?;
    for b in &proposal.spec.bindings {
        check_pair(registry, b)?;
    }
    Ok(())
}

fn check_pair(registry: &LocalRegistry, b: &ProposedBinding) -> Result<()> {
    let consumer = registry
        .by_name(&b.consumer)
        .ok_or_else(|| Error::Binding(format!("unknown consumer `{}`", b.consumer)))?;
    let provider = registry
        .by_name(&b.provider)
        .ok_or_else(|| Error::Binding(format!("unknown provider `{}`", b.provider)))?;

    if !provider.state().can_route() {
        return Err(Error::Binding(format!(
            "provider `{}` not routable ({:?})",
            b.provider,
            provider.state()
        )));
    }

    let provides = provider
        .manifest
        .spec
        .provides
        .iter()
        .any(|p| p.name == b.capability);
    if !provides {
        return Err(Error::Binding(format!(
            "provider `{}` does not provide `{}`",
            b.provider, b.capability
        )));
    }

    let requires = consumer
        .manifest
        .spec
        .requires
        .iter()
        .any(|r| r.name == b.capability)
        || consumer
            .manifest
            .spec
            .ports
            .iter()
            .any(|p| p.name == b.capability);
    // Soft check: consumer should declare the capability as a requirement when present.
    if !requires && !consumer.manifest.spec.requires.is_empty() {
        // If the consumer lists requirements, capability must be among them (or optional peers).
        let listed = consumer
            .manifest
            .spec
            .requires
            .iter()
            .any(|r| r.name == b.capability);
        if !listed {
            return Err(Error::Binding(format!(
                "consumer `{}` does not require `{}`",
                b.consumer, b.capability
            )));
        }
    }

    Ok(())
}

/// Atomic swap of binding generation after validation.
pub fn apply_proposal(
    registry: &LocalRegistry,
    current: &BindingSet,
    proposal: BindingProposal,
) -> Result<(BindingSet, BindingApplyResult)> {
    if let Some(expected) = proposal.spec.replace_generation {
        if expected != current.generation {
            return Ok((
                current.clone(),
                BindingApplyResult::Rejected {
                    reason: format!(
                        "replaceGeneration {expected} != current {}",
                        current.generation
                    ),
                },
            ));
        }
    }
    if proposal.metadata.generation <= current.generation {
        return Ok((
            current.clone(),
            BindingApplyResult::Rejected {
                reason: format!(
                    "proposal generation {} not greater than current {}",
                    proposal.metadata.generation, current.generation
                ),
            },
        ));
    }

    validate_proposal(registry, &proposal)?;

    let next = BindingSet {
        generation: proposal.metadata.generation,
        bindings: proposal
            .spec
            .bindings
            .into_iter()
            .map(|b| ActiveBinding {
                consumer: b.consumer,
                provider: b.provider,
                capability: b.capability,
                required: b.required,
            })
            .collect(),
    };
    let gen = next.generation;
    Ok((next, BindingApplyResult::Applied { generation: gen }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::CellState;
    use crate::manifest::{
        Capability, CellManifest, CellMetadata, CellSpec, Communication, Permissions, Requirement,
        RuntimeKind, RuntimeSpec,
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
                restart_policy: crate::manifest::RestartPolicy::OnFailure,
            },
        }
    }

    #[test]
    fn apply_binding_when_ready() {
        let mut reg = LocalRegistry::new();
        reg.register("1", cell("web", &[], &["llm"])).unwrap();
        reg.register("2", cell("brain", &["llm"], &[])).unwrap();
        for name in ["web", "brain"] {
            let r = reg.by_name_mut(name).unwrap();
            r.lifecycle.activate().unwrap();
            assert_eq!(r.state(), CellState::Active);
        }

        let proposal: BindingProposal = serde_yaml::from_str(
            r#"
apiVersion: kcell.dev/v1
kind: BindingProposal
metadata:
  proposer: auto-config
  generation: 1
spec:
  bindings:
    - consumer: web
      provider: brain
      capability: llm
"#,
        )
        .unwrap();

        let (set, result) = apply_proposal(&reg, &BindingSet::default(), proposal).unwrap();
        assert!(matches!(
            result,
            BindingApplyResult::Applied { generation: 1 }
        ));
        assert_eq!(set.bindings().len(), 1);
    }
}
