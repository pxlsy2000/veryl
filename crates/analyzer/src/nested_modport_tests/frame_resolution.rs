#[test]
fn finalizer_resolves_semantic_records_and_phase_cursors_drive_specializations()
-> Result<(), Box<dyn std::error::Error>> {
    let session = AnalysisSessionId::new();
    let owner8 = key_in_session(session, 1, 8)?;
    let owner16 = key_in_session(session, 1, 16)?;
    let target8 = key_in_session(session, 2, 8)?;
    let target16 = key_in_session(session, 2, 16)?;
    let member = [
        resource_table::insert_str("child"),
        resource_table::insert_str("member"),
    ];
    let field = resource_table::insert_str("field");
    let lowering8 = lowering_with_terminal(&member);
    let lowering16 = lowering_with_terminal(&member);
    let rewrite8 = OccurrenceRewriteKey {
        owner: owner8.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(80),
    };
    let rewrite16 = OccurrenceRewriteKey {
        owner: owner16.clone(),
        kind: OccurrenceKind::HierarchicalIdentifier,
        token: TokenId(160),
    };
    let port8 = ExpandedPortKey {
        owner: owner8.clone(),
        token: TokenId(81),
    };
    let port16 = ExpandedPortKey {
        owner: owner16.clone(),
        token: TokenId(161),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_interface_lowering(target8.clone(), Arc::clone(&lowering8));
    pending.record_interface_lowering(target16.clone(), Arc::clone(&lowering16));
    pending.record_lowering(owner8.clone(), LoweringAvailability::NotNested);
    pending.record_lowering(owner16.clone(), LoweringAvailability::NotNested);
    pending
        .record_rewrite_candidate(
            rewrite8.clone(),
            PendingPathRewriteCandidate {
                target: target8.clone(),
                semantic_segments: [member.as_slice(), &[field]].concat(),
                replace_from_segment: 1,
                consumed_segments: 4,
            },
        )
        .expect("first semantic rewrite should be accepted");
    pending
        .record_rewrite_candidate(
            rewrite16.clone(),
            PendingPathRewriteCandidate {
                target: target16.clone(),
                semantic_segments: member.to_vec(),
                replace_from_segment: 2,
                consumed_segments: 4,
            },
        )
        .expect("second semantic rewrite should be accepted");
    pending
        .record_expanded_port_candidate(
            port8.clone(),
            PendingExpandedPortCandidate {
                target: target8.clone(),
                modport: StrId(40),
            },
        )
        .expect("first expanded port should be accepted");
    pending
        .record_expanded_port_candidate(
            port16.clone(),
            PendingExpandedPortCandidate {
                target: target16.clone(),
                modport: StrId(40),
            },
        )
        .expect("second expanded port should be accepted");
    let mut binding8 = direct_binding(&owner8, 8, 70);
    binding8.required_rewrites = Arc::from([rewrite8.clone()]);
    binding8.required_expanded_ports = Arc::from([port8.clone()]);
    binding8.emission_context.generic_map.id = Some(SymbolId(8));
    let mut binding16 = direct_binding(&owner16, 16, 70);
    binding16.required_rewrites = Arc::from([rewrite16.clone()]);
    binding16.required_expanded_ports = Arc::from([port16.clone()]);
    binding16.emission_context.generic_map.id = Some(SymbolId(16));
    pending.record_emission_binding(binding8);
    pending.record_emission_binding(binding16);

    let analysis = pending
        .finalize(session)
        .expect("complete semantic session should finalize");
    assert!(Arc::ptr_eq(
        analysis
            .lowering(&target8)
            .expect("8-bit lowering should exist"),
        &lowering8
    ));
    let resolved = analysis.rewrite(&rewrite8).expect("8-bit rewrite");
    assert_eq!(resolved.terminal.terminal, ResolvedNestedTerminalId(0));
    assert_eq!(resolved.semantic_segments, member.to_vec());
    assert_eq!(resolved.replace_from_segment, 1);
    assert_eq!(resolved.consumed_segments, 3);
    assert!(matches!(
        analysis.expanded_port(&port16),
        Some(ExpandedPortResolution::Nested { target, modport, .. })
            if target == &target16 && *modport == StrId(40)
    ));

    for phase in [EmissionPhase::Align, EmissionPhase::Build] {
        let mut prepared = analysis.prepare_emission(PathId(1), phase)?;
        assert_eq!(prepared.phase(), phase);
        let batch = prepared.take_owners(TokenId(70), EmissionOwnerKind::Interface)?;
        let frames: Vec<_> = batch.iter().collect();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].id(), EmissionBindingId::new(8));
        assert_eq!(frames[0].specialization(), &owner8);
        assert_eq!(
            frames[0].emission_context().generic_map.id,
            Some(SymbolId(8))
        );
        assert!(
            frames[0]
                .rewrite(OccurrenceKind::ExpressionIdentifier, TokenId(80))?
                .is_some()
        );
        assert!(
            frames[1]
                .rewrite(OccurrenceKind::HierarchicalIdentifier, TokenId(160))?
                .is_some()
        );
        assert!(frames[1].expanded_port(TokenId(161))?.is_some());
        prepared.finish()?;
    }
    Ok(())
}

#[test]
fn finalizer_failure_is_atomic_and_context_reports_typed_once_error()
-> Result<(), SpecializationIdentityError> {
    let session = AnalysisSessionId::new();
    let owner = key_in_session(session, 1, 8)?;
    let target = key_in_session(session, 2, 8)?;
    let rewrite = OccurrenceRewriteKey {
        owner: owner.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(9),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner, LoweringAvailability::NotNested);
    pending
        .record_rewrite_candidate(
            rewrite.clone(),
            PendingPathRewriteCandidate {
                target: target.clone(),
                semantic_segments: vec![resource_table::insert_str("member")],
                replace_from_segment: 0,
                consumed_segments: 2,
            },
        )
        .expect("candidate insertion should succeed");
    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::UnresolvedRewrite
        ))
    ));
    pending.record_interface_lowering(
        target,
        lowering_with_terminal(&[resource_table::insert_str("member")]),
    );
    assert!(pending.finalize(session).is_ok());
    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::AlreadyFinalized
        ))
    ));

    let mut context = Context::default();
    assert!(context.finish_nested_modport_analysis().is_ok());
    let error = context
        .finish_nested_modport_analysis()
        .expect_err("second Context finalization must be rejected");
    assert!(matches!(
        error,
        crate::AnalyzerError::NestedModportAnalysisInvariant {
            kind: NestedModportAnalysisInvariant::AlreadyFinalized,
            ..
        }
    ));
    assert_eq!(
        error.code().map(|code| code.to_string()).as_deref(),
        Some("nested_modport_analysis_invariant")
    );
    assert_eq!(
        error.to_string(),
        "nested modport analysis invariant: nested modport analysis was already finalized"
    );
    Ok(())
}
