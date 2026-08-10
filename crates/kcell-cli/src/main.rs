use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use kcell_core::{
    load_aiman_from_path, load_binding_proposal_from_path, load_cell_from_path, Host,
};

#[derive(Parser, Debug)]
#[command(name = "kcell", version, about = "KCell — compose AI cells into an AI-man")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Validate a cell.yaml, ai-man.yaml, or binding proposal
    Validate {
        /// Path to YAML manifest
        path: PathBuf,
        /// Emit machine-readable JSON on success
        #[arg(long)]
        json: bool,
    },
    /// Inspect a Cell or AI-man manifest (JSON)
    Inspect {
        path: PathBuf,
    },
    /// Scaffold a minimal Cell package directory
    New {
        /// Cell name (dns-label)
        name: String,
        /// Output directory (default: ./<name>)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
    /// Load cells from an AI-man and activate them in-process (no executors yet)
    Run {
        /// Path to ai-man.yaml
        path: PathBuf,
        /// Root directory for relative cell paths (default: current working directory)
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Validate { path, json } => cmd_validate(&path, json)?,
        Commands::Inspect { path } => cmd_inspect(&path)?,
        Commands::New { name, dir } => cmd_new(&name, dir)?,
        Commands::Run { path, root, json } => cmd_run(&path, root, json)?,
    }
    Ok(())
}

fn detect_kind(path: &Path) -> Result<&'static str, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let v: serde_yaml::Value = serde_yaml::from_str(&text)?;
    let kind = v
        .get("kind")
        .and_then(|k| k.as_str())
        .ok_or("manifest missing kind")?;
    Ok(match kind {
        "Cell" => "Cell",
        "AIMan" => "AIMan",
        "BindingProposal" => "BindingProposal",
        other => return Err(format!("unknown kind `{other}`").into()),
    })
}

fn cmd_validate(path: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let kind = detect_kind(path)?;
    match kind {
        "Cell" => {
            let c = load_cell_from_path(path)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "kind": "Cell",
                        "name": c.metadata.name,
                        "version": c.metadata.version
                    })
                );
            } else {
                println!("ok: Cell {}@{}", c.metadata.name, c.metadata.version);
            }
        }
        "AIMan" => {
            let m = load_aiman_from_path(path)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "kind": "AIMan",
                        "name": m.metadata.name,
                        "cells": m.spec.cells.len()
                    })
                );
            } else {
                println!("ok: AIMan {} ({} cells)", m.metadata.name, m.spec.cells.len());
            }
        }
        "BindingProposal" => {
            let p = load_binding_proposal_from_path(path)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "kind": "BindingProposal",
                        "generation": p.metadata.generation,
                        "bindings": p.spec.bindings.len()
                    })
                );
            } else {
                println!(
                    "ok: BindingProposal gen={} ({} bindings)",
                    p.metadata.generation,
                    p.spec.bindings.len()
                );
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn cmd_inspect(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let kind = detect_kind(path)?;
    let text = std::fs::read_to_string(path)?;
    let v: serde_yaml::Value = serde_yaml::from_str(&text)?;
    // Re-validate via typed loaders.
    match kind {
        "Cell" => {
            let _ = load_cell_from_path(path)?;
        }
        "AIMan" => {
            let _ = load_aiman_from_path(path)?;
        }
        "BindingProposal" => {
            let _ = load_binding_proposal_from_path(path)?;
        }
        _ => unreachable!(),
    }
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn cmd_new(name: &str, dir: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let out = dir.unwrap_or_else(|| PathBuf::from(name));
    std::fs::create_dir_all(&out)?;
    let cell_yaml = format!(
        r#"apiVersion: kcell.dev/v1
kind: Cell
metadata:
  name: {name}
  version: 0.1.0
  description: Scaffolded by kcell new
spec:
  runtime:
    kind: inprocess
    entrypoint: main
  provides:
    - name: {name}
      version: "1"
  requires: []
  communication:
    active: false
    passive: true
  ports: []
  permissions: {{}}
  resources:
    memoryMb: 64
    timeoutMs: 5000
    concurrency: 4
  restartPolicy: on-failure
"#
    );
    let path = out.join("cell.yaml");
    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }
    std::fs::write(&path, cell_yaml)?;
    std::fs::write(
        out.join("README.md"),
        format!("# {name}\n\nScaffolded Cell. Edit `cell.yaml`, then run `kcell validate cell.yaml`.\n"),
    )?;
    println!("created {}", path.display());
    Ok(())
}

fn cmd_run(
    path: &Path,
    root: Option<PathBuf>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let aiman = load_aiman_from_path(path)?;
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let mut host = Host::new();
    // Deny-by-default: only Cells with empty permission requests activate without grants.
    let activated = host.activate_aiman(&aiman, &root)?;
    if json {
        let states: Vec<_> = host
            .registry()
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.manifest.metadata.name,
                    "version": r.manifest.metadata.version,
                    "state": format!("{:?}", r.state()).to_ascii_lowercase(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "aiman": aiman.metadata.name,
                "activated": activated,
                "cells": states
            })
        );
    } else {
        println!("AI-man `{}` activated: {}", aiman.metadata.name, activated.join(", "));
        for r in host.registry().iter() {
            println!("  - {} => {:?}", r.manifest.metadata.name, r.state());
        }
    }
    Ok(())
}
