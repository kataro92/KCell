use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIManManifest {
    pub api_version: String,
    pub kind: String,
    pub metadata: AIManMetadata,
    pub spec: AIManSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIManMetadata {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIManSpec {
    pub cells: Vec<CellRef>,
    #[serde(default)]
    pub bindings: Vec<StaticBinding>,
    #[serde(default)]
    pub policy: AIManPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRef {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticBinding {
    pub consumer: String,
    pub provider: String,
    pub capability: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIManPolicy {
    #[serde(default)]
    pub default_allow: bool,
    #[serde(default = "default_failure")]
    pub failure_strategy: FailureStrategy,
}

impl Default for AIManPolicy {
    fn default() -> Self {
        Self {
            default_allow: false,
            failure_strategy: FailureStrategy::ContinueOptional,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureStrategy {
    FailFast,
    ContinueOptional,
}

fn default_failure() -> FailureStrategy {
    FailureStrategy::ContinueOptional
}

pub fn load_aiman_from_path(path: impl AsRef<std::path::Path>) -> Result<AIManManifest> {
    let text = std::fs::read_to_string(path)?;
    let man: AIManManifest = serde_yaml::from_str(&text)?;
    validate_aiman(&man)?;
    Ok(man)
}

pub fn validate_aiman(man: &AIManManifest) -> Result<()> {
    if man.api_version != "kcell.dev/v1" {
        return Err(Error::Validation(format!(
            "unsupported apiVersion: {}",
            man.api_version
        )));
    }
    if man.kind != "AIMan" {
        return Err(Error::Validation(format!("expected kind AIMan, got {}", man.kind)));
    }
    if man.spec.cells.is_empty() {
        return Err(Error::Validation("ai-man must list at least one cell".into()));
    }
    let mut names = std::collections::BTreeSet::new();
    for cell in &man.spec.cells {
        if !names.insert(cell.name.as_str()) {
            return Err(Error::Validation(format!("duplicate cell name `{}`", cell.name)));
        }
        if cell.path.is_empty() {
            return Err(Error::Validation(format!("cell `{}` path is empty", cell.name)));
        }
    }
    for b in &man.spec.bindings {
        if !names.contains(b.consumer.as_str()) {
            return Err(Error::Validation(format!(
                "binding consumer `{}` not in cells",
                b.consumer
            )));
        }
        if !names.contains(b.provider.as_str()) {
            return Err(Error::Validation(format!(
                "binding provider `{}` not in cells",
                b.provider
            )));
        }
        if b.capability.is_empty() {
            return Err(Error::Validation("binding capability must be non-empty".into()));
        }
    }
    for cell in &man.spec.cells {
        for dep in &cell.depends_on {
            if !names.contains(dep.as_str()) {
                return Err(Error::Validation(format!(
                    "cell `{}` depends on unknown `{}`",
                    cell.name, dep
                )));
            }
        }
    }
    Ok(())
}
