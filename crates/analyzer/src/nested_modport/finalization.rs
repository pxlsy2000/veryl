use super::analysis_snapshot::NestedModportAnalysis;
use super::collection_work::record_collection_work;
use super::emission_index::FrozenEmissionIndex;
use super::errors::*;
use super::identity::*;
use super::pending_records::PendingNestedModportAnalysis;
use std::sync::Arc;

impl PendingNestedModportAnalysis {
    pub fn finalize(
        &mut self,
        session: AnalysisSessionId,
    ) -> Result<Arc<NestedModportAnalysis>, NestedModportFinalizeError> {
        self.validate_session(session)?;
        let empty_emission_owners = self.complete_generic_emission_owners(session)?;
        let records = self.resolve_semantic_records()?;
        let bindings = self.finalize_emission_bindings(&records)?;
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
        self.finalized = true;
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
