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
    pub granted_filesystem: Vec<String>,
}

impl Default for PolicyGate {
    fn default() -> Self {
        Self {
            default_allow: false,
            granted_network: Vec::new(),
            granted_process: Vec::new(),
            granted_secrets: Vec::new(),
            granted_peers: Vec::new(),
            granted_filesystem: Vec::new(),
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
        for fs in &cell.spec.permissions.filesystem {
            let key = format!("{}:{:?}", fs.path, fs.mode).to_ascii_lowercase();
            let granted = self.granted_filesystem.iter().any(|g| {
                g == "*"
                    || g.eq_ignore_ascii_case(&format!("{}:{:?}", fs.path, fs.mode))
                    || g == &fs.path
            });
            if !granted {
                denied.push(format!("filesystem:{key}"));
            }
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
        push_unique(&mut self.granted_network, target.into());
    }

    pub fn grant_process(&mut self, target: impl Into<String>) {
        push_unique(&mut self.granted_process, target.into());
    }

    pub fn grant_secret(&mut self, target: impl Into<String>) {
        push_unique(&mut self.granted_secrets, target.into());
    }

    pub fn grant_peer(&mut self, target: impl Into<String>) {
        push_unique(&mut self.granted_peers, target.into());
    }

    pub fn grant_filesystem(&mut self, target: impl Into<String>) {
        push_unique(&mut self.granted_filesystem, target.into());
    }

    pub fn revoke_network(&mut self, target: &str) {
        self.granted_network.retain(|g| g != target);
    }

    pub fn revoke_process(&mut self, target: &str) {
        self.granted_process.retain(|g| g != target);
    }

    pub fn revoke_secret(&mut self, target: &str) {
        self.granted_secrets.retain(|g| g != target);
    }

    pub fn revoke_peer(&mut self, target: &str) {
        self.granted_peers.retain(|g| g != target);
    }

    pub fn revoke_filesystem(&mut self, target: &str) {
        self.granted_filesystem.retain(|g| g != target);
    }
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.iter().any(|v| v == &value) {
        list.push(value);
    }
}
