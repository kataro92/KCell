//! KCell core — mechanisms only: manifests, lifecycle, registry, binding, policy.

mod binding;
mod error;
mod host;
mod lifecycle;
mod manifest;
mod policy;
mod registry;

pub use binding::{apply_proposal, validate_proposal, BindingApplyResult, BindingSet};
pub use error::{Error, Result};
pub use host::Host;
pub use lifecycle::{CellState, Lifecycle, TransitionEvent};
pub use manifest::{
    load_aiman_from_path, load_binding_proposal_from_path, load_cell_from_path, validate_aiman,
    validate_binding_proposal, validate_cell, AIManManifest, BindingProposal, Capability,
    CellManifest, CellMetadata, CellSpec, Communication, Permissions, ProposedBinding,
    Requirement, RestartPolicy, RuntimeKind, RuntimeSpec,
};
pub use policy::{AdmissionDecision, PolicyGate};
pub use registry::{CellRecord, LocalRegistry};
