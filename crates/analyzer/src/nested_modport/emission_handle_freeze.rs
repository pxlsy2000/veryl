use super::binding_order::order_source_bindings;
use super::binding_specialization_identity::BindingSpecializationIdentity;
use super::emission_index::{EmissionOwnerBinding, FrozenOwnerHandle, PendingEmissionBinding};
use super::errors::NestedModportAnalysisInvariant;
use super::identity::NestedModportLoweringKey;
use super::record_resolution::FrozenSemanticRecords;
use super::specialization_context::EmissionSpecializationContext;
use crate::HashMap;
use veryl_parser::resource_table::TokenId;

pub(super) fn freeze_source_bindings(
    bindings: Vec<PendingEmissionBinding>,
    records: &FrozenSemanticRecords,
    contexts_by_owner: &HashMap<
        NestedModportLoweringKey,
        Vec<(TokenId, EmissionSpecializationContext)>,
    >,
) -> Result<Vec<EmissionOwnerBinding>, NestedModportAnalysisInvariant> {
    let bindings = order_source_bindings(bindings);
    let mut owner_handles = HashMap::default();
    let mut next_owner = 0;
    for binding in &bindings {
        let session = binding.specialization.session;
        let identity = binding
            .specialization_identity
            .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?;
        owner_handles.entry(identity).or_insert_with(|| {
            let handle = FrozenOwnerHandle::new(session, next_owner);
            next_owner += 1;
            handle
        });
    }
    bindings
        .into_iter()
        .map(|binding| {
            let identity: BindingSpecializationIdentity = binding
                .specialization_identity
                .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?;
            let owner_handle = *owner_handles
                .get(&identity)
                .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?;
            let enclosing_owner_handle = binding
                .enclosing_owner
                .as_ref()
                .and_then(|_| binding.enclosing_owner_identity)
                .and_then(|owner| owner_handles.get(&owner))
                .copied();
            let instantiation_contexts = contexts_by_owner
                .get(&binding.specialization)
                .map(Vec::as_slice)
                .unwrap_or_default();
            EmissionOwnerBinding::freeze(
                binding,
                owner_handle,
                enclosing_owner_handle,
                instantiation_contexts,
                records,
            )
        })
        .collect()
}
