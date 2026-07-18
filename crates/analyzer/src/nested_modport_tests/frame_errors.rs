#[test]
fn frame_and_cursor_reject_swapped_specializations_and_unpublished_records()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let owner8 = key_in_session(session, 1, 8)?;
    let owner16 = key_in_session(session, 1, 16)?;
    let rewrite16 = OccurrenceRewriteKey {
        owner: owner16.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(40),
    };
    let mut swapped = PendingNestedModportAnalysis::default();
    swapped.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    swapped.record_lowering(owner16.clone(), LoweringAvailability::NotNested);
    swapped.record_rewrite(
        rewrite16.clone(),
        ResolvedPathRewrite {
            terminal: ResolvedTerminalRef::fixture(
                owner16.clone(),
                0,
                Arc::new(NestedModportLowering::default()),
                ResolvedNestedTerminalId(0),
            ),
            semantic_segments: vec![StrId(1)],
            replace_from_segment: 0,
            consumed_segments: 1,
        },
    )?;
    let mut wrong_binding = direct_binding(&owner8, 1, 10);
    wrong_binding.required_rewrites = Arc::from([rewrite16]);
    swapped.record_emission_binding(wrong_binding);
    assert!(matches!(
        swapped.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::CrossOwnerRecord
        ))
    ));

    let port = ExpandedPortKey {
        owner: owner8.clone(),
        token: TokenId(41),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    pending.record_expanded_port(port.clone(), ExpandedPortResolution::DirectLegacy)?;
    pending.record_emission_binding(direct_binding(&owner8, 2, 10));
    let analysis = pending.finalize(session)?;
    let mut prepared = analysis.prepare_emission(PathId(1), EmissionPhase::Align)?;
    assert!(matches!(
        prepared.take_owners(TokenId(10), EmissionOwnerKind::Module),
        Err(NestedModportAnalysisInvariant::DeclarationOrder)
    ));
    let frame = prepared
        .take_owners(TokenId(10), EmissionOwnerKind::Interface)?
        .iter()
        .next()
        .ok_or("expected one owner frame")?;
    assert!(matches!(
        frame.expanded_port(port.token),
        Err(NestedModportAnalysisInvariant::RecordNotRequired)
    ));
    prepared.finish()?;

    let mut absent_source = analysis.prepare_emission(PathId(99), EmissionPhase::Build)?;
    assert!(matches!(
        absent_source.take_owners(TokenId(10), EmissionOwnerKind::Interface),
        Err(NestedModportAnalysisInvariant::DeclarationOrder)
    ));
    absent_source.finish()?;
    Ok(())
}

#[test]
fn finalizer_rejects_missing_expanded_port_before_emission()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let owner = key_in_session(session, 1, 8)?;
    let port = ExpandedPortKey {
        owner: owner.clone(),
        token: TokenId(51),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner.clone(), LoweringAvailability::NotNested);
    let mut binding = direct_binding(&owner, 1, 10);
    binding.required_expanded_ports = Arc::from([port]);
    pending.record_emission_binding(binding);

    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingExpandedPort
        ))
    ));
    Ok(())
}

#[test]
fn finalizer_rejects_expanded_port_from_swapped_specialization()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let owner8 = key_in_session(session, 1, 8)?;
    let owner16 = key_in_session(session, 1, 16)?;
    let port16 = ExpandedPortKey {
        owner: owner16.clone(),
        token: TokenId(52),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    pending.record_lowering(owner16, LoweringAvailability::NotNested);
    pending.record_expanded_port(port16.clone(), ExpandedPortResolution::DirectLegacy)?;
    let mut binding8 = direct_binding(&owner8, 1, 10);
    binding8.required_expanded_ports = Arc::from([port16]);
    pending.record_emission_binding(binding8);

    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::CrossOwnerRecord
        ))
    ));
    Ok(())
}
