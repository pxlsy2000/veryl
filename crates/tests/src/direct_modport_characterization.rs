use std::collections::BTreeMap;
use std::path::PathBuf;
use veryl_analyzer::symbol::{Direction, SymbolKind};
use veryl_analyzer::{Analyzer, Context, attribute_table, symbol_table};
use veryl_emitter::Emitter;
use veryl_metadata::Metadata;
use veryl_parser::Parser;
use veryl_parser::resource_table;

const SOURCE: &str = r#"interface DirectModportCharacterization {
    var z: logic;
    var a: logic;
    var b: logic;

    function observe () -> logic {
        return z;
    }

    modport first {
        z      : input ,
        a      : output,
        observe: import,
    }

    modport second {
        z      : output,
        b      : inout ,
        observe: import,
    }

    modport same_overlap {
        ..same(first, second)
    }

    modport converse_overlap {
        ..converse(first, second)
    }

    modport explicit_same {
        z: inout,
        ..same(first, second)
    }
}
"#;

#[derive(Debug)]
struct DirectBehavior {
    members: BTreeMap<String, Vec<(String, Direction)>>,
    emitted: String,
    source_map: String,
}

fn observe_direct_behavior() -> DirectBehavior {
    symbol_table::clear();
    attribute_table::clear();

    let metadata = Metadata::create_default("direct_characterization").unwrap();
    let source_path = PathBuf::from("direct_modport_characterization.veryl");
    let output_path = PathBuf::from("direct_modport_characterization.sv");
    let map_path = PathBuf::from("direct_modport_characterization.sv.map");
    let parsed = Parser::parse(SOURCE, &source_path).unwrap();
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();

    let pass1_errors = analyzer.analyze_pass1(&metadata.project.name, &parsed.veryl);
    assert!(pass1_errors.is_empty(), "pass1 errors: {pass1_errors:?}");
    let post_pass1_errors = Analyzer::analyze_post_pass1();
    assert!(
        post_pass1_errors.is_empty(),
        "post-pass1 errors: {post_pass1_errors:?}"
    );
    let pass2_errors = analyzer.analyze_pass2(&parsed.veryl, &mut context, None);
    assert!(pass2_errors.is_empty(), "pass2 errors: {pass2_errors:?}");
    let analysis = context
        .finish_nested_modport_analysis()
        .expect("direct analysis must finalize");

    let members = symbol_table::get_all()
        .into_iter()
        .filter_map(|symbol| {
            let SymbolKind::Modport(modport) = symbol.kind else {
                return None;
            };
            let name = resource_table::get_str_value(symbol.token.text).unwrap();
            let members = modport
                .members
                .iter()
                .map(|id| {
                    let member = symbol_table::get(*id).unwrap();
                    let name = resource_table::get_str_value(member.token.text).unwrap();
                    let direction = match member.kind {
                        SymbolKind::ModportVariableMember(property) => property.direction,
                        SymbolKind::ModportFunctionMember(_) => Direction::Import,
                        kind => panic!("unexpected direct modport member: {kind:?}"),
                    };
                    (name, direction)
                })
                .collect();
            Some((name, members))
        })
        .collect();

    let mut emitter = Emitter::new(&metadata, &source_path, &output_path, &map_path);
    emitter
        .emit(&parsed.veryl, SOURCE, analysis.as_ref())
        .expect("direct emission must succeed");
    let source_map = String::from_utf8(emitter.source_map().to_bytes().unwrap()).unwrap();

    DirectBehavior {
        members,
        emitted: emitter.as_str().to_string(),
        source_map,
    }
}

fn modport_block<'a>(emitted: &'a str, name: &str, next_name: &str) -> &'a str {
    let start = emitted.find(&format!("    modport {name} (")).unwrap();
    let end = emitted[start..]
        .find(&format!("    modport {next_name} ("))
        .map(|offset| start + offset)
        .unwrap_or(emitted.len());
    emitted[start..end].trim_end()
}

#[test]
fn direct_modport_overlap_uses_later_target_direction_in_declaration_order() {
    // Given a direct modport whose defaults overlap on z and mention members out of declaration order.
    let behavior = observe_direct_behavior();

    // When the analyzer expands the multi-target `same` default.
    let observed = &behavior.members["same_overlap"];

    // Then later-target precedence and declaration ordering are exact in both IR and emitted SV.
    assert_eq!(
        &vec![
            ("z".to_string(), Direction::Output),
            ("a".to_string(), Direction::Output),
            ("b".to_string(), Direction::Inout),
            ("observe".to_string(), Direction::Import),
        ],
        observed
    );
    assert_eq!(
        "    modport same_overlap (\n        output z      ,\n        output a      ,\n        inout  b      ,\n        import observe\n    );",
        modport_block(&behavior.emitted, "same_overlap", "converse_overlap")
    );
}

#[test]
fn direct_modport_explicit_member_precedes_and_overrides_default() {
    // Given a direct modport with an explicit z entry before an overlapping `same` default.
    let behavior = observe_direct_behavior();

    // When explicit and default-derived members are combined.
    let observed = &behavior.members["explicit_same"];

    // Then the explicit entry wins, stays first, and the remaining defaults keep declaration order.
    assert_eq!(
        &vec![
            ("z".to_string(), Direction::Inout),
            ("a".to_string(), Direction::Output),
            ("b".to_string(), Direction::Inout),
            ("observe".to_string(), Direction::Import),
        ],
        observed
    );
    assert_eq!(
        "    modport explicit_same (\n        inout  z      ,\n        output a      ,\n        inout  b      ,\n        import observe\n    );\nendinterface\n//# sourceMappingURL=direct_modport_characterization.sv.map",
        modport_block(&behavior.emitted, "explicit_same", "missing")
    );
}

#[test]
fn direct_modport_same_retains_function_imports_and_source_map() {
    // Given direct targets that both import a function.
    let behavior = observe_direct_behavior();
    println!("IR_MEMBER_MAP={:#?}", behavior.members);
    println!("EMITTED_SV_BEGIN\n{}EMITTED_SV_END", behavior.emitted);
    println!("SOURCE_MAP_BEGIN\n{}\nSOURCE_MAP_END", behavior.source_map);

    // When `same` expands those targets and emission builds its source map.
    let observed = &behavior.members["same_overlap"];

    // Then the import remains a member and the generated map names the portable source artifact.
    assert_eq!(
        Some(&("observe".to_string(), Direction::Import)),
        observed.last()
    );
    assert!(
        behavior
            .source_map
            .contains("\"sources\":[\"direct_modport_characterization.veryl\"]")
    );
    assert!(
        behavior
            .source_map
            .contains("\"names\":[\"\",\"interface\"")
    );
    assert!(behavior.source_map.contains("\"mappings\":\""));
}

#[test]
fn direct_modport_converse_omits_function_imports() {
    // Given direct targets containing variables and a function import.
    let behavior = observe_direct_behavior();

    // When `converse` expands the overlapping targets.
    let observed = &behavior.members["converse_overlap"];

    // Then variable directions are reversed, inout is retained, and functions are omitted exactly.
    assert_eq!(
        &vec![
            ("z".to_string(), Direction::Input),
            ("a".to_string(), Direction::Input),
            ("b".to_string(), Direction::Inout),
        ],
        observed
    );
    assert_eq!(
        "    modport converse_overlap (\n        input z,\n        input a,\n        inout b\n    );",
        modport_block(&behavior.emitted, "converse_overlap", "explicit_same")
    );
    assert!(
        !modport_block(&behavior.emitted, "converse_overlap", "explicit_same").contains("observe")
    );
}
