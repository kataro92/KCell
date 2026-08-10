use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::manifest::CellManifest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyGate {
    /// When false (default), only explicitly granted permissions are allowed.
    pub default_allow: bool,
    pub granted_network: Vec<String>,
    pub granted_process: Vec<String>,
    pub granted_secrets: Vec<String>,
    pub granted_peers: Vec<String>,
}

impl Default for PolicyGate {
    fn default() -> Self {
        Self {
            default_allow: false,
            granted_network: Vec::new(),
            granted_process: Vec::new(),
            granted_secrets: Vec::new(),
            granted_peers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionDecision {
    pub admitted: bool,
    pub reasons: Vec<String>,
}

impl PolicyGate {
    pub fn admit(&self, cell: &CellManifest) -> Result<AdmissionDecision> {
        if self.default_allow {
            return Ok(AdmissionDecision {
                admitted: true,
                reasons: vec!["defaultAllow=true".into()],
            });
        }

        let mut denied = Vec::new();
        for n in &cell.spec.permissions.network {
            if !self.granted_network.iter().any(|g| g == n || g == "*") {
                denied.push(format!("network:{n}"));
            }
        }
        for p in &cell.spec.permissions.process {
            if !self.granted_process.iter().any(|g| g == p || g == "*") {
                denied.push(format!("process:{p}"));
            }
        }
        for s in &cell.spec.permissions.secrets {
            if !self.granted_secrets.iter().any(|g| g == s || g == "*") {
                denied.push(format!("secret:{s}"));
            }
        }
        for peer in &cell.spec.permissions.peers {
            if !self.granted_peers.iter().any(|g| g == peer || g == "*") {
                denied.push(format!("peer:{peer}"));
            }
        }
        // Filesystem grants must be reviewed; deny any path unless defaultAllow.
        for fs in &cell.spec.permissions.filesystem {
            denied.push(format!("filesystem:{}:{:?}", fs.path, fs.mode));
        }

        if denied.is_empty() {
            Ok(AdmissionDecision {
                admitted: true,
                reasons: vec!["all requested permissions empty or granted".into()],
            })
        } else {
            Err(Error::PolicyDenied(format!(
                "cell `{}` needs approval for: {}",
                cell.metadata.name,
                denied.join(", ")
            )))
        }
    }

    pub fn grant_network(&mut self, target: impl Into<String>) {
        self.granted_network.push(target.into());
    }

    pub fn grant_process(&mut self, target: impl Into<String>) {
        self.granted_process.push(target.into());
    }
}
