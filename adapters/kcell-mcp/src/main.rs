mod bridge;
mod mcp;

use std::io::{self, BufReader};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use serde_json::{json, Value};

use bridge::Bridge;
use mcp::{
    err_result, initialize_result, is_notification, ok_result, read_message, write_message,
};

#[derive(Parser, Debug)]
#[command(
    name = "kcell-mcp",
    version,
    about = "KCell MCP adapter — Host capabilities as MCP tools (stdio)"
)]
struct Cli {
    /// Path to `kcell serve` Unix control socket
    #[arg(long, default_value = ".kcell/kcell.sock")]
    socket: PathBuf,
    /// Consumer Cell used for invoke (must hold bindings)
    #[arg(long)]
    consumer: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kcell-mcp error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let bridge = Bridge {
        socket: cli.socket,
        consumer: cli.consumer,
    };

    let stdin = io::stdin();
    let mut stdin = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();

    eprintln!(
        "kcell-mcp listening on stdio (socket={}, consumer={})",
        bridge.socket.display(),
        bridge.consumer
    );

    loop {
        let Some(req) = read_message(&mut stdin)? else {
            break;
        };

        if is_notification(&req) {
            if req.method == "notifications/initialized" {
                // no response for notifications
                continue;
            }
            eprintln!("kcell-mcp: ignoring notification `{}`", req.method);
            continue;
        }

        let id = req.id.clone();
        let resp = match req.method.as_str() {
            "initialize" => ok_result(id, initialize_result()),
            "ping" => ok_result(id, json!({})),
            "tools/list" => match bridge.list_tools() {
                Ok(tools) => {
                    // Serialize with MCP field name inputSchema
                    let tools_json: Vec<Value> = tools
                        .into_iter()
                        .map(|t| {
                            json!({
                                "name": t.name,
                                "description": t.description,
                                "inputSchema": t.input_schema,
                            })
                        })
                        .collect();
                    ok_result(id, json!({ "tools": tools_json }))
                }
                Err(e) => err_result(id, -32000, e),
            },
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
                if name.is_empty() {
                    err_result(id, -32602, "tools/call requires params.name")
                } else {
                    match bridge.call_tool(&name, &arguments) {
                        Ok(result) => {
                            let text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            ok_result(
                                id,
                                json!({
                                    "content": [{ "type": "text", "text": text }],
                                    "isError": false
                                }),
                            )
                        }
                        Err(e) => ok_result(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": e }],
                                "isError": true
                            }),
                        ),
                    }
                }
            }
            other => err_result(id, -32601, format!("method not found: {other}")),
        };

        write_message(&mut stdout, &resp)?;
    }

    Ok(())
}
