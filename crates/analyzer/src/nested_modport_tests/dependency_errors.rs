#[test]
fn finalizer_rejects_every_missing_or_cross_session_dependency()
-> Result<(), Box<dyn std::error::Error>> {
    let key = lowering_key()?;

    let mut missing_lowering = PendingNestedModportAnalysis::default();
    missing_lowering.record_emission_binding(direct_binding(&key, 1, 10));
    assert!(matches!(
        missing_lowering.finalize(key.session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingLowering
        ))
    ));

    let rewrite = OccurrenceRewriteKey {
        owner: key.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(20),
    };
    let mut missing_rewrite = PendingNestedModportAnalysis::default();
    missing_rewrite.record_lowering(key.clone(), LoweringAvailability::NotNested);
    let mut binding = direct_binding(&key, 2, 10);
    binding.required_rewrites = Arc::from([rewrite]);
    missing_rewrite.record_emission_binding(binding);
    assert!(matches!(
        missing_rewrite.finalize(key.session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingRewrite
        ))
    ));

    let port = ExpandedPortKey {
        owner: key.clone(),
        token: TokenId(21),
    };
    let mut missing_port = PendingNestedModportAnalysis::default();
    missing_port.record_lowering(key.clone(), LoweringAvailability::NotNested);
    let mut binding = direct_binding(&key, 3, 10);
    binding.required_expanded_ports = Arc::from([port]);
    missing_port.record_emission_binding(binding);
    assert!(matches!(
        missing_port.finalize(key.session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingExpandedPort
        ))
    ));

    let mut unresolved_port = PendingNestedModportAnalysis::default();
    unresolved_port.record_expanded_port_candidate(
        ExpandedPortKey {
            owner: key.clone(),
            token: TokenId(22),
        },
        PendingExpandedPortCandidate {
            target: key.clone(),
            modport: StrId(4),
        },
    )?;
    assert!(matches!(
        unresolved_port.finalize(key.session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::UnresolvedExpandedPort
        ))
    ));

    let mut cross_session = PendingNestedModportAnalysis::default();
    cross_session.record_lowering(key.clone(), LoweringAvailability::NotNested);
    assert!(matches!(
        cross_session.finalize(AnalysisSessionId::new()),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::CrossSession
        ))
    ));
    Ok(())
}

#[test]
fn semantic_candidate_insertion_is_idempotent_and_rejects_conflicts()
-> Result<(), Box<dyn std::error::Error>> {
    let key = lowering_key()?;
    let rewrite_key = OccurrenceRewriteKey {
        owner: key.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(30),
    };
    let rewrite = PendingPathRewriteCandidate {
        target: key.clone(),
        semantic_segments: vec![StrId(1)],
        replace_from_segment: 0,
        consumed_segments: 2,
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_rewrite_candidate(rewrite_key.clone(), rewrite.clone())?;
    pending.record_rewrite_candidate(rewrite_key.clone(), rewrite.clone())?;
    let mut conflict = rewrite;
    conflict.consumed_segments = 3;
    assert!(matches!(
        pending.record_rewrite_candidate(rewrite_key, conflict),
        Err(NestedModportAnalysisInvariant::ConflictingRewrite)
    ));

    let port_key = ExpandedPortKey {
        owner: key.clone(),
        token: TokenId(31),
    };
    let port = PendingExpandedPortCandidate {
        target: key.clone(),
        modport: StrId(2),
    };
    pending.record_expanded_port_candidate(port_key.clone(), port.clone())?;
    pending.record_expanded_port_candidate(port_key.clone(), port)?;
    assert!(matches!(
        pending.record_expanded_port_candidate(
            port_key,
            PendingExpandedPortCandidate {
                target: key.clone(),
                modport: StrId(3),
            },
        ),
        Err(NestedModportAnalysisInvariant::ConflictingExpandedPort)
    ));

    let direct_key = OccurrenceRewriteKey {
        owner: key.clone(),
        kind: OccurrenceKind::HierarchicalIdentifier,
        token: TokenId(32),
    };
    let direct = ResolvedPathRewrite {
        terminal: ResolvedTerminalRef::fixture(
            key.clone(),
            0,
            Arc::new(NestedModportLowering::default()),
            ResolvedNestedTerminalId(0),
        ),
        semantic_segments: vec![StrId(1)],
        replace_from_segment: 0,
        consumed_segments: 1,
    };
    let mut resolved = PendingNestedModportAnalysis::default();
    resolved.record_rewrite(direct_key.clone(), direct.clone())?;
    resolved.record_rewrite(direct_key.clone(), direct.clone())?;
    let mut conflicting_direct = direct;
    conflicting_direct.consumed_segments = 2;
    assert!(matches!(
        resolved.record_rewrite(direct_key, conflicting_direct),
        Err(NestedModportAnalysisInvariant::ConflictingRewrite)
    ));

    let direct_port_key = ExpandedPortKey {
        owner: key,
        token: TokenId(33),
    };
    resolved.record_expanded_port(
        direct_port_key.clone(),
        ExpandedPortResolution::DirectLegacy,
    )?;
    resolved.record_expanded_port(
        direct_port_key.clone(),
        ExpandedPortResolution::DirectLegacy,
    )?;
    assert!(matches!(
        resolved.record_expanded_port(
            direct_port_key,
            ExpandedPortResolution::Nested {
                target: lowering_key()?,
                modport: StrId(4),
                interface: ResolvedExpandedPortInterface {
                    symbol: SymbolId(1),
                    generic_map: GenericMap::default(),
                },
                members: Arc::new(ResolvedExpandedMemberSet::default()),
            },
        ),
        Err(NestedModportAnalysisInvariant::ConflictingExpandedPort)
    ));
    Ok(())
}
