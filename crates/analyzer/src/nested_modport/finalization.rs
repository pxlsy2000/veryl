use super::analysis_snapshot::NestedModportAnalysis;
use super::collection_work::record_collection_work;
use super::emission_index::FrozenEmissionIndex;
use super::errors::*;
use super::identity::*;
use super::pending_records::PendingNestedModportAnalysis;
use std::sync::Arc;

impl PendingNestedModportAnalysis {
    fn clone_for_finalization(&self) -> Self {
        Self {
            interface_lowerings: self.interface_lowerings.clone(),
            lowerings: self.lowerings.clone(),
            rewrites: self.rewrites.clone(),
            rewrite_candidates: self.rewrite_candidates.clone(),
            optional_rewrite_candidates: self.optional_rewrite_candidates.clone(),
            rewrite_order: self.rewrite_order.clone(),
            rewrite_order_by_owner: self.rewrite_order_by_owner.clone(),
            expanded_ports: self.expanded_ports.clone(),
            expanded_port_candidates: self.expanded_port_candidates.clone(),
            expanded_port_order: self.expanded_port_order.clone(),
            expanded_port_order_by_owner: self.expanded_port_order_by_owner.clone(),
            instantiation_context_candidates: self.instantiation_context_candidates.clone(),
            generic_emission_owners: self.generic_emission_owners.clone(),
            generic_emission_owner_set: self.generic_emission_owner_set.clone(),
            emission_bindings: self.emission_bindings.clone(),
            binding_specialization_interner: self.binding_specialization_interner.clone(),
            emission_binding_records: self.emission_binding_records.clone(),
            emission_binding_specializations: self.emission_binding_specializations.clone(),
            emission_binding_declarations: self.emission_binding_declarations.clone(),
            emission_binding_scopes: self.emission_binding_scopes.clone(),
            emission_declaration_specializations: self.emission_declaration_specializations.clone(),
            max_emission_binding_id: self.max_emission_binding_id,
            inferred_binding_ids: self.inferred_binding_ids.clone(),
            recorded_sessions: self.recorded_sessions.clone(),
            finalized: self.finalized,
        }
    }

    pub fn finalize(
        &mut self,
        session: AnalysisSessionId,
    ) -> Result<Arc<NestedModportAnalysis>, NestedModportFinalizeError> {
        self.validate_session(session)?;
        let mut staged = self.clone_for_finalization();
        let empty_emission_owners = staged.complete_generic_emission_owners(session)?;
        let records = staged.resolve_semantic_records()?;
        let bindings = staged.finalize_emission_bindings(&records)?;
        let super::binding_equivalence::FinalizedEmissionBindings {
            values,
            instantiation_contexts,
        } = bindings;
        let emission_index = FrozenEmissionIndex::build(
            values,
            empty_emission_owners,
            &records,
            &instantiation_contexts,
        )?;
        let analysis = NestedModportAnalysis {
            availability: records.availability,
            rewrites: records.rewrites,
            expanded_ports: records.expanded_ports,
            #[cfg(any(test, feature = "nested-modport-test-utils"))]
            instantiation_contexts,
            emission_index,
        };
        staged.finalized = true;
        *self = staged;
        Ok(Arc::new(analysis))
    }

    pub(super) fn validate_session(
        &self,
        session: AnalysisSessionId,
    ) -> Result<(), NestedModportAnalysisInvariant> {
        if self.finalized {
            return Err(NestedModportAnalysisInvariant::AlreadyFinalized);
        }
        record_collection_work(1);
        let crosses_session = self.recorded_sessions.len() > 1
            || (self.recorded_sessions.len() == 1 && !self.recorded_sessions.contains(&session));
        if crosses_session {
            Err(NestedModportAnalysisInvariant::CrossSession)
        } else {
            Ok(())
        }
    }
}
