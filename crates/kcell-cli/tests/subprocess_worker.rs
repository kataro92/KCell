//! Integration: Host spawns `kcell worker` as a subprocess Cell.

use std::path::PathBuf;

use kcell_core::{Envelope, Host};
use serde_json::json;

#[test]
fn subprocess_worker_invoke() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_kcell"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let mut host = Host::new();
    host.set_program_alias("kcell", bin);
    let aiman = kcell_core::load_aiman_from_path(root.join("examples/echo-sub-aiman/ai-man.yaml"))
        .expect("load aiman");
    let activated = host.activate_aiman(&aiman, &root).expect("activate");
    assert!(activated.contains(&"echo-sub-cell".to_string()));

    let reply = host
        .invoke(
            "caller-sub-cell",
            "echo-sub",
            Envelope::request("echo-sub", json!({"hello": "sub"})),
        )
        .expect("invoke");
    assert_eq!(reply.payload["runtime"], "subprocess");
    assert_eq!(reply.payload["echo"]["hello"], "sub");
}
