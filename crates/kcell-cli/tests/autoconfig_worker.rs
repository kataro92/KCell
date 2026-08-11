//! Integration: auto-config Cell proposes bindings; Host applies.

use std::path::PathBuf;

use kcell_core::{Envelope, Host};
use serde_json::json;

#[test]
fn autoconfig_propose_and_apply() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_kcell"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let mut host = Host::new();
    host.set_program_alias("kcell", bin);
    let aiman =
        kcell_core::load_aiman_from_path(root.join("examples/echo-autoconfig-aiman/ai-man.yaml"))
            .expect("load aiman");
    let activated = host.activate_aiman(&aiman, &root).expect("activate");
    assert!(activated.contains(&"auto-config-cell".to_string()));
    assert_eq!(host.bindings().bindings().len(), 0);

    let (proposal, applied) = host
        .propose_from_cell("auto-config-cell", true)
        .expect("propose_from_cell");
    assert_eq!(proposal.metadata.proposer, "cell:auto-config-cell");
    assert!(applied.is_some());
    assert!(host
        .bindings()
        .provider_for("caller-cell", "echo")
        .is_some());

    let reply = host
        .invoke(
            "caller-cell",
            "echo",
            Envelope::request("echo", json!({"via": "autoconfig"})),
        )
        .expect("invoke after propose");
    assert_eq!(reply.payload["echo"]["via"], "autoconfig");
}
