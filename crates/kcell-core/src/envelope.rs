//! Internal message envelope — protocol adapters map to/from this at the edge.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

pub const ENVELOPE_SCHEMA: &str = "kcell.envelope.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub schema: String,
    pub correlation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    pub capability: String,
    #[serde(default)]
    pub payload: Value,
}

impl Envelope {
    pub fn request(capability: impl Into<String>, payload: Value) -> Self {
        Self {
            schema: ENVELOPE_SCHEMA.into(),
            correlation_id: new_id(),
            idempotency_key: None,
            timeout_ms: None,
            capability: capability.into(),
            payload,
        }
    }

    pub fn reply_to(&self, payload: Value) -> Self {
        Self {
            schema: ENVELOPE_SCHEMA.into(),
            correlation_id: self.correlation_id.clone(),
            idempotency_key: self.idempotency_key.clone(),
            timeout_ms: self.timeout_ms,
            capability: self.capability.clone(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != ENVELOPE_SCHEMA {
            return Err(Error::Validation(format!(
                "unsupported envelope schema `{}`",
                self.schema
            )));
        }
        if self.correlation_id.is_empty() {
            return Err(Error::Validation("correlationId required".into()));
        }
        if self.capability.is_empty() {
            return Err(Error::Validation("capability required".into()));
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
    format!("corr-{nanos}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip() {
        let env = Envelope::request("echo", json!({"text": "hi"}));
        env.validate().unwrap();
        let reply = env.reply_to(json!({"text": "hi"}));
        assert_eq!(reply.correlation_id, env.correlation_id);
    }
}
