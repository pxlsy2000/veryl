#[test]
fn nested_terminal_types_and_unrelated_state_are_specialization_local() {
    let pending = PendingComponentLowering {
        declarations: vec![pending_declaration(
            "sink",
            vec![(&["child", "ready"], Direction::Input)],
            None,
        )],
    };
    let mut variables8 = default_fixture_variables(8);
    variables8.insert(
        VarId::from_raw(99),
        variable(99, &["unrelated", "ready"], 99, &[]),
    );
    let lowering8 = resolve_pending_nested_modport_lowering(
        &pending,
        &variables8,
        &HashMap::from_iter([(VarId::from_raw(98), function(98, "unrelated_import"))]),
        16,
    )
    .expect("unrelated state must not affect path-local legality")
    .expect("nested member requires lowering");
    let lowering16 = resolve_pending_nested_modport_lowering(
        &pending,
        &default_fixture_variables(16),
        &HashMap::default(),
        16,
    )
    .expect("second specialization should resolve independently")
    .expect("nested member requires lowering");

    assert_eq!(
        lowering8.terminals[0]
            .resolved_type
            .declaration
            .packed
            .as_slice(),
        &[Some(8)]
    );
    assert_eq!(
        lowering16.terminals[0]
            .resolved_type
            .declaration
            .packed
            .as_slice(),
        &[Some(16)]
    );
    assert_eq!(
        lowering8.terminals[0]
            .resolved_type
            .declaration
            .unpacked
            .as_slice(),
        &[Some(2)]
    );
    assert_eq!(lowering8.terminals.len(), 1);
}

#[test]
fn separator_ambiguous_paths_collide_before_partial_lowering_is_returned() {
    let pending = PendingComponentLowering {
        declarations: vec![pending_declaration(
            "sink",
            vec![
                (&["a__b", "c"], Direction::Input),
                (&["a", "b__c"], Direction::Output),
            ],
            None,
        )],
    };
    let variables = HashMap::from_iter([
        (VarId::from_raw(1), variable(1, &["a__b", "c"], 8, &[])),
        (VarId::from_raw(2), variable(2, &["a", "b__c"], 8, &[])),
    ]);

    assert!(matches!(
        resolve_pending_nested_modport_lowering(
            &pending,
            &variables,
            &HashMap::default(),
            16,
        ),
        Err(NestedLoweringResolveError::FlatNameCollision { flat, .. })
            if flat == id("a__b__c")
    ));
}

#[test]
fn otherwise_plain_unrenderable_terminal_returns_typed_error_without_fallback() {
    let pending = PendingComponentLowering {
        declarations: vec![pending_declaration(
            "sink",
            vec![(&["child", "opaque"], Direction::Input)],
            None,
        )],
    };
    let mut opaque = variable(1, &["child", "opaque"], 8, &[]);
    opaque.r#type = Type::new(TypeKind::SystemVerilog);

    let result = resolve_pending_nested_modport_lowering(
        &pending,
        &HashMap::from_iter([(VarId::from_raw(1), opaque)]),
        &HashMap::default(),
        16,
    );

    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::UnemittableTerminal {
            path: error_path,
            actual_type,
            ..
        }) if error_path == path(&["child", "opaque"])
            && actual_type == TypeKind::SystemVerilog.to_string()
    ));
}

#[test]
fn nested_owner_rejects_unrenderable_direct_terminal_without_partial_lowering() {
    let pending = PendingComponentLowering {
        declarations: vec![pending_declaration(
            "sink",
            vec![
                (&["child", "ready"], Direction::Input),
                (&["opaque"], Direction::Input),
            ],
            None,
        )],
    };
    let child = variable(1, &["child", "ready"], 1, &[]);
    let mut opaque = variable(2, &["opaque"], 1, &[]);
    opaque.r#type = Type::new(TypeKind::SystemVerilog);

    let result = resolve_pending_nested_modport_lowering(
        &pending,
        &HashMap::from_iter([(VarId::from_raw(1), child), (VarId::from_raw(2), opaque)]),
        &HashMap::default(),
        16,
    );

    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::UnemittableTerminal {
            path: error_path,
            actual_type,
            ..
        }) if error_path == path(&["opaque"])
            && actual_type == TypeKind::SystemVerilog.to_string()
    ));
}
