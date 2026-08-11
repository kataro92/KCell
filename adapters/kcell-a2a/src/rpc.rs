//! JSON-RPC 2.0 dispatch for A2A `message/send`.

use serde_json::{json, Value};

use crate::bridge::Bridge;

pub fn handle_rpc(bridge: &Bridge, body: &[u8]) -> Result<Value, String> {
    let req: Value =
        serde_json::from_slice(body).map_err(|e| format!("invalid JSON-RPC body: {e}"))?;
    let jsonrpc = req.get("jsonrpc").and_then(|v| v.as_str()).unwrap_or("");
    if jsonrpc != "2.0" {
        return Ok(rpc_error(
            req.get("id").cloned(),
            -32600,
            "jsonrpc must be 2.0",
        ));
    }
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "message/send" | "message/sendMessage" => match message_send(bridge, &params) {
            Ok(task) => Ok(rpc_ok(id, task)),
            Err(e) => Ok(rpc_error(id, -32000, e)),
        },
        "" => Ok(rpc_error(id, -32600, "missing method")),
        other => Ok(rpc_error(id, -32601, format!("method not found: {other}"))),
    }
}

fn message_send(bridge: &Bridge, params: &Value) -> Result<Value, String> {
    let message = params
        .get("message")
        .ok_or_else(|| "params.message required".to_string())?;
    let payload = payload_from_message(message)?;
    let result = bridge.invoke(payload)?;
    let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
    let stamp = now_stamp();
    Ok(json!({
        "id": format!("task-{stamp}"),
        "contextId": message.get("contextId").cloned().unwrap_or(Value::Null),
        "status": { "state": "completed" },
        "artifacts": [{
            "artifactId": format!("art-{stamp}"),
            "parts": [{ "kind": "text", "text": text }]
        }]
    }))
}

/// Extract text parts → envelope payload.
pub fn payload_from_message(message: &Value) -> Result<Value, String> {
    let parts = message
        .get("parts")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "message.parts required".to_string())?;
    let mut texts = Vec::new();
    for part in parts {
        let kind = part
            .get("kind")
            .or_else(|| part.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if kind == "text" {
            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                texts.push(t.to_string());
            }
        }
    }
    if texts.is_empty() {
        return Err("message requires a text part".into());
    }
    let joined = texts.join("\n");
    if let Ok(v) = serde_json::from_str::<Value>(&joined) {
        if v.is_object() || v.is_array() {
            return Ok(v);
        }
    }
    Ok(json!({ "text": joined }))
}

pub fn rpc_ok(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": result
    })
}

pub fn rpc_error(id: Option<Value>, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

fn now_stamp() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_from_text_and_json() {
        let msg = json!({
            "messageId": "m1",
            "role": "user",
            "parts": [{ "kind": "text", "text": "hello" }]
        });
        assert_eq!(payload_from_message(&msg).unwrap(), json!({"text":"hello"}));

        let msg2 = json!({
            "parts": [{ "type": "text", "text": "{\"ping\":true}" }]
        });
        assert_eq!(payload_from_message(&msg2).unwrap(), json!({"ping":true}));
    }

    #[test]
    fn rpc_envelopes() {
        let ok = rpc_ok(Some(json!(1)), json!({"id":"task-1"}));
        assert_eq!(ok["result"]["id"], "task-1");
        let err = rpc_error(Some(json!(2)), -32601, "nope");
        assert_eq!(err["error"]["code"], -32601);
    }
}
