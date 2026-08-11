//! Durable Host state — cell dirs + bindings + policy grants (JSON under `.kcell/`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::binding::ActiveBinding;
use crate::error::{Error, Result};

pub const HOST_STATE_SCHEMA: &str = "kcell.host-state.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostStateFile {
    pub schema: String,
    #[serde(default)]
    pub cell_dirs: BTreeMap<String, String>,
    #[serde(default)]
    pub bindings: HostStateBindings,
    #[serde(default)]
    pub policy: HostStatePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostStateBindings {
    pub generation: u64,
    #[serde(default)]
    pub items: Vec<ActiveBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostStatePolicy {
    #[serde(default)]
    pub default_allow: bool,
    #[serde(default)]
    pub granted_network: Vec<String>,
    #[serde(default)]
    pub granted_process: Vec<String>,
    #[serde(default)]
    pub granted_filesystem: Vec<String>,
    #[serde(default)]
    pub granted_secrets: Vec<String>,
    #[serde(default)]
    pub granted_peers: Vec<String>,
}

pub fn default_state_path(socket: &Path) -> PathBuf {
    match socket.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join("host-state.json"),
        _ => PathBuf::from(".kcell/host-state.json"),
    }
}

pub fn load_host_state(path: impl AsRef<Path>) -> Result<HostStateFile> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("read host state {}: {e}", path.display()),
        ))
    })?;
    let state: HostStateFile = serde_json::from_str(&text)?;
    if state.schema != HOST_STATE_SCHEMA {
        return Err(Error::Validation(format!(
            "unsupported host state schema `{}`",
            state.schema
        )));
    }
    Ok(state)
}

pub fn save_host_state(path: impl AsRef<Path>, state: &HostStateFile) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(state)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_beside_socket() {
        let p = default_state_path(Path::new(".kcell/kcell.sock"));
        assert_eq!(p, PathBuf::from(".kcell/host-state.json"));
    }

    #[test]
    fn old_state_without_policy_loads() {
        let json = r#"{
            "schema": "kcell.host-state.v1",
            "cellDirs": {},
            "bindings": { "generation": 0, "items": [] }
        }"#;
        let state: HostStateFile = serde_json::from_str(json).unwrap();
        assert!(!state.policy.default_allow);
        assert!(state.policy.granted_process.is_empty());
    }

    #[test]
    fn compat_fixture_host_state_v1_no_policy_loads() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/compat/host-state.v1.no-policy.json");
        let state = load_host_state(&path).expect("compat host-state without policy must load");
        assert_eq!(state.schema, HOST_STATE_SCHEMA);
        assert!(!state.policy.default_allow);
        assert!(state.policy.granted_network.is_empty());
    }
}
