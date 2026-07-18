fn rewrite_resolution_work(scale: usize, scan_mutation: bool) -> (usize, Vec<u32>) {
    let session = AnalysisSessionId::new();
    let owner = NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
            Signature::new(SymbolId(900_001)),
            [],
        )
        .into(),
    };
    let target = NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
            Signature::new(SymbolId(900_002)),
            [],
        )
        .into(),
    };
    let child = resource_table::insert_str("rewrite_scale_child");
    let unrelated = resource_table::insert_str("rewrite_scale_unrelated");
    let terminals: Vec<_> = (0..scale)
        .map(|index| {
            let member = resource_table::insert_str(&format!("rewrite_scale_member_{index}"));
            let identifier = flatten_identifier_segments(&[child, member]);
            ResolvedNestedTerminal {
                id: ResolvedNestedTerminalId(index as u32),
                emitted_identifier: EmittedIdentifierIdentity::from_logical(identifier.logical),
                identifier,
                variable: VarId::default(),
                symbol: SymbolId(index),
                token: Default::default(),
                resolved_type: ResolvedTerminalType::try_from_ir(&Type::new(TypeKind::Logic))
                    .expect("logic terminal"),
            }
        })
        .collect();
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner.clone(), LoweringAvailability::NotNested);
    pending.record_interface_lowering(
        target.clone(),
        Arc::new(NestedModportLowering::new(
            Default::default(),
            terminals.into(),
            Default::default(),
        )),
    );
    for index in 0..scale {
        let key = OccurrenceRewriteKey {
            owner: owner.clone(),
            kind: OccurrenceKind::ExpressionIdentifier,
            token: TokenId(500_000 + index),
        };
        let is_rewrite = index % 2 == 0;
        let root = if is_rewrite { child } else { unrelated };
        let member = resource_table::insert_str(&format!("rewrite_scale_member_{index}"));
        let candidate = PendingPathRewriteCandidate {
            target: target.clone(),
            semantic_segments: vec![root, member],
            replace_from_segment: 0,
            consumed_segments: 2,
        };
        if is_rewrite {
            pending
                .record_rewrite_candidate(key, candidate)
                .expect("required candidate");
        } else {
            pending
                .record_optional_rewrite_candidate(key, candidate, Default::default())
                .expect("optional unrelated candidate");
        }
    }
    reset_collection_work();
    let analysis = if scan_mutation {
        with_terminal_scan_mutation(|| pending.finalize(session))
    } else {
        pending.finalize(session)
    }
    .expect("production rewrite resolution should finalize");
    let resolved = (0..scale)
        .filter_map(|index| {
            analysis
                .rewrite(&OccurrenceRewriteKey {
                    owner: owner.clone(),
                    kind: OccurrenceKind::ExpressionIdentifier,
                    token: TokenId(500_000 + index),
                })
                .map(|rewrite| rewrite.terminal.terminal.0)
        })
        .collect();
    (collection_work(), resolved)
}

#[test]
fn production_rewrite_resolution_work_is_affine_when_candidates_and_terminals_scale_together() {
    let samples = [128, 256, 512].map(|scale| rewrite_resolution_work(scale, false));
    for (scale, (_, rewrites)) in [128, 256, 512].into_iter().zip(&samples) {
        assert_eq!(rewrites.len(), scale / 2);
    }
    for pair in samples.windows(2) {
        assert!(
            pair[1].0 <= 3 * pair[0].0,
            "doubling candidates and terminals must remain affine: {samples:?}"
        );
    }
}

#[test]
fn terminal_scan_mutation_restores_product_growth_without_changing_rewrites() {
    let indexed = [32, 64, 128].map(|scale| rewrite_resolution_work(scale, false));
    let scanned = [32, 64, 128].map(|scale| rewrite_resolution_work(scale, true));
    for (indexed_sample, scanned_sample) in indexed.iter().zip(&scanned) {
        assert_eq!(indexed_sample.1, scanned_sample.1);
    }
    assert!(scanned[1].0 > 3 * scanned[0].0, "scan work: {scanned:?}");
    assert!(scanned[2].0 > 3 * scanned[1].0, "scan work: {scanned:?}");
    assert!(
        scanned[2].0 > 3 * indexed[2].0,
        "mutation must exercise the actual terminal-scan seam: indexed={indexed:?}, scanned={scanned:?}"
    );
}

#[test]
fn indexed_rewrite_resolution_preserves_longest_prefix_and_optional_root_legality() {
    let session = AnalysisSessionId::new();
    let key = |symbol| NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
            Signature::new(SymbolId(symbol)),
            [],
        )
        .into(),
    };
    let owner = key(910_001);
    let target = key(910_002);
    let child = resource_table::insert_str("indexed_child");
    let branch = resource_table::insert_str("indexed_branch");
    let leaf = resource_table::insert_str("indexed_leaf");
    let field = resource_table::insert_str("indexed_field");
    let unrelated = resource_table::insert_str("indexed_unrelated");
    let paths = [vec![child, branch], vec![child, branch, leaf]];
    let terminals: Vec<_> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let identifier = flatten_identifier_segments(path);
            ResolvedNestedTerminal {
                id: ResolvedNestedTerminalId(index as u32),
                emitted_identifier: EmittedIdentifierIdentity::from_logical(identifier.logical),
                identifier,
                variable: VarId::default(),
                symbol: SymbolId(index),
                token: Default::default(),
                resolved_type: ResolvedTerminalType::try_from_ir(&Type::new(TypeKind::Logic))
                    .expect("logic terminal"),
            }
        })
        .collect();
    let required = OccurrenceRewriteKey {
        owner: owner.clone(),
        kind: OccurrenceKind::HierarchicalIdentifier,
        token: TokenId(910_010),
    };
    let optional = OccurrenceRewriteKey {
        owner: owner.clone(),
        kind: OccurrenceKind::ExpressionIdentifier,
        token: TokenId(910_011),
    };
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(owner, LoweringAvailability::NotNested);
    pending.record_interface_lowering(
        target.clone(),
        Arc::new(NestedModportLowering::new(
            Default::default(),
            terminals.into(),
            Default::default(),
        )),
    );
    pending
        .record_rewrite_candidate(
            required.clone(),
            PendingPathRewriteCandidate {
                target: target.clone(),
                semantic_segments: vec![child, branch, leaf, field],
                replace_from_segment: 1,
                consumed_segments: 6,
            },
        )
        .expect("required candidate");
    pending
        .record_optional_rewrite_candidate(
            optional.clone(),
            PendingPathRewriteCandidate {
                target,
                semantic_segments: vec![unrelated, field],
                replace_from_segment: 0,
                consumed_segments: 2,
            },
            Default::default(),
        )
        .expect("optional unrelated candidate");
    let analysis = pending.finalize(session).expect("indexed resolution");
    let rewrite = analysis.rewrite(&required).expect("longest-prefix rewrite");
    assert_eq!(rewrite.terminal.terminal, ResolvedNestedTerminalId(1));
    assert_eq!(rewrite.semantic_segments, vec![child, branch, leaf]);
    assert_eq!(rewrite.replace_from_segment, 1);
    assert_eq!(rewrite.consumed_segments, 5);
    assert!(analysis.rewrite(&optional).is_none());
}
