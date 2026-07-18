use super::binding_key_semantic_scaling_tests::material_parameter;
use super::binding_scaling_tests::direct_binding;
use super::function_index_scaling_tests::package_function_binding;
use super::semantic_work_scaling_tests::generic_path;
use super::*;
use crate::ir::Signature;
use crate::symbol::{GenericMap, SymbolId};
use std::sync::Arc;
use veryl_parser::resource_table::{self, PathId, TokenId};

type FrameSnapshot = Vec<(u32, usize, usize, usize, bool)>;

fn material_signature(symbol: usize, label: &str, width: usize) -> Signature {
    let mut signature = Signature::new(SymbolId(symbol));
    signature.full_path.push(resource_table::insert_str(label));
    for index in 0..width {
        signature.parameters.push((
            resource_table::insert_str(&format!("P{index}")),
            material_parameter(index),
        ));
        signature.add_generic_parameter(
            resource_table::insert_str(&format!("G{index}")),
            generic_path(3, index),
        );
    }
    signature
}

fn material_identity(symbol: usize, label: &str, width: usize) -> ComponentSpecializationIdentity {
    ComponentSpecializationIdentity::new(
        material_signature(symbol, label, width),
        [ConnectedInterfaceSpecialization {
            formal_port: resource_table::insert_str("child"),
            actual: material_signature(symbol + 10_000, "connected_actual", width),
        }],
    )
    .expect("one connected actual")
}

fn staged_parent_fixture(deep_clone_mutation: bool) -> (FrameSnapshot, identity_clone_work::KeyCloneWork) {
    let session = AnalysisSessionId::new();
    let source = PathId(71);
    let parent_identity = Arc::new(material_identity(700, "material_parent", 6));
    let parent_key = Arc::new(NestedModportLoweringKey {
        session,
        specialization: Arc::clone(&parent_identity),
    });
    let mut parent = direct_binding(session, 0, source.0, 100);
    parent.specialization = Arc::clone(&parent_key);
    parent.emission_context = EmissionSpecializationContext::from_specialization(
        EmissionOwnerKind::Interface,
        &parent_identity,
    )
    .expect("material parent context");

    let mut functions = Vec::new();
    let mut function_identities = Vec::new();
    for index in 0..4 {
        let identity = Arc::new(material_identity(
            800 + index,
            &format!("material_function_{index}"),
            6,
        ));
        let mut binding = package_function_binding(
            session,
            1 + index as u32,
            source.0,
            200 + index,
            800 + index,
            EmissionScopeIdentity::test_scope(
                session,
                source,
                TokenId(900 + index),
                SymbolId(1_000 + index),
                &GenericMap::default(),
            ),
        );
        binding.specialization = Arc::new(NestedModportLoweringKey {
            session,
            specialization: Arc::clone(&identity),
        });
        binding.emission_context = EmissionSpecializationContext::from_specialization(
            EmissionOwnerKind::Function,
            &identity,
        )
        .expect("material function context");
        binding.enclosing_owner = Some(Arc::new(NestedModportLoweringKey {
            session,
            specialization: Arc::clone(&parent_identity),
        }));
        binding.enclosing_generic_map = binding.emission_context.enclosing_generic_map.clone();
        binding.package_scope = None;
        function_identities.push(identity);
        functions.push(binding);
    }

    identity_clone_work::reset_key_clone_work();
    let mutation = deep_clone_mutation.then(
        super::binding_specialization_identity::inject_deep_clone_registration_key,
    );
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(parent_key.as_ref().clone(), LoweringAvailability::NotNested);
    for binding in functions.iter().take(3) {
        pending.record_lowering(
            binding.specialization.as_ref().clone(),
            LoweringAvailability::NotNested,
        );
    }
    let missing_lowering = functions[3].specialization.as_ref().clone();
    pending.record_emission_binding(parent);
    for binding in functions {
        pending.record_emission_binding(binding);
    }
    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingLowering
        ))
    ));
    assert_eq!(pending.emission_bindings().len(), 5);
    pending.record_lowering(missing_lowering, LoweringAvailability::NotNested);
    let analysis = pending.finalize(session).expect("retry keeps staged bindings");
    drop(mutation);
    let clone_work = identity_clone_work::key_clone_work();

    let mut prepared = analysis
        .prepare_emission(source, EmissionPhase::Build)
        .expect("material source");
    let parent_batch = prepared
        .take_owners(TokenId(100), EmissionOwnerKind::Interface)
        .expect("parent declaration order");
    let parent_frame = parent_batch.iter().next().expect("one retained parent");
    assert!(Arc::ptr_eq(
        &parent_frame.specialization().specialization,
        &parent_identity
    ));
    let mut snapshot = vec![(
        parent_frame.id().0,
        parent_frame.binding.source.0,
        parent_frame.specialization().specialization.owner.full_path.len(),
        parent_frame.specialization().specialization.connected_actuals.len(),
        parent_frame.has_enclosing_frame(),
    )];
    for (index, expected) in function_identities.iter().enumerate() {
        let batch = prepared
            .take_function_owners(TokenId(200 + index), Some(parent_frame), None)
            .expect("function declaration order");
        let frame = batch.iter().next().expect("one retained function");
        assert!(frame.matches_enclosing_frame(parent_frame));
        assert!(Arc::ptr_eq(&frame.specialization().specialization, expected));
        snapshot.push((
            frame.id().0,
            frame.binding.source.0,
            frame.specialization().specialization.owner.full_path.len(),
            frame.specialization().specialization.connected_actuals.len(),
            frame.has_enclosing_frame(),
        ));
    }
    prepared.finish().expect("all staged frames consumed");
    (snapshot, clone_work)
}

#[test]
fn material_parent_staging_is_retry_safe_and_clone_affine() {
    let (normal_snapshot, normal_work) = staged_parent_fixture(false);
    let (mutation_snapshot, mutation_work) = staged_parent_fixture(true);
    assert_eq!(normal_snapshot, mutation_snapshot);
    assert_eq!(
        normal_snapshot,
        vec![
            (0, 71, 1, 1, false),
            (1, 71, 1, 1, true),
            (2, 71, 1, 1, true),
            (3, 71, 1, 1, true),
            (4, 71, 1, 1, true),
        ]
    );
    assert_eq!(normal_work.recursive_nodes, normal_work.events);
    assert_eq!(normal_work.allocations, 0);
    assert!(mutation_work.recursive_nodes > normal_work.recursive_nodes);
    assert!(mutation_work.allocations > 0);
}
