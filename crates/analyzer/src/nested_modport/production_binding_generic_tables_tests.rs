use super::super::semantic_work::{reset_semantic_work, semantic_work, without_semantic_work};
use super::semantic_work_scaling_tests::{generic_path, lowering};
use super::*;
use crate::ir::Signature;
use crate::namespace::DefineContext;
use crate::scope::ScopeId;
use crate::symbol::SymbolId;
use crate::HashMap;
use std::sync::Arc;
use veryl_parser::resource_table::{self, PathId, TokenId};

#[derive(Clone, Copy, Debug)]
struct ProductionPathMeasurement {
    semantic_work: usize,
    retained_bindings: usize,
}

fn named_lowering(table_count: usize, mismatch_last: bool) -> Arc<NestedModportLowering> {
    let mut value = lowering(1);
    let terminals = Arc::make_mut(&mut value.terminals);
    let ResolvedDeclarationKind::Struct(named) =
        &mut terminals[0].resolved_type.declaration.kind
    else {
        unreachable!()
    };
    named.generic_tables.clear();
    for index in 0..table_count {
        let suffix = if mismatch_last && index + 1 == table_count {
            50_000
        } else {
            index
        };
        named.generic_tables.insert(
            (ScopeId(index as u32 + 1), DefineContext::default()),
            HashMap::from_iter([(
                resource_table::insert_str(&format!("Outer{index}")),
                generic_path(2, suffix),
            )]),
        );
    }
    Arc::new(value)
}

fn specialization(
    session: AnalysisSessionId,
    connected_path: &str,
) -> NestedModportLoweringKey {
    let mut actual = Signature::new(SymbolId(702));
    actual
        .full_path
        .push(resource_table::insert_str(connected_path));
    NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
            Signature::new(SymbolId(701)),
            [ConnectedInterfaceSpecialization {
                formal_port: resource_table::insert_str("child"),
                actual,
            }],
        )
        .into(),
    }
}

fn binding(
    id: u32,
    specialization: NestedModportLoweringKey,
    lowering: Arc<NestedModportLowering>,
) -> PendingEmissionBinding {
    let context = EmissionSpecializationContext::from_specialization(
        EmissionOwnerKind::Interface,
        &specialization.specialization,
    )
    .expect("material interface context");
    PendingEmissionBinding {
        id: EmissionBindingId::new(id),
        source: PathId(901),
        declaration: TokenId(902),
        kind: EmissionOwnerKind::Interface,
        enclosing_owner: None,
        enclosing_owner_identity: None,
        namespace_parent_fallback: false,
        enclosing_generic_map: None,
        package_scope: None,
        specialization: Arc::new(specialization),
        specialization_identity: None,
        emission_context: context,
        lowering: LoweringAvailability::Found(lowering),
        required_rewrites: Arc::from([]),
        required_expanded_ports: Arc::from([]),
    }
}

fn measure_production_path(table_count: usize) -> ProductionPathMeasurement {
    let session = AnalysisSessionId::new();
    let base_key = specialization(session, "base_connected_actual");
    let mismatch_key = specialization(session, "mismatch_connected_actual");
    let base = named_lowering(table_count, false);
    let mismatch = named_lowering(table_count, true);
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(
        base_key.clone(),
        LoweringAvailability::Found(Arc::clone(&base)),
    );
    pending.record_lowering(
        mismatch_key.clone(),
        LoweringAvailability::Found(Arc::clone(&mismatch)),
    );
    let duplicate = Arc::new((*base).clone());
    pending.record_emission_binding(binding(0, base_key.clone(), Arc::clone(&base)));
    pending.record_emission_binding(binding(1, base_key, duplicate));
    pending.record_emission_binding(binding(2, mismatch_key, mismatch));

    reset_semantic_work();
    let analysis = pending
        .finalize(session)
        .expect("named-terminal bindings should finalize through production deduplication");
    let measurement = ProductionPathMeasurement {
        semantic_work: semantic_work(),
        retained_bindings: 0,
    };

    without_semantic_work(|| {
        let mut prepared = analysis
            .prepare_emission(PathId(901), EmissionPhase::Build)
            .expect("material source should prepare");
        let retained = {
            let batch = prepared
                .take_owners(TokenId(902), EmissionOwnerKind::Interface)
                .expect("material declaration should have a frame batch");
            let frames: Vec<_> = batch.iter().collect();
            assert_eq!(frames.len(), 2, "duplicate collapses and late mismatch remains");
            let mut last_values = Vec::new();
            for frame in &frames {
                let LoweringAvailability::Found(lowering) = frame.lowering() else {
                    unreachable!()
                };
                let ResolvedDeclarationKind::Struct(named) =
                    &lowering.terminals[0].resolved_type.declaration.kind
                else {
                    unreachable!()
                };
                assert_eq!(named.generic_tables.len(), table_count);
                last_values.push(
                    named
                        .generic_tables
                        .get(&(ScopeId(table_count as u32), DefineContext::default()))
                        .expect("last outer table")
                        .values()
                        .next()
                        .expect("last named generic value")
                        .clone(),
                );
            }
            assert_ne!(last_values[0], last_values[1]);
            frames.len()
        };
        prepared.finish().expect("all material frames consumed");
        ProductionPathMeasurement {
            retained_bindings: retained,
            ..measurement
        }
    })
}

#[test]
fn named_terminal_outer_tables_are_affine_through_production_binding_deduplication() {
    let normal = [64, 128, 256].map(measure_production_path);

    for measurement in &normal {
        assert_eq!(measurement.retained_bindings, 2);
        assert!(measurement.semantic_work > 0);
    }
    for pair in normal.windows(2) {
        assert!(pair[1].semantic_work <= 3 * pair[0].semantic_work, "{normal:?}");
    }
}

#[test]
fn deduplication_cannot_hide_a_mismatched_binding_lowering() {
    let session = AnalysisSessionId::new();
    let key = specialization(session, "same_connected_actual");
    let known = named_lowering(4, false);
    let mismatched = named_lowering(4, true);
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(
        key.clone(),
        LoweringAvailability::Found(Arc::clone(&known)),
    );
    pending.record_emission_binding(binding(0, key.clone(), known));
    pending.record_emission_binding(binding(1, key, mismatched));

    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingLowering
        ))
    ));
}
