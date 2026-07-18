#[test]
fn finalizer_binds_function_parent_by_full_component_emission_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let emitted_parent = key_in_session(session, 1, 16)?;
    let conversion_parent = Arc::new(emitted_parent.clone());
    let function = key_in_session(session, 2, 8)?;
    let mut component_binding = direct_binding(&emitted_parent, 1, 5);
    component_binding.kind = EmissionOwnerKind::Module;
    let mut function_binding = direct_binding(&function, 2, 10);
    function_binding.kind = EmissionOwnerKind::Function;
    function_binding.enclosing_owner = Some(conversion_parent);
    function_binding.enclosing_generic_map = Some(Default::default());
    function_binding
        .emission_context
        .test_set_enclosing_generic_map(Default::default());
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(emitted_parent.clone(), LoweringAvailability::NotNested);
    pending.record_lowering(function, LoweringAvailability::NotNested);
    pending.record_emission_binding(component_binding);
    pending.record_emission_binding(function_binding);
    let analysis = pending.finalize(session)?;

    for phase in [EmissionPhase::Align, EmissionPhase::Build] {
        let mut prepared = analysis.prepare_emission(PathId(1), phase)?;
        let enclosing = prepared
            .take_owners(TokenId(5), EmissionOwnerKind::Module)?
            .iter()
            .next()
            .ok_or("component frame must exist")?;
        assert_eq!(
            prepared
                .take_function_owners(TokenId(10), Some(enclosing), None)?
                .iter()
                .count(),
            1
        );
        prepared.finish()?;
    }
    Ok(())
}

#[test]
fn package_scope_handles_use_exact_identity_and_reject_alternate_actuals()
-> Result<(), Box<dyn std::error::Error>> {
    let key = lowering_key()?;
    let a = resource_table::insert_str("A");
    let b = resource_table::insert_str("B");
    let value =
        |width: usize| GenericSymbolPath::from(&Token::from_external_text(&width.to_string()));
    let mut scope_map = GenericMap::default();
    scope_map.map.insert(a, value(8));
    scope_map.map.insert(b, value(16));
    let mut binding = direct_binding(&key, 1, 10);
    binding.kind = EmissionOwnerKind::Function;
    binding
        .emission_context
        .test_set_enclosing_generic_map(Default::default());
    let package_declaration = TokenId(20);
    let package_symbol = key.specialization.owner.symbol;
    binding.package_scope = Some(EmissionScopeIdentity::test_scope(
        key.session,
        PathId(1),
        package_declaration,
        package_symbol,
        &scope_map,
    ));
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(binding);
    let analysis = pending.finalize(key.session)?;
    let mut prepared = analysis.prepare_emission(PathId(1), EmissionPhase::Build)?;

    let mut first = GenericMap::default();
    first.map.insert(b, value(16));
    let mut second = GenericMap::default();
    second.map.insert(a, value(8));
    let mut first_order = first.clone();
    first_order.map.extend(second.map.clone());
    let first_handle = prepared
        .package_scope(package_declaration, package_symbol, &first_order)?
        .ok_or("matching package scope must exist")?;
    let frame = prepared
        .take_function_owners(TokenId(10), None, Some(&first_handle))?
        .iter()
        .next()
        .ok_or("function frame should exist")?;
    assert!(frame.matches_scope(&first_handle));
    let mut second_order = second;
    second_order.map.extend(first.map);
    let second_handle = prepared
        .package_scope(package_declaration, package_symbol, &second_order)?
        .ok_or("matching package scope must exist")?;
    assert!(frame.matches_scope(&second_handle));
    let mut alternate = GenericMap::default();
    alternate.map.insert(a, value(8));
    alternate.map.insert(b, value(32));
    assert!(matches!(
        prepared.package_scope(package_declaration, package_symbol, &alternate),
        Err(NestedModportAnalysisInvariant::PackageScopeMismatch)
    ));
    Ok(())
}

#[test]
fn package_scope_nonempty_generic_matching_is_linear_in_total_map_input()
-> Result<(), Box<dyn std::error::Error>> {
    fn measure(width: usize, extras: usize) -> Result<usize, Box<dyn std::error::Error>> {
        let key = lowering_key()?;
        let value = |suffix: usize| GenericSymbolPath {
            paths: vec![GenericSymbol {
                base: Token::from_external_text(&format!("layer_{suffix}")),
                arguments: vec![GenericSymbolPath::from(&Token::from_external_text(
                    &suffix.to_string(),
                ))],
            }],
            kind: GenericSymbolPathKind::ValueLiteral,
            range: Default::default(),
        };
        let mut scope_map = GenericMap::default();
        let mut actual = GenericMap::default();
        for index in 0..width {
            let name = resource_table::insert_str(&format!("P{index}"));
            let path = value(index);
            scope_map.map.insert(name, path.clone());
            actual.map.insert(name, path);
        }
        for index in 0..extras {
            actual.map.insert(
                resource_table::insert_str(&format!("UNRELATED{index}")),
                value(width + index),
            );
        }
        let mut binding = direct_binding(&key, 1, 10);
        binding.kind = EmissionOwnerKind::Function;
        binding
            .emission_context
            .test_set_enclosing_generic_map(Default::default());
        let declaration = TokenId(20);
        let symbol = key.specialization.owner.symbol;
        binding.package_scope = Some(EmissionScopeIdentity::test_scope(
            key.session,
            PathId(1),
            declaration,
            symbol,
            &scope_map,
        ));
        let mut pending = PendingNestedModportAnalysis::default();
        pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
        pending.record_emission_binding(binding);
        let analysis = pending.finalize(key.session)?;
        let prepared = analysis.prepare_emission(PathId(1), EmissionPhase::Build)?;
        reset_semantic_work();
        assert!(
            prepared
                .package_scope(declaration, symbol, &actual)?
                .is_some()
        );
        let mut changed = actual.clone();
        changed
            .map
            .insert(resource_table::insert_str("P0"), value(10_000));
        assert!(matches!(
            prepared.package_scope(declaration, symbol, &changed),
            Err(NestedModportAnalysisInvariant::PackageScopeMismatch)
        ));
        let mut missing = actual;
        missing.map.remove(&resource_table::insert_str("P0"));
        assert!(matches!(
            prepared.package_scope(declaration, symbol, &missing),
            Err(NestedModportAnalysisInvariant::PackageScopeMismatch)
        ));
        Ok(semantic_work())
    }

    let work: Vec<_> = [1, 2, 4]
        .into_iter()
        .map(|width| measure(width, width))
        .collect::<Result<_, _>>()?;
    assert!(work[1] > work[0]);
    assert!(work[2] > work[1]);
    assert!(work[1] <= 3 * work[0]);
    assert!(work[2] <= 3 * work[1]);
    let fixed_expected: Vec<_> = [1, 64, 256]
        .into_iter()
        .map(|extras| measure(1, extras))
        .collect::<Result<_, _>>()?;
    assert_eq!(fixed_expected, vec![fixed_expected[0]; 3]);
    Ok(())
}
