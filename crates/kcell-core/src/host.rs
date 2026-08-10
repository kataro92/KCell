use crate::binding::{apply_proposal, BindingApplyResult, BindingSet};
use crate::error::{Error, Result};
use crate::lifecycle::CellState;
use crate::manifest::{load_cell_from_path, AIManManifest, BindingProposal, CellManifest};
use crate::policy::{AdmissionDecision, PolicyGate};
use crate::registry::LocalRegistry;

/// In-process host: register, admit, activate Cells; apply binding proposals.
#[derive(Debug, Default)]
pub struct Host {
    registry: LocalRegistry,
    policy: PolicyGate,
    bindings: BindingSet,
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

    pub fn policy_mut(&mut self) -> &mut PolicyGate {
        &mut self.policy
    }

    pub fn register_cell(&mut self, instance_id: impl Into<String>, manifest: CellManifest) -> Result<()> {
        self.registry.register(instance_id, manifest)
    }

    pub fn register_cell_path(
        &mut self,
        instance_id: impl Into<String>,
        path: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        let cell = load_cell_from_path(path)?;
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
        self.admit(name)?;
        let rec = self
            .registry
            .by_name_mut(name)
            .ok_or_else(|| Error::NotFound(name.into()))?;
        rec.lifecycle.activate()?;
        Ok(rec.state())
    }

    pub fn stop_cell(&mut self, name: &str) -> Result<CellState> {
        let rec = self
            .registry
            .by_name_mut(name)
            .ok_or_else(|| Error::NotFound(name.into()))?;
        rec.lifecycle.drain_and_stop()?;
        Ok(rec.state())
    }

    pub fn apply_binding_proposal(&mut self, proposal: BindingProposal) -> Result<BindingApplyResult> {
        let (next, result) = apply_proposal(&self.registry, &self.bindings, proposal)?;
        if matches!(result, BindingApplyResult::Applied { .. }) {
            self.bindings = next;
        }
        Ok(result)
    }

    /// Load cells listed in an AI-man, register them, activate required ones.
    pub fn activate_aiman(&mut self, aiman: &AIManManifest, root: impl AsRef<std::path::Path>) -> Result<Vec<String>> {
        let root = root.as_ref();
        let mut activated = Vec::new();
        for cell_ref in &aiman.spec.cells {
            let path = root.join(&cell_ref.path).join("cell.yaml");
            let manifest = load_cell_from_path(&path)?;
            if manifest.metadata.name != cell_ref.name {
                return Err(Error::Validation(format!(
                    "ai-man name `{}` != cell.yaml name `{}`",
                    cell_ref.name, manifest.metadata.name
                )));
            }
            let instance = format!("{}@{}", cell_ref.name, manifest.metadata.version);
            if self.registry.by_name(&cell_ref.name).is_none() {
                self.register_cell(instance, manifest)?;
            }
            match self.activate_cell(&cell_ref.name) {
                Ok(_) => activated.push(cell_ref.name.clone()),
                Err(e) if cell_ref.optional => {
                    // Optional cells may fail without aborting the AI-man.
                    let _ = e;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(activated)
    }
}
