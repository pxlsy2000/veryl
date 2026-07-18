use super::binding_key_model::{BindingEquivalenceKey, BindingVariantGroupKey};
use super::binding_specialization_identity::BindingSpecializationIdentity;
use super::collection_work::{counted, record_collection_work};
use super::emission_index::PendingEmissionBinding;
use super::errors::NestedModportAnalysisInvariant;
use super::record_resolution::FrozenSemanticRecords;
use super::specialization_context::EmissionOwnerKind;
use crate::{HashMap, HashSet};

pub(super) struct EmissionBindingDeduplication {
    pub(super) retained: Vec<bool>,
    pub(super) emit_connected_variants: Vec<bool>,
}

pub(super) fn plan_emission_binding_deduplication(
    bindings: &[PendingEmissionBinding],
    specialization_identities: &[BindingSpecializationIdentity],
    records: &FrozenSemanticRecords,
) -> Result<EmissionBindingDeduplication, NestedModportAnalysisInvariant> {
    let mut instantiated = HashSet::default();
    for binding in counted(bindings.iter()) {
        if binding.kind != EmissionOwnerKind::Function
            && !binding
                .specialization
                .specialization
                .owner
                .generic_parameters
                .is_empty()
        {
            instantiated.insert((
                binding.source,
                binding.declaration,
                binding.kind,
                binding.specialization.specialization.owner.symbol,
            ));
            record_collection_work(1);
        }
    }
    let initially_retained: Vec<_> = bindings
        .iter()
        .map(|binding| {
            record_collection_work(1);
            let owner = &binding.specialization.specialization.owner;
            binding.kind == EmissionOwnerKind::Function
                || !owner.generic_parameters.is_empty()
                || !instantiated.contains(&(
                    binding.source,
                    binding.declaration,
                    binding.kind,
                    owner.symbol,
                ))
        })
        .collect();
    let keyed: Vec<_> = counted(
        bindings
            .iter()
            .zip(specialization_identities)
            .zip(&initially_retained),
    )
    .filter(|(_, keep)| **keep)
    .map(|((binding, identity), _)| BindingEquivalenceKey::build(binding, *identity, records))
    .collect::<Result<_, _>>()?;
    let mut concrete_bases = HashSet::default();
    for (_, key) in counted(&keyed) {
        if !key.is_template() {
            concrete_bases.insert(key.base.clone());
            record_collection_work(1);
        }
    }
    let mut seen = HashSet::default();
    let mut keyed_retained = Vec::with_capacity(keyed.len());
    for (_, key) in counted(&keyed) {
        keyed_retained.push(
            !(key.is_template() && concrete_bases.contains(&key.base)) && seen.insert(key.clone()),
        );
    }
    let mut variants: HashMap<BindingVariantGroupKey, HashSet<BindingEquivalenceKey>> =
        HashMap::default();
    for ((group, key), keep) in counted(keyed.iter().zip(&keyed_retained)) {
        if *keep {
            variants.entry(*group).or_default().insert(key.clone());
        }
    }
    let keyed_emit_connected_variants: Vec<_> = counted(keyed.iter().zip(&keyed_retained))
        .map(|((group, key), keep)| {
            *keep && !key.is_template() && variants.get(group).is_some_and(|keys| keys.len() > 1)
        })
        .collect();
    drop(variants);
    drop(seen);
    drop(concrete_bases);
    let mut retained = Vec::with_capacity(bindings.len());
    let mut emit_connected_variants = Vec::with_capacity(bindings.len());
    let mut keyed_index = 0;
    for initially_retained in initially_retained {
        if initially_retained {
            retained.push(keyed_retained[keyed_index]);
            emit_connected_variants.push(keyed_emit_connected_variants[keyed_index]);
            keyed_index += 1;
        } else {
            retained.push(false);
            emit_connected_variants.push(false);
        }
    }
    drop(keyed);
    Ok(EmissionBindingDeduplication {
        retained,
        emit_connected_variants,
    })
}
