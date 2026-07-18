#[test]
fn finalizer_rejects_swapped_emission_specialization_context_before_publication()
-> Result<(), SpecializationIdentityError> {
    let session = AnalysisSessionId::new();
    let owner8 = generic_key_in_session(session, 1, 8)?;
    let owner16 = generic_key_in_session(session, 1, 16)?;
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    let mut swapped = direct_binding(&owner8, 0, 10);
    swapped.emission_context.owner = owner16.specialization.owner.clone();
    swapped.emission_context.generic_map.map = owner8
        .specialization
        .owner
        .generic_parameters
        .iter()
        .cloned()
        .collect();
    pending.record_emission_binding(swapped);

    for _ in 0..2 {
        assert!(matches!(
            pending.finalize(session),
            Err(NestedModportFinalizeError::Invariant(
                NestedModportAnalysisInvariant::MismatchedEmissionContext
            ))
        ));
    }
    Ok(())
}

#[test]
fn finalized_emission_prepares_after_live_symbol_registry_is_cleared()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let owner = generic_key_in_session(session, 1, 8)?;
    let mut binding = direct_binding(&owner, 0, 10);
    let mut generic_map = GenericMap::default();
    generic_map.map = owner
        .specialization
        .owner
        .generic_parameters
        .iter()
        .cloned()
        .collect();
    binding.emission_context = EmissionSpecializationContext::new(
        owner.specialization.as_ref().clone(),
        generic_map,
        None,
        Vec::new(),
    );
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner, LoweringAvailability::NotNested);
    pending.record_emission_binding(binding);
    let analysis = pending.finalize(session)?;

    symbol_table::clear();
    for phase in [EmissionPhase::Align, EmissionPhase::Build] {
        let mut prepared = analysis.prepare_emission(PathId(1), phase)?;
        let frames = prepared.take_owners(TokenId(10), EmissionOwnerKind::Interface)?;
        assert_eq!(frames.iter().count(), 1);
        prepared.finish()?;
    }
    Ok(())
}

fn consume_both_emission_phases(
    analysis: &NestedModportAnalysis,
) -> Result<(), NestedModportAnalysisInvariant> {
    for phase in [EmissionPhase::Align, EmissionPhase::Build] {
        let mut prepared = analysis.prepare_emission(PathId(1), phase)?;
        let frames = prepared.take_owners(TokenId(10), EmissionOwnerKind::Interface)?;
        assert_eq!(frames.iter().count(), 1);
        prepared.finish()?;
    }
    Ok(())
}

fn frozen_analysis_with_live_owner()
-> Result<(Arc<NestedModportAnalysis>, Symbol), Box<dyn std::error::Error>> {
    symbol_table::clear();
    let token = Token::from_external_text("LiveOwner");
    let mut owner = Symbol::new(
        &token,
        SymbolKind::Namespace,
        &Namespace::new(),
        false,
        DocComment::default(),
    );
    owner.generic_instances = vec![SymbolId(usize::MAX - 1), SymbolId(usize::MAX)];
    symbol_table::insert(&token, owner.clone()).ok_or("live owner must be inserted")?;

    let session = AnalysisSessionId::new();
    let key = generic_key_in_session(session, owner.id.0, 8)?;
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&key, 0, 10));
    Ok((pending.finalize(session)?, owner))
}

#[test]
fn finalized_emission_ignores_live_generic_instance_reordering()
-> Result<(), Box<dyn std::error::Error>> {
    let (analysis, mut owner) = frozen_analysis_with_live_owner()?;
    owner.generic_instances.reverse();
    symbol_table::update(owner);
    consume_both_emission_phases(&analysis)?;
    Ok(())
}

#[test]
fn finalized_emission_ignores_unrelated_live_symbol_addition()
-> Result<(), Box<dyn std::error::Error>> {
    let (analysis, _) = frozen_analysis_with_live_owner()?;
    let token = Token::from_external_text("UnrelatedAfterFreeze");
    let unrelated = Symbol::new(
        &token,
        SymbolKind::Namespace,
        &Namespace::new(),
        false,
        DocComment::default(),
    );
    symbol_table::insert(&token, unrelated).ok_or("unrelated symbol must be inserted")?;
    consume_both_emission_phases(&analysis)?;
    Ok(())
}

#[test]
fn finalized_emission_ignores_unrelated_same_shape_live_generic_map() {
    let (analysis, _) =
        frozen_analysis_with_live_owner().expect("frozen analysis fixture should finalize");
    let code = r#"
module SameShape::<W: u32> {}
module UnrelatedUse {
    inst same_shape: SameShape::<16>;
}
"#;
    symbol_table::clear();
    attribute_table::clear();
    let metadata =
        Metadata::create_default("unrelated_same_shape").expect("test metadata should construct");
    let parser = Parser::parse(code, &"").expect("same-shape fixture should parse");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();
    let mut errors = analyzer.analyze_pass1("unrelated_same_shape", &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    assert!(errors.is_empty(), "same-shape fixture errors: {errors:?}");

    consume_both_emission_phases(&analysis)
        .expect("frozen Align and Build frames must ignore unrelated live maps");
}

#[test]
fn finalizer_rejects_corrupt_semantic_generic_map_before_publication()
-> Result<(), SpecializationIdentityError> {
    let session = AnalysisSessionId::new();
    let owner8 = generic_key_in_session(session, 1, 8)?;
    let owner16 = generic_key_in_session(session, 1, 16)?;
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    let mut corrupt = direct_binding(&owner8, 0, 10);
    corrupt.emission_context.generic_map.map = owner16
        .specialization
        .owner
        .generic_parameters
        .iter()
        .cloned()
        .collect();
    pending.record_emission_binding(corrupt);

    let invariant = pending
        .finalize(session)
        .expect_err("corrupt generic map must prevent immutable publication");
    assert_eq!(
        invariant,
        NestedModportAnalysisInvariant::MismatchedEmissionContext.into()
    );
    let diagnostic = crate::AnalyzerError::from(invariant);
    assert!(matches!(
        &diagnostic,
        crate::AnalyzerError::NestedModportAnalysisInvariant {
            kind: NestedModportAnalysisInvariant::MismatchedEmissionContext,
            ..
        }
    ));
    assert_eq!(
        diagnostic.code().map(|code| code.to_string()).as_deref(),
        Some("nested_modport_analysis_invariant")
    );
    assert_eq!(
        diagnostic.to_string(),
        "nested modport analysis invariant: emission specialization context does not match its binding specialization"
    );

    let mut missing = PendingNestedModportAnalysis::default();
    missing.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    let mut missing_binding = direct_binding(&owner8, 1, 11);
    missing_binding.emission_context.generic_map.map.clear();
    missing.record_emission_binding(missing_binding);
    assert!(matches!(
        missing.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MismatchedEmissionContext
        ))
    ));

    let mut extra = PendingNestedModportAnalysis::default();
    extra.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    let mut extra_binding = direct_binding(&owner8, 2, 12);
    extra_binding.emission_context.generic_map.map = owner8
        .specialization
        .owner
        .generic_parameters
        .iter()
        .cloned()
        .collect();
    extra_binding.emission_context.generic_map.map.insert(
        resource_table::insert_str("EXTRA"),
        GenericSymbolPath::from(&Token::from_external_text("99")),
    );
    extra.record_emission_binding(extra_binding);
    assert!(matches!(
        extra.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MismatchedEmissionContext
        ))
    ));
    Ok(())
}
