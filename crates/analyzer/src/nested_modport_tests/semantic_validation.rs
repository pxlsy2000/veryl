#[test]
fn finalizer_rejects_same_terminal_spelling_on_a_different_semantic_path()
-> Result<(), SpecializationIdentityError> {
    let session = AnalysisSessionId::new();
    let generic_name = resource_table::insert_str("W");
    let value = Token::from_external_text("VALUE");
    let mut owner = signature(1, 1);
    owner
        .generic_parameters
        .push((generic_name, GenericSymbolPath::from(&value)));
    let key = NestedModportLoweringKey {
        session,
        specialization: valid_identity(owner, vec![])?.into(),
    };
    let scope = Token::from_external_text("scope");
    let scoped_value = Token::from_external_text("VALUE");
    let mut binding = direct_binding(&key, 0, 10);
    binding.emission_context.generic_map.map.insert(
        generic_name,
        GenericSymbolPath {
            paths: vec![
                GenericSymbol {
                    base: scope,
                    arguments: vec![],
                },
                GenericSymbol {
                    base: scoped_value,
                    arguments: vec![],
                },
            ],
            kind: GenericSymbolPathKind::Identifier,
            range: TokenRange {
                beg: scope,
                end: scoped_value,
            },
        },
    );
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key, LoweringAvailability::NotNested);
    pending.record_emission_binding(binding);

    assert!(matches!(
        pending.finalize(session),
        Err(NestedModportFinalizeError::Invariant(
            NestedModportAnalysisInvariant::MismatchedEmissionContext
        ))
    ));
    Ok(())
}

#[test]
fn normal_conversion_with_evaluated_generic_consts_finalizes() {
    let code = r#"
module Consumer::<W: u32> {
    gen NEXT: u32 = W + 1;
}
module Use8 {
    inst consumer: Consumer::<8>;
}
module Use16 {
    inst consumer: Consumer::<16>;
}
"#;
    symbol_table::clear();
    attribute_table::clear();
    let metadata = Metadata::create_default("nested_modport_generic_const")
        .expect("test metadata should construct");
    let parser = Parser::parse(code, &"").expect("generic-const fixture should parse");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();
    let mut errors = analyzer.analyze_pass1("nested_modport_generic_const", &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    assert!(errors.is_empty(), "fixture analysis errors: {errors:?}");

    let analysis = context
        .finish_nested_modport_analysis()
        .expect("evaluated generic constants must preserve semantic emission context");
    let consumer = symbol_table::get_all()
        .into_iter()
        .find(|symbol| {
            symbol.token.text.to_string() == "Consumer"
                && matches!(symbol.kind, SymbolKind::Module(_))
        })
        .expect("Consumer declaration should exist");
    let source = consumer
        .token
        .source
        .get_path()
        .expect("Consumer should have a source path");
    let mut prepared = analysis
        .prepare_emission(source, EmissionPhase::Build)
        .expect("generic-const analysis should preflight");
    let frames: Vec<_> = prepared
        .take_owners(consumer.token.id, EmissionOwnerKind::Module)
        .expect("Consumer owner batch should be published")
        .iter()
        .collect();
    assert_eq!(frames.len(), 2);
    let next_name = consumer
        .generic_consts()
        .into_iter()
        .find_map(|(name, _)| (name.to_string() == "NEXT").then_some(name))
        .expect("NEXT should be registered as a generic constant");
    let mut resolved: Vec<_> = frames
        .iter()
        .map(|frame| {
            let width = frame
                .specialization()
                .specialization
                .owner
                .generic_parameters[0]
                .1
                .to_string();
            let next = frame.emission_context().generic_map.map[&next_name].to_string();
            (width, next)
        })
        .collect();
    resolved.sort();
    assert_eq!(
        resolved,
        [
            ("16".to_string(), "17".to_string()),
            ("8".to_string(), "9".to_string())
        ]
    );
    let mut remaining_modules: Vec<_> = symbol_table::get_all()
        .into_iter()
        .filter(|symbol| {
            symbol.token.source.get_path() == Some(source)
                && symbol.token.id != consumer.token.id
                && matches!(symbol.kind, SymbolKind::Module(_))
        })
        .collect();
    remaining_modules.sort_by_key(|symbol| symbol.token.id);
    for module in remaining_modules {
        let frames: Vec<_> = prepared
            .take_owners(module.token.id, EmissionOwnerKind::Module)
            .expect("remaining owner batch should be published")
            .iter()
            .collect();
        assert_eq!(frames.len(), 1);
    }
    prepared
        .finish()
        .expect("the Consumer owner batch should be fully consumed");
}

#[test]
fn duplicate_emission_binding_ids_are_rejected_before_publication()
-> Result<(), SpecializationIdentityError> {
    let key = lowering_key()?;
    let mut pending = PendingNestedModportAnalysis::default();
    pending.record_lowering(key.clone(), LoweringAvailability::NotNested);
    pending.record_emission_binding(direct_binding(&key, 7, 10));
    pending.record_emission_binding(direct_binding(&key, 7, 11));

    assert!(pending.finalize(key.session).is_err());
    assert_eq!(pending.emission_bindings().len(), 2);
    Ok(())
}

fn key_in_session(
    session: AnalysisSessionId,
    symbol: usize,
    width: usize,
) -> Result<NestedModportLoweringKey, SpecializationIdentityError> {
    Ok(NestedModportLoweringKey {
        session,
        specialization: valid_identity(signature(symbol, width), vec![])?.into(),
    })
}

fn lowering_with_terminal(segments: &[StrId]) -> Arc<NestedModportLowering> {
    let resolved_type = ResolvedTerminalType::try_from_ir(&Type::new(TypeKind::Logic))
        .expect("logic must be an emit-capable terminal type");
    let terminal = ResolvedNestedTerminal {
        id: ResolvedNestedTerminalId(0),
        identifier: flatten_identifier_segments(segments),
        emitted_identifier: EmittedIdentifierIdentity::from_logical(
            flatten_identifier_segments(segments).logical,
        ),
        variable: VarId::default(),
        symbol: SymbolId(0),
        token: Default::default(),
        resolved_type,
    };
    let entry = ResolvedModportEntry {
        path: crate::ir::ModportMemberPath::from_slice(segments),
        direction: crate::symbol::Direction::Input,
        terminal: Some(ResolvedModportTerminal::FlattenedVariable {
            terminal: terminal.id,
        }),
        terminal_site: None,
    };
    let resolved = ResolvedModport {
        entries: Arc::from([entry]),
    };
    let mut modports = crate::HashMap::default();
    modports.insert(StrId(40), resolved.clone());
    if let Some(name) = segments.last() {
        modports.insert(*name, resolved);
    }
    Arc::new(NestedModportLowering::new(
        modports,
        Arc::from([terminal]),
        Default::default(),
    ))
}
