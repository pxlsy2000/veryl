#[test]
fn frame_local_query_work_is_constant_for_late_record_membership_and_map_probes() {
    let work = [1, 2, 4].map(measured_frame_local_query_work);
    assert!(work[0] > 0);
    let minimum = *work.iter().min().expect("three measurements");
    let maximum = *work.iter().max().expect("three measurements");
    assert!(maximum <= minimum + 8, "{work:?}");
}

fn measured_all_frame_local_query_work(record_count: usize, scan_mutation: bool) -> (usize, usize) {
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
    let mut resolved = 0;
    for index in 0..record_count {
        fixture
            .pending
            .record_rewrite_candidate(
                OccurrenceRewriteKey {
                    owner: owner.as_ref().clone(),
                    kind: OccurrenceKind::ExpressionIdentifier,
                    token: TokenId(600_000 + index),
                },
                rewrite_candidate.clone(),
            )
            .expect("additional rewrite candidate");
        fixture
            .pending
            .record_expanded_port_candidate(
                ExpandedPortKey {
                    owner: owner.as_ref().clone(),
                    token: TokenId(700_000 + index),
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
    let frame = batch.iter().next().expect("one function frame");
    let mutation = scan_mutation.then(
        super::super::frame_record_query_mutation::inject_required_record_scan_mutation,
    );
    reset_semantic_work();
    for index in 0..record_count {
        assert!(frame
            .rewrite(
                OccurrenceKind::ExpressionIdentifier,
                TokenId(600_000 + index),
            )
            .expect("required rewrite")
            .is_some());
        assert!(frame
            .expanded_port(TokenId(700_000 + index))
            .expect("required expanded port")
            .is_some());
        resolved += 2;
    }
    let work = semantic_work();
    drop(mutation);
    (work, resolved)
}

#[test]
fn querying_every_required_frame_record_has_affine_work() {
    let indexed = [32, 64, 128].map(|count| measured_all_frame_local_query_work(count, false));
    let scanned = [32, 64, 128].map(|count| measured_all_frame_local_query_work(count, true));
    assert_eq!(indexed.map(|value| value.1), scanned.map(|value| value.1));
    let work = indexed.map(|value| value.0);
    let scanned = scanned.map(|value| value.0);
    println!("frame indexed={work:?} mutation={scanned:?}");
    assert!(work[0] > 0, "{work:?}");
    assert!(work[1] <= 3 * work[0], "{work:?}");
    assert!(work[2] <= 3 * work[1], "{work:?}");
    assert!(scanned[1] > 3 * scanned[0], "{scanned:?}");
    assert!(scanned[2] > 3 * scanned[1], "{scanned:?}");
}
