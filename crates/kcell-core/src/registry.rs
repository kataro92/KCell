use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::lifecycle::{CellState, Lifecycle};
use crate::manifest::CellManifest;

#[derive(Debug, Clone)]
pub struct CellRecord {
    pub instance_id: String,
    pub manifest: CellManifest,
    pub lifecycle: Lifecycle,
}

impl CellRecord {
    pub fn state(&self) -> CellState {
        self.lifecycle.state()
    }

    pub fn provides(&self) -> impl Iterator<Item = (&str, &str)> {
        self.manifest
            .spec
            .provides
            .iter()
            .map(|c| (c.name.as_str(), c.version.as_str()))
    }
}

#[derive(Debug, Default)]
pub struct LocalRegistry {
    by_instance: BTreeMap<String, CellRecord>,
    by_name: BTreeMap<String, String>,
}

impl LocalRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        instance_id: impl Into<String>,
        manifest: CellManifest,
    ) -> Result<()> {
        let instance_id = instance_id.into();
        let name = manifest.metadata.name.clone();
        if self.by_instance.contains_key(&instance_id) {
            return Err(Error::Validation(format!(
                "instance `{instance_id}` exists"
            )));
        }
        if self.by_name.contains_key(&name) {
            return Err(Error::Validation(format!(
                "cell name `{name}` already registered"
            )));
        }
        let lifecycle = Lifecycle::new(name.clone());
        self.by_name.insert(name, instance_id.clone());
        self.by_instance.insert(
            instance_id.clone(),
            CellRecord {
                instance_id,
                manifest,
                lifecycle,
            },
        );
        Ok(())
    }

    pub fn get(&self, instance_id: &str) -> Option<&CellRecord> {
        self.by_instance.get(instance_id)
    }

    pub fn get_mut(&mut self, instance_id: &str) -> Option<&mut CellRecord> {
        self.by_instance.get_mut(instance_id)
    }

    pub fn by_name(&self, name: &str) -> Option<&CellRecord> {
        self.by_name
            .get(name)
            .and_then(|id| self.by_instance.get(id))
    }

    pub fn by_name_mut(&mut self, name: &str) -> Option<&mut CellRecord> {
        let id = self.by_name.get(name)?.clone();
        self.by_instance.get_mut(&id)
    }

    pub fn find_providers(&self, capability: &str, version: Option<&str>) -> Vec<&CellRecord> {
        self.by_instance
            .values()
            .filter(|r| {
                r.state().can_route()
                    && r.manifest.spec.provides.iter().any(|p| {
                        p.name == capability && version.map(|v| v == p.version).unwrap_or(true)
                    })
            })
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &CellRecord> {
        self.by_instance.values()
    }

    pub fn len(&self) -> usize {
        self.by_instance.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_instance.is_empty()
    }

    pub fn remove(&mut self, instance_id: &str) -> Result<CellRecord> {
        let rec = self
            .by_instance
            .remove(instance_id)
            .ok_or_else(|| Error::NotFound(instance_id.into()))?;
        self.by_name.remove(&rec.manifest.metadata.name);
        Ok(rec)
    }

    pub fn remove_by_name(&mut self, name: &str) -> Result<CellRecord> {
        let id = self
            .by_name
            .get(name)
            .cloned()
            .ok_or_else(|| Error::NotFound(name.into()))?;
        self.remove(&id)
    }
}
