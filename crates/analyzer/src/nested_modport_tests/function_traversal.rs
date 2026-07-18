#[test]
fn function_traversal_is_scope_exact_across_interleaved_declarations_and_phases()
-> Result<(), Box<dyn std::error::Error>> {
    let first_parent = lowering_key()?;
    let second_parent = key_in_session(first_parent.session, 8, 16)?;
    let unrelated_parent = key_in_session(first_parent.session, 9, 32)?;
    let function_binding = |id, declaration, parent: &NestedModportLoweringKey| {
        let mut binding = direct_binding(&first_parent, id, declaration);
        binding.kind = EmissionOwnerKind::Function;
        binding
            .emission_context
            .test_set_enclosing_generic_map(Default::default());
        binding.enclosing_owner = Some(Arc::new(parent.clone()));
        binding
    };
    let mut ordinary = direct_binding(&first_parent, 30, 20);
    ordinary.kind = EmissionOwnerKind::Module;
    let mut pending = PendingNestedModportAnalysis::default();
    for (index, parent) in [&first_parent, &second_parent, &unrelated_parent]
        .into_iter()
        .enumerate()
    {
        pending.record_lowering(parent.clone(), LoweringAvailability::NotNested);
        pending.record_emission_binding(direct_binding(parent, u32::try_from(index + 1)?, 5));
    }
    pending.record_emission_binding(function_binding(10, 10, &first_parent));
    pending.record_emission_binding(function_binding(11, 10, &second_parent));
    pending.record_emission_binding(ordinary);
    pending.record_emission_binding(function_binding(12, 11, &first_parent));
    pending.record_emission_binding(function_binding(13, 11, &second_parent));
    let analysis = pending.finalize(first_parent.session)?;

    for phase in [EmissionPhase::Align, EmissionPhase::Build] {
        let mut prepared = analysis.prepare_emission(PathId(1), phase)?;
        let enclosing: Vec<_> = prepared
            .take_owners(TokenId(5), EmissionOwnerKind::Interface)?
            .iter()
            .collect();
        let first = enclosing
            .iter()
            .copied()
            .find(|frame| frame.specialization() == &first_parent)
            .ok_or("first enclosing frame must exist")?;
        let second = enclosing
            .iter()
            .copied()
            .find(|frame| frame.specialization() == &second_parent)
            .ok_or("second enclosing frame must exist")?;
        let unrelated = enclosing
            .iter()
            .copied()
            .find(|frame| frame.specialization() == &unrelated_parent)
            .ok_or("unrelated enclosing frame must exist")?;

        assert_eq!(
            prepared
                .take_function_owners(TokenId(10), Some(first), None)?
                .iter()
                .count(),
            1
        );
        assert_eq!(
            prepared
                .take_function_owners(TokenId(10), Some(second), None)?
                .iter()
                .count(),
            1
        );
        assert!(matches!(
            prepared.take_function_owners(TokenId(10), Some(unrelated), None),
            Err(NestedModportAnalysisInvariant::DeclarationOrder)
        ));
        assert_eq!(
            prepared
                .take_owners(TokenId(20), EmissionOwnerKind::Module)?
                .iter()
                .count(),
            1
        );
        assert_eq!(
            prepared
                .take_function_owners(TokenId(11), Some(first), None)?
                .iter()
                .count(),
            1
        );
        assert_eq!(
            prepared
                .take_function_owners(TokenId(11), Some(second), None)?
                .iter()
                .count(),
            1
        );
        prepared.finish()?;

        let mut skipped = analysis.prepare_emission(PathId(1), phase)?;
        let enclosing: Vec<_> = skipped
            .take_owners(TokenId(5), EmissionOwnerKind::Interface)?
            .iter()
            .collect();
        let first = enclosing[0];
        let second = enclosing[1];
        let _ = skipped.take_function_owners(TokenId(10), Some(first), None)?;
        let _ = skipped.take_function_owners(TokenId(10), Some(second), None)?;
        let _ = skipped.take_owners(TokenId(20), EmissionOwnerKind::Module)?;
        let _ = skipped.take_function_owners(TokenId(11), Some(first), None)?;
        assert_eq!(
            skipped.finish(),
            Err(NestedModportAnalysisInvariant::UnconsumedBindings)
        );
    }
    Ok(())
}
