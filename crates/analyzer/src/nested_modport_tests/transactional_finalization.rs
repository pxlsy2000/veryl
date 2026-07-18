#[derive(Clone, Copy)]
enum LateFailure {
    Rewrite,
    ExpandedPort,
}

struct LateFailureFixture {
    pending: PendingNestedModportAnalysis,
    target: NestedModportLoweringKey,
    rewrite: OccurrenceRewriteKey,
    expanded_port: ExpandedPortKey,
    source: PathId,
    declaration: TokenId,
}

#[derive(Debug, Eq, PartialEq)]
struct FinalizedSnapshot {
    lowering: Option<Arc<NestedModportLowering>>,
    rewrites: Vec<OccurrenceRewriteKey>,
    expanded_ports: Vec<ExpandedPortKey>,
    rewrite: Option<ResolvedPathRewrite>,
    expanded_port: Option<ExpandedPortResolution>,
    emission_batches: Vec<(EmissionPhase, Vec<EmissionBindingId>)>,
}

fn empty_generic_interface() -> Symbol {
    let project = "transactional_nested_modport_finalization";
    let path = format!("{project}.veryl");
    let metadata = Metadata::create_default(project).expect("fixture metadata should construct");
    let parser = Parser::parse("interface Empty::<W: u32> {}", &path)
        .expect("empty generic interface should parse");
    let analyzer = Analyzer::new(&metadata);
    let mut errors = analyzer.analyze_pass1(project, &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    assert!(errors.is_empty(), "fixture analysis errors: {errors:?}");
    let interface = symbol_table::get_all()
        .into_iter()
        .find(|symbol| {
            symbol.token.text.to_string() == "Empty"
                && matches!(symbol.kind, SymbolKind::Interface(_))
        })
        .expect("Empty interface should be registered");
    assert!(interface.has_generic_paramters());
    assert!(interface.generic_maps().is_empty());
    interface
}

fn pending_with_late_failure(
    session: AnalysisSessionId,
    interface: &Symbol,
    failure: LateFailure,
) -> Result<LateFailureFixture, Box<dyn std::error::Error>> {
    let owner = key_in_session(session, interface.id.0, 8)?;
    let target = key_in_session(session, interface.id.0 + 1, 8)?;
    let rewrite = OccurrenceRewriteKey {
        owner: owner.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(90),
    };
    let expanded_port = ExpandedPortKey {
        owner,
        token: TokenId(91),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    let source = interface
        .token
        .source
        .get_path()
        .expect("fixture interface should have a source path");
    pending.record_generic_emission_owner(PendingGenericEmissionOwner {
        session,
        source,
        declaration: interface.token.id,
        kind: EmissionOwnerKind::Interface,
        symbol: interface.id,
    });
    match failure {
        LateFailure::Rewrite => pending.record_rewrite_candidate(
            rewrite.clone(),
            PendingPathRewriteCandidate {
                target: target.clone(),
                semantic_segments: vec![resource_table::insert_str("member")],
                replace_from_segment: 0,
                consumed_segments: 1,
            },
        )?,
        LateFailure::ExpandedPort => pending.record_expanded_port_candidate(
            expanded_port.clone(),
            PendingExpandedPortCandidate {
                target: target.clone(),
                modport: StrId(40),
            },
        )?,
    }
    Ok(LateFailureFixture {
        pending,
        target,
        rewrite,
        expanded_port,
        source,
        declaration: interface.token.id,
    })
}

fn finalized_snapshot(
    analysis: &NestedModportAnalysis,
    fixture: &LateFailureFixture,
) -> Result<FinalizedSnapshot, NestedModportAnalysisInvariant> {
    let mut emission_batches = Vec::new();
    for phase in [EmissionPhase::Align, EmissionPhase::Build] {
        let mut prepared = analysis.prepare_emission(fixture.source, phase)?;
        let batch = prepared.take_owners(fixture.declaration, EmissionOwnerKind::Interface)?;
        let ids = batch.iter().map(EmissionFrame::id).collect();
        prepared.finish()?;
        emission_batches.push((phase, ids));
    }
    let mut rewrites: Vec<_> = analysis.rewrite_keys().cloned().collect();
    rewrites.sort();
    let mut expanded_ports: Vec<_> = analysis.expanded_port_keys().cloned().collect();
    expanded_ports.sort();
    Ok(FinalizedSnapshot {
        lowering: analysis.lowering(&fixture.target).cloned(),
        rewrites,
        expanded_ports,
        rewrite: analysis.rewrite(&fixture.rewrite).cloned(),
        expanded_port: analysis.expanded_port(&fixture.expanded_port).cloned(),
        emission_batches,
    })
}

#[test]
fn failed_finalization_preserves_empty_generic_owner_for_repair_and_retry()
-> Result<(), Box<dyn std::error::Error>> {
    symbol_table::clear();
    attribute_table::clear();
    let interface = empty_generic_interface();

    for (failure, expected) in [
        (
            LateFailure::Rewrite,
            NestedModportAnalysisInvariant::UnresolvedRewrite,
        ),
        (
            LateFailure::ExpandedPort,
            NestedModportAnalysisInvariant::UnresolvedExpandedPort,
        ),
    ] {
        let session = AnalysisSessionId::new();
        let mut repaired_fixture = pending_with_late_failure(session, &interface, failure)?;
        assert!(matches!(
            repaired_fixture.pending.finalize(session),
            Err(NestedModportFinalizeError::Invariant(kind)) if kind == expected
        ));

        let terminal_lowering = lowering_with_terminal(&[resource_table::insert_str("member")]);
        repaired_fixture.pending.record_interface_lowering(
            repaired_fixture.target.clone(),
            Arc::clone(&terminal_lowering),
        );
        let repaired = repaired_fixture.pending.finalize(session)?;
        assert!(matches!(
            repaired_fixture.pending.finalize(session),
            Err(NestedModportFinalizeError::Invariant(
                NestedModportAnalysisInvariant::AlreadyFinalized
            ))
        ));

        let mut fresh_fixture = pending_with_late_failure(session, &interface, failure)?;
        fresh_fixture
            .pending
            .record_interface_lowering(fresh_fixture.target.clone(), terminal_lowering);
        let one_shot = fresh_fixture.pending.finalize(session)?;
        let repaired_snapshot = finalized_snapshot(&repaired, &repaired_fixture)?;
        let fresh_snapshot = finalized_snapshot(&one_shot, &fresh_fixture)?;
        assert_eq!(repaired_snapshot, fresh_snapshot);
        assert!(repaired_snapshot.lowering.is_some());
        assert_eq!(
            repaired_snapshot.emission_batches,
            [
                (EmissionPhase::Align, Vec::new()),
                (EmissionPhase::Build, Vec::new())
            ]
        );
        match failure {
            LateFailure::Rewrite => {
                assert_eq!(
                    repaired_snapshot.rewrites,
                    [repaired_fixture.rewrite.clone()]
                );
                assert!(repaired_snapshot.expanded_ports.is_empty());
                assert!(repaired_snapshot.rewrite.is_some());
                assert!(repaired_snapshot.expanded_port.is_none());
            }
            LateFailure::ExpandedPort => {
                assert!(repaired_snapshot.rewrites.is_empty());
                assert_eq!(
                    repaired_snapshot.expanded_ports,
                    [repaired_fixture.expanded_port.clone()]
                );
                assert!(repaired_snapshot.rewrite.is_none());
                assert!(repaired_snapshot.expanded_port.is_some());
            }
        }
        assert!(matches!(
            fresh_fixture.pending.finalize(session),
            Err(NestedModportFinalizeError::Invariant(
                NestedModportAnalysisInvariant::AlreadyFinalized
            ))
        ));
    }
    Ok(())
}
