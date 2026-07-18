#[test]
fn empty_analysis_has_no_direct_only_rewrite_or_expanded_port()
-> Result<(), SpecializationIdentityError> {
    let owner = lowering_key()?;
    let rewrite = OccurrenceRewriteKey {
        owner: owner.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(3),
    };
    let expanded = ExpandedPortKey {
        owner,
        token: TokenId(4),
    };
    let analysis = NestedModportAnalysis::default();

    assert_eq!(analysis.rewrite(&rewrite), None);
    assert_eq!(analysis.expanded_port(&expanded), None);
    Ok(())
}

#[test]
fn pending_records_cannot_cross_read_between_reused_token_shapes()
-> Result<(), SpecializationIdentityError> {
    let first_owner = lowering_key()?;
    let second_owner = lowering_key()?;
    let first_key = OccurrenceRewriteKey {
        owner: first_owner.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(7),
    };
    let reused_shape_key = OccurrenceRewriteKey {
        owner: second_owner,
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(7),
    };
    let value = ResolvedPathRewrite {
        terminal: ResolvedTerminalRef::fixture(
            first_owner.clone(),
            0,
            Arc::new(NestedModportLowering::default()),
            ResolvedNestedTerminalId(0),
        ),
        semantic_segments: vec![StrId(5)],
        replace_from_segment: 0,
        consumed_segments: 1,
    };
    let mut first = PendingNestedModportAnalysis::default();
    let second = PendingNestedModportAnalysis::default();

    first
        .record_rewrite(first_key.clone(), value.clone())
        .expect("first resolved rewrite should be accepted");

    assert_eq!(first.rewrite(&first_key), Some(&value));
    assert_eq!(first.rewrite(&reused_shape_key), None);
    assert_eq!(second.rewrite(&first_key), None);
    assert_eq!(second.rewrite(&reused_shape_key), None);
    Ok(())
}

#[test]
fn direct_owner_is_explicitly_not_nested_while_analysis_remains_pending()
-> Result<(), SpecializationIdentityError> {
    let key = lowering_key()?;
    let mut pending = PendingNestedModportAnalysis::default();

    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(PendingEmissionBinding {
        id: EmissionBindingId::new(0),
        source: PathId(0),
        declaration: TokenId(0),
        kind: EmissionOwnerKind::Interface,
        enclosing_owner: None,
        enclosing_owner_identity: None,
        namespace_parent_fallback: false,
        enclosing_generic_map: None,
        package_scope: None,
        specialization: Arc::new(key.clone()),
        specialization_identity: None,
        emission_context: EmissionSpecializationContext::new(
            key.specialization.as_ref().clone(),
            Default::default(),
            None,
            Vec::new(),
        ),
        lowering: LoweringAvailability::NotNested,
        required_rewrites: Arc::from([]),
        required_expanded_ports: Arc::from([]),
    });

    assert!(matches!(
        pending.lowering(&key),
        Some(LoweringAvailability::NotNested)
    ));
    assert_eq!(*pending.emission_bindings()[0].specialization, key);
    assert!(NestedModportAnalysis::default().lowering(&key).is_none());
    Ok(())
}

fn lowering_key() -> Result<NestedModportLoweringKey, SpecializationIdentityError> {
    Ok(NestedModportLoweringKey { session: AnalysisSessionId::new(), specialization: (valid_identity(signature(1, 1), vec![])?).into() })
}

fn direct_binding(
    key: &NestedModportLoweringKey,
    id: u32,
    declaration: usize,
) -> PendingEmissionBinding {
    let mut generic_map = GenericMap::default();
    generic_map
        .map
        .extend(key.specialization.owner.generic_parameters.iter().cloned());
    let connected_generic_maps = key
        .specialization
        .connected_actuals
        .iter()
        .map(|connected| {
            let mut map = GenericMap::default();
            map.map
                .extend(connected.actual.generic_parameters.iter().cloned());
            (connected.formal_port, map)
        })
        .collect();
    PendingEmissionBinding {
        id: EmissionBindingId::new(id),
        source: PathId(1),
        declaration: TokenId(declaration),
        kind: EmissionOwnerKind::Interface,
        enclosing_owner: None,
        enclosing_owner_identity: None,
        namespace_parent_fallback: false,
        enclosing_generic_map: None,
        package_scope: None,
        specialization: Arc::new(key.clone()),
        specialization_identity: None,
        emission_context: EmissionSpecializationContext::new(
            key.specialization.as_ref().clone(),
            generic_map,
            None,
            connected_generic_maps,
        ),
        lowering: LoweringAvailability::NotNested,
        required_rewrites: Arc::from([]),
        required_expanded_ports: Arc::from([]),
    }
}

#[test]
fn binding_finalization_error_keeps_pending_bindings_for_retry()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let owner = generic_key_in_session(session, 20, 8)?;
    let target = generic_key_in_session(session, 21, 8)?;
    let context_key = InstantiationContextKey {
        owner: owner.clone(),
        token: TokenId(90),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&owner, 1, 10));
    pending.record_instantiation_context_candidate(
        context_key,
        PendingInstantiationContextCandidate {
            target: target.clone(),
        },
    )?;

    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MissingLowering
        ))
    ));
    assert_eq!(pending.emission_bindings().len(), 1);

    pending.record_lowering(target.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&target, 2, 11));
    assert!(pending.finalize(session).is_ok());
    Ok(())
}

fn generic_key_in_session(
    session: AnalysisSessionId,
    symbol: usize,
    width: usize,
) -> Result<NestedModportLoweringKey, SpecializationIdentityError> {
    let generic_name = resource_table::insert_str("W");
    let generic_value = GenericSymbolPath::from(&Token::from_external_text(&width.to_string()));
    let mut owner = signature(symbol, width);
    owner.generic_parameters.push((generic_name, generic_value));
    Ok(NestedModportLoweringKey {
        session,
        specialization: valid_identity(owner, vec![])?.into(),
    })
}
