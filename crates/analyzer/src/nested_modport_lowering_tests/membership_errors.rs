#[test]
fn nested_lowering_retains_direct_same_import_and_omits_absent_converse_import() {
    let same = id("same");
    let converse = id("converse");
    let observe = path(&["observe"]);
    let signal = path(&["child", "ready"]);
    let modports = HashMap::from_iter([
        (
            same,
            vec![
                (signal.clone(), Direction::Input),
                (observe.clone(), Direction::Import),
            ],
        ),
        (converse, vec![(signal, Direction::Output)]),
    ]);
    let variables =
        HashMap::from_iter([(VarId::from_raw(1), variable(1, &["child", "ready"], 8, &[]))]);
    let functions = HashMap::from_iter([(VarId::from_raw(2), function(2, "observe"))]);

    let lowering = resolve_pending_nested_modport_lowering(
        &pending_from_effective(&modports),
        &variables,
        &functions,
        16,
    )
    .expect("same/converse effective sets should resolve")
    .expect("dotted members require nested lowering");
    let same_entries = lowering.modports[&same].entries.as_ref();
    let converse_entries = lowering.modports[&converse].entries.as_ref();

    assert!(matches!(
        same_entries[1].terminal,
        Some(ResolvedModportTerminal::DirectFunction { function }) if function == VarId::from_raw(2)
    ));
    assert_eq!(same_entries[1].direction, Direction::Import);
    assert!(
        converse_entries
            .iter()
            .all(|entry| entry.direction != Direction::Import)
    );
}

#[test]
fn nested_lowering_rejects_unsupported_direction_before_publishing_a_value() {
    let modports = HashMap::from_iter([(
        id("sink"),
        vec![(path(&["child", "ready"]), Direction::Import)],
    )]);
    let variables =
        HashMap::from_iter([(VarId::from_raw(1), variable(1, &["child", "ready"], 8, &[]))]);

    let result = resolve_pending_nested_modport_lowering(
        &pending_from_effective(&modports),
        &variables,
        &HashMap::default(),
        16,
    );

    assert!(
        matches!(
            &result,
            Err(NestedLoweringResolveError::UnsupportedMemberDirection {
                path: error_path,
                direction: Direction::Import,
                ..
            }) if *error_path == path(&["child", "ready"])
        ),
        "unexpected result: {result:?}"
    );
}

#[test]
fn nested_lowering_rejects_empty_effective_modport() {
    let result = resolve_pending_nested_modport_lowering(
        &PendingComponentLowering {
            declarations: vec![PendingModportDeclaration {
                name: id("empty"),
                explicit: Vec::new(),
                default: None,
                origin: TokenRange::default(),
                contains_nested_item: true,
            }],
        },
        &HashMap::default(),
        &HashMap::default(),
        16,
    );

    assert!(
        matches!(
            &result,
            Err(NestedLoweringResolveError::EmptyModport { name, .. })
                if *name == id("empty")
        ),
        "unexpected result: {result:?}"
    );
}

#[test]
fn nested_lowering_budget_failure_is_bounded_and_all_or_nothing() {
    let modports = HashMap::from_iter([(
        id("sink"),
        vec![
            (path(&["child", "a"]), Direction::Input),
            (path(&["child", "b"]), Direction::Input),
        ],
    )]);

    let result = resolve_pending_nested_modport_lowering(
        &pending_from_effective(&modports),
        &HashMap::default(),
        &HashMap::default(),
        1,
    );

    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::ExpansionBudget {
            requested: 2,
            limit: 1,
        })
    ));
}
