use super::*;
use crate::Emitter;
use std::path::Path;
use std::path::PathBuf;
use veryl_analyzer::{Analyzer, Context, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::Parser;

mod ownership;
mod t9;
mod type_trivia;

const VALID_SV: &str = r#"
interface ChildIf;
    logic [7:0] fatal;
    modport source (output fatal);
endinterface

interface ParentIf;
    logic [7:0] child__fatal [2];
    modport source (output child__fatal);
endinterface

module Consumer;
    ParentIf p();
    logic [7:0] observed;
    assign observed = p.child__fatal;
endmodule
"#;

fn parse(source: &str) -> Result<SvStructure, SvStructureError> {
    SvStructure::parse(source, Path::new("generated.sv"))
}

#[test]
fn selected_structure_is_extracted_when_generated_sv_is_valid() {
    // Given
    let source = VALID_SV;

    // When
    let structure = parse(source).expect("valid generated SystemVerilog must parse");

    // Then
    structure
        .expect_component(ComponentKind::Interface, "ParentIf")
        .expect("interface component must be present");
    structure
        .expect_component(ComponentKind::Module, "Consumer")
        .expect("module component must be present");
    structure
        .expect_declaration(DeclarationExpectation {
            owner: "ParentIf",
            name: "child__fatal",
            type_text: "logic [7:0] [2]",
        })
        .expect("flattened declaration must have its resolved type");
    structure
        .expect_modport_member(ModportMemberExpectation {
            owner: "ParentIf",
            modport: "source",
            member: "child__fatal",
            direction: PortDirection::Output,
        })
        .expect("flattened member must retain its direction");
    structure
        .expect_instance(InstanceExpectation {
            owner: "Consumer",
            component: "ParentIf",
            instance: "p",
        })
        .expect("parent interface instance must be present");
    structure
        .expect_identifier_chain_absent("Consumer", &["p", "child", "fatal"])
        .expect("stale nested chain must be absent");
    structure
        .expect_identifier_chain("Consumer", &["p", "child__fatal"])
        .expect("flattened identifier chain must be present");
}

#[test]
fn selected_structure_is_extracted_from_real_emitter_output() {
    // Given
    let veryl = r#"interface ChildIf {
    var fatal: logic;
    modport sink {
        fatal: input,
    }
}

interface ParentIf {
    inst child: ChildIf;
    modport sink {
        child.sink: modport,
    }
}
"#;
    let metadata = Metadata::create_default("prj").expect("test metadata must be valid");
    symbol_table::clear();
    attribute_table::clear();
    let parser = Parser::parse(veryl, &"generated.veryl").expect("test Veryl must parse");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    analyzer.analyze_pass1("prj", &parser.veryl);
    Analyzer::analyze_post_pass1();
    analyzer.analyze_pass2(&parser.veryl, &mut context, None);
    let analysis = context
        .finish_nested_modport_analysis()
        .expect("emitter analysis must finalize");
    let mut emitter = Emitter::new(
        &metadata,
        &PathBuf::from("generated.veryl"),
        &PathBuf::from("generated.sv"),
        &PathBuf::from("generated.sv.map"),
    );

    // When
    emitter
        .emit(&parser.veryl, veryl, analysis.as_ref())
        .expect("emission must succeed");
    let structure = parse(emitter.as_str()).expect("emitted SystemVerilog must parse");

    // Then
    structure
        .expect_declaration(DeclarationExpectation {
            owner: "prj_ParentIf",
            name: "child__fatal",
            type_text: "logic",
        })
        .expect("flattened declaration must have the selected owner and type");
    structure
        .expect_modport_member(ModportMemberExpectation {
            owner: "prj_ParentIf",
            modport: "sink",
            member: "child__fatal",
            direction: PortDirection::Input,
        })
        .expect("flattened member must have the selected direction");
    structure
        .expect_instance_absent(InstanceExpectation {
            owner: "prj_ParentIf",
            component: "prj_ChildIf",
            instance: "child",
        })
        .expect("flattened child instance must be absent");
    structure
        .expect_identifier_chain_absent("prj_ParentIf", &["child", "fatal"])
        .expect("flattened interface must not retain a stale dotted chain");
}

