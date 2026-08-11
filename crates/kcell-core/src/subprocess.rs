//! Subprocess executor — one JSON envelope line in, one JSON envelope line out (stdio).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::envelope::Envelope;
use crate::error::{Error, Result};
use crate::manifest::RuntimeKind;

#[derive(Debug, Clone)]
pub struct SubprocessSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout_ms: u64,
    pub workdir: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct SubprocessExecutor {
    cells: BTreeMap<String, SubprocessSpec>,
}

impl SubprocessExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, cell_name: impl Into<String>, spec: SubprocessSpec) {
        self.cells.insert(cell_name.into(), spec);
    }

    pub fn remove(&mut self, cell_name: &str) -> Option<SubprocessSpec> {
        self.cells.remove(cell_name)
    }

    pub fn get(&self, cell_name: &str) -> Option<&SubprocessSpec> {
        self.cells.get(cell_name)
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
            .ok_or_else(|| Error::NotFound(format!("no subprocess for cell `{cell_name}`")))?;

        let timeout = request.timeout_ms.unwrap_or(spec.timeout_ms).max(1);

        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &spec.workdir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "spawn `{}` for cell `{cell_name}`: {e}",
                    spec.program.display()
                ),
            ))
        })?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Validation("subprocess stdin missing".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Validation("subprocess stdout missing".into()))?;

        let line = serde_json::to_string(request)? + "\n";
        stdin.write_all(line.as_bytes())?;
        drop(stdin);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = String::new();
            let res = reader.read_line(&mut buf).map(|_| buf);
            let _ = tx.send(res);
        });

        let raw = match rx.recv_timeout(Duration::from_millis(timeout)) {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Io(e));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Timeout(format!(
                    "subprocess `{cell_name}` exceeded {timeout}ms"
                )));
            }
        };

        let status = child.wait()?;
        if !status.success() {
            return Err(Error::Validation(format!(
                "subprocess `{cell_name}` exited with {status}"
            )));
        }

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::Validation(format!(
                "subprocess `{cell_name}` returned empty stdout"
            )));
        }
        let reply: Envelope = serde_json::from_str(trimmed)?;
        reply.validate()?;
        if reply.correlation_id != request.correlation_id {
            return Err(Error::Validation(
                "subprocess reply correlationId mismatch".into(),
            ));
        }
        Ok(reply)
    }
}

impl crate::execute::CellExecutor for SubprocessExecutor {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Subprocess
    }

    fn invoke(&self, cell_name: &str, request: &Envelope) -> Result<Envelope> {
        SubprocessExecutor::invoke(self, cell_name, request)
    }
}
