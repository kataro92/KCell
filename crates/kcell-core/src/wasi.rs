//! WASI executor — same JSON-line envelope protocol as subprocess, over WASI stdin/stdout.
//!
//! Enabled only with Cargo feature `wasi` (pulls in wasmtime). Default builds stay lean.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::envelope::Envelope;
use crate::error::{Error, Result};
use crate::manifest::RuntimeKind;

/// Spec for a WASI Cell module.
#[derive(Debug, Clone)]
pub struct WasiSpec {
    /// Absolute path to the `.wasm` module.
    pub module_path: PathBuf,
    /// Export to call (default `_start`).
    pub export: String,
    pub timeout_ms: u64,
}

#[derive(Default)]
pub struct WasiExecutor {
    cells: BTreeMap<String, WasiSpec>,
}

impl std::fmt::Debug for WasiExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasiExecutor")
            .field("cells", &self.cells.len())
            .field("enabled", &cfg!(feature = "wasi"))
            .finish()
    }
}

impl WasiExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this build includes the wasmtime-backed WASI executor.
    pub fn feature_enabled() -> bool {
        cfg!(feature = "wasi")
    }

    pub fn register(&mut self, cell_name: impl Into<String>, spec: WasiSpec) {
        self.cells.insert(cell_name.into(), spec);
    }

    pub fn get(&self, cell_name: &str) -> Option<&WasiSpec> {
        self.cells.get(cell_name)
    }

    pub fn remove(&mut self, cell_name: &str) -> Option<WasiSpec> {
        self.cells.remove(cell_name)
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn invoke(&self, cell_name: &str, request: &Envelope) -> Result<Envelope> {
        request.validate()?;
        let spec = self
            .cells
            .get(cell_name)
            .ok_or_else(|| Error::NotFound(format!("no wasi module for cell `{cell_name}`")))?;

        #[cfg(not(feature = "wasi"))]
        {
            let _ = (cell_name, request, spec);
            return Err(Error::Validation(
                "WASI executor requires building with `--features wasi`".into(),
            ));
        }

        #[cfg(feature = "wasi")]
        {
            invoke_wasmtime(cell_name, spec, request)
        }
    }
}

#[cfg(feature = "wasi")]
fn shared_engine() -> &'static wasmtime::Engine {
    use std::sync::OnceLock;
    use wasmtime::{Config, Engine};

    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.epoch_interruption(true);
        Engine::new(&config).expect("wasmtime engine")
    })
}

#[cfg(feature = "wasi")]
fn invoke_wasmtime(cell_name: &str, spec: &WasiSpec, request: &Envelope) -> Result<Envelope> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use wasmtime::{Linker, Module, Store};
    use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
    use wasmtime_wasi::p2::WasiCtxBuilder;
    use wasmtime_wasi::preview1::{self, WasiP1Ctx};
    use wasmtime_wasi::I32Exit;

    let timeout = request
        .timeout_ms
        .unwrap_or(spec.timeout_ms)
        .max(1);

    let engine = shared_engine();
    let module = Module::from_file(engine, &spec.module_path).map_err(|e| {
        Error::Validation(format!(
            "load wasm `{}` for `{cell_name}`: {e}",
            spec.module_path.display()
        ))
    })?;

    let line = serde_json::to_string(request)? + "\n";
    let stdin = MemoryInputPipe::new(line.into_bytes());
    let stdout = MemoryOutputPipe::new(1024 * 1024);

    let wasi_ctx = WasiCtxBuilder::new()
        .stdin(stdin)
        .stdout(stdout.clone())
        .build_p1();

    let mut linker: Linker<WasiP1Ctx> = Linker::new(engine);
    preview1::add_to_linker_sync(&mut linker, |t| t)
        .map_err(|e| Error::Validation(format!("wasi linker: {e}")))?;

    let mut store = Store::new(engine, wasi_ctx);
    store.set_epoch_deadline(1);

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_tick = Arc::clone(&cancel);
    let engine_tick = engine.clone();
    let ticker = thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(timeout) {
            if cancel_tick.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !cancel_tick.load(Ordering::Relaxed) {
            engine_tick.increment_epoch();
        }
    });

    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| Error::Validation(format!("instantiate `{cell_name}`: {e}")))?;

    let export = if spec.export.is_empty() {
        "_start"
    } else {
        spec.export.as_str()
    };
    let start = instance
        .get_typed_func::<(), ()>(&mut store, export)
        .map_err(|e| Error::Validation(format!("export `{export}` on `{cell_name}`: {e}")))?;

    let call_result = start.call(&mut store, ());
    cancel.store(true, Ordering::Relaxed);
    let _ = ticker.join();

    match call_result {
        Ok(()) => {}
        Err(e) => {
            if let Some(I32Exit(0)) = e.downcast_ref::<I32Exit>() {
                // clean WASI exit
            } else if let Some(I32Exit(code)) = e.downcast_ref::<I32Exit>() {
                return Err(Error::Validation(format!(
                    "wasi `{cell_name}` exited with status {code}"
                )));
            } else if format!("{e:#}").contains("epoch") {
                return Err(Error::Timeout(format!(
                    "wasi `{cell_name}` exceeded {timeout}ms"
                )));
            } else {
                return Err(Error::Validation(format!("wasi `{cell_name}` trap: {e:#}")));
            }
        }
    }

    let raw = String::from_utf8_lossy(&stdout.contents()).into_owned();
    let trimmed = raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if trimmed.is_empty() {
        return Err(Error::Validation(format!(
            "wasi `{cell_name}` returned empty stdout"
        )));
    }
    let reply: Envelope = serde_json::from_str(trimmed)?;
    reply.validate()?;
    if reply.correlation_id != request.correlation_id {
        return Err(Error::Validation(
            "wasi reply correlationId mismatch".into(),
        ));
    }
    Ok(reply)
}

