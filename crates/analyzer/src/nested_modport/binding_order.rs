use super::collection_work::{counted, record_collection_work};
use super::emission_index::PendingEmissionBinding;
use super::specialization_context::EmissionOwnerKind;

pub(super) fn order_source_bindings(
    mut bindings: Vec<PendingEmissionBinding>,
) -> Vec<PendingEmissionBinding> {
    bindings = stable_byte_pass(bindings, |binding| kind_byte(binding.kind));
    for byte in 0..std::mem::size_of::<usize>() {
        bindings = stable_byte_pass(bindings, |binding| {
            binding.declaration.0.to_le_bytes()[byte]
        });
    }
    bindings
}

fn stable_byte_pass(
    bindings: Vec<PendingEmissionBinding>,
    key: impl Fn(&PendingEmissionBinding) -> u8,
) -> Vec<PendingEmissionBinding> {
    let capacity = bindings.len();
    let mut buckets: [Vec<PendingEmissionBinding>; 256] = std::array::from_fn(|_| Vec::new());
    for binding in counted(bindings) {
        let bucket = usize::from(key(&binding));
        buckets[bucket].push(binding);
        record_collection_work(1);
    }
    let mut ordered = Vec::with_capacity(capacity);
    for bucket in counted(buckets) {
        ordered.extend(counted(bucket));
    }
    ordered
}

const fn kind_byte(kind: EmissionOwnerKind) -> u8 {
    match kind {
        EmissionOwnerKind::Module => 0,
        EmissionOwnerKind::Interface => 1,
        EmissionOwnerKind::Function => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Signature;
    use crate::nested_modport::*;
    use crate::symbol::{GenericMap, SymbolId};
    use std::sync::Arc;
    use veryl_parser::resource_table::{PathId, TokenId};

    fn binding(id: u32, declaration: usize, kind: EmissionOwnerKind) -> PendingEmissionBinding {
        let specialization = ComponentSpecializationIdentity::from_unique_connected_actuals(
            Signature::new(SymbolId(id as usize + 1)),
            [],
        );
        PendingEmissionBinding {
            id: EmissionBindingId::new(id),
            source: PathId(1),
            declaration: TokenId(declaration),
            kind,
            enclosing_owner: None,
            enclosing_owner_identity: None,
            namespace_parent_fallback: false,
            enclosing_generic_map: None,
            package_scope: None,
            specialization: Arc::new(NestedModportLoweringKey {
                session: AnalysisSessionId::new(),
                specialization: (specialization.clone()).into(),
            }),
            specialization_identity: None,
            emission_context: EmissionSpecializationContext::new(
                specialization,
                GenericMap::default(),
                None,
                Vec::new(),
            ),
            lowering: LoweringAvailability::NotNested,
            required_rewrites: Arc::from([]),
            required_expanded_ports: Arc::from([]),
        }
    }

    #[test]
    fn radix_order_matches_stable_reference_for_arbitrary_tokens_and_equal_keys() {
        let mut input = Vec::new();
        for (id, declaration, kind) in [
            (0, usize::MAX - 2, EmissionOwnerKind::Function),
            (1, 0x0100, EmissionOwnerKind::Interface),
            (2, 1, EmissionOwnerKind::Module),
            (3, 0x0100, EmissionOwnerKind::Module),
            (4, usize::MAX - 2, EmissionOwnerKind::Function),
            (5, 0x00ff, EmissionOwnerKind::Interface),
            (6, 1, EmissionOwnerKind::Function),
            (7, 0x0100, EmissionOwnerKind::Interface),
        ] {
            input.push(binding(id, declaration, kind));
        }
        let mut expected = input.clone();
        expected.sort_by_key(|binding| (binding.declaration, binding.kind));
        let actual = order_source_bindings(input);
        assert!(!actual.is_empty());
        assert_eq!(
            actual.iter().map(|binding| binding.id).collect::<Vec<_>>(),
            expected
                .iter()
                .map(|binding| binding.id)
                .collect::<Vec<_>>()
        );
    }
}
