mod agui;
mod bridge;
mod http;

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use serde_json::json;

use agui::{
    event_run_error, event_run_finished, event_run_started, event_text_content, event_text_end,
    event_text_start, payload_from_messages, resolve_ids, RunAgentInput,
};
use bridge::Bridge;
use http::{read_request, write_json, write_plain, write_sse_event, write_sse_headers};

#[derive(Parser, Debug)]
#[command(
    name = "kcell-agui",
    version,
    about = "KCell AG-UI adapter — HTTP+SSE over Host control socket"
)]
struct Cli {
    /// Path to `kcell serve` Unix control socket
    #[arg(long, default_value = ".kcell/kcell.sock")]
    socket: PathBuf,
    /// Consumer Cell used for invoke
    #[arg(long)]
    consumer: String,
    /// Capability to invoke
    #[arg(long)]
    capability: String,
    /// HTTP bind address
    #[arg(long, default_value = "127.0.0.1:3456")]
    bind: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kcell-agui error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let bridge = Bridge {
        socket: cli.socket,
        consumer: cli.consumer,
        capability: cli.capability,
    };

    let listener = TcpListener::bind(&cli.bind)?;
    eprintln!(
        "kcell-agui listening on http://{} (socket={}, consumer={}, capability={})",
        cli.bind,
        bridge.socket.display(),
        bridge.consumer,
        bridge.capability
    );

    for conn in listener.incoming() {
        let mut stream = match conn {
            Ok(s) => s,
            Err(e) => {
                eprintln!("kcell-agui accept error: {e}");
                continue;
            }
        };
        if let Err(e) = handle_connection(&mut stream, &bridge) {
            eprintln!("kcell-agui request error: {e}");
            let _ = write_plain(&mut stream, 500, "Internal Server Error", &e);
        }
    }
    Ok(())
}

fn handle_connection(
    stream: &mut std::net::TcpStream,
    bridge: &Bridge,
) -> Result<(), String> {
    let req = read_request(stream)?;
    let path = req.path.split('?').next().unwrap_or(req.path.as_str());

    match (req.method.as_str(), path) {
        ("GET", "/health") => write_json(stream, 200, "OK", &json!({"ok": true})),
        ("POST", "/agent") | ("POST", "/") => handle_agent(stream, bridge, &req.body),
        _ => write_plain(stream, 404, "Not Found", "not found"),
    }
}

fn handle_agent(
    stream: &mut std::net::TcpStream,
    bridge: &Bridge,
    body: &[u8],
) -> Result<(), String> {
    let input: RunAgentInput = serde_json::from_slice(body)
        .map_err(|e| format!("invalid RunAgentInput JSON: {e}"))?;
    let ids = resolve_ids(&input);
    let payload = payload_from_messages(&input.messages)?;

    write_sse_headers(stream)?;
    write_sse_event(stream, &event_run_started(&ids))?;

    match bridge.invoke(payload) {
        Ok(result) => {
            let delta = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
            write_sse_event(stream, &event_text_start(&ids))?;
            write_sse_event(stream, &event_text_content(&ids, &delta))?;
            write_sse_event(stream, &event_text_end(&ids))?;
            write_sse_event(stream, &event_run_finished(&ids))?;
        }
        Err(e) => {
            write_sse_event(stream, &event_run_error(&ids, &e))?;
        }
    }
    Ok(())
}
