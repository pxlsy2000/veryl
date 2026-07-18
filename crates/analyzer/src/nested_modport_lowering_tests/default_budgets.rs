#[test]
fn deep_acyclic_default_chain_is_budgeted_without_recursing() {
    let depth = 50_000usize;
    let mut declarations = Vec::with_capacity(depth);
    declarations.push(pending_declaration(
        "deep_0",
        vec![(&["child", "ready"], Direction::Input)],
        None,
    ));
    for index in 1..depth {
        declarations.push(pending_declaration(
            &format!("deep_{index}"),
            Vec::new(),
            Some(PendingModportDefault::Same(vec![(
                id(&format!("deep_{}", index - 1)),
                TokenRange::default(),
            )])),
        ));
    }
    declarations.reverse();

    let result = resolve_pending_nested_modport_lowering(
        &PendingComponentLowering { declarations },
        &default_fixture_variables(8),
        &HashMap::default(),
        1,
    );

    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::ExpansionBudget { limit: 1, .. })
    ));
}

#[test]
fn deep_acyclic_default_chain_below_limit_preserves_member_order() {
    let depth = 5_000usize;
    let mut declarations = Vec::with_capacity(depth);
    declarations.push(pending_declaration(
        "green_0",
        vec![
            (&["z"], Direction::Input),
            (&["child", "ready"], Direction::Output),
        ],
        None,
    ));
    for index in 1..depth {
        declarations.push(pending_declaration(
            &format!("green_{index}"),
            Vec::new(),
            Some(PendingModportDefault::Same(vec![(
                id(&format!("green_{}", index - 1)),
                TokenRange::default(),
            )])),
        ));
    }
    declarations.reverse();

    let lowering = resolve_pending_nested_modport_lowering(
        &PendingComponentLowering { declarations },
        &default_fixture_variables(8),
        &HashMap::default(),
        depth * 10,
    )
    .expect("below-limit deep chain must not overflow")
    .expect("nested terminal requires lowering");
    let entries = &lowering.modports[&id(&format!("green_{}", depth - 1))].entries;
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.path.to_string(), entry.direction))
            .collect::<Vec<_>>(),
        vec![
            ("z".to_owned(), Direction::Input),
            ("child.ready".to_owned(), Direction::Output),
        ]
    );
}

#[test]
fn deep_default_cycle_returns_typed_cycle_instead_of_overflowing() {
    let depth = 5_000usize;
    let mut declarations = Vec::with_capacity(depth);
    for index in 0..depth {
        let target = if index == 0 { depth - 1 } else { index - 1 };
        declarations.push(pending_declaration(
            &format!("cycle_{index}"),
            Vec::new(),
            Some(PendingModportDefault::Same(vec![(
                id(&format!("cycle_{target}")),
                TokenRange::default(),
            )])),
        ));
    }
    declarations.reverse();

    let result = resolve_pending_nested_modport_lowering(
        &PendingComponentLowering { declarations },
        &default_fixture_variables(8),
        &HashMap::default(),
        depth * 2,
    );
    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::DefaultCycle { target, .. })
            if target == id(&format!("cycle_{}", depth - 1))
    ));
}

fn empty_default_chain(prefix: &str, depth: usize) -> PendingComponentLowering {
    let mut declarations = Vec::with_capacity(depth);
    declarations.push(pending_declaration(
        &format!("{prefix}_0"),
        Vec::new(),
        None,
    ));
    for index in 1..depth {
        declarations.push(pending_declaration(
            &format!("{prefix}_{index}"),
            Vec::new(),
            Some(PendingModportDefault::Same(vec![(
                id(&format!("{prefix}_{}", index - 1)),
                TokenRange::default(),
            )])),
        ));
    }
    declarations.reverse();
    PendingComponentLowering { declarations }
}

#[test]
fn fifty_thousand_empty_default_edges_fail_at_limit_two() {
    let result = resolve_direct_modport_effective(
        &empty_default_chain("empty_over", 50_000),
        &HashMap::default(),
        &HashMap::default(),
        2,
    );
    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::ExpansionBudget {
            requested: 3,
            limit: 2,
        })
    ));
}

#[test]
fn five_thousand_empty_default_edges_pass_with_sufficient_budget() {
    let depth = 5_000;
    let result = resolve_direct_modport_effective(
        &empty_default_chain("empty_green", depth),
        &HashMap::default(),
        &HashMap::default(),
        depth,
    )
    .expect("one work unit per empty dependency edge must remain within the limit");
    assert_eq!(result[&id(&format!("empty_green_{}", depth - 1))].len(), 0);
}

#[test]
fn high_fan_in_empty_targets_consume_cumulative_edge_budget() {
    let target_count = 5_000usize;
    let targets: Vec<_> = (0..target_count)
        .map(|index| (id(&format!("fan_{index}")), TokenRange::default()))
        .collect();
    let mut declarations = vec![pending_declaration(
        "fan_root",
        Vec::new(),
        Some(PendingModportDefault::Same(targets.clone())),
    )];
    declarations.extend(
        targets
            .into_iter()
            .map(|(name, _)| PendingModportDeclaration {
                name,
                explicit: Vec::new(),
                default: None,
                origin: TokenRange::default(),
                contains_nested_item: false,
            }),
    );

    let result = resolve_direct_modport_effective(
        &PendingComponentLowering { declarations },
        &HashMap::default(),
        &HashMap::default(),
        2,
    );
    assert!(matches!(
        result,
        Err(NestedLoweringResolveError::ExpansionBudget {
            requested: 3,
            limit: 2,
        })
    ));
}
