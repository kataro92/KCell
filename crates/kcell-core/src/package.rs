//! Minimal Cell package build — content digest of `cell.yaml` (immutable revision id).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::manifest::{load_cell_from_path, CellManifest, RuntimeKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellPackageMeta {
    pub name: String,
    pub version: String,
    pub digest: String,
    pub runtime: RuntimeKind,
    pub source: String,
}

/// Build package metadata from a Cell directory containing `cell.yaml`.
/// Writes `.kcell/package.json` next to the manifest.
pub fn build_cell_dir(cell_dir: impl AsRef<Path>) -> Result<(CellPackageMeta, PathBuf)> {
    let cell_dir = cell_dir.as_ref();
    let yaml_path = cell_dir.join("cell.yaml");
    let bytes = std::fs::read(&yaml_path)?;
    let manifest = load_cell_from_path(&yaml_path)?;
    let digest = sha256_hex(&bytes);
    let meta = CellPackageMeta {
        name: manifest.metadata.name.clone(),
        version: manifest.metadata.version.clone(),
        digest: format!("sha256:{digest}"),
        runtime: manifest.spec.runtime.kind,
        source: yaml_path.display().to_string(),
    };
    let out_dir = cell_dir.join(".kcell");
    std::fs::create_dir_all(&out_dir)?;
    let out = out_dir.join("package.json");
    std::fs::write(&out, serde_json::to_vec_pretty(&meta)?)?;
    let _ = manifest;
    Ok((meta, out))
}

pub fn package_from_manifest(manifest: &CellManifest, yaml_bytes: &[u8], source: &str) -> CellPackageMeta {
    CellPackageMeta {
        name: manifest.metadata.name.clone(),
        version: manifest.metadata.version.clone(),
        digest: format!("sha256:{}", sha256_hex(yaml_bytes)),
        runtime: manifest.spec.runtime.kind,
        source: source.into(),
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_stable() {
        assert_eq!(sha256_hex(b"abc"), sha256_hex(b"abc"));
        assert_ne!(sha256_hex(b"abc"), sha256_hex(b"abd"));
    }
}
