//! Minimal MCP JSON-RPC 2.0 over Content-Length framed stdio.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub fn encode_tool_name(cell: &str, capability: &str) -> String {
    format!("{cell}__{capability}")
}

pub fn decode_tool_name(name: &str) -> Option<(String, String)> {
    let (cell, capability) = name.split_once("__")?;
    if cell.is_empty() || capability.is_empty() || capability.contains("__") {
        // capability may contain nothing else; disallow empty parts
        return None;
    }
    // Allow capability without further __; cell is dns-label (no __).
    if cell.contains("__") {
        return None;
    }
    Some((cell.into(), capability.into()))
}

pub fn ok_result(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(result),
        error: None,
    }
}

pub fn err_result(id: Option<Value>, code: i64, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.into(),
            data: None,
        }),
    }
}

pub fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "kcell-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// Read one Content-Length framed JSON message from stdin.
pub fn read_message(stdin: &mut impl BufRead) -> io::Result<Option<JsonRpcRequest>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = stdin.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            let v = rest.trim().parse::<usize>().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("content-length: {e}"))
            })?;
            content_length = Some(v);
        }
        // ignore other headers (Content-Type, …)
    }
    let len = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;
    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf)?;
    let req: JsonRpcRequest =
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(req))
}

pub fn write_message(stdout: &mut impl Write, msg: &JsonRpcResponse) -> io::Result<()> {
    let body =
        serde_json::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write!(stdout, "Content-Length: {}\r\n\r\n", body.len())?;
    stdout.write_all(&body)?;
    stdout.flush()?;
    Ok(())
}

/// Notifications have no id; write nothing (or empty ack is not required by MCP).
pub fn is_notification(req: &JsonRpcRequest) -> bool {
    req.id.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_roundtrip() {
        assert_eq!(encode_tool_name("echo-cell", "echo"), "echo-cell__echo");
        assert_eq!(
            decode_tool_name("echo-cell__echo"),
            Some(("echo-cell".into(), "echo".into()))
        );
        assert!(decode_tool_name("nounderscore").is_none());
        assert!(decode_tool_name("__echo").is_none());
        assert!(decode_tool_name("cell__").is_none());
    }

    #[test]
    fn framing_roundtrip() {
        let mut buf = Vec::new();
        let resp = ok_result(Some(json!(1)), json!({"ok": true}));
        write_message(&mut buf, &resp).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("Content-Length:"));
        assert!(s.contains("\r\n\r\n"));
        let req_body = serde_json::to_vec(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ping",
            "params": {}
        }))
        .unwrap();
        let framed = format!("Content-Length: {}\r\n\r\n", req_body.len());
        let mut raw = framed.into_bytes();
        raw.extend_from_slice(&req_body);
        let mut reader = io::Cursor::new(raw);
        let msg = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(msg.method, "ping");
    }
}
