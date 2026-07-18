use super::binding_specialization_identity::{
    BindingSpecializationIdentity, BindingSpecializationInterner,
};
use super::collection_work::record_collection_work;
use super::emission_index::*;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::*;
use super::lowering_records::*;
use super::semantic_records::*;
use super::specialization_context::{EmissionBindingId, LoweringAvailability};
use crate::{HashMap, HashSet};
use std::sync::Arc;
use veryl_parser::token_range::TokenRange;
#[derive(Debug, Default)]
pub struct PendingNestedModportAnalysis {
    pub(super) interface_lowerings: HashMap<NestedModportLoweringKey, Arc<NestedModportLowering>>,
    pub(super) lowerings: HashMap<NestedModportLoweringKey, LoweringAvailability>,
    pub(super) rewrites: HashMap<OccurrenceRewriteKey, Arc<ResolvedPathRewrite>>,
    pub(super) rewrite_candidates: HashMap<OccurrenceRewriteKey, PendingPathRewriteCandidate>,
    pub(super) optional_rewrite_candidates: HashMap<OccurrenceRewriteKey, TokenRange>,
    pub(super) rewrite_order: Vec<OccurrenceRewriteKey>,
    pub(super) rewrite_order_by_owner: HashMap<NestedModportLoweringKey, Vec<OccurrenceRewriteKey>>,
    pub(super) expanded_ports: HashMap<ExpandedPortKey, Arc<ExpandedPortResolution>>,
    pub(super) expanded_port_candidates: HashMap<ExpandedPortKey, PendingExpandedPortCandidate>,
    pub(super) expanded_port_order: Vec<ExpandedPortKey>,
    pub(super) expanded_port_order_by_owner:
        HashMap<NestedModportLoweringKey, Vec<ExpandedPortKey>>,
    pub(super) instantiation_context_candidates:
        HashMap<InstantiationContextKey, PendingInstantiationContextCandidate>,
    pub(super) generic_emission_owners: Vec<PendingGenericEmissionOwner>,
    pub(super) generic_emission_owner_set: HashSet<PendingGenericEmissionOwner>,
    pub(super) emission_bindings: Vec<PendingEmissionBinding>,
    pub(super) binding_specialization_interner: BindingSpecializationInterner,
    pub(super) emission_binding_records: HashMap<EmissionBindingRecordKey, usize>,
    pub(super) emission_binding_specializations: HashMap<BindingSpecializationIdentity, usize>,
    pub(super) emission_binding_declarations: HashSet<EmissionDeclarationKey>,
    pub(super) emission_binding_scopes: HashSet<EmissionDeclarationScopeKey>,
    pub(super) emission_declaration_specializations:
        HashSet<(EmissionDeclarationKey, BindingSpecializationIdentity)>,
    pub(super) max_emission_binding_id: Option<u32>,
    pub(super) inferred_binding_ids: HashSet<EmissionBindingId>,
    pub(super) recorded_sessions: HashSet<AnalysisSessionId>,
    pub(super) finalized: bool,
}

impl PendingNestedModportAnalysis {
    pub(super) fn record_session(&mut self, session: AnalysisSessionId) {
        record_collection_work(1);
        self.recorded_sessions.insert(session);
    }

    pub fn record_interface_lowering(
        &mut self,
        key: NestedModportLoweringKey,
        lowering: Arc<NestedModportLowering>,
    ) -> Arc<NestedModportLowering> {
        self.record_session(key.session);
        record_collection_work(1);
        Arc::clone(self.interface_lowerings.entry(key).or_insert(lowering))
    }

    pub fn interface_lowering(
        &self,
        key: &NestedModportLoweringKey,
    ) -> Option<&Arc<NestedModportLowering>> {
        self.interface_lowerings.get(key)
    }

    pub fn record_lowering(
        &mut self,
        key: NestedModportLoweringKey,
        availability: LoweringAvailability,
    ) {
        self.record_session(key.session);
        self.lowerings.insert(key, availability);
        record_collection_work(1);
    }

    pub fn lowering(&self, key: &NestedModportLoweringKey) -> Option<&LoweringAvailability> {
        self.lowerings.get(key)
    }