#[test]
fn parse_error_is_distinct_when_sv_is_malformed() {
    // Given
    let source = "interface Broken; logic x;";

    // When
    let error = parse(source).expect_err("unterminated interface must not parse");

    // Then
    assert!(matches!(error, SvStructureError::Parse { .. }));
}

#[test]
fn duplicate_declaration_is_an_invariant_error_when_sv_is_valid() {
    // Given
    let source = VALID_SV.replace(
        "logic [7:0] child__fatal [2];",
        "logic [7:0] child__fatal [2]; logic [7:0] child__fatal [2];",
    );

    // When
    let structure = parse(&source).expect("duplicate declarations remain valid SV syntax");
    let error = structure
        .expect_declaration(DeclarationExpectation {
            owner: "ParentIf",
            name: "child__fatal",
            type_text: "logic [7:0] [2]",
        })
        .expect_err("duplicate declaration must violate selected structure");

    // Then
    assert_eq!(
        error,
        SvStructureError::Invariant {
            detail:
                "expected one declaration ParentIf.child__fatal with type logic [7:0] [2], found 2"
                    .to_string(),
        }
    );
}

#[test]
fn wrong_owner_declaration_is_an_invariant_error_when_sv_is_valid() {
    // Given
    let source = VALID_SV
        .replace("    logic [7:0] child__fatal [2];\n", "")
        .replace(
            "module Consumer;",
            "module Consumer;\n    logic [7:0] child__fatal;",
        );

    // When
    let structure = parse(&source).expect("moving a declaration keeps valid SV syntax");
    let error = structure
        .expect_declaration(DeclarationExpectation {
            owner: "ParentIf",
            name: "child__fatal",
            type_text: "logic [7:0] [2]",
        })
        .expect_err("wrong declaration owner must be rejected");

    // Then
    assert!(matches!(error, SvStructureError::Invariant { .. }));
}

#[test]
fn wrong_modport_direction_is_an_invariant_error_when_sv_is_valid() {
    // Given
    let source = VALID_SV.replace(
        "modport source (output child__fatal);",
        "modport source (input child__fatal);",
    );

    // When
    let structure = parse(&source).expect("direction mutation keeps valid SV syntax");
    let error = structure
        .expect_modport_member(ModportMemberExpectation {
            owner: "ParentIf",
            modport: "source",
            member: "child__fatal",
            direction: PortDirection::Output,
        })
        .expect_err("wrong direction must be rejected");

    // Then
    assert!(matches!(error, SvStructureError::Invariant { .. }));
}

#[test]
fn stale_dotted_chain_is_an_invariant_error_when_sv_is_valid() {
    // Given
    let source = VALID_SV.replace("p.child__fatal", "p.child.fatal");

    // When
    let structure = parse(&source).expect("dotted-chain mutation keeps valid SV syntax");
    let error = structure
        .expect_identifier_chain_absent("Consumer", &["p", "child", "fatal"])
        .expect_err("stale dotted chain must be rejected");

    // Then
    assert!(matches!(error, SvStructureError::Invariant { .. }));
}

#[test]
fn undeleted_child_instance_is_an_invariant_error_when_sv_is_valid() {
    // Given
    let source = VALID_SV.replace(
        "interface ParentIf;",
        "interface ParentIf;\n    ChildIf child();",
    );

    // When
    let structure = parse(&source).expect("child instance keeps valid SV syntax");
    let error = structure
        .expect_instance_absent(InstanceExpectation {
            owner: "ParentIf",
            component: "ChildIf",
            instance: "child",
        })
        .expect_err("undeleted nested child must be rejected");

    // Then
    assert!(matches!(error, SvStructureError::Invariant { .. }));
}
