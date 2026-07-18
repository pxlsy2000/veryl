use super::collection_work::{
    collection_work as binding_finalization_work,
    reset_collection_work as reset_binding_finalization_work,
};
use super::function_index_scaling_tests::package_function_analysis;
use super::prepared_emission::{prepare_emission_work, reset_prepare_emission_work};
use super::*;
use crate::ir::Signature;
use crate::symbol::{GenericMap, SymbolId};
use veryl_parser::resource_table::{PathId, TokenId};

#[test]
fn production_registration_helpers_are_inside_the_work_counter_window() {
    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    reset_binding_finalization_work();
    for index in 0..64 {
        let specialization = NestedModportLoweringKey {
            session,
            specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
                Signature::new(SymbolId(1_000 + index)),
                [],
            )
            .into(),
        };
        pending
            .record_emission_owner(
                PathId(1 + index % 2),
                TokenId(index + 1),
                EmissionOwnerKind::Interface,
                specialization.clone(),
                EmissionSpecializationContext::from_specialization(
                    EmissionOwnerKind::Interface,
                    &specialization.specialization,
                )
                .expect("fixture specialization should be valid"),
                LoweringAvailability::NotNested,
            )
            .expect("incremental owner registration should succeed");
        pending.set_emission_owner_scope(&specialization, &GenericMap::default());
        pending.record_generic_emission_owner(PendingGenericEmissionOwner {
            session,
            source: PathId(1),
            declaration: TokenId(10_000 + index),
            kind: EmissionOwnerKind::Function,
            symbol: SymbolId(20_000 + index),
        });
        pending
            .record_instantiation_context_candidate(
                InstantiationContextKey {
                    owner: specialization.clone(),
                    token: TokenId(30_000 + index),
                },
                PendingInstantiationContextCandidate {
                    target: specialization,
                },
            )
            .expect("candidate registration should succeed");
    }
    assert!(
        binding_finalization_work() > 0,
        "production registration performed work outside the counter"
    );
}

#[test]
fn function_scope_index_never_visits_unrelated_large_source() {
    let (mut pending, session, lookups) = package_function_analysis(64, 256);
    reset_binding_finalization_work();
    let analysis = pending.finalize(session).expect("combined fixture should finalize");
    let build_work = binding_finalization_work();
    reset_prepare_emission_work();
    let mut prepared = analysis
        .prepare_emission(PathId(1), EmissionPhase::Align)
        .expect("target source should prepare");
    for (index, (declaration, symbol, map)) in lookups.iter().enumerate() {
        let scope = prepared
            .package_scope(*declaration, *symbol, map)
            .expect("target scope should resolve")
            .expect("target scope should exist");
        prepared
            .take_function_owners(TokenId(20_000 + index), None, Some(&scope))
            .expect("target function should resolve");
    }
    prepared.finish().expect("target source should be consumed");
    assert_eq!(build_work, 73 * 320 + 4_621);
    assert_eq!(prepare_emission_work(), 15 * 64 + 1);
}

#[test]
fn whole_path_counter_detects_an_injected_quadratic_index_scan() {
    let (mut pending, session, _) = package_function_analysis(64, 0);
    let _guard = super::emission_index_build::inject_extra_full_index_scan();
    reset_binding_finalization_work();
    let _analysis = pending.finalize(session).expect("mutation fixture should finalize");
    let work = binding_finalization_work();
    assert_eq!(work, 73 * 64 + 2_311 + 64 + 64 * 64);
    assert!(work > 60 * 64);
}