    pub fn record_rewrite(
        &mut self,
        key: OccurrenceRewriteKey,
        value: ResolvedPathRewrite,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        match self.rewrites.get(&key) {
            Some(previous) if previous.as_ref() != &value => {
                Err(NestedModportAnalysisInvariant::ConflictingRewrite)
            }
            Some(_) => Ok(()),
            None => {
                if !self.rewrite_candidates.contains_key(&key) {
                    self.rewrite_order.push(key.clone());
                    self.rewrite_order_by_owner
                        .entry(key.owner.clone())
                        .or_default()
                        .push(key.clone());
                }
                self.rewrites.insert(key, Arc::new(value));
                Ok(())
            }
        }
    }

    pub fn record_rewrite_candidate(
        &mut self,
        key: OccurrenceRewriteKey,
        value: PendingPathRewriteCandidate,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        self.record_session(key.owner.session);
        match self.rewrite_candidates.get(&key) {
            Some(previous) if previous != &value => {
                Err(NestedModportAnalysisInvariant::ConflictingRewrite)
            }
            Some(_) => Ok(()),
            None => {
                self.rewrite_order.push(key.clone());
                self.rewrite_order_by_owner
                    .entry(key.owner.clone())
                    .or_default()
                    .push(key.clone());
                self.rewrite_candidates.insert(key, value);
                Ok(())
            }
        }
    }

    pub(crate) fn record_optional_rewrite_candidate(
        &mut self,
        key: OccurrenceRewriteKey,
        value: PendingPathRewriteCandidate,
        occurrence: TokenRange,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        self.record_rewrite_candidate(key.clone(), value)?;
        self.optional_rewrite_candidates.insert(key, occurrence);
        Ok(())
    }

    pub fn record_expanded_port(
        &mut self,
        key: ExpandedPortKey,
        value: ExpandedPortResolution,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        match self.expanded_ports.get(&key) {
            Some(previous) if previous.as_ref() != &value => {
                Err(NestedModportAnalysisInvariant::ConflictingExpandedPort)
            }
            Some(_) => Ok(()),
            None => {
                if !self.expanded_port_candidates.contains_key(&key) {
                    self.expanded_port_order.push(key.clone());
                    self.expanded_port_order_by_owner
                        .entry(key.owner.clone())
                        .or_default()
                        .push(key.clone());
                }
                self.expanded_ports.insert(key, Arc::new(value));
                Ok(())
            }
        }
    }

    pub fn record_expanded_port_candidate(
        &mut self,
        key: ExpandedPortKey,
        value: PendingExpandedPortCandidate,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        self.record_session(key.owner.session);
        match self.expanded_port_candidates.get(&key) {
            Some(previous) if previous != &value => {
                Err(NestedModportAnalysisInvariant::ConflictingExpandedPort)
            }
            Some(_) => Ok(()),
            None => {
                self.expanded_port_order.push(key.clone());
                self.expanded_port_order_by_owner
                    .entry(key.owner.clone())
                    .or_default()
                    .push(key.clone());
                self.expanded_port_candidates.insert(key, value);
                Ok(())
            }
        }
    }

    pub(crate) fn record_instantiation_context_candidate(
        &mut self,
        key: InstantiationContextKey,
        value: PendingInstantiationContextCandidate,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        self.record_session(key.owner.session);
        self.record_session(value.target.session);
        record_collection_work(1);
        match self.instantiation_context_candidates.get(&key) {
            Some(previous) if previous != &value => {
                Err(NestedModportAnalysisInvariant::ConflictingExpandedPort)
            }
            Some(_) => Ok(()),
            None => {
                self.instantiation_context_candidates.insert(key, value);
                record_collection_work(1);
                Ok(())
            }
        }
    }
    pub fn rewrite(&self, key: &OccurrenceRewriteKey) -> Option<&ResolvedPathRewrite> {
        self.rewrites.get(key).map(Arc::as_ref)
    }

    pub fn expanded_port(&self, key: &ExpandedPortKey) -> Option<&ExpandedPortResolution> {
        self.expanded_ports.get(key).map(Arc::as_ref)
    }
}
