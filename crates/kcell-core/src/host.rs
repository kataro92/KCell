use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::audit::{AuditKind, AuditLog};
use crate::binding::{apply_proposal, BindingApplyResult, BindingSet};
use crate::bus::{BusEvent, EventBus};
use crate::discover::{discover_providers, ProviderInfo};
use crate::envelope::Envelope;
use crate::error::{Error, Result};
use crate::execute::InProcessExecutor;
use crate::lifecycle::CellState;
use crate::manifest::{
    load_cell_from_path, validate_binding_proposal, AIManManifest, BindingProposal, CellManifest,
    ProposedBinding, RuntimeKind,
};
use crate::policy::{AdmissionDecision, PolicyGate};
use crate::registry::LocalRegistry;
use crate::subprocess::{SubprocessExecutor, SubprocessSpec};
use crate::wasi::{WasiExecutor, WasiSpec};

/// Host: register, admit, activate Cells; apply bindings; invoke in-process, subprocess, or WASI.
#[derive(Default)]
pub struct Host {
    registry: LocalRegistry,
    policy: PolicyGate,
    bindings: BindingSet,
    inprocess: InProcessExecutor,
    subprocess: SubprocessExecutor,
    wasi: WasiExecutor,
    cell_dirs: BTreeMap<String, PathBuf>,
    /// Map entrypoint token → absolute program (e.g. `kcell` → current CLI binary).
    program_aliases: BTreeMap<String, PathBuf>,
    audit: AuditLog,
    bus: EventBus,
    /// Optional durable state path (serve).
    persist_path: Option<PathBuf>,
    persist_enabled: bool,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("cells", &self.registry.len())
            .field("bindings", &self.bindings.generation())
            .field("inprocess", &self.inprocess.len())
            .field("subprocess", &self.subprocess.len())
            .field("wasi", &self.wasi.len())
            .finish()
    }
}

impl Host {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registry(&self) -> &LocalRegistry {
        &self.registry
    }

    pub fn bindings(&self) -> &BindingSet {
        &self.bindings
    }

    pub fn policy(&self) -> &PolicyGate {
        &self.policy
    }

    pub fn policy_mut(&mut self) -> &mut PolicyGate {
        &mut self.policy
    }

    /// Apply session grants and record audit events.
    pub fn grant_many(
        &mut self,
        process: Vec<String>,
        network: Vec<String>,
        filesystem: Vec<String>,
    ) {
        for g in process {
            self.policy.grant_process(g.clone());
            self.audit
                .record(AuditKind::PolicyGrant, None, format!("process:{g}"));
        }
        for g in network {
            self.policy.grant_network(g.clone());
            self.audit
                .record(AuditKind::PolicyGrant, None, format!("network:{g}"));
        }
        for g in filesystem {
            self.policy.grant_filesystem(g.clone());
            self.audit
                .record(AuditKind::PolicyGrant, None, format!("filesystem:{g}"));
        }
        self.maybe_save_state();
    }

    pub fn executor_mut(&mut self) -> &mut InProcessExecutor {
        &mut self.inprocess
    }

    pub fn subprocess_mut(&mut self) -> &mut SubprocessExecutor {
        &mut self.subprocess
    }

    pub fn wasi_mut(&mut self) -> &mut WasiExecutor {
        &mut self.wasi
    }

    pub fn set_program_alias(&mut self, name: impl Into<String>, path: PathBuf) {
        self.program_aliases.insert(name.into(), path);
    }

    /// Enable/disable durable state writes after mutations.
    pub fn set_persist(&mut self, path: impl Into<PathBuf>, enabled: bool) {
        self.persist_path = Some(path.into());
        self.persist_enabled = enabled;
    }

    pub fn persist_path(&self) -> Option<&Path> {
        self.persist_path.as_deref()
    }

    pub fn export_state(&self) -> crate::state::HostStateFile {
        use crate::state::{HostStateBindings, HostStateFile, HostStatePolicy, HOST_STATE_SCHEMA};
        let mut cell_dirs = BTreeMap::new();
        for (name, path) in &self.cell_dirs {
            cell_dirs.insert(name.clone(), path.display().to_string());
        }
        HostStateFile {
            schema: HOST_STATE_SCHEMA.into(),
            cell_dirs,
            bindings: HostStateBindings {
                generation: self.bindings.generation(),
                items: self.bindings.bindings().to_vec(),
            },
            policy: HostStatePolicy {
                default_allow: self.policy.default_allow,
                granted_network: self.policy.granted_network.clone(),
                granted_process: self.policy.granted_process.clone(),
                granted_filesystem: self.policy.granted_filesystem.clone(),
                granted_secrets: self.policy.granted_secrets.clone(),
                granted_peers: self.policy.granted_peers.clone(),
            },
        }
    }

