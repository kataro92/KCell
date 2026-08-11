//! Local control plane over Unix domain sockets (one JSON line request/response).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::envelope::Envelope;
use crate::error::{Error, Result};
use crate::host::Host;

pub const CONTROL_SCHEMA: &str = "kcell.control.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlRequest {
    pub schema: String,
    pub id: String,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    #[serde(default)]
    pub replace: bool,
    /// When true, `auto_bind` applies the proposal if it changes bindings.
    #[serde(default)]
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlResponse {
    pub schema: String,
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub result: Value,
}

impl ControlRequest {
    fn base(op: &str) -> Self {
        Self {
            schema: CONTROL_SCHEMA.into(),
            id: new_id(),
            op: op.into(),
            capability: None,
            consumer: None,
            payload: None,
            limit: None,
            path: None,
            cell: None,
            replace: false,
            apply: false,
        }
    }

    pub fn ping() -> Self {
        Self::base("ping")
    }

    pub fn status() -> Self {
        Self::base("status")
    }

    pub fn discover(capability: Option<String>) -> Self {
        let mut r = Self::base("discover");
        r.capability = capability;
        r
    }

    pub fn invoke(consumer: impl Into<String>, capability: impl Into<String>, payload: Value) -> Self {
        let mut r = Self::base("invoke");
        r.capability = Some(capability.into());
        r.consumer = Some(consumer.into());
        r.payload = Some(payload);
        r
    }

    pub fn audit(limit: Option<usize>) -> Self {
        let mut r = Self::base("audit");
        r.limit = limit;
        r
    }

    pub fn load(path: impl Into<String>, replace: bool) -> Self {
        let mut r = Self::base("load");
        r.path = Some(path.into());
        r.replace = replace;
        r
    }

    pub fn unload(cell: impl Into<String>) -> Self {
        let mut r = Self::base("unload");
        r.cell = Some(cell.into());
        r
    }

    pub fn apply_bindings(path: impl Into<String>) -> Self {
        let mut r = Self::base("apply_bindings");
        r.path = Some(path.into());
        r
    }

    pub fn auto_bind(apply: bool) -> Self {
        let mut r = Self::base("auto_bind");
        r.apply = apply;
        r
    }

    pub fn propose_from(cell: impl Into<String>, apply: bool) -> Self {
        let mut r = Self::base("propose_from");
        r.cell = Some(cell.into());
        r.apply = apply;
        r
    }

    pub fn shutdown() -> Self {
        Self::base("shutdown")
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != CONTROL_SCHEMA {
            return Err(Error::Validation(format!(
                "unsupported control schema `{}`",
                self.schema
            )));
        }
        if self.id.is_empty() || self.op.is_empty() {
            return Err(Error::Validation("control id/op required".into()));
        }
        Ok(())
    }
}

fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("ctrl-{nanos}")
}

fn ok_resp(id: &str, result: Value) -> ControlResponse {
    ControlResponse {
        schema: CONTROL_SCHEMA.into(),
        id: id.into(),
        ok: true,
        error: None,
        result,
    }
}

fn err_resp(id: &str, error: impl Into<String>) -> ControlResponse {
    ControlResponse {
        schema: CONTROL_SCHEMA.into(),
        id: id.into(),
        ok: false,
        error: Some(error.into()),
        result: Value::Null,
    }
}

