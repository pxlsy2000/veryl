use super::collection_work::{
    collection_work as binding_finalization_work,
    reset_collection_work as reset_binding_finalization_work,
};
use super::binding_scaling_tests::direct_binding;
use super::prepared_emission::{prepare_emission_work, reset_prepare_emission_work};
use super::*;
use crate::ir::Signature;
use crate::symbol::{GenericMap, SymbolId};
use std::sync::Arc;
use veryl_parser::resource_table::{PathId, TokenId};

pub(super) fn package_function_binding(
    session: AnalysisSessionId,
    id: u32,
    source: usize,
    declaration: usize,
    symbol: usize,
    scope: EmissionScopeIdentity,
) -> PendingEmissionBinding {
    let specialization = ComponentSpecializationIdentity::from_unique_connected_actuals(
        Signature::new(SymbolId(symbol)),
        [],
    );
    PendingEmissionBinding {
        id: EmissionBindingId::new(id),
        source: PathId(source),
        declaration: TokenId(declaration),
        kind: EmissionOwnerKind::Function,
        enclosing_owner: None,
        enclosing_owner_identity: None,
        namespace_parent_fallback: false,
        enclosing_generic_map: None,
        package_scope: Some(scope),
        specialization: Arc::new(NestedModportLoweringKey {
            session,
            specialization: specialization.clone().into(),
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

pub(super) fn package_function_analysis(
    binding_count: usize,
    unrelated_count: usize,
) -> (
    PendingNestedModportAnalysis,
    AnalysisSessionId,
    Vec<(TokenId, SymbolId, GenericMap)>,
) {
    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    let mut lookups = Vec::new();
    for index in 0..binding_count + unrelated_count {
        let source = usize::from(index >= binding_count) + 1;
        let scope_declaration = TokenId(index + 1);
        let scope_symbol = SymbolId(10_000 + index);
        let map = GenericMap::default();
        let scope = EmissionScopeIdentity::test_scope(
            session,
            PathId(source),
            scope_declaration,
            scope_symbol,
            &map,
        );
        let binding = package_function_binding(
            session,
            index as u32,
            source,
            20_000 + index,
            30_000 + index,
            scope,
        );
        pending.record_lowering(
            binding.specialization.as_ref().clone(),
            LoweringAvailability::NotNested,
        );
        pending.record_emission_binding(binding);
        if index < binding_count {
            lookups.push((scope_declaration, scope_symbol, map));
        }
    }
    (pending, session, lookups)
}

fn parent_and_function_bindings(
    binding_count: usize,
) -> (PendingNestedModportAnalysis, AnalysisSessionId) {
    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    for index in 0..binding_count {
        let mut parent = direct_binding(session, index as u32, 1, index + 1);
        let parent_specialization =
            ComponentSpecializationIdentity::from_unique_connected_actuals(
                Signature::new(SymbolId(1_000 + index)),
                [],
            );
        parent.specialization = Arc::new(NestedModportLoweringKey {
            session,
            specialization: parent_specialization.clone().into(),
        });
        parent.emission_context = EmissionSpecializationContext::new(
            parent_specialization,
            GenericMap::default(),
            None,
            Vec::new(),
        );
        pending.record_lowering(
            parent.specialization.as_ref().clone(),
            LoweringAvailability::NotNested,
        );
        let mut function = package_function_binding(
            session,
            (binding_count + index) as u32,
            1,
            10_000 + index,
            20_000 + index,
            EmissionScopeIdentity::test_scope(
                session,
                PathId(1),
                TokenId(30_000 + index),
                SymbolId(40_000 + index),
                &GenericMap::default(),
            ),
        );
        function.enclosing_owner = Some(Arc::clone(&parent.specialization));
        function.enclosing_generic_map = Some(GenericMap::default());
        function.package_scope = None;
        pending.record_lowering(
            function.specialization.as_ref().clone(),
            LoweringAvailability::NotNested,
        );
        pending.record_emission_binding(parent);
        pending.record_emission_binding(function);
    }
    (pending, session)
}

#[test]
fn package_and_function_scope_work_is_linear_at_1x_2x_4x() {
    for binding_count in [64, 128, 256] {
        let (mut pending, session, lookups) = package_function_analysis(binding_count, 0);
        reset_binding_finalization_work();
        reset_prepare_emission_work();
        let analysis = pending.finalize(session).expect("package fixture should finalize");
        let build_work = binding_finalization_work();
        let mut prepared = analysis
            .prepare_emission(PathId(1), EmissionPhase::Build)
            .expect("package preparation should succeed");
        for (index, (declaration, symbol, map)) in lookups.iter().enumerate() {
            let scope = prepared
                .package_scope(*declaration, *symbol, map)
                .expect("exact package scope should resolve")
                .expect("package scope should exist");
            assert_eq!(
                prepared
                    .take_function_owners(TokenId(20_000 + index), None, Some(&scope))
                    .expect("exact function scope should resolve")
                    .iter()
                    .count(),
                1
            );
        }
        prepared.finish().expect("every function should be consumed");
        let query_work = prepare_emission_work();
        assert_eq!(build_work, 73 * binding_count + 2_311);
        assert_eq!(query_work, 15 * binding_count + 1);
        assert_eq!(build_work + query_work, 88 * binding_count + 2_312);
    }
}

#[test]
fn function_parent_resolution_work_is_linear_at_1x_2x_4x() {
    for binding_count in [64, 128, 256] {
        let (mut pending, session) = parent_and_function_bindings(binding_count);
        reset_binding_finalization_work();
        reset_prepare_emission_work();
        let analysis = pending.finalize(session).expect("parent fixture should finalize");
        let build_work = binding_finalization_work();
        let mut prepared = analysis
            .prepare_emission(PathId(1), EmissionPhase::Build)
            .expect("parent fixture should prepare");
        let mut parents = Vec::new();
        for index in 0..binding_count {
            parents.extend(
                prepared
                    .take_owners(TokenId(index + 1), EmissionOwnerKind::Interface)
                    .expect("parent order should be stable")
                    .iter(),
            );
        }
        for (index, parent) in parents.into_iter().enumerate() {
            assert_eq!(
                prepared
                    .take_function_owners(TokenId(10_000 + index), Some(parent), None)
                    .expect("function should select its exact parent")
                    .iter()
                    .count(),
                1
            );
        }
        prepared.finish().expect("all bindings consumed");
        let query_work = prepare_emission_work();
        assert_eq!(build_work, 141 * binding_count + 2_313);
        assert_eq!(query_work, 19 * binding_count + 1);
        assert_eq!(build_work + query_work, 160 * binding_count + 2_314);
    }
}

#[test]
fn function_parent_index_preserves_missing_and_ambiguous_errors() {
    let (mut missing, session) = parent_and_function_bindings(1);
    missing.emission_bindings.remove(0);
    assert!(matches!(
        missing.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MismatchedEmissionContext
        ))
    ));

    let (mut ambiguous, session) = parent_and_function_bindings(1);
    let mut duplicate = ambiguous.emission_bindings[0].clone();
    duplicate.id = EmissionBindingId::new(99);
    duplicate.declaration = TokenId(99);
    ambiguous.record_emission_binding(duplicate);
    assert!(matches!(
        ambiguous.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MismatchedEmissionContext
        ))
    ));
}
