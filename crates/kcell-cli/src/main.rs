use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use kcell_core::{
    build_cell_dir, call_unix, default_state_path, default_stem_dir, load_aiman_from_path,
    load_binding_proposal_from_path, load_cell_from_path, load_host_state, notify_feature_enabled,
    parse_cap_token, serve_unix_with_watch, specialize, ControlRequest, Envelope, Host,
    RuntimeKind, SpecializeRequest, WatchConfig, ENVELOPE_SCHEMA,
};
use serde_json::json;

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
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Inspect a Cell or AI-man manifest (JSON)
    Inspect { path: PathBuf },
    /// Scaffold a minimal Cell package directory (specialize with defaults)
    New {
        name: String,
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Stem template directory (default: templates/stem-cell)
        #[arg(long, default_value = "templates/stem-cell")]
        from: PathBuf,
    },
    /// Specialize a Stem template into a Cell package
    Specialize {
        name: String,
        /// Output directory (default: ./<name>)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Stem template directory
        #[arg(long, default_value = "templates/stem-cell")]
        from: PathBuf,
        /// Capability provided (`name` or `name:version`); repeatable
        #[arg(long = "provide", value_name = "CAP")]
        provide: Vec<String>,
        /// Capability required (`name` or `name:version`); repeatable
        #[arg(long = "require", value_name = "CAP")]
        require: Vec<String>,
        /// Runtime kind: inprocess | subprocess | wasi
        #[arg(long, default_value = "inprocess")]
        runtime: String,
        #[arg(long)]
        entrypoint: Option<String>,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long)]
        description: Option<String>,
        /// Enable active communication
        #[arg(long)]
        active: bool,
        /// Enable passive communication
        #[arg(long)]
        passive: bool,
        /// Write .kcell/package.json digest after specialize
        #[arg(long)]
        build: bool,
        #[arg(long)]
        json: bool,
    },
    /// Build Cell package metadata (digest → .kcell/package.json)
    Build {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Stdio JSON-line worker for subprocess Cells (one request → one reply)
    Worker,
    /// Stdio worker for auto-config Cells (`binding-propose`)
    WorkerAutoconfig,
    /// Activate an AI-man and list routable capability providers
    Discover {
        path: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        capability: Option<String>,
        #[arg(long = "grant-process", value_name = "TARGET")]
        grant_process: Vec<String>,
        #[arg(long = "grant-network", value_name = "TARGET")]
        grant_network: Vec<String>,
        #[arg(long = "grant-filesystem", value_name = "TARGET")]
        grant_filesystem: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Load an AI-man, activate Cells, apply static bindings
    Run {
        path: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        invoke: Option<String>,
        #[arg(long)]
        capability: Option<String>,
        /// Grant process permission (repeatable), e.g. `--grant-process '*'`
        #[arg(long = "grant-process", value_name = "TARGET")]
        grant_process: Vec<String>,
        #[arg(long = "grant-network", value_name = "TARGET")]
        grant_network: Vec<String>,
        #[arg(long = "grant-filesystem", value_name = "TARGET")]
        grant_filesystem: Vec<String>,
        /// Include audit trail and bus snapshot in output
        #[arg(long)]
        audit: bool,
        #[arg(long)]
        json: bool,
    },
    /// Activate an AI-man and serve control API on a Unix socket
    Serve {
        path: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
        /// Socket path (default: .kcell/kcell.sock)
        #[arg(long, default_value = ".kcell/kcell.sock")]
        socket: PathBuf,
        /// Watch directory of Cell packages (repeatable); hot-load on add/change
        #[arg(long = "watch", value_name = "DIR")]
        watch: Vec<PathBuf>,
        /// Poll interval for --watch / notify debounce (milliseconds)
        #[arg(long = "watch-interval-ms", default_value_t = 1000)]
        watch_interval_ms: u64,
        /// Use OS file events for --watch (requires build with `--features notify`)
        #[arg(long)]
        watch_notify: bool,
        /// After activate / watch load, auto-bind unmatched requires
        #[arg(long)]
        auto_bind: bool,
        /// Durable Host state path (default: beside socket → host-state.json)
        #[arg(long)]
        state: Option<PathBuf>,
        /// Do not write host-state.json after mutations
        #[arg(long)]
        no_persist: bool,
        /// Do not restore host-state.json after AI-man activate
        #[arg(long)]
        no_restore: bool,
        #[arg(long = "grant-process", value_name = "TARGET")]
        grant_process: Vec<String>,
        #[arg(long = "grant-network", value_name = "TARGET")]
        grant_network: Vec<String>,
        #[arg(long = "grant-filesystem", value_name = "TARGET")]
        grant_filesystem: Vec<String>,
    },
    /// Call a running `kcell serve` control socket
    Call {
        #[arg(long, default_value = ".kcell/kcell.sock")]
        socket: PathBuf,
        #[command(subcommand)]
        op: CallOp,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CallOp {
    Ping,
    Status,
    Discover {
        #[arg(long)]
        capability: Option<String>,
    },
    Invoke {
        #[arg(long)]
        consumer: String,
        #[arg(long)]
        capability: String,
        /// JSON payload (default: {"ping":true})
        #[arg(long)]
        payload: Option<String>,
    },
    Audit {
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Hot-load a Cell directory into the running Host
    Load {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        replace: bool,
    },
    /// Stop and remove a Cell from the running Host
    Unload {
        #[arg(long)]
        cell: String,
    },
    /// Apply a BindingProposal YAML into the running Host
    ApplyBindings {
        #[arg(long)]
        path: PathBuf,
    },
    /// Propose (and optionally apply) bindings for unmatched requires
    AutoBind {
        /// Apply the proposal when it changes the binding set
        #[arg(long)]
        apply: bool,
    },
    /// Ask an auto-config Cell to propose bindings (Host applies when --apply)
    ProposeFrom {
        #[arg(long)]
        cell: String,
        #[arg(long)]
        apply: bool,
    },
    Shutdown,
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
        Commands::New { name, dir, from } => cmd_new(&name, dir, from)?,
        Commands::Specialize {
            name,
            dir,
            from,
            provide,
            require,
            runtime,
            entrypoint,
            artifact,
            version,
            description,
            active,
            passive,
            build,
            json,
        } => cmd_specialize(
            &name,
            dir,
            from,
            provide,
            require,
            &runtime,
            entrypoint,
            artifact,
            &version,
            description,
            active,
            passive,
            build,
            json,
        )?,
        Commands::Build { path, json } => cmd_build(&path, json)?,
        Commands::Worker => cmd_worker()?,
        Commands::WorkerAutoconfig => cmd_worker_autoconfig()?,
        Commands::Discover {
            path,
            root,
            capability,
            grant_process,
            grant_network,
            grant_filesystem,
            json,
        } => cmd_discover(
            &path,
            root,
            capability,
            grant_process,
            grant_network,
            grant_filesystem,
            json,
        )?,
        Commands::Run {
            path,
            root,
            invoke,
            capability,
            grant_process,
            grant_network,
            grant_filesystem,
            audit,
            json,
        } => cmd_run(
            &path,
            root,
            invoke,
            capability,
            grant_process,
            grant_network,
            grant_filesystem,
            audit,
            json,
        )?,
        Commands::Serve {
            path,
            root,
            socket,
            watch,
            watch_interval_ms,
            watch_notify,
            auto_bind,
            state,
            no_persist,
            no_restore,
            grant_process,
            grant_network,
            grant_filesystem,
        } => cmd_serve(
            &path,
            root,
            socket,
            watch,
            watch_interval_ms,
            watch_notify,
            auto_bind,
            state,
            no_persist,
            no_restore,
            grant_process,
            grant_network,
            grant_filesystem,
        )?,
        Commands::Call { socket, op, json } => cmd_call(&socket, op, json)?,
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
                    json!({
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
                    json!({
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
                    json!({
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

fn parse_runtime(s: &str) -> Result<RuntimeKind, Box<dyn std::error::Error>> {
    match s.trim().to_ascii_lowercase().as_str() {
        "inprocess" => Ok(RuntimeKind::Inprocess),
        "subprocess" => Ok(RuntimeKind::Subprocess),
        "wasi" => Ok(RuntimeKind::Wasi),
        other => Err(format!("unknown runtime `{other}` (inprocess|subprocess|wasi)").into()),
    }
}

fn cmd_specialize(
    name: &str,
    dir: Option<PathBuf>,
    from: PathBuf,
    provide: Vec<String>,
    require: Vec<String>,
    runtime: &str,
    entrypoint: Option<String>,
    artifact: Option<String>,
    version: &str,
    description: Option<String>,
    active: bool,
    passive: bool,
    build: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = dir.unwrap_or_else(|| PathBuf::from(name));
    let stem_dir = if from.as_os_str().is_empty() {
        default_stem_dir(std::env::current_dir()?)
    } else {
        from
    };

    let mut provides = Vec::new();
    for p in &provide {
        provides.push(parse_cap_token(p)?);
    }
    let mut requires = Vec::new();
    for r in &require {
        requires.push(parse_cap_token(r)?);
    }
    // Default: provide the cell name when nothing specified (same as `new`).
    if provides.is_empty() && requires.is_empty() {
        provides.push((name.to_string(), "1".into()));
    }

    let artifact = artifact.and_then(|a| {
        let t = a.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });

    let (active_opt, passive_opt) = if active || passive {
        (Some(active), Some(passive))
    } else {
        (None, None)
    };

    let result = specialize(SpecializeRequest {
        name: name.into(),
        version: version.into(),
        description,
        runtime: parse_runtime(runtime)?,
        entrypoint,
        artifact,
        provides,
        requires,
        active: active_opt,
        passive: passive_opt,
        stem_dir,
        out_dir,
        run_build: build,
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "specialized {}@{} → {}",
            result.name,
            result.version,
            result.cell_yaml.display()
        );
        if let Some(pkg) = &result.package {
            println!("digest {}", pkg.digest);
        }
    }
    Ok(())
}

fn cmd_new(
    name: &str,
    dir: Option<PathBuf>,
    from: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    cmd_specialize(
        name,
        dir,
        from,
        vec![format!("{name}:1")],
        vec![],
        "inprocess",
        Some("main".into()),
        None,
        "0.1.0",
        Some("Scaffolded by kcell new".into()),
        false,
        true,
        false,
        false,
    )
}

fn cmd_build(path: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (meta, out) = build_cell_dir(path)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&meta)?);
    } else {
        println!(
            "built {}@{} {} → {}",
            meta.name,
            meta.version,
            meta.digest,
            out.display()
        );
    }
    Ok(())
}

fn cmd_worker() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let req: Envelope = serde_json::from_str(line.trim())?;
    req.validate()?;
    let reply = Envelope {
        schema: ENVELOPE_SCHEMA.into(),
        correlation_id: req.correlation_id.clone(),
        idempotency_key: req.idempotency_key.clone(),
        timeout_ms: req.timeout_ms,
        capability: req.capability.clone(),
        payload: json!({
            "runtime": "subprocess",
            "capability": req.capability,
            "echo": req.payload,
        }),
    };
    let mut out = io::stdout().lock();
    writeln!(out, "{}", serde_json::to_string(&reply)?)?;
    out.flush()?;
    Ok(())
}

/// Reference auto-config Cell: greedy binding-propose from a Host snapshot.
fn cmd_worker_autoconfig() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let req: Envelope = serde_json::from_str(line.trim())?;
    req.validate()?;
    if req.capability != "binding-propose" {
        return Err(format!(
            "worker-autoconfig expects capability binding-propose, got {}",
            req.capability
        )
        .into());
    }
    let proposal = propose_from_snapshot(&req.payload)?;
    let reply = Envelope {
        schema: ENVELOPE_SCHEMA.into(),
        correlation_id: req.correlation_id.clone(),
        idempotency_key: req.idempotency_key.clone(),
        timeout_ms: req.timeout_ms,
        capability: req.capability.clone(),
        payload: json!({ "proposal": proposal }),
    };
    let mut out = io::stdout().lock();
    writeln!(out, "{}", serde_json::to_string(&reply)?)?;
    out.flush()?;
    Ok(())
}

fn propose_from_snapshot(payload: &serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let generation = payload.get("generation").and_then(|v| v.as_u64()).unwrap_or(0);
    let cells = payload
        .get("cells")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let existing = payload
        .get("bindings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut bindings = existing.clone();

    for cell in &cells {
        let consumer = cell
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if consumer.is_empty() {
            continue;
        }
        let requires = cell
            .get("requires")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for req in requires {
            let cap = req
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if cap.is_empty() {
                continue;
            }
            let optional = req.get("optional").and_then(|v| v.as_bool()).unwrap_or(false);
            let version = req
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("1")
                .to_string();
            if bindings.iter().any(|b| {
                b.get("consumer").and_then(|v| v.as_str()) == Some(consumer.as_str())
                    && b.get("capability").and_then(|v| v.as_str()) == Some(cap.as_str())
            }) {
                continue;
            }
            let mut providers: Vec<&serde_json::Value> = cells
                .iter()
                .filter(|c| c.get("name").and_then(|v| v.as_str()) != Some(consumer.as_str()))
                .filter(|c| {
                    c.get("provides")
                        .and_then(|v| v.as_array())
                        .map(|ps| {
                            ps.iter().any(|p| p.get("name").and_then(|v| v.as_str()) == Some(cap.as_str()))
                        })
                        .unwrap_or(false)
                })
                .collect();
            providers.sort_by(|a, b| {
                let a_ver = a
                    .get("provides")
                    .and_then(|v| v.as_array())
                    .and_then(|ps| {
                        ps.iter().find(|p| p.get("name").and_then(|v| v.as_str()) == Some(cap.as_str()))
                    })
                    .and_then(|p| p.get("version").and_then(|v| v.as_str()))
                    .unwrap_or("");
                let b_ver = b
                    .get("provides")
                    .and_then(|v| v.as_array())
                    .and_then(|ps| {
                        ps.iter().find(|p| p.get("name").and_then(|v| v.as_str()) == Some(cap.as_str()))
                    })
                    .and_then(|p| p.get("version").and_then(|v| v.as_str()))
                    .unwrap_or("");
                let a_match = (a_ver == version) as u8;
                let b_match = (b_ver == version) as u8;
                b_match.cmp(&a_match).then_with(|| {
                    let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    an.cmp(bn)
                })
            });
            if let Some(p) = providers.first() {
                let provider = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if !provider.is_empty() {
                    bindings.push(json!({
                        "consumer": consumer,
                        "provider": provider,
                        "capability": cap,
                        "required": !optional,
                    }));
                }
            }
        }
    }

    if bindings.is_empty() {
        return Err("auto-config produced empty proposal (no bindings)".into());
    }

    Ok(json!({
        "apiVersion": "kcell.dev/v1",
        "kind": "BindingProposal",
        "metadata": {
            "proposer": "cell:auto-config-cell",
            "generation": generation + 1,
            "reason": "auto-config greedy match from snapshot",
        },
        "spec": {
            "bindings": bindings,
            "replaceGeneration": generation,
        }
    }))
}

fn cmd_discover(
    path: &Path,
    root: Option<PathBuf>,
    capability: Option<String>,
    grant_process: Vec<String>,
    grant_network: Vec<String>,
    grant_filesystem: Vec<String>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let aiman = load_aiman_from_path(path)?;
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let mut host = Host::new();
    if let Ok(exe) = std::env::current_exe() {
        host.set_program_alias("kcell", exe);
    }
    host.grant_many(grant_process, grant_network, grant_filesystem);
    let _ = host.activate_aiman(&aiman, &root)?;
    let providers = host.discover(capability.as_deref());
    if json {
        println!(
            "{}",
            json!({
                "ok": true,
                "aiman": aiman.metadata.name,
                "providers": providers,
            })
        );
    } else {
        println!(
            "providers for AI-man `{}` ({}):",
            aiman.metadata.name,
            providers.len()
        );
        for p in providers {
            println!(
                "  {}@{} provides {}@{} ({:?}, {:?})",
                p.cell, p.cell_version, p.capability, p.capability_version, p.runtime, p.state
            );
        }
    }
    Ok(())
}

fn cmd_run(
    path: &Path,
    root: Option<PathBuf>,
    invoke: Option<String>,
    capability: Option<String>,
    grant_process: Vec<String>,
    grant_network: Vec<String>,
    grant_filesystem: Vec<String>,
    audit: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if invoke.is_some() != capability.is_some() {
        return Err("--invoke and --capability must be used together".into());
    }
    let aiman = load_aiman_from_path(path)?;
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let mut host = Host::new();
    if let Ok(exe) = std::env::current_exe() {
        host.set_program_alias("kcell", exe);
    }
    host.grant_many(grant_process, grant_network, grant_filesystem);

    let activated = host.activate_aiman(&aiman, &root)?;
    let providers = host.discover(None);

    let mut invoke_result = None;
    if let (Some(consumer), Some(cap)) = (invoke, capability) {
        let reply = host.invoke(
            &consumer,
            &cap,
            Envelope::request(cap.clone(), json!({"ping": true})),
        )?;
        invoke_result = Some(reply);
    }

    if json {
        let states: Vec<_> = host
            .registry()
            .iter()
            .map(|r| {
                json!({
                    "name": r.manifest.metadata.name,
                    "version": r.manifest.metadata.version,
                    "runtime": format!("{:?}", r.manifest.spec.runtime.kind).to_ascii_lowercase(),
                    "state": format!("{:?}", r.state()).to_ascii_lowercase(),
                })
            })
            .collect();
        let bindings: Vec<_> = host
            .bindings()
            .bindings()
            .iter()
            .map(|b| {
                json!({
                    "consumer": b.consumer,
                    "provider": b.provider,
                    "capability": b.capability,
                })
            })
            .collect();
        let mut body = json!({
            "ok": true,
            "aiman": aiman.metadata.name,
            "activated": activated,
            "bindingGeneration": host.bindings().generation(),
            "bindings": bindings,
            "providers": providers,
            "cells": states,
            "invoke": invoke_result,
        });
        if audit {
            body["audit"] = serde_json::to_value(host.audit().events().iter().collect::<Vec<_>>())?;
            body["bus"] = serde_json::to_value(
                host.bus()
                    .snapshot()
                    .into_iter()
                    .map(|(seq, ev)| json!({"seq": seq, "event": ev}))
                    .collect::<Vec<_>>(),
            )?;
        }
        println!("{body}");
    } else {
        println!(
            "AI-man `{}` activated: {}",
            aiman.metadata.name,
            activated.join(", ")
        );
        println!(
            "bindings gen={}: {}",
            host.bindings().generation(),
            host.bindings().bindings().len()
        );
        for b in host.bindings().bindings() {
            println!(
                "  {} -[{}]-> {}",
                b.consumer, b.capability, b.provider
            );
        }
        println!("providers: {}", providers.len());
        for p in &providers {
            println!(
                "  {} provides {}@{}",
                p.cell, p.capability, p.capability_version
            );
        }
        for r in host.registry().iter() {
            println!(
                "  - {} ({:?}) => {:?}",
                r.manifest.metadata.name, r.manifest.spec.runtime.kind, r.state()
            );
        }
        if let Some(reply) = invoke_result {
            println!("invoke => {}", serde_json::to_string_pretty(&reply)?);
        }
        if audit {
            println!("audit ({}):", host.audit().len());
            for ev in host.audit().events() {
                println!(
                    "  #{} {:?} {} {}",
                    ev.seq,
                    ev.kind,
                    ev.cell.as_deref().unwrap_or("-"),
                    ev.detail
                );
            }
        }
    }
    Ok(())
}

fn cmd_serve(
    path: &Path,
    root: Option<PathBuf>,
    socket: PathBuf,
    watch: Vec<PathBuf>,
    watch_interval_ms: u64,
    watch_notify: bool,
    auto_bind: bool,
    state: Option<PathBuf>,
    no_persist: bool,
    no_restore: bool,
    grant_process: Vec<String>,
    grant_network: Vec<String>,
    grant_filesystem: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if watch_notify && watch.is_empty() {
        return Err("--watch-notify requires at least one --watch DIR".into());
    }
    if watch_notify && !notify_feature_enabled() {
        return Err(
            "--watch-notify requires building with `--features notify` (e.g. cargo run -p kcell --features notify -- serve …)"
                .into(),
        );
    }
    let aiman = load_aiman_from_path(path)?;
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let state_path = state.unwrap_or_else(|| default_state_path(&socket));
    let mut host = Host::new();
    if let Ok(exe) = std::env::current_exe() {
        host.set_program_alias("kcell", exe);
    }
    host.grant_many(grant_process, grant_network, grant_filesystem);
    host.set_persist(state_path.clone(), !no_persist);
    let activated = host.activate_aiman(&aiman, &root)?;
    if !no_restore && state_path.is_file() {
        match load_host_state(&state_path) {
            Ok(st) => {
                let n = host.restore_state(&st)?;
                eprintln!(
                    "restored state from {} (loaded {n} extra cells, bindings gen={})",
                    state_path.display(),
                    host.bindings().generation()
                );
            }
            Err(e) => eprintln!("warn: could not restore {}: {e}", state_path.display()),
        }
    }
    if auto_bind {
        let (proposal, applied) = host.auto_bind(true, false)?;
        eprintln!(
            "auto-bind: {} edges, applied={:?}",
            proposal.spec.bindings.len(),
            applied
        );
    }
    let watch_cfg = if watch.is_empty() {
        None
    } else {
        let mode = if watch_notify { "notify" } else { "poll" };
        eprintln!(
            "watching {} ({mode}, interval/debounce {}ms, auto_bind={})",
            watch
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            watch_interval_ms,
            auto_bind
        );
        Some(WatchConfig {
            roots: watch,
            interval_ms: watch_interval_ms,
            auto_bind,
            use_notify: watch_notify,
        })
    };
    eprintln!(
        "serving AI-man `{}` on {} (cells: {}, persist={} state={})",
        aiman.metadata.name,
        socket.display(),
        activated.join(", "),
        !no_persist,
        state_path.display()
    );
    serve_unix_with_watch(&socket, host, watch_cfg)?;
    eprintln!("shutdown");
    Ok(())
}

fn cmd_call(socket: &Path, op: CallOp, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let req = match op {
        CallOp::Ping => ControlRequest::ping(),
        CallOp::Status => ControlRequest::status(),
        CallOp::Discover { capability } => ControlRequest::discover(capability),
        CallOp::Invoke {
            consumer,
            capability,
            payload,
        } => {
            let payload = match payload {
                Some(s) => serde_json::from_str(&s)?,
                None => json!({"ping": true}),
            };
            ControlRequest::invoke(consumer, capability, payload)
        }
        CallOp::Audit { limit } => ControlRequest::audit(limit),
        CallOp::Load { path, replace } => {
            ControlRequest::load(path.display().to_string(), replace)
        }
        CallOp::Unload { cell } => ControlRequest::unload(cell),
        CallOp::ApplyBindings { path } => {
            ControlRequest::apply_bindings(path.display().to_string())
        }
        CallOp::AutoBind { apply } => ControlRequest::auto_bind(apply),
        CallOp::ProposeFrom { cell, apply } => ControlRequest::propose_from(cell, apply),
        CallOp::Shutdown => ControlRequest::shutdown(),
    };
    let resp = call_unix(socket, req)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else if resp.ok {
        println!("{}", serde_json::to_string_pretty(&resp.result)?);
    } else {
        eprintln!("{}", resp.error.as_deref().unwrap_or("call failed"));
    }
    if !resp.ok {
        return Err(resp.error.unwrap_or_else(|| "call failed".into()).into());
    }
    Ok(())
}
