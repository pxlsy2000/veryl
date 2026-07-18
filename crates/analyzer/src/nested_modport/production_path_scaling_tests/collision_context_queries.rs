#[test]
fn frame_local_records_survive_hash_collisions_and_same_tokens_in_distinct_owners() {
    let normal = joint_axis_frame_query_work(8, false, false);
    let collided = joint_axis_frame_query_work(8, false, true);
    assert_eq!(normal.1, collided.1);
}

#[test]
fn legacy_owner_lookup_mutation_preserves_frame_record_results() {
    let mutation =
        super::super::frame_record_query_mutation::inject_owner_record_lookup_mutation();
    let legacy = joint_axis_frame_query_work(8, false, false);
    drop(mutation);
    assert_eq!(legacy.1, 32);
}

fn instantiation_context_joint_axis_work(scale: usize, mutation: bool) -> (usize, usize) {
    let session = AnalysisSessionId::new();
    let source = PathId(880_000);
    let declaration = TokenId(880_001);
    let formal = resource_table::insert_str("instantiation_child");
    let mut pending = PendingNestedModportAnalysis::default();
    for owner_index in 0..2 {
        let mut owner_signature = Signature::new(SymbolId(881_000 + owner_index));
        let mut actual_signature = Signature::new(SymbolId(882_000 + owner_index));
        for width_index in 0..scale {
            owner_signature.add_generic_parameter(
                resource_table::insert_str(&format!("OWNER_CONTEXT_{width_index}")),
                super::semantic_work_scaling_tests::generic_path(
                    2,
                    30_000 + width_index + owner_index,
                ),
            );
            actual_signature.add_generic_parameter(
                resource_table::insert_str(&format!("ACTUAL_CONTEXT_{width_index}")),
                super::semantic_work_scaling_tests::generic_path(
                    2,
                    40_000 + width_index + owner_index,
                ),
            );
        }
        let specialization = ComponentSpecializationIdentity::new(
            owner_signature,
            [ConnectedInterfaceSpecialization {
                formal_port: formal,
                actual: actual_signature,
            }],
        )
        .expect("material instantiation owner");
        let owner = NestedModportLoweringKey {
            session,
            specialization: specialization.clone().into(),
        };
        let mut context = EmissionSpecializationContext::new(
            specialization,
            GenericMap::default(),
            None,
            Vec::new(),
        );
        context.generic_map.id = Some(SymbolId(if owner_index == 0 { 8 } else { 16 }));
        let mut binding = super::binding_scaling_tests::direct_binding(
            session,
            owner_index as u32,
            source.0,
            declaration.0,
        );
        binding.specialization = Arc::new(owner.clone());
        binding.emission_context = context;
        pending.record_lowering(owner.clone(), LoweringAvailability::NotNested);
        pending.record_emission_binding(binding);
        for token_index in 0..scale {
            pending
                .record_instantiation_context_candidate(
                    InstantiationContextKey {
                        owner: owner.clone(),
                        token: TokenId(883_000 + token_index),
                    },
                    PendingInstantiationContextCandidate {
                        target: owner.clone(),
                    },
                )
                .expect("same-token owner-local context");
        }
    }
    let analysis = pending
        .finalize(session)
        .expect("instantiation context fixture finalizes");
    let mut prepared = analysis
        .prepare_emission(source, EmissionPhase::Build)
        .expect("instantiation source prepares");
    let batch = prepared
        .take_owners(declaration, EmissionOwnerKind::Interface)
        .expect("two instantiation owner frames");
    let guard = mutation.then(
        super::super::frame_record_query_mutation::inject_instantiation_owner_lookup_mutation,
    );
    reset_semantic_work();
    let mut resolved = 0;
    let mut context_ids = Vec::new();
    for frame in batch.iter() {
        for token_index in 0..scale {
            let context = frame
                .published_instantiation_context(TokenId(883_000 + token_index))
                .expect("frame-local instantiation context");
            context_ids.push(context.generic_map.id);
            resolved += 1;
        }
        assert!(frame
            .published_instantiation_context(TokenId(884_000 + scale))
            .is_none());
    }
    let work = semantic_work();
    drop(guard);
    assert!(context_ids.contains(&Some(SymbolId(8))));
    assert!(context_ids.contains(&Some(SymbolId(16))));
    prepared.finish().expect("instantiation frames consumed");
    (work, resolved)
}

#[test]
fn instantiation_context_queries_use_frame_local_tokens_on_joint_axes() {
    let compact = [8, 16, 32].map(|scale| instantiation_context_joint_axis_work(scale, false));
    let material = [8, 16, 32].map(|scale| instantiation_context_joint_axis_work(scale, true));
    assert_eq!(compact.map(|sample| sample.1), [16, 32, 64]);
    assert_eq!(compact.map(|sample| sample.1), material.map(|sample| sample.1));
    let compact = compact.map(|sample| sample.0);
    let material = material.map(|sample| sample.0);
    println!("instantiation compact={compact:?} mutation={material:?}");
    assert!(compact[1] <= 3 * compact[0], "{compact:?}");
    assert!(compact[2] <= 3 * compact[1], "{compact:?}");
    assert!(material[1] > 3 * material[0], "{material:?}");
    assert!(material[2] > 3 * material[1], "{material:?}");
}
