//! Bridge: AG-UI run → KCell control `invoke`.

use std::path::{Path, PathBuf};

use kcell_core::{call_unix, ControlRequest};
use serde_json::Value;

pub struct Bridge {
    pub socket: PathBuf,
    pub consumer: String,
    pub capability: String,
}

impl Bridge {
    pub fn invoke(&self, payload: Value) -> Result<Value, String> {
        let resp = call_unix(
            Path::new(&self.socket),
            ControlRequest::invoke(self.consumer.clone(), self.capability.clone(), payload),
        )
        .map_err(|e| e.to_string())?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "invoke failed".into()));
        }
        Ok(resp.result)
    }
}
