use super::analysis_snapshot::NestedModportAnalysis;
use super::emission_index::*;
use super::emission_scope::*;
use super::identity::*;
use super::semantic_work::record_semantic_work;
use super::specialization_context::*;
use crate::symbol::GenericMap;
use veryl_parser::resource_table::StrId;

pub struct EmissionOwnerBatch<'a> {
    pub(super) analysis: &'a NestedModportAnalysis,
    pub(super) bindings: Vec<&'a EmissionOwnerBinding>,
}

impl<'a> EmissionOwnerBatch<'a> {
    pub fn iter(&self) -> EmissionFrameIterator<'_, 'a> {
        EmissionFrameIterator {
            analysis: self.analysis,
            bindings: self.bindings.iter(),
        }
    }
}

pub struct EmissionFrameIterator<'batch, 'analysis> {
    analysis: &'analysis NestedModportAnalysis,
    bindings: std::slice::Iter<'batch, &'analysis EmissionOwnerBinding>,
}

impl<'analysis> Iterator for EmissionFrameIterator<'_, 'analysis> {
    type Item = EmissionFrame<'analysis>;

    fn next(&mut self) -> Option<Self::Item> {
        record_semantic_work(1);
        self.bindings.next().copied().map(|binding| {
            record_semantic_work(1);
            EmissionFrame {
                analysis: self.analysis,
                binding,
            }
        })
    }
}

#[derive(Clone, Copy)]
pub struct EmissionFrame<'a> {
    pub(super) analysis: &'a NestedModportAnalysis,
    pub(super) binding: &'a EmissionOwnerBinding,
}

impl<'a> EmissionFrame<'a> {
    pub fn id(self) -> EmissionBindingId {
        self.binding.id
    }

    pub fn specialization(self) -> &'a NestedModportLoweringKey {
        &self.binding.specialization
    }

    pub fn matches_enclosing_frame(self, enclosing: EmissionFrame<'_>) -> bool {
        self.binding.enclosing_owner_handle == Some(enclosing.binding.owner_handle)
    }

    pub fn scope_handle(self) -> EmissionScopeHandle {
        EmissionScopeHandle(self.binding.package_scope.clone().unwrap_or_else(|| {
            EmissionScopeIdentity {
                session: self.binding.specialization.session,
                source: self.binding.source,
                declaration: self.binding.declaration,
                symbol: self.binding.specialization.specialization.owner.symbol,
                semantic_path: self
                    .binding
                    .specialization
                    .specialization
                    .owner
                    .full_path
                    .clone(),
                generic_map: SemanticGenericMap::from(&self.binding.emission_context.generic_map),
            }
        }))
    }

    pub fn matches_scope(self, scope: &EmissionScopeHandle) -> bool {
        self.binding.package_scope.as_ref() == Some(&scope.0)
    }

    pub fn has_enclosing_frame(self) -> bool {
        self.binding.enclosing_owner.is_some()
    }

    pub fn emission_context(self) -> &'a EmissionSpecializationContext {
        &self.binding.emission_context
    }

    pub fn connected_generic_map(self, formal_port: StrId) -> Option<&'a GenericMap> {
        self.binding
            .emission_context
            .connected_generic_map(formal_port)
    }

    pub fn lowering(self) -> &'a LoweringAvailability {
        &self.binding.lowering
    }
}
