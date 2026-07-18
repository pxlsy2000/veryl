#[test]
fn prepared_emission_rejects_order_duplicate_cross_owner_and_unconsumed()
-> Result<(), Box<dyn std::error::Error>> {
    let key = lowering_key()?;
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&key, 1, 10));
    pending.record_emission_binding(direct_binding(&key, 2, 11));
    let analysis = pending.finalize(key.session)?;

    let mut reordered = analysis.prepare_emission(PathId(1), EmissionPhase::Align)?;
    assert!(matches!(
        reordered.take_owners(TokenId(11), EmissionOwnerKind::Interface),
        Err(NestedModportAnalysisInvariant::DeclarationOrder)
    ));
    let mut duplicated = analysis.prepare_emission(PathId(1), EmissionPhase::Build)?;
    let _ = duplicated.take_owners(TokenId(10), EmissionOwnerKind::Interface)?;
    assert!(matches!(
        duplicated.take_owners(TokenId(10), EmissionOwnerKind::Interface),
        Err(NestedModportAnalysisInvariant::DeclarationOrder)
    ));
    assert_eq!(
        analysis
            .prepare_emission(PathId(1), EmissionPhase::Align)?
            .finish(),
        Err(NestedModportAnalysisInvariant::UnconsumedBindings)
    );

    let other = key_in_session(key.session, 9, 9)?;
    let foreign = OccurrenceRewriteKey {
        owner: other,
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(99),
    };
    let mut invalid = PendingNestedModportAnalysis::default();
    invalid.record_lowering(key.clone(), LoweringAvailability::NotNested);
    invalid.record_rewrite(
        foreign.clone(),
        ResolvedPathRewrite {
            terminal: ResolvedTerminalRef::fixture(
                key.clone(),
                0,
                Arc::new(NestedModportLowering::default()),
                ResolvedNestedTerminalId(0),
            ),
            semantic_segments: vec![],
            replace_from_segment: 0,
            consumed_segments: 0,
        },
    )?;
    let mut binding = direct_binding(&key, 3, 12);
    binding.required_rewrites = Arc::from([foreign]);
    invalid.record_emission_binding(binding);
    assert!(matches!(
        invalid.finalize(key.session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::CrossOwnerRecord
        ))
    ));
    Ok(())
}

#[test]
fn function_owner_batches_reject_swapped_duplicate_and_unconsumed_records()
-> Result<(), Box<dyn std::error::Error>> {
    let key = lowering_key()?;
    let mut first = direct_binding(&key, 1, 10);
    first.kind = EmissionOwnerKind::Function;
    first
        .emission_context
        .test_set_enclosing_generic_map(Default::default());
    first.enclosing_owner = Some(Arc::new(key.clone()));
    let mut second = direct_binding(&key, 2, 11);
    second.kind = EmissionOwnerKind::Function;
    second
        .emission_context
        .test_set_enclosing_generic_map(Default::default());
    second.enclosing_owner = Some(Arc::new(key.clone()));
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&key, 0, 5));
    pending.record_emission_binding(first);
    pending.record_emission_binding(second);
    let analysis = pending.finalize(key.session)?;

    let mut swapped = analysis.prepare_emission(PathId(1), EmissionPhase::Align)?;
    let enclosing = swapped
        .take_owners(TokenId(5), EmissionOwnerKind::Interface)?
        .iter()
        .next()
        .ok_or("enclosing frame must exist")?;
    assert!(matches!(
        swapped.take_function_owners(TokenId(11), Some(enclosing), None),
        Err(NestedModportAnalysisInvariant::DeclarationOrder)
    ));

    let mut duplicate = analysis.prepare_emission(PathId(1), EmissionPhase::Build)?;
    let enclosing = duplicate
        .take_owners(TokenId(5), EmissionOwnerKind::Interface)?
        .iter()
        .next()
        .ok_or("enclosing frame must exist")?;
    let _ = duplicate.take_function_owners(TokenId(10), Some(enclosing), None)?;
    assert!(matches!(
        duplicate.take_function_owners(TokenId(10), Some(enclosing), None),
        Err(NestedModportAnalysisInvariant::DeclarationOrder)
    ));

    let mut unconsumed = analysis.prepare_emission(PathId(1), EmissionPhase::Align)?;
    let enclosing = unconsumed
        .take_owners(TokenId(5), EmissionOwnerKind::Interface)?
        .iter()
        .next()
        .ok_or("enclosing frame must exist")?;
    let _ = unconsumed.take_function_owners(TokenId(10), Some(enclosing), None)?;
    assert_eq!(
        unconsumed.finish(),
        Err(NestedModportAnalysisInvariant::UnconsumedBindings)
    );
    Ok(())
}

#[test]
fn partially_selected_function_batch_remains_visible_to_finish()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two enclosing owners share one function declaration token.
    let key = lowering_key()?;
    let mut first = direct_binding(&key, 1, 10);
    first.kind = EmissionOwnerKind::Function;
    first
        .emission_context
        .test_set_enclosing_generic_map(Default::default());
    first.enclosing_owner = Some(Arc::new(key.clone()));
    let alternate_parent = key_in_session(key.session, 8, 16)?.specialization;
    let mut second = direct_binding(&key, 2, 10);
    second.kind = EmissionOwnerKind::Function;
    second
        .emission_context
        .test_set_enclosing_generic_map(Default::default());
    second.enclosing_owner = Some(Arc::new(NestedModportLoweringKey { session: key.session, specialization: (alternate_parent).into() }));
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&key, 3, 5));
    pending.record_emission_binding(first);
    pending.record_emission_binding(second);
    let analysis = pending.finalize(key.session)?;
    let mut prepared = analysis.prepare_emission(PathId(1), EmissionPhase::Align)?;

    // When: the exact enclosing scope consumes only its matching function.
    let enclosing = prepared
        .take_owners(TokenId(5), EmissionOwnerKind::Interface)?
        .iter()
        .next()
        .ok_or("enclosing frame must exist")?;
    let batch = prepared.take_function_owners(TokenId(10), Some(enclosing), None)?;
    assert_eq!(batch.iter().count(), 1);
    drop(batch);

    // Then: the unmatched frame must remain visible to phase completion.
    assert_eq!(
        prepared.finish(),
        Err(NestedModportAnalysisInvariant::UnconsumedBindings)
    );
    Ok(())
}
