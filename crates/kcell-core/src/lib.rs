//! KCell core — mechanisms only.

mod audit;
mod autobind;
mod binding;
mod bus;
mod control;
mod discover;
mod envelope;
mod error;
mod execute;
mod host;
mod lifecycle;
mod manifest;
mod package;
mod policy;
mod registry;
mod specialize;
mod state;
mod subprocess;
mod wasi;
mod watch;

pub use audit::{AuditEvent, AuditKind, AuditLog};
pub use autobind::propose_auto_bindings;
pub use binding::{
    apply_proposal, validate_proposal, ActiveBinding, BindingApplyResult, BindingSet,
};
pub use bus::{BusEvent, EventBus};
pub use control::{
    call_unix, handle_control, serve_unix, serve_unix_with_watch, ControlRequest, ControlResponse,
    CONTROL_SCHEMA,
};
pub use discover::{discover_providers, ProviderInfo};
pub use envelope::{Envelope, ENVELOPE_SCHEMA};
pub use error::{Error, Result};
pub use execute::{CellExecutor, CellHandler, InProcessExecutor, PassthroughHandler};
pub use host::Host;
pub use lifecycle::{CellState, Lifecycle, TransitionEvent};
pub use manifest::{
    load_aiman_from_path, load_binding_proposal_from_path, load_cell_from_path, validate_aiman,
    validate_binding_proposal, validate_cell, AIManManifest, BindingProposal, Capability,
    CellManifest, CellMetadata, CellSpec, Communication, Permissions, ProposedBinding,
    Requirement, RestartPolicy, RuntimeKind, RuntimeSpec, StaticBinding,
};
pub use package::{build_cell_dir, package_from_manifest, CellPackageMeta};
pub use policy::{AdmissionDecision, PolicyGate};
pub use registry::{CellRecord, LocalRegistry};
pub use specialize::{
    default_stem_dir, parse_cap_token, specialize, SpecializeRequest, SpecializeResult,
};
pub use state::{
    default_state_path, load_host_state, save_host_state, HostStateBindings, HostStateFile,
    HostStatePolicy, HOST_STATE_SCHEMA,
};
pub use subprocess::{SubprocessExecutor, SubprocessSpec};
pub use wasi::{WasiExecutor, WasiSpec};
pub use watch::{
    apply_watch_actions, diff_watch, notify_feature_enabled, scan_watch_roots, WatchAction,
    WatchConfig, WatchedCell,
};
