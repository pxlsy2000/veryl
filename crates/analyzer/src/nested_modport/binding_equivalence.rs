use super::binding_deduplication::plan_emission_binding_deduplication;
use super::collection_work::{counted, record_collection_work as record_binding_work};
use super::emission_index::*;
use super::errors::NestedModportAnalysisInvariant;
use super::parent_resolution::resolve_function_enclosing_owners;
use super::pending_records::PendingNestedModportAnalysis;
use super::record_resolution::FrozenSemanticRecords;
use super::semantic_records::*;
use super::specialization_context::*;
use crate::HashMap;
use std::sync::Arc;

pub(super) struct FinalizedEmissionBindings {
    pub(super) values: Vec<PendingEmissionBinding>,
    pub(super) instantiation_contexts:
        HashMap<InstantiationContextKey, EmissionSpecializationContext>,
}

impl PendingNestedModportAnalysis {
    pub(super) fn finalize_emission_bindings(
        &mut self,
        records: &FrozenSemanticRecords,
    ) -> Result<FinalizedEmissionBindings, NestedModportAnalysisInvariant> {
        let specialization_identities: Vec<_> = self
            .emission_bindings
            .iter()
            .map(|binding| binding.specialization_identity)
            .collect::<Option<_>>()
            .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?;
        if self.emission_bindings.len() != specialization_identities.len() {
            return Err(NestedModportAnalysisInvariant::MismatchedEmissionContext);
        }
        for binding in counted(&mut self.emission_bindings) {
            if self.inferred_binding_ids.contains(&binding.id) {
                record_binding_work(1);
                binding.emission_context = EmissionSpecializationContext::from_specialization(
                    binding.kind,
                    &binding.specialization.specialization,
                )?;
                binding.required_rewrites = counted(
                    self.rewrite_order_by_owner
                        .get(&binding.specialization)
                        .into_iter()
                        .flatten(),
                )
                .filter(|key| records.rewrites.contains_key(*key))
                .cloned()
                .collect::<Vec<_>>()
                .into();
                binding.required_expanded_ports = counted(
                    self.expanded_port_order_by_owner
                        .get(&binding.specialization)
                        .into_iter()
                        .flatten(),
                )
                .cloned()
                .collect::<Vec<_>>()
                .into();
            }
            binding.lowering = match (
                &binding.lowering,
                records.availability.get(&binding.specialization),
            ) {
                (LoweringAvailability::NotNested, Some(LoweringAvailability::NotNested)) => {
                    LoweringAvailability::NotNested
                }
                (LoweringAvailability::Found(bound), Some(LoweringAvailability::Found(known)))
                    if Arc::ptr_eq(bound, known) || bound == known =>
                {
                    LoweringAvailability::Found(Arc::clone(known))
                }
                _ => return Err(NestedModportAnalysisInvariant::MissingLowering),
            };
        }
        let deduplication = plan_emission_binding_deduplication(
            &self.emission_bindings,
            &specialization_identities,
            records,
        )?;
        resolve_function_enclosing_owners(&mut self.emission_bindings, &deduplication.retained)?;
        let mut contexts = HashMap::default();
        for (index, (binding, keep)) in counted(
            self.emission_bindings
                .iter()
                .zip(&deduplication.retained)
                .enumerate(),
        ) {
            if !keep {
                continue;
            }
            record_binding_work(1);
            let mut context = binding.emission_context.clone();
            context.emit_connected_variant = deduplication.emit_connected_variants[index];
            contexts
                .entry(binding.specialization.clone())
                .or_insert(context);
        }
        let mut instantiation_contexts = HashMap::default();
        for (key, candidate) in counted(&self.instantiation_context_candidates) {
            record_binding_work(1);
            let context = contexts
                .get(&candidate.target)
                .cloned()
                .ok_or(NestedModportAnalysisInvariant::MissingLowering)?;
            instantiation_contexts.insert(key.clone(), context);
        }
        super::emission_index_build::validate_bindings(
            self.emission_bindings
                .iter()
                .zip(&deduplication.retained)
                .filter_map(|(binding, keep)| keep.then_some(binding)),
            records,
        )?;
        for (binding, emit_connected_variant) in counted(
            self.emission_bindings
                .iter_mut()
                .zip(&deduplication.emit_connected_variants),
        ) {
            binding.emission_context.emit_connected_variant = *emit_connected_variant;
        }
        let bindings = counted(
            std::mem::take(&mut self.emission_bindings)
                .into_iter()
                .zip(deduplication.retained),
        )
        .filter_map(|(binding, keep)| keep.then_some(binding))
        .collect();
        Ok(FinalizedEmissionBindings {
            values: bindings,
            instantiation_contexts,
        })
    }
}
