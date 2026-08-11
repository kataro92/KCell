//! Execution interface — adapters (WASI/subprocess) implement `CellExecutor`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::json;

use crate::envelope::Envelope;
use crate::error::{Error, Result};
use crate::manifest::RuntimeKind;

/// Handles one invoke for a Cell instance (in-process MVP).
pub trait CellHandler: Send + Sync {
    fn invoke(&self, request: &Envelope) -> Result<Envelope>;
}

/// Default in-process handler: echoes payload under the Cell name.
/// Universal passthrough — not a product feature.
#[derive(Debug, Clone)]
pub struct PassthroughHandler {
    pub cell_name: String,
}

impl CellHandler for PassthroughHandler {
    fn invoke(&self, request: &Envelope) -> Result<Envelope> {
        request.validate()?;
        Ok(request.reply_to(json!({
            "cell": self.cell_name,
            "capability": request.capability,
            "echo": request.payload,
        })))
    }
}

/// Maps Cell names to handlers for `RuntimeKind::Inprocess`.
#[derive(Default)]
pub struct InProcessExecutor {
    handlers: BTreeMap<String, Arc<dyn CellHandler>>,
}

impl InProcessExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, cell_name: impl Into<String>, handler: Arc<dyn CellHandler>) {
        self.handlers.insert(cell_name.into(), handler);
    }

    pub fn ensure_passthrough(&mut self, cell_name: &str) {
        if !self.handlers.contains_key(cell_name) {
            self.register(
                cell_name.to_string(),
                Arc::new(PassthroughHandler {
                    cell_name: cell_name.to_string(),
                }),
            );
        }
    }

    pub fn remove(&mut self, cell_name: &str) -> bool {
        self.handlers.remove(cell_name).is_some()
    }

    pub fn invoke(&self, cell_name: &str, request: &Envelope) -> Result<Envelope> {
        let h = self
            .handlers
            .get(cell_name)
            .ok_or_else(|| Error::NotFound(format!("no handler for cell `{cell_name}`")))?;
        h.invoke(request)
    }

    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

/// Narrow facade used by Host; later WASI/subprocess plug in beside this.
pub trait CellExecutor: Send + Sync {
    fn kind(&self) -> RuntimeKind;
    fn invoke(&self, cell_name: &str, request: &Envelope) -> Result<Envelope>;
}

impl CellExecutor for InProcessExecutor {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Inprocess
    }

    fn invoke(&self, cell_name: &str, request: &Envelope) -> Result<Envelope> {
        InProcessExecutor::invoke(self, cell_name, request)
    }
}