    pub fn save_state(&self) -> Result<()> {
        if !self.persist_enabled {
            return Ok(());
        }
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        crate::state::save_host_state(path, &self.export_state())
    }

    fn maybe_save_state(&self) {
        if let Err(e) = self.save_state() {
            // Best-effort persist; surface via audit only.
            // (Avoid failing unload/load on disk errors.)
            let _ = e;
        }
    }

    /// Restore hot-loaded cells + bindings + policy grants from durable state.
    pub fn restore_state(&mut self, state: &crate::state::HostStateFile) -> Result<usize> {
        let was_persist = self.persist_enabled;
        self.persist_enabled = false;

        // Merge policy grants (union); OR default_allow.
        self.policy.default_allow |= state.policy.default_allow;
        for g in &state.policy.granted_process {
            self.policy.grant_process(g.clone());
        }
        for g in &state.policy.granted_network {
            self.policy.grant_network(g.clone());
        }
        for g in &state.policy.granted_filesystem {
            self.policy.grant_filesystem(g.clone());
        }
        for g in &state.policy.granted_secrets {
            self.policy.grant_secret(g.clone());
        }
        for g in &state.policy.granted_peers {
            self.policy.grant_peer(g.clone());
        }

        let mut loaded = 0usize;
        let entries: Vec<(String, String)> = state
            .cell_dirs
            .iter()
            .map(|(n, p)| (n.clone(), p.clone()))
            .collect();
        for (name, path) in entries {
            if self.registry.by_name(&name).is_some() {
                continue;
            }
            let p = PathBuf::from(&path);
            if !p.join("cell.yaml").is_file() {
                self.audit.record(
                    AuditKind::InvokeFailed,
                    Some(name.clone()),
                    format!("restore skip missing {}", p.display()),
                );
                continue;
            }
            match self.load_cell_dir(&p, false) {
                Ok(_) => loaded += 1,
                Err(e) => {
                    self.audit.record(
                        AuditKind::InvokeFailed,
                        Some(name),
                        format!("restore load failed: {e}"),
                    );
                }
            }
        }

        if !state.bindings.items.is_empty() {
            let generation = self.bindings.generation() + 1;
            let proposal = BindingProposal {
                api_version: "kcell.dev/v1".into(),
                kind: "BindingProposal".into(),
                metadata: crate::manifest::ProposalMetadata {
                    proposer: "kcell:restore-state".into(),
                    generation,
                    reason: Some("restore durable host state".into()),
                },
                spec: crate::manifest::ProposalSpec {
                    bindings: state
                        .bindings
                        .items
                        .iter()
                        .map(|b| ProposedBinding {
                            consumer: b.consumer.clone(),
                            provider: b.provider.clone(),
                            capability: b.capability.clone(),
                            required: b.required,
                        })
                        .collect(),
                    replace_generation: Some(self.bindings.generation()),
                },
            };
            let _ = self.apply_binding_proposal(proposal)?;
        }
        self.persist_enabled = was_persist;
        self.maybe_save_state();
        Ok(loaded)
    }

    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }

    pub fn bus(&self) -> &EventBus {
        &self.bus
    }

    /// Routable capability providers (optionally filtered).
    pub fn discover(&self, capability: Option<&str>) -> Vec<ProviderInfo> {
        let found = discover_providers(&self.registry, capability);
        // Discovery itself is not audited per-call to keep hot path quiet; use explicit API.
        found
    }

    pub fn register_cell(
        &mut self,
        instance_id: impl Into<String>,
        manifest: CellManifest,
    ) -> Result<()> {
        let name = manifest.metadata.name.clone();
        self.registry.register(instance_id, manifest)?;
        self.audit
            .record(AuditKind::Registered, Some(name.clone()), "cell registered");
        Ok(())
    }

    pub fn register_cell_path(
        &mut self,
        instance_id: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        let path = path.as_ref();
        let cell = load_cell_from_path(path)?;
        let name = cell.metadata.name.clone();
        if let Some(dir) = path.parent() {
            self.cell_dirs.insert(name, dir.to_path_buf());
        }
        self.register_cell(instance_id, cell)
    }

    pub fn admit(&self, name: &str) -> Result<AdmissionDecision> {
        let rec = self
            .registry
            .by_name(name)
            .ok_or_else(|| Error::NotFound(name.into()))?;
        self.policy.admit(&rec.manifest)
    }

    /// Resolve → verify → admit → stage → start → ready → active.
    pub fn activate_cell(&mut self, name: &str) -> Result<CellState> {
        match self.admit(name) {
            Ok(decision) => {
                self.audit.record(
                    AuditKind::Admitted,
                    Some(name.into()),
                    decision.reasons.join("; "),
                );
            }
            Err(e) => {
                self.audit
                    .record(AuditKind::AdmitDenied, Some(name.into()), e.to_string());
                return Err(e);
            }
        }
        let (runtime, entrypoint, artifact, timeout_ms) = {
            let rec = self
                .registry
                .by_name(name)
                .ok_or_else(|| Error::NotFound(name.into()))?;
            (
                rec.manifest.spec.runtime.kind,
                rec.manifest.spec.runtime.entrypoint.clone(),
                rec.manifest.spec.runtime.artifact.clone(),
                rec.manifest.spec.resources.timeout_ms.unwrap_or(5_000),
            )
        };
        {
            let rec = self
                .registry
                .by_name_mut(name)
                .ok_or_else(|| Error::NotFound(name.into()))?;
            rec.lifecycle.activate()?;
        }
        match runtime {
            RuntimeKind::Inprocess => self.inprocess.ensure_passthrough(name),
            RuntimeKind::Subprocess => {
                let program_token = entrypoint.unwrap_or_else(|| "kcell".into());
                let program = self
                    .program_aliases
                    .get(&program_token)
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from(&program_token));
                let args = match artifact {
                    Some(a) if !a.is_empty() => a.split_whitespace().map(str::to_string).collect(),
                    _ => vec!["worker".into()],
                };
                self.subprocess.register(
                    name,
                    SubprocessSpec {
                        program,
                        args,
                        timeout_ms,
                        workdir: self.cell_dirs.get(name).cloned(),
                    },
                );
            }
            RuntimeKind::Wasi => {
                if !WasiExecutor::feature_enabled() {
                    return Err(Error::Validation(
                        "WASI executor requires building with `--features wasi`".into(),
                    ));
                }
                let artifact = artifact.ok_or_else(|| {
                    Error::Validation(format!(
                        "wasi cell `{name}` requires runtime.artifact (.wasm path)"
                    ))
                })?;
                let module_path = match self.cell_dirs.get(name) {
                    Some(dir) => dir.join(&artifact),
                    None => PathBuf::from(&artifact),
                };
                if !module_path.is_file() {
                    return Err(Error::Validation(format!(
                        "wasi artifact not found: {}",
                        module_path.display()
                    )));
                }
                let export = entrypoint.unwrap_or_else(|| "_start".into());
                self.wasi.register(
                    name,
                    WasiSpec {
                        module_path,
                        export,
                        timeout_ms,
                    },
                );
            }
        }
        let state = self.registry.by_name(name).expect("just activated").state();
        let caps: Vec<(String, String)> = self
            .registry
            .by_name(name)
            .map(|rec| {
                rec.manifest
                    .spec
                    .provides
                    .iter()
                    .map(|c| (c.name.clone(), c.version.clone()))
                    .collect()
            })
            .unwrap_or_default();
        self.audit.record(
            AuditKind::Activated,
            Some(name.into()),
            format!("{state:?}"),
        );
        self.bus.publish(BusEvent::CellState {
            cell: name.into(),
            state,
        });
        for (capability, version) in caps {
            self.bus.publish(BusEvent::CapabilityAvailable {
                cell: name.into(),
                capability,
                version,
            });
        }
        Ok(state)
    }

    pub fn stop_cell(&mut self, name: &str) -> Result<CellState> {
        let rec = self
            .registry
            .by_name_mut(name)
            .ok_or_else(|| Error::NotFound(name.into()))?;
        rec.lifecycle.drain_and_stop()?;
        let state = rec.state();
        self.audit
            .record(AuditKind::Stopped, Some(name.into()), format!("{state:?}"));
        self.bus.publish(BusEvent::CellState {
            cell: name.into(),
            state,
        });
        Ok(state)
    }

    /// Load (or replace) a Cell package directory containing `cell.yaml`, then activate it.
    pub fn load_cell_dir(
        &mut self,
        dir: impl AsRef<Path>,
        replace: bool,
    ) -> Result<(String, CellState)> {
        let dir = dir.as_ref();
        let yaml = dir.join("cell.yaml");
        let manifest = load_cell_from_path(&yaml)?;
        let name = manifest.metadata.name.clone();
        let version = manifest.metadata.version.clone();

        if self.registry.by_name(&name).is_some() {
            if !replace {
                return Err(Error::Validation(format!(
                    "cell `{name}` already loaded (pass replace=true to reload)"
                )));
            }
            let _ = self.stop_cell(&name);
            let _ = self.registry.remove_by_name(&name)?;
            self.cell_dirs.remove(&name);
            self.forget_cell_runtime(&name);
            self.detach_bindings_for_cell(&name);
            self.audit
                .record(AuditKind::Stopped, Some(name.clone()), "removed for reload");
        }

        self.cell_dirs.insert(name.clone(), dir.to_path_buf());
        let instance = format!("{name}@{version}");
        self.register_cell(instance, manifest)?;
        let state = self.activate_cell(&name)?;
        self.audit.record(
            AuditKind::Registered,
            Some(name.clone()),
            format!("hot-load from {}", dir.display()),
        );
        self.maybe_save_state();
        Ok((name, state))
    }

    /// Stop and remove a Cell from the registry.
    pub fn unload_cell(&mut self, name: &str) -> Result<CellState> {
        let state = self.stop_cell(name)?;
        let _ = self.registry.remove_by_name(name)?;
        self.cell_dirs.remove(name);
        self.forget_cell_runtime(name);
        self.detach_bindings_for_cell(name);
        self.audit
            .record(AuditKind::Stopped, Some(name.into()), "unloaded");
        self.maybe_save_state();
        Ok(state)
    }

    /// Drop executor registrations for a Cell name.
    pub fn forget_cell_runtime(&mut self, name: &str) {
        self.inprocess.remove(name);
        self.subprocess.remove(name);
        self.wasi.remove(name);
    }

    /// Remove bindings that reference `name` as consumer or provider.
    fn detach_bindings_for_cell(&mut self, name: &str) {
        if self.bindings.retain_without_cell(name) {
            let generation = self.bindings.generation();
            let count = self.bindings.bindings().len();
            self.audit.record(
                AuditKind::BindingApplied,
                Some(name.into()),
                format!("detach bindings generation={generation} count={count}"),
            );
            self.bus
                .publish(BusEvent::BindingChanged { generation, count });
        }
    }

    pub fn apply_binding_proposal_path(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<BindingApplyResult> {
        let proposal = crate::manifest::load_binding_proposal_from_path(path)?;
        self.apply_binding_proposal(proposal)
    }

    pub fn apply_binding_proposal(
        &mut self,
        proposal: BindingProposal,
    ) -> Result<BindingApplyResult> {
        let (next, result) = apply_proposal(&self.registry, &self.bindings, proposal)?;
        match &result {
            BindingApplyResult::Applied { generation } => {
                let count = next.bindings().len();
                self.bindings = next;
                self.audit.record(
                    AuditKind::BindingApplied,
                    None,
                    format!("generation={generation} count={count}"),
                );
                self.bus.publish(BusEvent::BindingChanged {
                    generation: *generation,
                    count,
                });
            }
            BindingApplyResult::Rejected { reason } => {
                self.audit
                    .record(AuditKind::BindingRejected, None, reason.clone());
            }
        }
        if matches!(result, BindingApplyResult::Applied { .. }) {
            self.maybe_save_state();
        }
        Ok(result)
    }

    /// Propose bindings for unmatched requires; optionally apply when the set changes.
    pub fn auto_bind(
        &mut self,
        apply: bool,
        include_optional: bool,
    ) -> Result<(crate::manifest::BindingProposal, Option<BindingApplyResult>)> {
        let proposal = crate::autobind::propose_auto_bindings(
            &self.registry,
            &self.bindings,
            include_optional,
        );
        let mut changed = proposal.spec.bindings.len() != self.bindings.bindings().len();
        if !changed {
            for b in &proposal.spec.bindings {
                match self.bindings.provider_for(&b.consumer, &b.capability) {
                    Some(x) if x.provider == b.provider => {}
                    _ => {
                        changed = true;
                        break;
                    }
                }
            }
        }
        if !apply || !changed {
            return Ok((proposal, None));
        }
        let result = self.apply_binding_proposal(proposal.clone())?;
        Ok((proposal, Some(result)))
    }

    /// Registry + bindings snapshot for auto-config Cells (`binding-propose`).
    pub fn registry_snapshot(&self) -> Value {
        let cells: Vec<Value> = self
            .registry
            .iter()
            .filter(|r| r.state().can_route())
            .map(|r| {
                json!({
                    "name": r.manifest.metadata.name,
                    "provides": r.manifest.spec.provides.iter().map(|c| json!({
                        "name": c.name,
                        "version": c.version,
                    })).collect::<Vec<_>>(),
                    "requires": r.manifest.spec.requires.iter().map(|c| json!({
                        "name": c.name,
                        "version": c.version,
                        "optional": c.optional,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let bindings: Vec<Value> = self
            .bindings
            .bindings()
            .iter()
            .map(|b| {
                json!({
                    "consumer": b.consumer,
                    "provider": b.provider,
                    "capability": b.capability,
                    "required": b.required,
                })
            })
            .collect();
        json!({
            "generation": self.bindings.generation(),
            "cells": cells,
            "bindings": bindings,
        })
    }

    /// Invoke a Cell by name (Host-owned path; bypasses consumer binding lookup).
    pub fn invoke_cell(
        &mut self,
        cell_name: &str,
        capability: &str,
        mut request: Envelope,
    ) -> Result<Envelope> {
        request.capability = capability.into();
        let (can_route, state, runtime, timeout) = {
            let rec = self
                .registry
                .by_name(cell_name)
                .ok_or_else(|| Error::NotFound(cell_name.into()))?;
            (
                rec.state().can_route(),
                rec.state(),
                rec.manifest.spec.runtime.kind,
                rec.manifest.spec.resources.timeout_ms,
            )
        };
        if !can_route {
            let msg = format!("cell `{cell_name}` not routable ({state:?})");
            self.audit
                .record(AuditKind::InvokeFailed, Some(cell_name.into()), msg.clone());
            return Err(Error::Binding(msg));
        }
        if request.timeout_ms.is_none() {
            request.timeout_ms = timeout;
        }
        request.validate()?;

        let result = match runtime {
            RuntimeKind::Inprocess => self.inprocess.invoke(cell_name, &request),
            RuntimeKind::Subprocess => self.subprocess.invoke(cell_name, &request),
            RuntimeKind::Wasi => self.wasi.invoke(cell_name, &request),
        };

        match &result {
            Ok(_) => self.audit.record(
                AuditKind::Invoked,
                Some(cell_name.into()),
                format!("host -[{capability}]-> ok"),
            ),
            Err(e) => self.audit.record(
                AuditKind::InvokeFailed,
                Some(cell_name.into()),
                format!("host -[{capability}]-> {e}"),
            ),
        }
        result
    }

    /// Ask a Cell that provides `binding-propose` for a proposal; Host may apply.
    pub fn propose_from_cell(
        &mut self,
        cell_name: &str,
        apply: bool,
    ) -> Result<(BindingProposal, Option<BindingApplyResult>)> {
        {
            let rec = self
                .registry
                .by_name(cell_name)
                .ok_or_else(|| Error::NotFound(cell_name.into()))?;
            if !rec.state().can_route() {
                return Err(Error::Binding(format!(
                    "cell `{cell_name}` not routable ({:?})",
                    rec.state()
                )));
            }
            let provides = rec
                .manifest
                .spec
                .provides
                .iter()
                .any(|c| c.name == "binding-propose");
            if !provides {
                return Err(Error::Validation(format!(
                    "cell `{cell_name}` does not provide binding-propose"
                )));
            }
        }

        let snapshot = self.registry_snapshot();
        let reply = self.invoke_cell(
            cell_name,
            "binding-propose",
            Envelope::request("binding-propose", snapshot),
        )?;
        let proposal_val =
            reply.payload.get("proposal").cloned().ok_or_else(|| {
                Error::Validation("binding-propose reply missing proposal".into())
            })?;
        let proposal: BindingProposal = serde_json::from_value(proposal_val)?;
        validate_binding_proposal(&proposal)?;

        if !apply {
            return Ok((proposal, None));
        }
        let result = self.apply_binding_proposal(proposal.clone())?;
        Ok((proposal, Some(result)))
    }

    /// Apply static bindings declared on an AI-man as generation `current+1`.
    pub fn apply_aiman_bindings(&mut self, aiman: &AIManManifest) -> Result<BindingApplyResult> {
        if aiman.spec.bindings.is_empty() {
            return Ok(BindingApplyResult::Applied {
                generation: self.bindings.generation(),
            });
        }
        let generation = self.bindings.generation() + 1;
        let proposal = BindingProposal {
            api_version: "kcell.dev/v1".into(),
            kind: "BindingProposal".into(),
            metadata: crate::manifest::ProposalMetadata {
                proposer: format!("aiman:{}", aiman.metadata.name),
                generation,
                reason: Some("static ai-man bindings".into()),
            },
            spec: crate::manifest::ProposalSpec {
                bindings: aiman
                    .spec
                    .bindings
                    .iter()
                    .map(|b| ProposedBinding {
                        consumer: b.consumer.clone(),
                        provider: b.provider.clone(),
                        capability: b.capability.clone(),
                        required: b.required,
                    })
                    .collect(),
                replace_generation: Some(self.bindings.generation()),
            },
        };
        self.apply_binding_proposal(proposal)
    }

    /// Route invoke to the bound provider for `(consumer, capability)`.
    pub fn invoke(
        &mut self,
        consumer: &str,
        capability: &str,
        mut request: Envelope,
    ) -> Result<Envelope> {
        request.capability = capability.into();

        let binding = match self.bindings.provider_for(consumer, capability) {
            Some(b) => b.clone(),
            None => {
                let msg = format!("no binding for consumer `{consumer}` capability `{capability}`");
                self.audit
                    .record(AuditKind::InvokeFailed, Some(consumer.into()), msg.clone());
                return Err(Error::Binding(msg));
            }
        };

        let provider_name = binding.provider.clone();
        let (can_route, state, runtime, timeout) = {
            let provider = match self.registry.by_name(&provider_name) {
                Some(p) => p,
                None => {
                    let msg = format!("provider `{provider_name}` not found");
                    self.audit
                        .record(AuditKind::InvokeFailed, Some(consumer.into()), msg.clone());
                    return Err(Error::NotFound(provider_name));
                }
            };
            (
                provider.state().can_route(),
                provider.state(),
                provider.manifest.spec.runtime.kind,
                provider.manifest.spec.resources.timeout_ms,
            )
        };
        if !can_route {
            let msg = format!("provider `{provider_name}` not routable ({state:?})");
            self.audit
                .record(AuditKind::InvokeFailed, Some(consumer.into()), msg.clone());
            return Err(Error::Binding(msg));
        }

        if request.timeout_ms.is_none() {
            request.timeout_ms = timeout;
        }
        request.validate()?;

        let result = match runtime {
            RuntimeKind::Inprocess => self.inprocess.invoke(&provider_name, &request),
            RuntimeKind::Subprocess => self.subprocess.invoke(&provider_name, &request),
            RuntimeKind::Wasi => self.wasi.invoke(&provider_name, &request),
        };

        match &result {
            Ok(_) => self.audit.record(
                AuditKind::Invoked,
                Some(provider_name),
                format!("{consumer} -[{capability}]-> ok"),
            ),
            Err(e) => self.audit.record(
                AuditKind::InvokeFailed,
                Some(provider_name),
                format!("{consumer} -[{capability}]-> {e}"),
            ),
        }
        result
    }

    /// Load cells listed in an AI-man, activate, then apply static bindings.
    pub fn activate_aiman(
        &mut self,
        aiman: &AIManManifest,
        root: impl AsRef<Path>,
    ) -> Result<Vec<String>> {
        self.policy.default_allow = aiman.spec.policy.default_allow;
        let root = root.as_ref();
        let order = startup_order(aiman)?;
        let mut activated = Vec::new();
        for name in order {
            let cell_ref = aiman
                .spec
                .cells
                .iter()
                .find(|c| c.name == name)
                .expect("name from order");
            let path = root.join(&cell_ref.path).join("cell.yaml");
            let cell_dir = root.join(&cell_ref.path);
            let manifest = load_cell_from_path(&path)?;
            if manifest.metadata.name != cell_ref.name {
                return Err(Error::Validation(format!(
                    "ai-man name `{}` != cell.yaml name `{}`",
                    cell_ref.name, manifest.metadata.name
                )));
            }
            self.cell_dirs.insert(cell_ref.name.clone(), cell_dir);
            let instance = format!("{}@{}", cell_ref.name, manifest.metadata.version);
            if self.registry.by_name(&cell_ref.name).is_none() {
                self.register_cell(instance, manifest)?;
            }
            match self.activate_cell(&cell_ref.name) {
                Ok(_) => activated.push(cell_ref.name.clone()),
                Err(e) if cell_ref.optional => {
                    let _ = e;
                }
                Err(e) => return Err(e),
            }
        }
        let _ = self.apply_aiman_bindings(aiman)?;
        Ok(activated)
    }
}

fn startup_order(aiman: &AIManManifest) -> Result<Vec<String>> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    let names: BTreeSet<_> = aiman.spec.cells.iter().map(|c| c.name.as_str()).collect();
    let mut indeg: BTreeMap<&str, usize> = names.iter().map(|n| (*n, 0usize)).collect();
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for c in &aiman.spec.cells {
        for dep in &c.depends_on {
            if !names.contains(dep.as_str()) {
                return Err(Error::Validation(format!(
                    "cell `{}` depends on unknown `{}`",
                    c.name, dep
                )));
            }
            adj.entry(dep.as_str()).or_default().push(c.name.as_str());
            *indeg.entry(c.name.as_str()).or_default() += 1;
        }
    }
    let mut q: VecDeque<&str> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| *n)
        .collect();
    let mut zeros: Vec<_> = q.drain(..).collect();
    zeros.sort_unstable();
    q.extend(zeros);

    let mut out = Vec::new();
    while let Some(n) = q.pop_front() {
        out.push(n.to_string());
        if let Some(children) = adj.get(n) {
            let mut next_ready = Vec::new();
            for child in children {
                let e = indeg.get_mut(child).expect("indeg");
                *e -= 1;
                if *e == 0 {
                    next_ready.push(*child);
                }
            }
            next_ready.sort_unstable();
            q.extend(next_ready);
        }
    }
    if out.len() != names.len() {
        return Err(Error::Validation(
            "ai-man cell dependsOn graph has a cycle".into(),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{
        Capability, CellMetadata, CellSpec, Communication, Permissions, Requirement, RestartPolicy,
        RuntimeSpec,
    };
    use serde_json::json;

    fn cell(name: &str, provides: &[&str], requires: &[&str]) -> CellManifest {
        CellManifest {
            api_version: "kcell.dev/v1".into(),
            kind: "Cell".into(),
            metadata: CellMetadata {
                name: name.into(),
                version: "0.1.0".into(),
                description: None,
            },
            spec: CellSpec {
                runtime: RuntimeSpec {
                    kind: RuntimeKind::Inprocess,
                    entrypoint: Some("main".into()),
                    artifact: None,
                },
                provides: provides
                    .iter()
                    .map(|n| Capability {
                        name: (*n).into(),
                        version: "1".into(),
                        contract: None,
                    })
                    .collect(),
                requires: requires
                    .iter()
                    .map(|n| Requirement {
                        name: (*n).into(),
                        version: "1".into(),
                        optional: false,
                        contract: None,
                    })
                    .collect(),
                communication: Communication {
                    active: true,
                    passive: true,
                },
                ports: vec![],
                resources: Default::default(),
                permissions: Permissions::default(),
                health: Default::default(),
                restart_policy: RestartPolicy::OnFailure,
            },
        }
    }

    #[test]
    fn wasi_activate_gated_or_needs_artifact() {
        let mut host = Host::new();
        let mut c = cell("wasi-echo", &["echo"], &[]);
        c.spec.runtime.kind = RuntimeKind::Wasi;
        c.spec.runtime.artifact = Some("missing.wasm".into());
        host.register_cell("1", c).unwrap();
        let err = host.activate_cell("wasi-echo").unwrap_err().to_string();
        assert!(
            err.contains("features wasi") || err.contains("artifact not found"),
            "{err}"
        );
    }

    #[test]
    fn invoke_via_binding() {
        let mut host = Host::new();
        host.register_cell("1", cell("web", &[], &["echo"]))
            .unwrap();
        host.register_cell("2", cell("echo-cell", &["echo"], &[]))
            .unwrap();
        host.activate_cell("echo-cell").unwrap();
        host.activate_cell("web").unwrap();
        let proposal: BindingProposal = serde_yaml::from_str(
            r#"
apiVersion: kcell.dev/v1
kind: BindingProposal
metadata:
  proposer: test
  generation: 1
spec:
  bindings:
    - consumer: web
      provider: echo-cell
      capability: echo
"#,
        )
        .unwrap();
        host.apply_binding_proposal(proposal).unwrap();
        let reply = host
            .invoke("web", "echo", Envelope::request("echo", json!({"n": 1})))
            .unwrap();
        assert_eq!(reply.payload["cell"], "echo-cell");
        assert_eq!(reply.payload["echo"]["n"], 1);
    }

    #[test]
    fn unload_clears_executor_and_bindings() {
        let mut host = Host::new();
        host.register_cell("1", cell("web", &[], &["echo"]))
            .unwrap();
        host.register_cell("2", cell("echo-cell", &["echo"], &[]))
            .unwrap();
        host.activate_cell("echo-cell").unwrap();
        host.activate_cell("web").unwrap();
        assert_eq!(host.executor_mut().len(), 2);
        let proposal: BindingProposal = serde_yaml::from_str(
            r#"
apiVersion: kcell.dev/v1
kind: BindingProposal
metadata:
  proposer: test
  generation: 1
spec:
  bindings:
    - consumer: web
      provider: echo-cell
      capability: echo
"#,
        )
        .unwrap();
        host.apply_binding_proposal(proposal).unwrap();
        assert_eq!(host.bindings().bindings().len(), 1);

        host.unload_cell("echo-cell").unwrap();
        assert_eq!(host.executor_mut().len(), 1);
        assert!(host.bindings().bindings().is_empty());
        assert!(host.registry().by_name("echo-cell").is_none());
    }

    #[test]
    fn state_export_restore_roundtrip() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cell_dir = root.join("cells/echo-cell");
        let tmp = std::env::temp_dir().join(format!("kcell-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let state_path = tmp.join("host-state.json");

        let mut host = Host::new();
        host.set_persist(&state_path, true);
        host.register_cell("1", cell("web", &[], &["echo"]))
            .unwrap();
        host.activate_cell("web").unwrap();
        host.load_cell_dir(&cell_dir, false).unwrap();
        let proposal: BindingProposal = serde_yaml::from_str(
            r#"
apiVersion: kcell.dev/v1
kind: BindingProposal
metadata:
  proposer: test
  generation: 1
spec:
  bindings:
    - consumer: web
      provider: echo-cell
      capability: echo
"#,
        )
        .unwrap();
        host.apply_binding_proposal(proposal).unwrap();
        assert!(state_path.is_file());

        let saved = crate::state::load_host_state(&state_path).unwrap();
        let mut host2 = Host::new();
        host2
            .register_cell("1", cell("web", &[], &["echo"]))
            .unwrap();
        host2.activate_cell("web").unwrap();
        let loaded = host2.restore_state(&saved).unwrap();
        assert!(loaded >= 1);
        assert!(host2.registry().by_name("echo-cell").is_some());
        assert!(host2.bindings().provider_for("web", "echo").is_some());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn policy_grants_persist_roundtrip() {
        let mut host = Host::new();
        host.grant_many(
            vec!["bin/tool".into()],
            vec!["https://api.example".into()],
            vec![],
        );
        let exported = host.export_state();
        assert!(exported
            .policy
            .granted_process
            .iter()
            .any(|g| g == "bin/tool"));
        assert!(exported
            .policy
            .granted_network
            .iter()
            .any(|g| g == "https://api.example"));

        let mut host2 = Host::new();
        // CLI grant already present — restore must merge, not wipe.
        host2.grant_many(vec!["local-cli".into()], vec![], vec![]);
        host2.restore_state(&exported).unwrap();

        assert!(host2.policy.granted_process.iter().any(|g| g == "bin/tool"));
        assert!(host2
            .policy
            .granted_process
            .iter()
            .any(|g| g == "local-cli"));
        assert!(host2
            .policy
            .granted_network
            .iter()
            .any(|g| g == "https://api.example"));

        let mut needs = cell("needs-grants", &[], &[]);
        needs.spec.permissions.network = vec!["https://api.example".into()];
        needs.spec.permissions.process = vec!["bin/tool".into()];
        let decision = host2.policy.admit(&needs).unwrap();
        assert!(decision.admitted);

        // Without restore, a fresh host would deny.
        let bare = Host::new();
        assert!(bare.policy.admit(&needs).is_err());
    }

    #[test]
    fn grant_many_writes_state_when_persist_enabled() {
        let tmp = std::env::temp_dir().join(format!("kcell-grants-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let state_path = tmp.join("host-state.json");

        let mut host = Host::new();
        host.set_persist(&state_path, true);
        host.grant_many(vec!["proc-a".into()], vec!["net-a".into()], vec![]);
        assert!(state_path.is_file());
        let saved = crate::state::load_host_state(&state_path).unwrap();
        assert!(saved.policy.granted_process.iter().any(|g| g == "proc-a"));
        assert!(saved.policy.granted_network.iter().any(|g| g == "net-a"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
