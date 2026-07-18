fn joint_axis_frame_query_work(
    scale: usize,
    owner_lookup_mutation: bool,
    hash_collision: bool,
) -> (usize, usize) {
    let collision = hash_collision
        .then(super::super::frame_record_index::inject_frame_record_hash_collision);
    let session = AnalysisSessionId::new();
    let source = PathId(800_000);
    let declaration = TokenId(800_001);
    let child = resource_table::insert_str("joint_axis_child");
    let payload = resource_table::insert_str("joint_axis_payload");
    let formal = resource_table::insert_str("joint_axis_formal");
    let mut pending = PendingNestedModportAnalysis::default();
    let mut expected_targets = Vec::new();
    for frame_index in 0..2 {
        let mut owner_signature = Signature::new(SymbolId(810_000 + frame_index));
        let mut actual_signature = Signature::new(SymbolId(820_000 + frame_index));
        for width_index in 0..scale {
            owner_signature.add_generic_parameter(
                resource_table::insert_str(&format!("OWNER_{width_index}")),
                super::semantic_work_scaling_tests::generic_path(2, width_index + frame_index),
            );
            actual_signature.add_generic_parameter(
                resource_table::insert_str(&format!("ACTUAL_{width_index}")),
                super::semantic_work_scaling_tests::generic_path(
                    2,
                    10_000 + width_index + frame_index,
                ),
            );
        }
        let owner = NestedModportLoweringKey {
            session,
            specialization: ComponentSpecializationIdentity::new(
                owner_signature,
                [ConnectedInterfaceSpecialization {
                    formal_port: formal,
                    actual: actual_signature,
                }],
            )
            .expect("unique connected actual")
            .into(),
        };
        let mut target_signature = Signature::new(SymbolId(830_000 + frame_index));
        target_signature
            .full_path
            .push(resource_table::insert_str(&format!("target_{frame_index}")));
        for width_index in 0..scale {
            target_signature.add_generic_parameter(
                resource_table::insert_str(&format!("TARGET_{width_index}")),
                super::semantic_work_scaling_tests::generic_path(
                    2,
                    20_000 + width_index + frame_index,
                ),
            );
        }
        let target = NestedModportLoweringKey {
            session,
            specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
                target_signature,
                [],
            )
            .into(),
        };
        let material = lowering(&[child, payload]);
        pending.record_lowering(owner.clone(), LoweringAvailability::NotNested);
        pending.record_interface_lowering(target.clone(), Arc::clone(&material));
        let mut required_rewrites = Vec::with_capacity(scale);
        let mut required_ports = Vec::with_capacity(scale);
        for index in 0..scale {
            let rewrite = OccurrenceRewriteKey {
                owner: owner.clone(),
                kind: OccurrenceKind::ExpressionIdentifier,
                token: TokenId(840_000 + index),
            };
            let port = ExpandedPortKey {
                owner: owner.clone(),
                token: TokenId(850_000 + index),
            };
            pending
                .record_rewrite(
                    rewrite.clone(),
                    ResolvedPathRewrite {
                        terminal: ResolvedTerminalRef::fixture(
                            target.clone(),
                            frame_index,
                            Arc::clone(&material),
                            ResolvedNestedTerminalId(0),
                        ),
                        semantic_segments: material.terminals[0]
                            .identifier
                            .source_segments
                            .clone(),
                        replace_from_segment: 0,
                        consumed_segments: 2,
                    },
                )
                .expect("joint-axis rewrite");
            pending
                .record_expanded_port(
                    port.clone(),
                    ExpandedPortResolution::Nested {
                        target: target.clone(),
                        modport: payload,
                        interface: ResolvedExpandedPortInterface {
                            symbol: target.specialization.owner.symbol,
                            generic_map: GenericMap::default(),
                        },
                        members: Arc::new(ResolvedExpandedMemberSet::default()),
                    },
                )
                .expect("joint-axis expanded port");
            required_rewrites.push(rewrite);
            required_ports.push(port);
        }
        let specialization = owner.specialization.clone();
        let mut binding = super::binding_scaling_tests::direct_binding(
            session,
            frame_index as u32,
            source.0,
            declaration.0,
        );
        binding.specialization = Arc::new(owner);
        binding.emission_context = EmissionSpecializationContext::new(
            specialization.as_ref().clone(),
            GenericMap::default(),
            None,
            Vec::new(),
        );
        binding.required_rewrites = required_rewrites.into();
        binding.required_expanded_ports = required_ports.into();
        pending.record_emission_binding(binding);
        expected_targets.push(target);
    }
    let analysis = pending
        .finalize(session)
        .expect("joint-axis frame fixture finalizes");
    let mut prepared = analysis
        .prepare_emission(source, EmissionPhase::Build)
        .expect("joint-axis source prepares");
    let batch = prepared
        .take_owners(declaration, EmissionOwnerKind::Interface)
        .expect("joint-axis frames");
    let mutation = owner_lookup_mutation.then(
        super::super::frame_record_query_mutation::inject_terminal_availability_lookup_mutation,
    );
    reset_semantic_work();
    let mut resolved = 0;
    for (frame, expected_target) in batch.iter().zip(&expected_targets) {
        for index in 0..scale {
            let rewrite = frame
                .rewrite(
                    OccurrenceKind::ExpressionIdentifier,
                    TokenId(840_000 + index),
                )
                .expect("required rewrite")
                .expect("published rewrite");
            assert_eq!(
                frame
                    .resolve_terminal(&rewrite.terminal)
                    .expect("frozen terminal resolution")
                    .id,
                ResolvedNestedTerminalId(0)
            );
            let port = frame
                .expanded_port(TokenId(850_000 + index))
                .expect("required expanded port")
                .expect("published expanded port");
            assert!(matches!(
                port,
                ExpandedPortResolution::Nested { interface, .. }
                    if interface.symbol == expected_target.specialization.owner.symbol
            ));
            resolved += 2;
        }
        assert!(matches!(
            frame.rewrite(
                OccurrenceKind::ExpressionIdentifier,
                TokenId(860_000 + scale),
            ),
            Err(NestedModportAnalysisInvariant::RecordNotRequired)
        ));
        assert!(frame
            .published_expanded_port(TokenId(870_000 + scale))
            .is_none());
    }
    let work = semantic_work();
    drop(mutation);
    drop(collision);
    (work, resolved)
}

#[test]
fn frame_queries_do_not_hash_material_owner_specializations_per_record() {
    let indexed = [8, 16, 32].map(|scale| joint_axis_frame_query_work(scale, false, false));
    let legacy = [8, 16, 32].map(|scale| joint_axis_frame_query_work(scale, true, false));
    assert_eq!(indexed.map(|value| value.1), legacy.map(|value| value.1));
    assert_eq!(indexed.map(|value| value.1), [32, 64, 128]);
    let work = indexed.map(|value| value.0);
    let legacy = legacy.map(|value| value.0);
    println!("frame joint-axis indexed={work:?} mutation={legacy:?}");
    assert!(work[0] > 0, "{work:?}");
    assert!(work[1] <= 3 * work[0], "{work:?}");
    assert!(work[2] <= 3 * work[1], "{work:?}");
    assert!(legacy[1] > 3 * legacy[0], "{legacy:?}");
    assert!(legacy[2] > 3 * legacy[1], "{legacy:?}");
}
