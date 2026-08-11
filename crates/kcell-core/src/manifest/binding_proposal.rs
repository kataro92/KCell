use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingProposal {
    pub api_version: String,
    pub kind: String,
    pub metadata: ProposalMetadata,
    pub spec: ProposalSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalMetadata {
    pub proposer: String,
    pub generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalSpec {
    pub bindings: Vec<ProposedBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace_generation: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedBinding {
    pub consumer: String,
    pub provider: String,
    pub capability: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

pub fn load_binding_proposal_from_path(
    path: impl AsRef<std::path::Path>,
) -> Result<BindingProposal> {
    let text = std::fs::read_to_string(path)?;
    let p: BindingProposal = serde_yaml::from_str(&text)?;
    validate_binding_proposal(&p)?;
    Ok(p)
}

pub fn validate_binding_proposal(p: &BindingProposal) -> Result<()> {
    if p.api_version != "kcell.dev/v1" {
        return Err(Error::Validation(format!(
            "unsupported apiVersion: {}",
            p.api_version
        )));
    }
    if p.kind != "BindingProposal" {
        return Err(Error::Validation(format!(
            "expected kind BindingProposal, got {}",
            p.kind
        )));
    }
    if p.metadata.proposer.is_empty() {
        return Err(Error::Validation("proposer required".into()));
    }
    if p.metadata.generation == 0 {
        return Err(Error::Validation("generation must be >= 1".into()));
    }
    if p.spec.bindings.is_empty() {
        return Err(Error::Validation("proposal must include bindings".into()));
    }
    for b in &p.spec.bindings {
        if b.consumer.is_empty() || b.provider.is_empty() || b.capability.is_empty() {
            return Err(Error::Validation(
                "binding consumer/provider/capability required".into(),
            ));
        }
    }
    Ok(())
}
