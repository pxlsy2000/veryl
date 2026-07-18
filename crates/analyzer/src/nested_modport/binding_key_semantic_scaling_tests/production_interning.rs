fn finalized_joint_axis_work(
    scale: usize,
    deep_clone_mutation: bool,
) -> (
    usize,
    super::super::identity_clone_work::KeyCloneWork,
    usize,
    Vec<(usize, usize, usize)>,
) {
    let session = AnalysisSessionId::new();
    let target = NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
            crate::ir::Signature::new(SymbolId(80_000)),
            [],
        )
        .into(),
    };
    let material_lowering = Arc::new(lowering(scale));
    let interface_key = target.clone();
    let mut inputs = Vec::with_capacity(scale);
    for index in 0..scale {
        let mut binding = super::binding_scaling_tests::direct_binding(
            session,
            index as u32,
            10_000,
            20_000,
        );
        let owner_signature = crate::ir::Signature::new(SymbolId(90_000));
        let mut connected_actual = crate::ir::Signature::new(SymbolId(80_000));
        connected_actual
            .full_path
            .push(resource_table::insert_str(&format!("binding_{index}")));
        for width_index in 0..scale {
            connected_actual.parameters.push((
                resource_table::insert_str(&format!("P{width_index}")),
                material_parameter(width_index),
            ));
            connected_actual.add_generic_parameter(
                resource_table::insert_str(&format!("G{width_index}")),
                generic_path(2, index + width_index),
            );
        }
        let owner = NestedModportLoweringKey {
            session,
            specialization: ComponentSpecializationIdentity::new(
                owner_signature,
                [ConnectedInterfaceSpecialization {
                    formal_port: resource_table::insert_str("child"),
                    actual: connected_actual,
                }],
            )
            .expect("unique connected actual")
            .into(),
        };
        let rewrite = OccurrenceRewriteKey {
            owner: owner.clone(),
            kind: OccurrenceKind::ExpressionIdentifier,
            token: TokenId(30_000 + index),
        };
        let port = ExpandedPortKey {
            owner: owner.clone(),
            token: TokenId(40_000 + index),
        };
        let rewrite_value = ResolvedPathRewrite {
            terminal: ResolvedTerminalRef::fixture(
                target.clone(),
                0,
                Arc::clone(&material_lowering),
                ResolvedNestedTerminalId(0),
            ),
            semantic_segments: material_lowering.terminals[0]
                .identifier
                .source_segments
                .clone(),
            replace_from_segment: 0,
            consumed_segments: scale,
        };
        let port_value = ExpandedPortResolution::Nested {
            target: target.clone(),
            modport: resource_table::insert_str("view"),
            interface: ResolvedExpandedPortInterface {
                symbol: SymbolId(80_000),
                generic_map: GenericMap::default(),
            },
            members: Arc::new(ResolvedExpandedMemberSet::default()),
        };
        binding.specialization = Arc::new(owner.clone());
        binding.emission_context = EmissionSpecializationContext::new(
            owner.specialization.as_ref().clone(),
            GenericMap::default(),
            None,
            Vec::new(),
        );
        binding.required_rewrites = Arc::from([rewrite.clone()]);
        binding.required_expanded_ports = Arc::from([port.clone()]);
        inputs.push((owner, rewrite, rewrite_value, port, port_value, binding));
    }
    reset_semantic_work();
    super::super::identity_clone_work::reset_key_clone_work();
    super::super::binding_specialization_identity::reset_registration_work();
    let mutation = deep_clone_mutation.then(
        super::super::binding_specialization_identity::inject_deep_clone_registration_key,
    );
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_interface_lowering(interface_key, Arc::clone(&material_lowering));
    for (owner, rewrite, rewrite_value, port, port_value, binding) in inputs {
        pending.record_lowering(owner, LoweringAvailability::NotNested);
        pending
            .record_rewrite(rewrite, rewrite_value)
            .expect("rewrite record");
        pending
            .record_expanded_port(port, port_value)
            .expect("expanded-port record");
        pending.record_emission_binding(binding);
    }
    let analysis = pending
        .finalize(session)
        .expect("joint-axis production fixture finalizes");
    drop(mutation);
    let rewrite_count = analysis.rewrite_keys().count();
    assert_eq!(rewrite_count, scale);
    let semantic_work = semantic_work();
    let frame_snapshot = analysis
        .emission_index
        .source(PathId(10_000))
        .expect("material source table")
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.declaration.0,
                binding.required_rewrites.len(),
                binding.required_expanded_ports.len(),
            )
        })
        .collect();
    (
        super::super::binding_specialization_identity::registration_work(),
        super::super::identity_clone_work::key_clone_work(),
        semantic_work,
        frame_snapshot,
    )
}

#[test]
fn production_binding_keys_do_not_rewalk_material_lowerings_per_binding() {
    let indexed = [8, 16, 32].map(|scale| finalized_joint_axis_work(scale, false));
    let rescanned = [8, 16, 32].map(|scale| finalized_joint_axis_work(scale, true));
    assert_eq!(
        indexed.each_ref().map(|value| &value.3),
        rescanned.each_ref().map(|value| &value.3)
    );
    let work = indexed.each_ref().map(|value| value.0);
    let clones = indexed.each_ref().map(|value| value.1);
    let semantic_work = indexed.each_ref().map(|value| value.2);
    let rescanned_work = rescanned.each_ref().map(|value| value.0);
    let rescanned_clones = rescanned.map(|value| value.1);
    println!(
        "binding work={work:?} semantic={semantic_work:?} mutation_work={rescanned_work:?} clones={clones:?} mutation_clones={rescanned_clones:?}"
    );
    assert!(work[0] > 0, "{work:?}");
    assert!(work[1] <= 3 * work[0], "{work:?}");
    assert!(work[2] <= 3 * work[1], "{work:?}");
    assert_eq!(rescanned_work, work);
    for (index, scale) in [8_usize, 16, 32].into_iter().enumerate() {
        assert!(clones[index].events <= 12 * scale, "{clones:?}");
        assert_eq!(clones[index].recursive_nodes, clones[index].events);
        assert_eq!(clones[index].allocations, 0);
        assert_eq!(rescanned_clones[index].events - clones[index].events, scale);
    }
    let recursive_delta = [0, 1, 2].map(|index| {
        rescanned_clones[index].recursive_nodes - clones[index].recursive_nodes
    });
    let allocation_delta = [0, 1, 2]
        .map(|index| rescanned_clones[index].allocations - clones[index].allocations);
    assert!(recursive_delta[1] > 3 * recursive_delta[0], "{recursive_delta:?}");
    assert!(recursive_delta[2] > 3 * recursive_delta[1], "{recursive_delta:?}");
    assert!(recursive_delta[2] > 8 * recursive_delta[0], "{recursive_delta:?}");
    assert!(allocation_delta[1] > 3 * allocation_delta[0], "{allocation_delta:?}");
    assert!(allocation_delta[2] > 3 * allocation_delta[1], "{allocation_delta:?}");
    assert!(allocation_delta[2] > 8 * allocation_delta[0], "{allocation_delta:?}");
    let material_units = [8_usize, 16, 32].map(|scale| scale * scale + scale);
    for index in 0..3 {
        assert!(semantic_work[index] <= 1024 * material_units[index]);
    }
    assert!(semantic_work[1] <= 5 * semantic_work[0]);
    assert!(semantic_work[2] <= 5 * semantic_work[1]);
}
