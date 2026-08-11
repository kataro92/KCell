//! Bridge: MCP tools ↔ KCell control socket (`discover` / `invoke`).

use std::path::Path;

use kcell_core::{call_unix, ControlRequest};
use serde_json::{json, Value};

use crate::mcp::{decode_tool_name, encode_tool_name, McpTool};

pub struct Bridge {
    pub socket: std::path::PathBuf,
    pub consumer: String,
}

impl Bridge {
    pub fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        let resp = call_unix(Path::new(&self.socket), ControlRequest::discover(None))
            .map_err(|e| e.to_string())?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "discover failed".into()));
        }
        let providers = resp
            .result
            .get("providers")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        Ok(providers_to_tools(&providers))
    }

    pub fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        let (cell, capability) = decode_tool_name(name)
            .ok_or_else(|| format!("invalid tool name `{name}` (expect cell__capability)"))?;
        let payload = arguments
            .get("payload")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let resp = call_unix(
            Path::new(&self.socket),
            ControlRequest::invoke(self.consumer.clone(), capability.clone(), payload),
        )
        .map_err(|e| e.to_string())?;
        if !resp.ok {
            return Err(resp
                .error
                .unwrap_or_else(|| format!("invoke {cell}/{capability} failed")));
        }
        Ok(resp.result)
    }
}

pub fn providers_to_tools(providers: &Value) -> Vec<McpTool> {
    let Some(arr) = providers.as_array() else {
        return Vec::new();
    };
    let mut tools = Vec::new();
    for p in arr {
        let cell = p.get("cell").and_then(|v| v.as_str()).unwrap_or("");
        let capability = p.get("capability").and_then(|v| v.as_str()).unwrap_or("");
        if cell.is_empty() || capability.is_empty() {
            continue;
        }
        let version = p
            .get("capabilityVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let runtime = p
            .get("runtime")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into());
        tools.push(McpTool {
            name: encode_tool_name(cell, capability),
            description: format!(
                "KCell capability `{capability}@{version}` from cell `{cell}` ({runtime})"
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "payload": {
                        "type": "object",
                        "description": "Opaque JSON forwarded as the KCell envelope payload"
                    }
                },
                "additionalProperties": true
            }),
        });
    }
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_discover_providers() {
        let providers = json!([
            {
                "cell": "echo-cell",
                "cellVersion": "0.1.0",
                "capability": "echo",
                "capabilityVersion": "1",
                "runtime": "inprocess",
                "state": "active"
            },
            {
                "cell": "other",
                "capability": "llm",
                "capabilityVersion": "2",
                "runtime": "subprocess",
                "state": "active"
            }
        ]);
        let tools = providers_to_tools(&providers);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "echo-cell__echo");
        assert_eq!(tools[1].name, "other__llm");
        assert!(tools[0].input_schema["properties"]["payload"].is_object());
    }
}