impl crate::execute::CellExecutor for WasiExecutor {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Wasi
    }

    fn invoke(&self, cell_name: &str, request: &Envelope) -> Result<Envelope> {
        WasiExecutor::invoke(self, cell_name, request)
    }
}

#[cfg(all(test, feature = "wasi"))]
mod tests {
    use super::*;
    use crate::envelope::Envelope;
    use serde_json::json;
    use std::path::PathBuf;
    use std::process::Command;

    fn compile_fixture() -> Option<PathBuf> {
        let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/wasi-fixtures");
        let _ = std::fs::create_dir_all(&out_dir);
        let wasm = out_dir.join("echo-wasi.wasm");
        let src = out_dir.join("echo-wasi.rs");
        std::fs::write(
            &src,
            r#"
fn main() {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).unwrap();
    let s = String::from_utf8_lossy(&buf);
    let id = json_str(&s, "correlationId").unwrap_or("\"missing\"");
    let cap = json_str(&s, "capability").unwrap_or("\"echo-wasi\"");
    let out = format!(
        "{{\"schema\":\"kcell.envelope.v1\",\"correlationId\":{id},\"capability\":{cap},\"payload\":{{\"runtime\":\"wasi\",\"ok\":true}}}}\n"
    );
    std::io::stdout().write_all(out.as_bytes()).unwrap();
}

fn json_str<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)?;
    let rest = &s[i + pat.len()..];
    let colon = rest.find(':')?;
    let r = rest[colon + 1..].trim_start();
    if r.starts_with('"') {
        let end = r[1..].find('"')? + 1;
        Some(&r[..=end])
    } else {
        let end = r.find([',', '}']).unwrap_or(r.len());
        Some(r[..end].trim())
    }
}
"#,
        )
        .ok()?;

        let status = Command::new("rustc")
            .args(["--target", "wasm32-wasip1", "-O", "-o"])
            .arg(&wasm)
            .arg(&src)
            .status()
            .ok()?;
        if status.success() && wasm.is_file() {
            Some(wasm)
        } else {
            None
        }
    }

    #[test]
    fn wasi_stdio_envelope_roundtrip() {
        let Some(wasm) = compile_fixture() else {
            eprintln!("skip: rustc wasm32-wasip1 unavailable");
            return;
        };
        let mut exec = WasiExecutor::new();
        exec.register(
            "echo-wasi-cell",
            WasiSpec {
                module_path: wasm,
                export: "_start".into(),
                timeout_ms: 5_000,
            },
        );
        let mut req = Envelope::request("echo-wasi", json!({"n": 1}));
        req.correlation_id = "corr-wasi-1".into();
        let reply = exec.invoke("echo-wasi-cell", &req).expect("invoke");
        assert_eq!(reply.correlation_id, "corr-wasi-1");
        assert_eq!(reply.payload["runtime"], "wasi");
    }
}
