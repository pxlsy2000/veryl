fn pending_declaration(
    name: &str,
    explicit: Vec<(&[&str], Direction)>,
    default: Option<PendingModportDefault>,
) -> PendingModportDeclaration {
    PendingModportDeclaration {
        name: id(name),
        contains_nested_item: explicit.iter().any(|(path, _)| path.len() > 1),
        explicit: explicit
            .into_iter()
            .map(|(segments, direction)| PendingModportEntry {
                path: path(segments),
                direction,
                origin: TokenRange::default(),
            })
            .collect(),
        default,
        origin: TokenRange::default(),
    }
}

fn default_fixture_variables(width: usize) -> HashMap<VarId, Variable> {
    HashMap::from_iter([
        (VarId::from_raw(1), variable(1, &["z"], width, &[])),
        (VarId::from_raw(2), variable(2, &["a"], width, &[])),
        (
            VarId::from_raw(3),
            variable(3, &["child", "ready"], width, &[2]),
        ),
    ])
}

#[test]
fn nested_defaults_expand_full_same_and_converse_sets_with_direct_import_rules() {
    let base = pending_declaration(
        "base",
        vec![
            (&["z"], Direction::Input),
            (&["a"], Direction::Inout),
            (&["child", "ready"], Direction::Output),
            (&["observe"], Direction::Import),
        ],
        None,
    );
    let same = pending_declaration(
        "same",
        Vec::new(),
        Some(PendingModportDefault::Same(vec![(
            id("base"),
            TokenRange::default(),
        )])),
    );
    let converse = pending_declaration(
        "converse",
        Vec::new(),
        Some(PendingModportDefault::Converse(vec![(
            id("base"),
            TokenRange::default(),
        )])),
    );
    let pending = PendingComponentLowering {
        declarations: vec![base, same, converse],
    };
    let functions = HashMap::from_iter([(VarId::from_raw(4), function(4, "observe"))]);

    let lowering = resolve_pending_nested_modport_lowering(
        &pending,
        &default_fixture_variables(8),
        &functions,
        64,
    )
    .expect("acyclic defaults should resolve")
    .expect("nested member requires lowering");
    let same_entries = lowering.modports[&id("same")].entries.as_ref();
    let converse_entries = lowering.modports[&id("converse")].entries.as_ref();

    assert_eq!(
        same_entries
            .iter()
            .map(|entry| (entry.path.to_string(), entry.direction))
            .collect::<Vec<_>>(),
        vec![
            ("z".to_string(), Direction::Input),
            ("a".to_string(), Direction::Inout),
            ("child.ready".to_string(), Direction::Output),
            ("observe".to_string(), Direction::Import),
        ]
    );
    assert_eq!(
        converse_entries
            .iter()
            .map(|entry| (entry.path.to_string(), entry.direction))
            .collect::<Vec<_>>(),
        vec![
            ("z".to_string(), Direction::Output),
            ("a".to_string(), Direction::Inout),
            ("child.ready".to_string(), Direction::Input),
        ]
    );
    assert!(matches!(
        same_entries[3].terminal,
        Some(ResolvedModportTerminal::DirectFunction { function })
            if function == VarId::from_raw(4)
    ));
}

#[test]
fn nested_defaults_preserve_explicit_precedence_and_later_target_direction() {
    let first = pending_declaration(
        "first",
        vec![
            (&["z"], Direction::Input),
            (&["child", "ready"], Direction::Input),
        ],
        None,
    );
    let second = pending_declaration(
        "second",
        vec![
            (&["z"], Direction::Output),
            (&["child", "ready"], Direction::Output),
        ],
        None,
    );
    let combined = pending_declaration(
        "combined",
        vec![(&["z"], Direction::Inout)],
        Some(PendingModportDefault::Same(vec![
            (id("first"), TokenRange::default()),
            (id("second"), TokenRange::default()),
        ])),
    );

    let lowering = resolve_pending_nested_modport_lowering(
        &PendingComponentLowering {
            declarations: vec![first, second, combined],
        },
        &default_fixture_variables(8),
        &HashMap::default(),
        64,
    )
    .expect("overlapping targets should resolve")
    .expect("nested member requires lowering");
    let entries = lowering.modports[&id("combined")].entries.as_ref();

    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.path.to_string(), entry.direction))
            .collect::<Vec<_>>(),
        vec![
            ("z".to_string(), Direction::Inout),
            ("child.ready".to_string(), Direction::Output),
        ]
    );
}

#[test]
fn nested_defaults_expand_recursive_dag_and_reject_cycles_and_missing_targets() {
    let base = pending_declaration("base", vec![(&["child", "ready"], Direction::Input)], None);
    let middle = pending_declaration(
        "middle",
        Vec::new(),
        Some(PendingModportDefault::Same(vec![(
            id("base"),
            TokenRange::default(),
        )])),
    );
    let top = pending_declaration(
        "top",
        Vec::new(),
        Some(PendingModportDefault::Converse(vec![(
            id("middle"),
            TokenRange::default(),
        )])),
    );
    let lowering = resolve_pending_nested_modport_lowering(
        &PendingComponentLowering {
            declarations: vec![base, middle, top],
        },
        &default_fixture_variables(8),
        &HashMap::default(),
        64,
    )
    .expect("below-budget DAG should resolve")
    .expect("nested member requires lowering");
    assert_eq!(
        lowering.modports[&id("top")].entries[0].direction,
        Direction::Output
    );

    let cycle = PendingComponentLowering {
        declarations: vec![
            pending_declaration(
                "a",
                Vec::new(),
                Some(PendingModportDefault::Same(vec![(
                    id("b"),
                    TokenRange::default(),
                )])),
            ),
            pending_declaration(
                "b",
                Vec::new(),
                Some(PendingModportDefault::Converse(vec![(
                    id("a"),
                    TokenRange::default(),
                )])),
            ),
        ],
    };
    assert!(matches!(
        resolve_pending_nested_modport_lowering(
            &cycle,
            &default_fixture_variables(8),
            &HashMap::default(),
            16,
        ),
        Err(NestedLoweringResolveError::DefaultCycle { .. })
    ));

    let missing = PendingComponentLowering {
        declarations: vec![pending_declaration(
            "a",
            Vec::new(),
            Some(PendingModportDefault::Same(vec![(
                id("absent"),
                TokenRange::default(),
            )])),
        )],
    };
    assert!(matches!(
        resolve_pending_nested_modport_lowering(
            &missing,
            &default_fixture_variables(8),
            &HashMap::default(),
            16,
        ),
        Err(NestedLoweringResolveError::MissingModport { name, .. })
            if name == id("absent")
    ));
}
