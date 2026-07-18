#![cfg(test)]

use crate::analyzer_error::InvalidNestedModportKind;
use crate::ir::Ir;
use crate::{Analyzer, AnalyzerError, Context, attribute_table, symbol_table};
use miette::Diagnostic;
use veryl_metadata::Metadata;
use veryl_parser::{Parser, doc_comment_table};

fn analyze_and_finalize(code: &str) -> Vec<AnalyzerError> {
    symbol_table::clear();
    attribute_table::clear();
    doc_comment_table::clear();

    let metadata = Metadata::create_default("prj").expect("test metadata should be valid");
    let parser =
        Parser::parse(code, &"unlowered_local_reference.veryl").expect("test source should parse");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();

    let mut errors = analyzer.analyze_pass1("prj", &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    if let Err(error) = context.finish_nested_modport_analysis() {
        errors.push(error);
    }
    errors.append(&mut Analyzer::analyze_post_pass2(&ir));
    errors
}

fn assert_unlowered_reference(code: &str, path: &str) {
    let errors = analyze_and_finalize(code);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors
            .iter()
            .all(|error| !matches!(error, AnalyzerError::UndefinedIdentifier { .. })),
        "{errors:?}"
    );
    let nested: Vec<_> = errors
        .iter()
        .filter(|error| matches!(error, AnalyzerError::InvalidNestedModport { .. }))
        .collect();
    assert_eq!(nested.len(), 1, "{errors:?}");

    let error = nested[0];
    let AnalyzerError::InvalidNestedModport {
        path: diagnostic_path,
        kind,
        error_location,
        ..
    } = error
    else {
        unreachable!("filtered to InvalidNestedModport")
    };
    assert_eq!(diagnostic_path, path);
    assert_eq!(
        kind,
        &InvalidNestedModportKind::UnloweredLocalReference { path: path.into() }
    );
    assert_eq!(
        error.code().map(|code| code.to_string()).as_deref(),
        Some("invalid_nested_modport")
    );
    assert_eq!(
        error.to_string(),
        format!(
            "cannot lower nested modport \"{path}\": local reference \"{path}\" is not forwarded by the nested modport"
        )
    );
    assert_eq!(
        *error_location,
        (
            code.find(path).expect("fixture contains reference"),
            path.len()
        )
            .into()
    );
}

#[test]
fn removed_nested_root_rejects_unforwarded_local_rhs_reference() {
    assert_unlowered_reference(
        r#"
interface ChildIf {
    var forwarded: logic;
    var hidden: logic;
    modport sink { forwarded: input, }
}
interface ParentIf {
    inst cpu: ChildIf;
    #[allow(unused_variable)]
    let seen: logic = cpu.hidden;
    modport sink { cpu.sink: modport, }
}
"#,
        "cpu.hidden",
    );
}

#[test]
fn removed_nested_root_rejects_unforwarded_local_lhs_reference() {
    assert_unlowered_reference(
        r#"
interface ChildIf {
    var forwarded: logic;
    var hidden: logic;
    modport sink { forwarded: inout, }
}
interface ParentIf {
    inst cpu: ChildIf;
    always_comb { cpu.hidden = 1'b0; }
    modport sink { cpu.sink: modport, }
}
"#,
        "cpu.hidden",
    );
}

#[test]
fn removed_nested_root_rejects_unforwarded_local_function_reference() {
    assert_unlowered_reference(
        r#"
interface ChildIf {
    var forwarded: logic;
    function get_hidden() -> logic { return forwarded; }
    modport sink { forwarded: input, }
}
interface ParentIf {
    inst cpu: ChildIf;
    #[allow(unused_variable)]
    let seen: logic = cpu.get_hidden();
    modport sink { cpu.sink: modport, }
}
"#,
        "cpu.get_hidden",
    );
}

#[test]
fn removed_nested_root_rejects_unforwarded_deeper_reference() {
    assert_unlowered_reference(
        r#"
interface LeafIf {
    var forwarded: logic;
    var hidden: logic;
    modport sink { forwarded: input, }
}
interface ChildIf {
    inst sub: LeafIf;
    modport sink { sub.sink: modport, }
}
interface ParentIf {
    inst cpu: ChildIf;
    #[allow(unused_variable)]
    let seen: logic = cpu.sub.hidden;
    modport sink { cpu.sink: modport, }
}
"#,
        "cpu.sub.hidden",
    );
}

#[test]
fn forwarded_and_unrelated_roots_finalize_without_unlowered_reference_error() {
    let errors = analyze_and_finalize(
        r#"
interface ChildIf {
    var forwarded: logic;
    var hidden: logic;
    modport sink { forwarded: input, }
}
interface ParentIf {
    inst cpu: ChildIf;
    inst raw: ChildIf;
    let forwarded_seen: logic = cpu.forwarded;
    let unrelated_seen: logic = raw.hidden;
    modport sink { cpu.sink: modport, }
}
"#,
    );
    assert!(
        errors
            .iter()
            .all(|error| !matches!(error, AnalyzerError::InvalidNestedModport { .. })),
        "{errors:?}"
    );
}
