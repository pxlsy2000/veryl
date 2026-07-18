fn register_fixture(binding_count: usize) -> Fixture {
    symbol_table::clear();
    attribute_table::clear();
    let metadata = Metadata::create_default("scale").expect("metadata fixture");
    let analyzer = Analyzer::new(&metadata);
    let half = binding_count / 2;
    let inputs = [
        (PathBuf::from("scale_a.veryl"), source("PkgA", "fa", half)),
        (
            PathBuf::from("scale_b.veryl"),
            source("PkgB", "fb", binding_count - half),
        ),
    ];
    for (path, code) in &inputs {
        let parser = Parser::parse(code, path).expect("generated package should parse");
        assert!(analyzer.analyze_pass1("scale", &parser.veryl).is_empty());
    }
    assert!(Analyzer::analyze_post_pass1().is_empty());
    let source_ids: Vec<_> = inputs
        .iter()
        .map(|(path, _)| {
            veryl_parser::resource_table::get_path_id(path).expect("source path should be interned")
        })
        .collect();
    let mut functions: Vec<_> = symbol_table::get_all()
        .into_iter()
        .filter(|symbol| {
            matches!(symbol.kind, SymbolKind::Function(_))
                && symbol
                    .token
                    .source
                    .get_path()
                    .is_some_and(|source| source_ids.contains(&source))
        })
        .collect();
    functions.sort_by_key(|symbol| (symbol.token.source.get_path(), symbol.token.id));
    assert_eq!(functions.len(), binding_count);
    let generic_interfaces: Vec<_> = symbol_table::get_all()
        .into_iter()
        .filter(|symbol| {
            matches!(symbol.kind, SymbolKind::Interface(_))
                && symbol.token.to_string().contains("GenericIf")
                && symbol
                    .token
                    .source
                    .get_path()
                    .is_some_and(|source| source_ids.contains(&source))
        })
        .collect();
    assert_eq!(generic_interfaces.len(), 2);
    assert!(generic_interfaces
        .iter()
        .all(|symbol| symbol.generic_maps().len() == binding_count / 2));

    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    let child = resource_table::insert_str("child");
    let payload = resource_table::insert_str("payload");
    let port = resource_table::insert_str("port");
    let parent_path = resource_table::insert_str("material_parent");
    let mut parent_declarations = HashMap::default();
    reset_collection_work();
    reset_semantic_work();
    for (index, function) in functions.iter().rev().enumerate() {
        let source = function.token.source.get_path().expect("function source");
        let interface = generic_interfaces
            .iter()
            .find(|interface| interface.token.source.get_path() == Some(source))
            .expect("every function source has one parsed generic interface");
        let maps = interface.generic_maps();
        let map = &maps[if index % 2 == 0 { 7 } else { 15 }];
        let mut actual = Signature::new(interface.id);
        for (name, _) in interface.generic_parameters() {
            actual.add_generic_parameter(
                name,
                map.map.get(&name).expect("W=8/16 generic argument").clone(),
            );
        }
        let specialization = NestedModportLoweringKey {
            session,
            specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
                Signature::new(function.id),
                [ConnectedInterfaceSpecialization {
                    formal_port: port,
                    actual: actual.clone(),
                }],
            )
            .into(),
        };
        let mut target_signature = Signature::new(function.id);
        target_signature.full_path.push(resource_table::insert_str("lowering_target"));
        let target = NestedModportLoweringKey {
            session,
            specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
                target_signature,
                [],
            )
            .into(),
        };
        pending.record_lowering(
            specialization.clone(),
            LoweringAvailability::NotNested,
        );
        pending.record_interface_lowering(target.clone(), lowering(&[child, payload]));
        let rewrite = OccurrenceRewriteKey {
            owner: specialization.clone(),
            kind: OccurrenceKind::ExpressionIdentifier,
            token: TokenId(100_000 + index),
        };
        pending
            .record_rewrite_candidate(
                rewrite,
                PendingPathRewriteCandidate {
                    target: target.clone(),
                    semantic_segments: vec![child, payload],
                    replace_from_segment: 0,
                    consumed_segments: 2,
                },
            )
            .expect("rewrite candidate");
        pending
            .record_expanded_port_candidate(
                ExpandedPortKey {
                    owner: specialization.clone(),
                    token: TokenId(200_000 + index),
                },
                PendingExpandedPortCandidate {
                    target,
                    modport: payload,
                },
            )
            .expect("expanded port candidate");
        let context = EmissionSpecializationContext::from_specialization(
            EmissionOwnerKind::Function,
            &specialization.specialization,
        )
        .expect("function specialization should be valid");
        for _ in 0..2 {
            pending
                .record_emission_owner(
                    source,
                    function.token.id,
                    EmissionOwnerKind::Function,
                    specialization.clone(),
                    context.clone(),
                    LoweringAvailability::NotNested,
                )
                .expect("duplicate registration should be idempotent");
        }
        let parent = (index % 2 == 0).then(|| {
            let mut signature = Signature::new(function.id);
            signature.full_path.push(parent_path);
            let parent = NestedModportLoweringKey {
                session,
                specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
                    signature,
                    [],
                )
                .into(),
            };
            let declaration = function.token.id;
            pending.record_lowering(parent.clone(), LoweringAvailability::NotNested);
            pending
                .record_emission_owner(
                    source,
                    declaration,
                    EmissionOwnerKind::Interface,
                    parent.clone(),
                    EmissionSpecializationContext::from_specialization(
                        EmissionOwnerKind::Interface,
                        &parent.specialization,
                    )
                    .expect("parent context"),
                    LoweringAvailability::NotNested,
                )
                .expect("parent registration");
            parent_declarations.insert(function.id, declaration);
            parent.specialization
        });
        pending.set_emission_owner_parent(
            &specialization,
            parent,
            false,
            Some(GenericMap::default()),
        );
        let owner = PendingGenericEmissionOwner {
            session,
            source,
            declaration: function.token.id,
            kind: EmissionOwnerKind::Function,
            symbol: function.id,
        };
        pending.record_generic_emission_owner(owner.clone());
        pending.record_generic_emission_owner(owner);
        pending
            .record_instantiation_context_candidate(
                InstantiationContextKey {
                    owner: specialization.clone(),
                    token: function.token.id,
                },
                PendingInstantiationContextCandidate {
                    target: specialization,
                },
            )
            .expect("candidate should register");
    }
    for interface in &generic_interfaces {
        pending.record_generic_emission_owner(PendingGenericEmissionOwner {
            session,
            source: interface.token.source.get_path().expect("interface source"),
            declaration: interface.token.id,
            kind: EmissionOwnerKind::Interface,
            symbol: interface.id,
        });
    }
    Fixture {
        pending,
        session,
        functions,
        generic_interfaces,
        parent_declarations,
        sources: source_ids,
    }
}
