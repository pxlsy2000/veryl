use crate::{
    Analyzer, AnalyzerError, Context, analyzer_error::InvalidNestedModportKind, attribute_table,
    ir::Ir, symbol_table,
};
use miette::Diagnostic;
use veryl_metadata::Metadata;
use veryl_parser::{Parser, doc_comment_table};

fn analyze(code: &str, metadata: &Metadata) -> Vec<AnalyzerError> {
    symbol_table::clear();
    attribute_table::clear();
    doc_comment_table::clear();
    let parser =
        Parser::parse(code, &"collision.veryl").expect("collision regression source must parse");
    let analyzer = Analyzer::new(metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();
    let mut errors = analyzer.analyze_pass1("prj", &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    errors.append(&mut Analyzer::analyze_post_pass2(&ir));
    errors
}

struct ExpectedCollision<'a> {
    path: &'a str,
    flat: &'a str,
    first_path: &'a str,
    primary: (usize, usize),
    secondary: (usize, usize),
}

fn assert_collision(errors: &[AnalyzerError], expected: ExpectedCollision<'_>) {
    assert_eq!(errors.len(), 1, "expected only the collision: {errors:?}");
    let error = &errors[0];
    assert!(
        matches!(
            error,
            AnalyzerError::InvalidNestedModport {
                path: actual_path,
                kind: InvalidNestedModportKind::FlatNameCollision {
                    flat: actual_flat,
                    first_path: actual_first,
                },
                error_location,
                first_conflict_location,
                ..
            } if actual_path == expected.path
                && actual_flat == expected.flat
                && actual_first == expected.first_path
                && error_location.offset() == expected.primary.0
                && error_location.len() == expected.primary.1
                && first_conflict_location.len() == 1
                && first_conflict_location[0].offset() == expected.secondary.0
                && first_conflict_location[0].len() == expected.secondary.1
        ),
        "unexpected collision: {error:?}"
    );
    assert_eq!(
        error.code().map(|code| code.to_string()),
        Some("invalid_nested_modport".to_owned())
    );
    assert_eq!(
        error.to_string(),
        format!(
            "cannot lower nested modport \"{}\": flattened name \"{}\" also represents \"{}\"",
            expected.path, expected.flat, expected.first_path
        )
    );
}

#[test]
fn raw_direct_identifier_collides_with_flattened_terminal_before_publication() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    var r#child__payload: logic;
    modport sink {
        child.sink: modport,
        r#child__payload: input,
    }
}"#;
    let errors = analyze(
        code,
        &Metadata::create_default("prj").expect("valid metadata"),
    );
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "child.payload",
            flat: "child__payload",
            first_path: "r#child__payload",
            primary: (
                code.find("child.sink").expect("nested item"),
                "child.sink".len(),
            ),
            secondary: (
                code.find("r#child__payload").expect("direct declaration"),
                "r#child__payload".len(),
            ),
        },
    );
}

#[test]
fn retained_interface_function_collides_with_flattened_terminal_before_publication() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    function child__payload() -> logic { return 0; }
    modport sink { child.sink: modport, }
}"#;
    let errors = analyze(
        code,
        &Metadata::create_default("prj").expect("valid metadata"),
    );
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "child.payload",
            flat: "child__payload",
            first_path: "child__payload",
            primary: (
                code.find("child.sink").expect("nested item"),
                "child.sink".len(),
            ),
            secondary: (
                code.find("child__payload").expect("function declaration"),
                "child__payload".len(),
            ),
        },
    );
}

#[test]
fn clock_affix_identifier_collides_with_direct_terminal_before_publication() {
    let code = r#"interface ChildIf {
    var clk: clock;
    modport source { clk: output, }
}
interface ParentIf {
    inst child: ChildIf;
    var cp_child__clk_cs: logic;
    modport source {
        child.source: modport,
        cp_child__clk_cs: input,
    }
}"#;
    let mut metadata = Metadata::create_default("prj").expect("valid metadata");
    metadata.build.clock_posedge_prefix = Some("cp_".to_owned());
    metadata.build.clock_posedge_suffix = Some("_cs".to_owned());
    let errors = analyze(code, &metadata);
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "child.clk",
            flat: "cp_child__clk_cs",
            first_path: "cp_child__clk_cs",
            primary: (
                code.find("child.source").expect("nested item"),
                "child.source".len(),
            ),
            secondary: (
                code.find("cp_child__clk_cs").expect("direct declaration"),
                "cp_child__clk_cs".len(),
            ),
        },
    );
}

#[test]
fn reset_affix_and_name_mangling_options_share_the_collision_identity() {
    let code = r#"interface ChildIf {
    var rst: reset;
    modport source { rst: output, }
}
interface ParentIf {
    inst r#child: ChildIf;
    var rp_child__rst_rs: logic;
    modport source {
        r#child.source: modport,
        rp_child__rst_rs: input,
    }
}"#;
    let mut metadata = Metadata::create_default("prj").expect("valid metadata");
    metadata.build.reset_low_prefix = Some("rp_".to_owned());
    metadata.build.reset_low_suffix = Some("_rs".to_owned());
    metadata.build.hashed_mangled_name = true;
    metadata.build.omit_project_prefix = true;
    let errors = analyze(code, &metadata);
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "r#child.rst",
            flat: "rp_child__rst_rs",
            first_path: "rp_child__rst_rs",
            primary: (
                code.find("r#child.source").expect("nested item"),
                "r#child.source".len(),
            ),
            secondary: (
                code.find("rp_child__rst_rs").expect("direct declaration"),
                "rp_child__rst_rs".len(),
            ),
        },
    );
}
