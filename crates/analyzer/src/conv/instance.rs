use crate::HashMap;
use crate::conv::context::Config;
use crate::ir::{Component, Signature};
use crate::nested_modport::ComponentCacheKey;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct InstanceHistory {
    pub hierarchy: Vec<Signature>,
    /// `Arc`-wrapped so repeated `get` hands out references instead of
    /// deep-cloning the component tree — matters on testbench-heavy designs.
    full: HashMap<ComponentCacheKey, Option<(Arc<Component>, bool)>>,
}

impl InstanceHistory {
    pub fn get(&self, key: &ComponentCacheKey) -> Option<(Arc<Component>, bool)> {
        self.full.get(key).cloned().flatten()
    }

    pub fn set(&mut self, key: &ComponentCacheKey, component: Arc<Component>, in_generic: bool) {
        if let Some(x) = self.full.get_mut(key) {
            *x = Some((component, in_generic));
        }
    }

    pub fn remove(&mut self, key: &ComponentCacheKey) {
        self.full.remove(key);
    }

    pub fn get_current_signature(&self) -> Option<&Signature> {
        self.hierarchy.last()
    }

    pub fn push(
        &mut self,
        mut sig: Signature,
        key: ComponentCacheKey,
        config: &Config,
    ) -> Result<bool, InstanceHistoryError> {
        sig.normalize();
        if self.hierarchy.len() > config.instance_depth_limit {
            return Err(InstanceHistoryError::ExceedDepthLimit(self.hierarchy.len()));
        }
        if self.full.len() > config.instance_total_limit {
            return Err(InstanceHistoryError::ExceedTotalLimit(self.full.len()));
        }
        if self.hierarchy.contains(&sig) {
            return Err(InstanceHistoryError::InfiniteRecursion);
        }
        if self.full.contains_key(&key) {
            Ok(false)
        } else {
            self.hierarchy.push(sig);
            self.full.insert(key, None);
            Ok(true)
        }
    }

    pub fn pop(&mut self) {
        self.hierarchy.pop();
    }

    pub fn clear_hierarchy(&mut self) {
        self.hierarchy.clear();
    }

    pub fn clear(&mut self) {
        self.hierarchy.clear();
        self.full.clear();
    }
}

#[cfg(test)]
#[path = "instance_test.rs"]
mod tests;

#[derive(Debug, Eq, PartialEq)]
pub enum InstanceHistoryError {
    ExceedDepthLimit(usize),
    ExceedTotalLimit(usize),
    InfiniteRecursion,
}
