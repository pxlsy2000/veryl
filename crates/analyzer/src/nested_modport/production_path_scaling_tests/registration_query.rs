#[test]
fn real_registration_finalize_and_query_path_is_bounded_at_1x_2x_4x() {
    let mut totals = Vec::new();
    for binding_count in [64, 128, 256] {
        let mut fixture = register_fixture(binding_count);
        let registration_semantic_work = semantic_work();
        assert_eq!(fixture.pending.rewrite_candidates.len(), binding_count);
        assert_eq!(fixture.pending.expanded_port_candidates.len(), binding_count);
        assert_eq!(fixture.pending.rewrite_order_by_owner.len(), binding_count);
        assert_eq!(fixture.pending.expanded_port_order_by_owner.len(), binding_count);
        assert_eq!(
            fixture
                .pending
            .emission_bindings
            .iter()
            .filter(|binding| !binding.specialization.specialization.connected_actuals.is_empty())
            .count(),
            binding_count
        );
        assert_eq!(fixture.pending.interface_lowerings.len(), binding_count);
        assert_eq!(
            fixture
                .pending
                .emission_bindings
                .iter()
                .filter(|binding| {
                    binding.kind == EmissionOwnerKind::Function
                        && fixture.pending.inferred_binding_ids.contains(&binding.id)
                })
                .count(),
            binding_count
        );
        assert_eq!(
            fixture
                .generic_interfaces
                .iter()
                .map(|interface| interface.generic_maps().len())
                .sum::<usize>(),
            binding_count
        );
        let expected_connected_maps: HashSet<_> = fixture
            .generic_interfaces
            .iter()
            .flat_map(|interface| {
                let maps = interface.generic_maps();
                [
                    SemanticGenericMap::from(&maps[7]),
                    SemanticGenericMap::from(&maps[15]),
                ]
            })
            .collect();
        assert_eq!(expected_connected_maps.len(), 2);
        for binding in fixture
            .pending
            .emission_bindings
            .iter()
            .filter(|binding| binding.kind == EmissionOwnerKind::Function)
        {
            let connected = binding
                .specialization
                .specialization
                .connected_actuals
                .first()
                .expect("every material function has one connected actual");
            let source = binding.source;
            let interface = fixture
                .generic_interfaces
                .iter()
                .find(|interface| interface.token.source.get_path() == Some(source))
                .expect("connected actual uses the parsed interface from its source");
            assert_eq!(connected.actual.symbol, interface.id);
            assert!(!connected.actual.generic_parameters.is_empty());
            assert!(expected_connected_maps
                .contains(&SemanticGenericMap::from_signature(&connected.actual)));
        }
        assert_eq!(fixture.parent_declarations.len(), binding_count / 2);
        reset_semantic_work();
        let analysis = fixture
            .pending
            .finalize(fixture.session)
            .expect("real registration fixture should finalize");
        let build_collection_work = collection_work();
        let build_semantic_work = registration_semantic_work + semantic_work();
        reset_prepare_emission_work();
        reset_semantic_work();
        let mut consumed_generic_frames = 0;
        let mut consumed_function_frames = 0;
        let mut consumed_parent_frames = 0;
        let mut consumed_package_scopes = 0;
        let mut consumed_rewrites = 0;
        let mut consumed_expanded_ports = 0;
        let mut seen_connected_maps = HashSet::default();
        for source in fixture.sources {
            let source_functions: Vec<_> = fixture
                .functions
                .iter()
                .filter(|function| function.token.source.get_path() == Some(source))
                .collect();
            assert!(!source_functions.is_empty(), "every material source must have functions");
            assert_eq!(source_functions.len(), binding_count / 2);
            let mut prepared = analysis
                .prepare_emission(source, EmissionPhase::Build)
                .expect("source should prepare");
            for interface in fixture
                .generic_interfaces
                .iter()
                .filter(|interface| interface.token.source.get_path() == Some(source))
            {
                let batch = prepared
                    .take_owners(interface.token.id, EmissionOwnerKind::Interface)
                    .expect("completed generic interface owners");
                let count = batch.iter().count();
                assert_eq!(count, binding_count / 2);
                consumed_generic_frames += count;
            }
            for function in source_functions {
                let parent_batch = fixture.parent_declarations.get(&function.id).map(|declaration| {
                    prepared
                        .take_owners(*declaration, EmissionOwnerKind::Interface)
                        .expect("exact parent query")
                });
                let parent = parent_batch.as_ref().and_then(|batch| {
                    let mut frames = batch.iter();
                    let parent = frames.next();
                    assert!(frames.next().is_none());
                    consumed_parent_frames += usize::from(parent.is_some());
                    parent
                });
                let scope = if parent.is_none() {
                    let package = symbol_table::get_namespace_symbol(&function.namespace)
                        .expect("function package should exist");
                    prepared
                        .package_scope(package.token.id, package.id, &GenericMap::default())
                        .expect("package scope query should succeed")
                } else {
                    None
                };
                if scope.is_some() {
                    consumed_package_scopes += 1;
                }
                let batch = prepared
                    .take_function_owners(function.token.id, parent, scope.as_ref())
                    .expect("function query should succeed");
                let mut frames = batch.iter();
                let frame = frames.next().expect("one function frame");
                assert!(frames.next().is_none());
                consumed_function_frames += 1;
                let connected = frame
                    .specialization()
                    .specialization
                    .connected_actuals
                    .first()
                    .expect("finalized function frame retains one connected actual");
                let interface = fixture
                    .generic_interfaces
                    .iter()
                    .find(|interface| interface.token.source.get_path() == Some(source))
                    .expect("source-specific generic interface");
                assert_eq!(connected.actual.symbol, interface.id);
                assert!(!connected.actual.generic_parameters.is_empty());
                let connected_map = SemanticGenericMap::from_signature(&connected.actual);
                without_semantic_work(|| {
                    assert!(expected_connected_maps.contains(&connected_map));
                    seen_connected_maps.insert(connected_map);
                });
                let sorted_index = fixture
                    .functions
                    .iter()
                    .position(|candidate| candidate.id == function.id)
                    .expect("registered function");
                let record_index = binding_count - 1 - sorted_index;
                let rewrite = frame
                    .rewrite(
                        OccurrenceKind::ExpressionIdentifier,
                        TokenId(100_000 + record_index),
                    )
                    .expect("rewrite query")
                    .expect("resolved rewrite");
                consumed_rewrites += 1;
                assert!(frame
                    .expanded_port(TokenId(200_000 + record_index))
                    .expect("expanded port query")
                    .is_some());
                consumed_expanded_ports += 1;
                frame
                    .resolve_terminal(&rewrite.terminal)
                    .expect("rewrite terminal and lowering query");
            }
            prepared.finish().expect("all functions should be consumed");
        }
        assert_eq!(consumed_generic_frames, binding_count);
        assert_eq!(consumed_function_frames, binding_count);
        assert_eq!(consumed_parent_frames, binding_count / 2);
        assert_eq!(consumed_package_scopes, binding_count / 2);
        assert_eq!(consumed_rewrites, binding_count);
        assert_eq!(consumed_expanded_ports, binding_count);
        without_semantic_work(|| assert_eq!(seen_connected_maps, expected_connected_maps));
        totals.push((
            binding_count,
            build_collection_work,
            build_semantic_work,
            prepare_emission_work(),
            semantic_work(),
        ));
    }
    for &(bindings, build_collection, _, query_collection, _) in &totals {
        println!("registration bindings={bindings} build={build_collection} query={query_collection}");
        assert_eq!(build_collection, 523 * bindings / 2 + 4_661);
        assert_eq!(query_collection, 21 * bindings + 8);
    }
    for pair in totals.windows(2) {
        let (_, build_collection_left, build_left, query_collection_left, query_left) = pair[0];
        let (
            _,
            build_collection_right,
            build_right,
            query_collection_right,
            query_right,
        ) = pair[1];
        assert!(build_left > 0 && build_right <= 3 * build_left);
        assert!(query_left > 0 && query_right <= 3 * query_left);
        assert!(build_collection_right + build_right <= 3 * (build_collection_left + build_left));
        assert!(query_collection_right + query_right <= 3 * (query_collection_left + query_left));
    }
}
