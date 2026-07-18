fn material_shared_parent_query_work(scale: usize, mutation: bool) -> (usize, usize) {
    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    let mut parent_signature = Signature::new(SymbolId(700_000));
    let mut actual_signature = Signature::new(SymbolId(700_001));
    for index in 0..scale {
        parent_signature.add_generic_parameter(
            veryl_parser::resource_table::insert_str(&format!("PARENT_{index}")),
            super::semantic_work_scaling_tests::generic_path(2, index),
        );
        actual_signature.add_generic_parameter(
            veryl_parser::resource_table::insert_str(&format!("ACTUAL_{index}")),
            super::semantic_work_scaling_tests::generic_path(2, 10_000 + index),
        );
    }
    let parent_identity = ComponentSpecializationIdentity::new(
        parent_signature,
        [ConnectedInterfaceSpecialization {
            formal_port: veryl_parser::resource_table::insert_str("child"),
            actual: actual_signature,
        }],
    )
    .expect("material connected parent");
    let mut parent = direct_binding(session, 0, 1, 700_010);
    parent.specialization = Arc::new(NestedModportLoweringKey {
        session,
        specialization: parent_identity.clone().into(),
    });
    parent.emission_context = EmissionSpecializationContext::new(
        parent_identity.clone(),
        GenericMap::default(),
        None,
        Vec::new(),
    );
    pending.record_lowering(
        parent.specialization.as_ref().clone(),
        LoweringAvailability::NotNested,
    );
    let parent_specialization = Arc::clone(&parent.specialization);
    pending.record_emission_binding(parent);
    for index in 0..scale {
        let mut function = package_function_binding(
            session,
            (index + 1) as u32,
            1,
            710_000 + index,
            720_000 + index,
            EmissionScopeIdentity::test_scope(
                session,
                PathId(1),
                TokenId(730_000 + index),
                SymbolId(740_000 + index),
                &GenericMap::default(),
            ),
        );
        function.enclosing_owner = Some(Arc::clone(&parent_specialization));
        function.enclosing_generic_map = Some(GenericMap::default());
        function.package_scope = None;
        pending.record_lowering(
            function.specialization.as_ref().clone(),
            LoweringAvailability::NotNested,
        );
        pending.record_emission_binding(function);
    }
    let analysis = pending.finalize(session).expect("material owner finalizes");
    let mut prepared = analysis
        .prepare_emission(PathId(1), EmissionPhase::Build)
        .expect("material source prepares");
    let parent_batch = prepared
        .take_owners(TokenId(700_010), EmissionOwnerKind::Interface)
        .expect("material parent frame");
    let parent = parent_batch.iter().next().expect("one material parent");
    let guard = mutation.then(
        super::super::function_owner_query_mutation::inject_material_function_owner_mutation,
    );
    super::semantic_work::reset_semantic_work();
    let mut resolved = 0;
    for index in 0..scale {
        resolved += prepared
            .take_function_owners(TokenId(710_000 + index), Some(parent), None)
            .expect("compact owner query")
            .iter()
            .count();
    }
    let work = super::semantic_work::semantic_work();
    drop(guard);
    prepared.finish().expect("all material functions consumed");
    (work, resolved)
}

#[test]
fn function_queries_use_compact_session_owner_handles_on_joint_axes() {
    let compact = [8, 16, 32].map(|scale| material_shared_parent_query_work(scale, false));
    let material = [8, 16, 32].map(|scale| material_shared_parent_query_work(scale, true));
    assert_eq!(compact.map(|sample| sample.1), [8, 16, 32]);
    assert_eq!(compact.map(|sample| sample.1), material.map(|sample| sample.1));
    let compact = compact.map(|sample| sample.0);
    let material = material.map(|sample| sample.0);
    println!("function compact={compact:?} mutation={material:?}");
    assert!(compact[1] <= 3 * compact[0], "{compact:?}");
    assert!(compact[2] <= 3 * compact[1], "{compact:?}");
    assert!(material[1] > 3 * material[0], "{material:?}");
    assert!(material[2] > 3 * material[1], "{material:?}");
}
