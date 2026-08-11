mod bridge;
mod card;
mod http;
mod rpc;

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use serde_json::{json, Value};

use bridge::Bridge;
use card::{build_agent_card, CardConfig};
use http::{read_request, write_json, write_plain};
use rpc::handle_rpc;

#[derive(Parser, Debug)]
#[command(
    name = "kcell-a2a",
    version,
    about = "KCell A2A adapter — Agent Card + JSON-RPC over Host control socket"
)]
struct Cli {
    /// Path to `kcell serve` Unix control socket
    #[arg(long, default_value = ".kcell/kcell.sock")]
    socket: PathBuf,
    /// Consumer Cell used for invoke
    #[arg(long)]
    consumer: String,
    /// Capability to invoke on message/send
    #[arg(long)]
    capability: String,
    /// HTTP bind address
    #[arg(long, default_value = "127.0.0.1:3457")]
    bind: String,
    /// Public URL advertised in Agent Card (default: http://{bind})
    #[arg(long)]
    public_url: Option<String>,
    /// Agent Card name
    #[arg(long, default_value = "kcell-a2a")]
    name: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kcell-a2a error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let public_url = cli
        .public_url
        .unwrap_or_else(|| format!("http://{}", cli.bind));
    let bridge = Bridge {
        socket: cli.socket,
        consumer: cli.consumer,
        capability: cli.capability,
    };
    let agent_name = cli.name;

    let listener = TcpListener::bind(&cli.bind)?;
    eprintln!(
        "kcell-a2a listening on {} (socket={}, consumer={}, capability={})",
        public_url,
        bridge.socket.display(),
        bridge.consumer,
        bridge.capability
    );

    for conn in listener.incoming() {
        let mut stream = match conn {
            Ok(s) => s,
            Err(e) => {
                eprintln!("kcell-a2a accept error: {e}");
                continue;
            }
        };
        if let Err(e) = handle_connection(&mut stream, &bridge, &agent_name, &public_url) {
            eprintln!("kcell-a2a request error: {e}");
            let _ = write_plain(&mut stream, 500, "Internal Server Error", &e);
        }
    }
    Ok(())
}

fn handle_connection(
    stream: &mut std::net::TcpStream,
    bridge: &Bridge,
    agent_name: &str,
    public_url: &str,
) -> Result<(), String> {
    let req = read_request(stream)?;
    let path = req.path.split('?').next().unwrap_or(req.path.as_str());

    match (req.method.as_str(), path) {
        ("GET", "/health") => write_json(stream, 200, "OK", &json!({"ok": true})),
        ("GET", "/.well-known/agent-card.json") | ("GET", "/.well-known/agent.json") => {
            let providers = bridge.discover_providers().unwrap_or(Value::Array(vec![]));
            let card = build_agent_card(
                &CardConfig {
                    name: agent_name,
                    description: "KCell A2A adapter — Host capabilities via control socket",
                    url: public_url,
                    version: env!("CARGO_PKG_VERSION"),
                },
                &providers,
            );
            write_json(stream, 200, "OK", &card)
        }
        ("POST", "/") | ("POST", "/rpc") | ("POST", "/a2a") => {
            let resp = handle_rpc(bridge, &req.body)?;
            write_json(stream, 200, "OK", &resp)
        }
        _ => write_plain(stream, 404, "Not Found", "not found"),
    }
}