/// Handle one control request. Returns `(response, shutdown)`.
pub fn handle_control(host: &mut Host, req: ControlRequest) -> Result<(ControlResponse, bool)> {
    req.validate()?;
    let id = req.id.clone();
    match req.op.as_str() {
        "ping" => Ok((ok_resp(&id, json!({"pong": true})), false)),
        "status" => {
            let cells: Vec<_> = host
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
            Ok((
                ok_resp(
                    &id,
                    json!({
                        "cells": cells,
                        "bindingGeneration": host.bindings().generation(),
                        "bindings": host.bindings().bindings().len(),
                        "providers": host.discover(None).len(),
                        "audit": host.audit().len(),
                    }),
                ),
                false,
            ))
        }
        "discover" => {
            let providers = host.discover(req.capability.as_deref());
            Ok((ok_resp(&id, json!({ "providers": providers })), false))
        }
        "invoke" => {
            let consumer = req
                .consumer
                .as_deref()
                .ok_or_else(|| Error::Validation("invoke requires consumer".into()))?;
            let capability = req
                .capability
                .as_deref()
                .ok_or_else(|| Error::Validation("invoke requires capability".into()))?;
            let payload = req.payload.unwrap_or(Value::Null);
            match host.invoke(consumer, capability, Envelope::request(capability, payload)) {
                Ok(env) => Ok((ok_resp(&id, serde_json::to_value(env)?), false)),
                Err(e) => Ok((err_resp(&id, e.to_string()), false)),
            }
        }
        "audit" => {
            let limit = req.limit.unwrap_or(usize::MAX);
            let events: Vec<_> = host
                .audit()
                .events()
                .iter()
                .rev()
                .take(limit)
                .cloned()
                .collect();
            Ok((ok_resp(&id, json!({ "events": events })), false))
        }
        "load" => {
            let path = req
                .path
                .as_deref()
                .ok_or_else(|| Error::Validation("load requires path".into()))?;
            match host.load_cell_dir(path, req.replace) {
                Ok((name, state)) => Ok((
                    ok_resp(
                        &id,
                        json!({
                            "cell": name,
                            "state": format!("{state:?}").to_ascii_lowercase(),
                            "providers": host.discover(None),
                        }),
                    ),
                    false,
                )),
                Err(e) => Ok((err_resp(&id, e.to_string()), false)),
            }
        }
        "unload" => {
            let cell = req
                .cell
                .as_deref()
                .ok_or_else(|| Error::Validation("unload requires cell".into()))?;
            match host.unload_cell(cell) {
                Ok(state) => Ok((
                    ok_resp(
                        &id,
                        json!({
                            "cell": cell,
                            "state": format!("{state:?}").to_ascii_lowercase(),
                        }),
                    ),
                    false,
                )),
                Err(e) => Ok((err_resp(&id, e.to_string()), false)),
            }
        }
        "apply_bindings" => {
            let path = req
                .path
                .as_deref()
                .ok_or_else(|| Error::Validation("apply_bindings requires path".into()))?;
            match host.apply_binding_proposal_path(path) {
                Ok(result) => Ok((ok_resp(&id, serde_json::to_value(result)?), false)),
                Err(e) => Ok((err_resp(&id, e.to_string()), false)),
            }
        }
        "auto_bind" => match host.auto_bind(req.apply, false) {
            Ok((proposal, applied)) => Ok((
                ok_resp(
                    &id,
                    json!({
                        "proposal": proposal,
                        "applied": applied,
                        "bindingGeneration": host.bindings().generation(),
                        "bindings": host.bindings().bindings(),
                    }),
                ),
                false,
            )),
            Err(e) => Ok((err_resp(&id, e.to_string()), false)),
        },
        "propose_from" => {
            let cell = req
                .cell
                .as_deref()
                .ok_or_else(|| Error::Validation("propose_from requires cell".into()))?;
            match host.propose_from_cell(cell, req.apply) {
                Ok((proposal, applied)) => Ok((
                    ok_resp(
                        &id,
                        json!({
                            "proposal": proposal,
                            "applied": applied,
                            "bindingGeneration": host.bindings().generation(),
                            "bindings": host.bindings().bindings(),
                        }),
                    ),
                    false,
                )),
                Err(e) => Ok((err_resp(&id, e.to_string()), false)),
            }
        }
        "shutdown" => Ok((ok_resp(&id, json!({"bye": true})), true)),
        other => Ok((err_resp(&id, format!("unknown op `{other}`")), false)),
    }
}

/// Serve control requests on a Unix domain socket until `shutdown`.
#[cfg(unix)]
pub fn serve_unix(socket: &Path, host: Host) -> Result<()> {
    serve_unix_with_watch(socket, host, None)
}

/// Like [`serve_unix`], optionally spawning a Cell directory poller.
#[cfg(unix)]
pub fn serve_unix_with_watch(
    socket: &Path,
    mut host: Host,
    watch: Option<crate::watch::WatchConfig>,
) -> Result<()> {
    use std::os::unix::net::UnixListener;

    if socket.exists() {
        std::fs::remove_file(socket)?;
    }
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(socket)?;
    if let Some(cfg) = watch {
        crate::watch::spawn_watch_thread(socket.to_path_buf(), cfg);
    }
    loop {
        let (stream, _) = listener.accept()?;
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            continue;
        }
        let req: ControlRequest = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                let resp = err_resp("invalid", e.to_string());
                write_response(&stream, &resp)?;
                continue;
            }
        };
        let (resp, stop) = handle_control(&mut host, req)?;
        write_response(&stream, &resp)?;
        if stop {
            break;
        }
    }
    let _ = std::fs::remove_file(socket);
    Ok(())
}

