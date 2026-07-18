use super::binding_scaling_tests::direct_binding;
use super::function_index_scaling_tests::package_function_binding;
use super::*;
use crate::ir::{Signature, ValueVariant};
use crate::symbol::{GenericMap, SymbolId};
use std::sync::Arc;
use veryl_parser::resource_table::{PathId, StrId, TokenId};

fn connected(width: usize) -> ConnectedInterfaceSpecialization {
    let mut actual = Signature::new(SymbolId(888));
    actual.full_path.push(StrId(width));
    ConnectedInterfaceSpecialization {
        formal_port: StrId(5),
        actual,
    }
}

#[test]
fn parent_resolution_uses_full_owner_and_connected_signatures() {
    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    let mut base = Signature::new(SymbolId(777));
    base.full_path.push(StrId(1));
    let mut other_path = base.clone();
    other_path.full_path = vec![StrId(2)];
    let mut other_parameter = base.clone();
    other_parameter
        .parameters
        .push((StrId(9), ValueVariant::Unknown));
    let identities = vec![
        ComponentSpecializationIdentity::from_unique_connected_actuals(base.clone(), []),
        ComponentSpecializationIdentity::from_unique_connected_actuals(other_path, []),
        ComponentSpecializationIdentity::from_unique_connected_actuals(other_parameter, []),
        ComponentSpecializationIdentity::from_unique_connected_actuals(
            base.clone(),
            [connected(8)],
        ),
        ComponentSpecializationIdentity::from_unique_connected_actuals(base, [connected(16)]),
    ];
    for (index, identity) in identities.iter().enumerate() {
        let mut parent = direct_binding(session, index as u32, 1, index + 1);
        parent.specialization = Arc::new(NestedModportLoweringKey {
            session,
            specialization: identity.clone().into(),
        });
        parent.emission_context = EmissionSpecializationContext::new(
            identity.clone(),
            GenericMap::default(),
            None,
            Vec::new(),
        );
        let mut function = package_function_binding(
            session,
            (identities.len() + index) as u32,
            1,
            100 + index,
            900 + index,
            EmissionScopeIdentity::test_scope(
                session,
                PathId(1),
                TokenId(200 + index),
                SymbolId(300 + index),
                &GenericMap::default(),
            ),
        );
        function.enclosing_owner = Some(Arc::new(NestedModportLoweringKey {
            session,
            specialization: identity.clone().into(),
        }));
        function.enclosing_generic_map = Some(GenericMap::default());
        function.package_scope = None;
        for binding in [&parent, &function] {
            pending.record_lowering(
                binding.specialization.as_ref().clone(),
                LoweringAvailability::NotNested,
            );
        }
        pending.record_emission_binding(parent);
        pending.record_emission_binding(function);
    }

    let analysis = pending.finalize(session).expect("all exact parents must resolve");
    let mut prepared = analysis
        .prepare_emission(PathId(1), EmissionPhase::Build)
        .expect("exact fixture should prepare");
    for (index, identity) in identities.iter().enumerate() {
        let parent = prepared
            .take_owners(TokenId(index + 1), EmissionOwnerKind::Interface)
            .expect("parent order")
            .iter()
            .next()
            .expect("one parent");
        assert_eq!(parent.specialization().specialization.as_ref(), identity);
        assert_eq!(
            prepared
                .take_function_owners(TokenId(100 + index), Some(parent), None)
                .expect("exact function parent")
                .iter()
                .count(),
            1
        );
    }
    prepared.finish().expect("all exact bindings consumed");
}
