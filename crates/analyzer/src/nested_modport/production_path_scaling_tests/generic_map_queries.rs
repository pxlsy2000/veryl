#[test]
fn canonical_generic_map_builds_one_counted_index_for_many_exact_queries() {
    let fixture = register_fixture(64);
    let symbol = &fixture.generic_interfaces[0];
    let maps = symbol.generic_maps();
    assert_eq!(maps.len(), 32);
    clear_canonical_generic_map_cache();
    reset_collection_work();
    for map in &maps {
        let mut signature = Signature::new(symbol.id);
        for (name, _) in symbol.generic_parameters() {
            signature.add_generic_parameter(
                name,
                map.map.get(&name).expect("generic map parameter").clone(),
            );
        }
        let specialization = ComponentSpecializationIdentity::from_unique_connected_actuals(
            signature,
            [],
        );
        let context = EmissionSpecializationContext::from_specialization(
            EmissionOwnerKind::Interface,
            &specialization,
        )
        .expect("canonical generic map query");
        assert_eq!(
            SemanticGenericMap::from(&context.generic_map),
            SemanticGenericMap::from(map)
        );
    }
    assert_eq!(collection_work(), maps.len());
}

fn measured_frame_local_query_work(record_count: usize) -> usize {
    let mut fixture = register_fixture(64);
    let function = fixture.functions[0].clone();
    let source = function.token.source.get_path().expect("function source");
    let owner = fixture
        .pending
        .emission_bindings
        .iter()
        .find(|binding| {
            binding.kind == EmissionOwnerKind::Function
                && binding.specialization.specialization.owner.symbol == function.id
        })
        .expect("function binding")
        .specialization
        .clone();
    let rewrite_candidate = fixture
        .pending
        .rewrite_candidates
        .iter()
        .find(|(key, _)| key.owner == *owner)
        .map(|(_, candidate)| candidate.clone())
        .expect("base rewrite candidate");
    let port_candidate = fixture
        .pending
        .expanded_port_candidates
        .iter()
        .find(|(key, _)| key.owner == *owner)
        .map(|(_, candidate)| candidate.clone())
        .expect("base expanded-port candidate");
    for index in 0..record_count {
        fixture
            .pending
            .record_rewrite_candidate(
                OccurrenceRewriteKey {
                    owner: owner.as_ref().clone(),
                    kind: OccurrenceKind::ExpressionIdentifier,
                    token: TokenId(300_000 + index),
                },
                rewrite_candidate.clone(),
            )
            .expect("additional rewrite candidate");
        fixture
            .pending
            .record_expanded_port_candidate(
                ExpandedPortKey {
                    owner: owner.as_ref().clone(),
                    token: TokenId(400_000 + index),
                },
                port_candidate.clone(),
            )
            .expect("additional expanded-port candidate");
    }
    let analysis = fixture
        .pending
        .finalize(fixture.session)
        .expect("frame-local fixture finalizes");
    let mut prepared = analysis
        .prepare_emission(source, EmissionPhase::Build)
        .expect("source prepares");
    let interface = fixture
        .generic_interfaces
        .iter()
        .find(|interface| interface.token.source.get_path() == Some(source))
        .expect("source interface");
    prepared
        .take_owners(interface.token.id, EmissionOwnerKind::Interface)
        .expect("generic interface frames");
    let parent_batch = fixture.parent_declarations.get(&function.id).map(|declaration| {
        prepared
            .take_owners(*declaration, EmissionOwnerKind::Interface)
            .expect("parent frame")
    });
    let parent = parent_batch.as_ref().and_then(|batch| batch.iter().next());
    let scope = if parent.is_none() {
        let package = symbol_table::get_namespace_symbol(&function.namespace)
            .expect("function package");
        prepared
            .package_scope(package.token.id, package.id, &GenericMap::default())
            .expect("package scope")
    } else {
        None
    };
    let batch = prepared
        .take_function_owners(function.token.id, parent, scope.as_ref())
        .expect("function frame");
    reset_semantic_work();
    let frame = batch.iter().next().expect("one function frame");
    assert!(frame
        .rewrite(
            OccurrenceKind::ExpressionIdentifier,
            TokenId(300_000 + record_count - 1),
        )
        .expect("late rewrite")
        .is_some());
    assert!(frame
        .expanded_port(TokenId(400_000 + record_count - 1))
        .expect("late expanded port")
        .is_some());
    semantic_work()
}