#[cfg(not(unix))]
pub fn serve_unix(socket: &Path, host: Host) -> Result<()> {
    serve_unix_with_watch(socket, host, None)
}

#[cfg(not(unix))]
pub fn serve_unix_with_watch(
    _socket: &Path,
    _host: Host,
    _watch: Option<crate::watch::WatchConfig>,
) -> Result<()> {
    Err(Error::Validation(
        "kcell serve requires Unix domain sockets".into(),
    ))
}

/// One-shot control call over a Unix domain socket.
#[cfg(unix)]
pub fn call_unix(socket: &Path, req: ControlRequest) -> Result<ControlResponse> {
    use std::os::unix::net::UnixStream;

    req.validate()?;
    let mut stream = UnixStream::connect(socket)?;
    let line = serde_json::to_string(&req)? + "\n";
    stream.write_all(line.as_bytes())?;
    let mut reader = BufReader::new(&stream);
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    let resp: ControlResponse = serde_json::from_str(buf.trim())?;
    Ok(resp)
}

#[cfg(not(unix))]
pub fn call_unix(_socket: &Path, _req: ControlRequest) -> Result<ControlResponse> {
    Err(Error::Validation(
        "kcell call requires Unix domain sockets".into(),
    ))
}

fn write_response(mut stream: impl Write, resp: &ControlResponse) -> Result<()> {
    let line = serde_json::to_string(resp)? + "\n";
    stream.write_all(line.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::manifest::{
        Capability, CellManifest, CellMetadata, CellSpec, Communication, Permissions, RestartPolicy,
        RuntimeKind, RuntimeSpec,
    };
    use std::thread;
    use std::time::Duration;

    fn echo_cell() -> CellManifest {
        CellManifest {
            api_version: "kcell.dev/v1".into(),
            kind: "Cell".into(),
            metadata: CellMetadata {
                name: "echo-cell".into(),
                version: "0.1.0".into(),
                description: None,
            },
            spec: CellSpec {
                runtime: RuntimeSpec {
                    kind: RuntimeKind::Inprocess,
                    entrypoint: Some("main".into()),
                    artifact: None,
                },
                provides: vec![Capability {
                    name: "echo".into(),
                    version: "1".into(),
                    contract: None,
                }],
                requires: vec![],
                communication: Communication {
                    active: false,
                    passive: true,
                },
                ports: vec![],
                resources: Default::default(),
                permissions: Permissions::default(),
                health: Default::default(),
                restart_policy: RestartPolicy::OnFailure,
            },
        }
    }

    #[test]
    fn handle_ping_status() {
        let mut host = Host::new();
        host.register_cell("1", echo_cell()).unwrap();
        host.activate_cell("echo-cell").unwrap();
        let (pong, _) = handle_control(&mut host, ControlRequest::ping()).unwrap();
        assert!(pong.ok);
        let (st, _) = handle_control(&mut host, ControlRequest::status()).unwrap();
        assert!(st.ok);
        assert_eq!(st.result["cells"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn hot_load_cell_dir() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut host = Host::new();
        let (name, _) = host
            .load_cell_dir(root.join("cells/echo-cell"), false)
            .unwrap();
        assert_eq!(name, "echo-cell");
        assert_eq!(host.discover(Some("echo")).len(), 1);

        let (resp, _) = handle_control(
            &mut host,
            ControlRequest::load(root.join("cells/echo-sub-cell").display().to_string(), false),
        )
        .unwrap();
        assert!(resp.ok, "{resp:?}");
        assert!(host.discover(Some("echo-sub")).len() >= 1);

        let (resp, _) =
            handle_control(&mut host, ControlRequest::unload("echo-sub-cell")).unwrap();
        assert!(resp.ok);
        assert!(host.discover(Some("echo-sub")).is_empty());
    }

    #[test]
    fn unix_roundtrip() {
        let dir = std::env::temp_dir().join(format!("kcell-ctrl-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("kcell.sock");
        let _ = std::fs::remove_file(&sock);

        let mut host = Host::new();
        host.register_cell("1", echo_cell()).unwrap();
        host.activate_cell("echo-cell").unwrap();

        let sock2 = sock.clone();
        let handle = thread::spawn(move || serve_unix(&sock2, host));

        for _ in 0..50 {
            if sock.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let resp = call_unix(&sock, ControlRequest::discover(Some("echo".into()))).unwrap();
        assert!(resp.ok);
        assert_eq!(resp.result["providers"].as_array().unwrap().len(), 1);

        let _ = call_unix(&sock, ControlRequest::shutdown()).unwrap();
        let _ = handle.join();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
