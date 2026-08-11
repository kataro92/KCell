//! Stem → Specialized Cell — overlay a stem template into a Cell package directory.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::manifest::{
    load_cell_from_path, validate_cell, Capability, CellManifest, Communication, Requirement,
    RuntimeKind, RuntimeSpec,
};
use crate::package::{build_cell_dir, CellPackageMeta};

/// Inputs for specializing a Stem template into a Cell package.
#[derive(Debug, Clone)]
pub struct SpecializeRequest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub runtime: RuntimeKind,
    pub entrypoint: Option<String>,
    pub artifact: Option<String>,
    pub provides: Vec<(String, String)>,
    pub requires: Vec<(String, String)>,
    /// When set (either flag used), replace stem communication.
    pub active: Option<bool>,
    pub passive: Option<bool>,
    pub stem_dir: PathBuf,
    pub out_dir: PathBuf,
    pub run_build: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecializeResult {
    pub cell_yaml: PathBuf,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<CellPackageMeta>,
}

/// Parse `name` or `name:version` (default version `"1"`).
pub fn parse_cap_token(token: &str) -> Result<(String, String)> {
    let token = token.trim();
    if token.is_empty() {
        return Err(Error::Validation("empty capability token".into()));
    }
    let (name, version) = match token.split_once(':') {
        Some((n, v)) => (n.trim(), v.trim()),
        None => (token, "1"),
    };
    if name.is_empty() || version.is_empty() {
        return Err(Error::Validation(format!(
            "invalid capability token `{token}` (expect name or name:version)"
        )));
    }
    if name.contains(':') {
        return Err(Error::Validation(format!(
            "invalid capability name in `{token}`"
        )));
    }
    Ok((name.into(), version.into()))
}

/// Load stem, overlay request fields, validate, write package (optional build).
pub fn specialize(req: SpecializeRequest) -> Result<SpecializeResult> {
    let stem_yaml = req.stem_dir.join("cell.yaml");
    if !stem_yaml.is_file() {
        return Err(Error::NotFound(format!(
            "stem cell.yaml not found at {}",
            stem_yaml.display()
        )));
    }

    let out_yaml = req.out_dir.join("cell.yaml");
    if out_yaml.exists() {
        return Err(Error::Validation(format!(
            "{} already exists",
            out_yaml.display()
        )));
    }

    let mut cell = load_cell_from_path(&stem_yaml)?;
    apply_overlay(&mut cell, &req)?;
    validate_cell(&cell)?;

    std::fs::create_dir_all(&req.out_dir)?;
    let yaml = serde_yaml::to_string(&cell)?;
    std::fs::write(&out_yaml, &yaml)?;

    let readme = req.out_dir.join("README.md");
    if !readme.exists() {
        let desc = cell
            .metadata
            .description
            .as_deref()
            .unwrap_or("Specialized Cell");
        std::fs::write(
            &readme,
            format!(
                "# {}\n\n{}\n\nSpecialized from Stem via `kcell specialize`. Edit `cell.yaml`, then `kcell validate cell.yaml`.\n",
                cell.metadata.name, desc
            ),
        )?;
    }

    let package = if req.run_build {
        Some(build_cell_dir(&req.out_dir)?.0)
    } else {
        None
    };

    Ok(SpecializeResult {
        cell_yaml: out_yaml,
        name: cell.metadata.name,
        version: cell.metadata.version,
        package,
    })
}

fn apply_overlay(cell: &mut CellManifest, req: &SpecializeRequest) -> Result<()> {
    cell.metadata.name = req.name.clone();
    cell.metadata.version = req.version.clone();
    if let Some(d) = &req.description {
        cell.metadata.description = Some(d.clone());
    } else {
        cell.metadata.description = Some(format!("Specialized from stem ({})", req.name));
    }

    let mut runtime = RuntimeSpec {
        kind: req.runtime,
        entrypoint: req
            .entrypoint
            .clone()
            .or_else(|| cell.spec.runtime.entrypoint.clone()),
        artifact: req.artifact.clone().or_else(|| cell.spec.runtime.artifact.clone()),
    };

    match req.runtime {
        RuntimeKind::Wasi => {
            if runtime.artifact.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                return Err(Error::Validation(
                    "wasi specialize requires --artifact (.wasm path)".into(),
                ));
            }
            if runtime.entrypoint.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                runtime.entrypoint = Some("_start".into());
            }
        }
        RuntimeKind::Subprocess => {
            if runtime.entrypoint.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                runtime.entrypoint = Some("kcell".into());
            }
            if runtime.artifact.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                runtime.artifact = Some("worker".into());
            }
        }
        RuntimeKind::Inprocess => {
            if runtime.entrypoint.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                runtime.entrypoint = Some("main".into());
            }
            // Clear stem artifact unless caller set one.
            if req.artifact.is_none() {
                runtime.artifact = None;
            }
        }
    }
    cell.spec.runtime = runtime;

    if !req.provides.is_empty() {
        cell.spec.provides = req
            .provides
            .iter()
            .map(|(n, v)| Capability {
                name: n.clone(),
                version: v.clone(),
                contract: None,
            })
            .collect();
    }

    if !req.requires.is_empty() {
        cell.spec.requires = req
            .requires
            .iter()
            .map(|(n, v)| Requirement {
                name: n.clone(),
                version: v.clone(),
                optional: false,
                contract: None,
            })
            .collect();
    }

    if req.active.is_some() || req.passive.is_some() {
        cell.spec.communication = Communication {
            active: req.active.unwrap_or(false),
            passive: req.passive.unwrap_or(false),
        };
    }

    Ok(())
}

