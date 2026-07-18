use super::emission_index::*;
use super::identity::*;
use super::lowering_records::*;
use super::prepared_emission::PreparedEmission;
use super::prepared_emission::record_prepare_emission_work;
use super::semantic_records::*;
use super::specialization_context::*;
use crate::HashMap;
use std::sync::Arc;
use veryl_parser::resource_table::PathId;

#[derive(Debug, Default)]
pub struct NestedModportAnalysis {
    pub(super) availability: HashMap<NestedModportLoweringKey, LoweringAvailability>,
    pub(super) rewrites: HashMap<OccurrenceRewriteKey, Arc<ResolvedPathRewrite>>,
    pub(super) expanded_ports: HashMap<ExpandedPortKey, Arc<ExpandedPortResolution>>,
    #[cfg(any(test, feature = "nested-modport-test-utils"))]
    pub(super) instantiation_contexts:
        HashMap<InstantiationContextKey, EmissionSpecializationContext>,
    pub(super) emission_index: FrozenEmissionIndex,
}

impl NestedModportAnalysis {
    pub fn lowering(&self, key: &NestedModportLoweringKey) -> Option<&Arc<NestedModportLowering>> {
        match self.availability.get(key) {
            Some(LoweringAvailability::Found(lowering)) => Some(lowering),
            Some(LoweringAvailability::NotNested) | None => None,
        }
    }

    pub fn rewrite(&self, key: &OccurrenceRewriteKey) -> Option<&ResolvedPathRewrite> {
        self.rewrites.get(key).map(Arc::as_ref)
    }

    pub fn expanded_port(&self, key: &ExpandedPortKey) -> Option<&ExpandedPortResolution> {
        self.expanded_ports.get(key).map(Arc::as_ref)
    }

    #[cfg(test)]
    pub(crate) fn rewrite_keys(&self) -> impl Iterator<Item = &OccurrenceRewriteKey> {
        self.rewrites.keys()
    }

    #[cfg(test)]
    pub(crate) fn expanded_port_keys(&self) -> impl Iterator<Item = &ExpandedPortKey> {
        self.expanded_ports.keys()
    }

    pub fn prepare_emission(
        &self,
        source: PathId,
        phase: EmissionPhase,
    ) -> Result<PreparedEmission<'_>, super::errors::NestedModportAnalysisInvariant> {
        let table = self.emission_index.source(source);
        record_prepare_emission_work(1 + table.map_or(0, |table| table.bindings.len()));
        Ok(PreparedEmission {
            analysis: self,
            phase,
            table,
            cursor: 0,
            scope_cursors: HashMap::default(),
            consumed: vec![false; table.map_or(0, |table| table.bindings.len())],
        })
    }
}
