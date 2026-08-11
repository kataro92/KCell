use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellManifest {
    pub api_version: String,
    pub kind: String,
    pub metadata: CellMetadata,
    pub spec: CellSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellMetadata {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellSpec {
    pub runtime: RuntimeSpec,
    #[serde(default)]
    pub provides: Vec<Capability>,
    #[serde(default)]
    pub requires: Vec<Requirement>,
    pub communication: Communication,
    #[serde(default)]
    pub ports: Vec<Port>,
    #[serde(default)]
    pub resources: Resources,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub health: Health,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: RestartPolicy,
}

fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSpec {
    pub kind: RuntimeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    Wasi,
    Subprocess,
    Inprocess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Communication {
    pub active: bool,
    pub passive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    pub name: String,
    pub direction: PortDirection,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortDirection {
    In,
    Out,
    Inout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Resources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_millis: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub filesystem: Vec<FsGrant>,
    #[serde(default)]
    pub process: Vec<String>,
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(default)]
    pub peers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGrant {
    pub path: String,
    pub mode: FsMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FsMode {
    Ro,
    Rw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    #[serde(default = "default_readiness")]
    pub readiness_timeout_ms: u64,
    #[serde(default = "default_liveness")]
    pub liveness_interval_ms: u64,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            readiness_timeout_ms: default_readiness(),
            liveness_interval_ms: default_liveness(),
        }
    }
}

fn default_readiness() -> u64 {
    5_000
}

fn default_liveness() -> u64 {
    5_000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

pub fn load_cell_from_path(path: impl AsRef<std::path::Path>) -> Result<CellManifest> {
    let text = std::fs::read_to_string(path)?;
    let cell: CellManifest = serde_yaml::from_str(&text)?;
    validate_cell(&cell)?;
    Ok(cell)
}

pub fn validate_cell(cell: &CellManifest) -> Result<()> {
    if cell.api_version != "kcell.dev/v1" {
        return Err(Error::Validation(format!(
            "unsupported apiVersion: {}",
            cell.api_version
        )));
    }
    if cell.kind != "Cell" {
        return Err(Error::Validation(format!(
            "expected kind Cell, got {}",
            cell.kind
        )));
    }
    validate_name(&cell.metadata.name)?;
    validate_semver(&cell.metadata.version)?;
    if !cell.spec.communication.active && !cell.spec.communication.passive {
        return Err(Error::Validation(
            "communication must enable active and/or passive".into(),
        ));
    }
    if cell.spec.provides.is_empty() && cell.spec.requires.is_empty() {
        return Err(Error::Validation(
            "cell must provide or require at least one capability".into(),
        ));
    }
    for cap in &cell.spec.provides {
        if cap.name.is_empty() || cap.version.is_empty() {
            return Err(Error::Validation(
                "provide capability name/version required".into(),
            ));
        }
    }
    for req in &cell.spec.requires {
        if req.name.is_empty() || req.version.is_empty() {
            return Err(Error::Validation(
                "require capability name/version required".into(),
            ));
        }
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    let ok = name.len() <= 63
        && name.chars().enumerate().all(|(i, c)| match (i, c) {
            (0, c) => c.is_ascii_lowercase(),
            (_, c) => c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-',
        });
    if ok {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "invalid name `{name}` (expect lowercase dns-label)"
        )))
    }
}

fn validate_semver(v: &str) -> Result<()> {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let parts: Vec<_> = core.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
        return Err(Error::Validation(format!("invalid semver `{v}`")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_yaml() -> &'static str {
        r#"
apiVersion: kcell.dev/v1
kind: Cell
metadata:
  name: echo-cell
  version: 0.1.0
spec:
  runtime:
    kind: inprocess
    entrypoint: echo
  provides:
    - name: echo
      version: "1"
  requires: []
  communication:
    active: false
    passive: true
  permissions: {}
"#
    }

    #[test]
    fn parses_and_validates_cell() {
        let cell: CellManifest = serde_yaml::from_str(sample_yaml()).unwrap();
        validate_cell(&cell).unwrap();
        assert_eq!(cell.metadata.name, "echo-cell");
    }

    #[test]
    fn compat_fixture_v1_min_cell_loads() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/compat/cell.v1.min.yaml");
        let cell = load_cell_from_path(&path).expect("compat v1 min cell must load");
        assert_eq!(cell.api_version, "kcell.dev/v1");
        assert_eq!(cell.metadata.name, "compat-min-cell");
    }
}