/// Resolve default stem directory relative to `cwd` (typically `templates/stem-cell`).
pub fn default_stem_dir(cwd: impl AsRef<Path>) -> PathBuf {
    cwd.as_ref().join("templates/stem-cell")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_cap_tokens() {
        assert_eq!(parse_cap_token("echo").unwrap(), ("echo".into(), "1".into()));
        assert_eq!(
            parse_cap_token("echo:2").unwrap(),
            ("echo".into(), "2".into())
        );
        assert!(parse_cap_token("").is_err());
        assert!(parse_cap_token(":1").is_err());
    }

    fn stem_fixture(root: &Path) -> PathBuf {
        let stem = root.join("stem");
        fs::create_dir_all(&stem).unwrap();
        fs::write(
            stem.join("cell.yaml"),
            r#"
apiVersion: kcell.dev/v1
kind: Cell
metadata:
  name: stem-cell
  version: 0.1.0
spec:
  runtime:
    kind: inprocess
    entrypoint: main
  provides:
    - name: stem
      version: "1"
  requires: []
  communication:
    active: false
    passive: true
  permissions: {}
"#,
        )
        .unwrap();
        stem
    }

    #[test]
    fn specialize_overlay_and_refuse_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "kcell-specialize-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let stem = stem_fixture(&root);
        let out = root.join("my-echo");

        let res = specialize(SpecializeRequest {
            name: "my-echo".into(),
            version: "0.2.0".into(),
            description: Some("Echo cell".into()),
            runtime: RuntimeKind::Inprocess,
            entrypoint: Some("main".into()),
            artifact: None,
            provides: vec![("echo".into(), "1".into())],
            requires: vec![("llm".into(), "1".into())],
            active: Some(true),
            passive: Some(true),
            stem_dir: stem.clone(),
            out_dir: out.clone(),
            run_build: true,
        })
        .unwrap();

        assert_eq!(res.name, "my-echo");
        assert_eq!(res.version, "0.2.0");
        assert!(res.package.is_some());
        let cell = load_cell_from_path(&res.cell_yaml).unwrap();
        assert_eq!(cell.spec.provides[0].name, "echo");
        assert_eq!(cell.spec.requires[0].name, "llm");
        assert!(cell.spec.communication.active);
        assert!(cell.spec.communication.passive);

        let err = specialize(SpecializeRequest {
            name: "my-echo".into(),
            version: "0.2.0".into(),
            description: None,
            runtime: RuntimeKind::Inprocess,
            entrypoint: None,
            artifact: None,
            provides: vec![("echo".into(), "1".into())],
            requires: vec![],
            active: None,
            passive: None,
            stem_dir: stem,
            out_dir: out,
            run_build: false,
        })
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn wasi_requires_artifact() {
        let root = std::env::temp_dir().join(format!(
            "kcell-specialize-wasi-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let stem = stem_fixture(&root);
        let err = specialize(SpecializeRequest {
            name: "wasi-echo".into(),
            version: "0.1.0".into(),
            description: None,
            runtime: RuntimeKind::Wasi,
            entrypoint: None,
            artifact: None,
            provides: vec![("echo".into(), "1".into())],
            requires: vec![],
            active: None,
            passive: None,
            stem_dir: stem,
            out_dir: root.join("wasi-echo"),
            run_build: false,
        })
        .unwrap_err();
        assert!(err.to_string().contains("artifact"));
        let _ = fs::remove_dir_all(&root);
    }
}
