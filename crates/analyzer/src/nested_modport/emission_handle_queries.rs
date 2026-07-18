use super::emission_frame::EmissionFrame;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::NestedModportLoweringKey;
use super::lowering_records::{NestedModportLowering, ResolvedNestedTerminal};
use super::semantic_records::ResolvedTerminalRef;
use super::semantic_work::{record_emission_query_work, record_semantic_work};
use super::specialization_context::{EmissionSpecializationContext, LoweringAvailability};
use veryl_parser::resource_table::TokenId;

#[cfg(any(test, feature = "nested-modport-test-utils"))]
use super::semantic_records::InstantiationContextKey;

#[cfg(any(test, feature = "nested-modport-test-utils"))]
use super::frame_record_query_mutation::{
    instantiation_owner_lookup_mutation_enabled, terminal_availability_lookup_mutation_enabled,
};

impl<'a> EmissionFrame<'a> {
    pub fn published_instantiation_context(
        self,
        token: TokenId,
    ) -> Option<&'a EmissionSpecializationContext> {
        record_emission_query_work(1);
        #[cfg(any(test, feature = "nested-modport-test-utils"))]
        if instantiation_owner_lookup_mutation_enabled() {
            let before = super::semantic_work::semantic_work();
            let result = self
                .analysis
                .instantiation_contexts
                .get(&InstantiationContextKey {
                    owner: self.binding.specialization.as_ref().clone(),
                    token,
                });
            record_emission_query_work(
                super::semantic_work::semantic_work().saturating_sub(before),
            );
            return result;
        }
        self.binding.frame_records.instantiation_context(token)
    }

    pub fn resolve_lowering(
        self,
        target: &NestedModportLoweringKey,
    ) -> Result<&'a NestedModportLowering, NestedModportAnalysisInvariant> {
        match self.analysis.availability.get(target) {
            Some(LoweringAvailability::Found(lowering)) => Ok(lowering),
            Some(LoweringAvailability::NotNested) | None => {
                Err(NestedModportAnalysisInvariant::MissingLowering)
            }
        }
    }

    pub fn resolve_terminal<'terminal>(
        self,
        terminal: &'terminal ResolvedTerminalRef,
    ) -> Result<&'terminal ResolvedNestedTerminal, NestedModportAnalysisInvariant>
    where
        'a: 'terminal,
    {
        record_emission_query_work(1);
        #[cfg(any(test, feature = "nested-modport-test-utils"))]
        if terminal_availability_lookup_mutation_enabled() {
            let before = super::semantic_work::semantic_work();
            let lowering = self.resolve_lowering(&terminal.legacy_target)?;
            let result = lowering
                .terminals
                .get(terminal.terminal.0 as usize)
                .filter(|resolved| resolved.id == terminal.terminal)
                .ok_or(NestedModportAnalysisInvariant::MissingLowering);
            record_emission_query_work(
                super::semantic_work::semantic_work().saturating_sub(before),
            );
            return result;
        }
        let terminals = &terminal.lowering.terminals;
        record_semantic_work(1);
        terminals
            .get(terminal.terminal.0 as usize)
            .filter(|resolved| {
                record_semantic_work(1);
                resolved.id == terminal.terminal
            })
            .ok_or(NestedModportAnalysisInvariant::MissingLowering)
    }
}
