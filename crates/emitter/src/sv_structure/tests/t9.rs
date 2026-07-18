use super::*;
use std::env;
use std::fs;

#[test]
#[ignore = "T9 fixture path is supplied by the validation runner"]
fn generated_t9_fixture_satisfies_selected_nested_invariants() {
    // Given
    let case = env::var("VERYL_T9_CASE").expect("VERYL_T9_CASE must select a fixture");
    let path = env::var("VERYL_T9_SV").expect("VERYL_T9_SV must name generated SV");
    let source = fs::read_to_string(&path).expect("generated T9 SV must be readable");

    // When
    let structure = SvStructure::parse(&source, Path::new(&path))
        .expect("generated T9 SystemVerilog must parse");

    // Then
    match case.as_str() {
        "generic-context-matrix" => expect_generic_context_matrix(&structure),
        "defaults-imports" => expect_defaults_imports(&structure),
        "raw-identifier" => expect_raw_identifier(&structure),
        "regression-same-access" => expect_same_access(&structure),
        "genvar-control" => expect_genvar_control(&structure),
        "package-function" => expect_package_function(&structure),
        "package-function-combined" => expect_package_function_combined(&structure),
        _ => panic!("unsupported T9 structural fixture: {case}"),
    }
}

fn expect_genvar_control(structure: &SvStructure) {
    let owner = "nested_t9_genvar_control_ParentIf";
    structure
        .expect_declaration(DeclarationExpectation {
            owner,
            name: "child__payload",
            type_text: "logic",
        })
        .expect("the flattened terminal must remain declared beside a distinct genvar");
    structure
        .expect_modport_member(ModportMemberExpectation {
            owner,
            modport: "sink",
            member: "child__payload",
            direction: PortDirection::Input,
        })
        .expect("the control modport must select the flattened terminal");
    structure
        .expect_instance_absent(InstanceExpectation {
            owner,
            component: "nested_t9_genvar_control_ChildIf",
            instance: "child",
        })
        .expect("the forwarded child instance must be removed");
}

fn expect_package_function(structure: &SvStructure) {
    for (owner, width) in [
        ("package_function_remediation___ParentIf__8", "8-1:0"),
        ("package_function_remediation___ParentIf__16", "16-1:0"),
    ] {
        structure
            .expect_declaration(DeclarationExpectation {
                owner,
                name: "child__payload",
                type_text: &format!("logic[{width}]"),
            })
            .expect("package-function fixture must preserve each instantiated terminal width");
    }
    for parent in ["parent8", "parent16"] {
        structure
            .expect_identifier_chain_absent(
                "package_function_remediation_Top",
                &[parent, "child", "payload"],
            )
            .expect("package-function calls must not retain stale nested paths");
    }
    structure
        .expect_qualified_function_calls_declared()
        .expect("every package-qualified function call must have a declaration in that package");
}

fn expect_package_function_combined(structure: &SvStructure) {
    structure
        .expect_qualified_function_calls_declared()
        .expect("combined PW/FW calls must resolve within their exact package specializations");
}

fn expect_generic_context_matrix(structure: &SvStructure) {
    for (owner, width) in [
        ("nested_t9_generic_context___ParentIf__8", "8-1:0"),
        ("nested_t9_generic_context___ParentIf__16", "16-1:0"),
    ] {
        structure
            .expect_declaration(DeclarationExpectation {
                owner,
                name: "child__payload",
                type_text: &format!("logic [{width}]"),
            })
            .expect("flattened payload must use its instantiated width");
        structure
            .expect_modport_member(ModportMemberExpectation {
                owner,
                modport: "bus",
                member: "child__payload",
                direction: PortDirection::Inout,
            })
            .expect("flattened payload must retain direction");
        structure
            .expect_instance_absent(InstanceExpectation {
                owner,
                component: if width == "8-1:0" {
                    "nested_t9_generic_context___ChildIf__8"
                } else {
                    "nested_t9_generic_context___ChildIf__16"
                },
                instance: "child",
            })
            .expect("forwarded child instance must be removed");
    }

    structure
        .expect_identifier_chain_absent(
            "nested_t9_generic_context_Top",
            &["parent8", "child", "payload"],
        )
        .expect("ordinary parent access must not retain a stale chain");
    structure
        .expect_instance(InstanceExpectation {
            owner: "nested_t9_generic_context_ExpandedSink__p____ParentIf__16",
            component: "nested_t9_generic_context___ParentIf__16",
            instance: "p",
        })
        .expect("16-bit expanded module must reconstruct the 16-bit parent specialization");
}

fn expect_defaults_imports(structure: &SvStructure) {
    let owner = "nested_t9_defaults_imports_ParentIf";
    for (modport, request, response) in [
        ("forwarded", PortDirection::Output, PortDirection::Input),
        ("combined", PortDirection::Inout, PortDirection::Input),
        ("same_one", PortDirection::Output, PortDirection::Input),
        ("same_many", PortDirection::Inout, PortDirection::Input),
        ("converse_one", PortDirection::Input, PortDirection::Output),
        ("converse_many", PortDirection::Inout, PortDirection::Output),
    ] {
        for (member, direction) in [("child__request", request), ("child__response", response)] {
            structure
                .expect_modport_member(ModportMemberExpectation {
                    owner,
                    modport,
                    member,
                    direction,
                })
                .expect("same/converse expansion must retain the selected direction");
        }
    }
    structure
        .expect_instance_absent(InstanceExpectation {
            owner,
            component: "nested_t9_defaults_imports_ChildIf",
            instance: "child",
        })
        .expect("forwarded child instance must be removed");
}

fn expect_raw_identifier(structure: &SvStructure) {
    let owner = "nested_t9_raw_identifier_ParentIf";
    structure
        .expect_declaration(DeclarationExpectation {
            owner,
            name: "same__payload",
            type_text: "logic",
        })
        .expect("raw identifier must lower through normal escaped emission");
    structure
        .expect_identifier_chain("nested_t9_raw_identifier_Consumer", &["p", "same__payload"])
        .expect("raw identifier access must use the flattened chain");
    structure
        .expect_identifier_chain_absent(
            "nested_t9_raw_identifier_Consumer",
            &["p", "same", "payload"],
        )
        .expect("raw identifier access must not retain a stale chain");
}

fn expect_same_access(structure: &SvStructure) {
    structure
        .expect_modport_member(ModportMemberExpectation {
            owner: "nested_t9_regression_same_access_ParentIf",
            modport: "same_one",
            member: "child__request",
            direction: PortDirection::Output,
        })
        .expect("same must expose the fully expanded nested member");
    structure
        .expect_identifier_chain(
            "nested_t9_regression_same_access_Consumer",
            &["p", "child__request"],
        )
        .expect("same-inherited assignment must use the flattened path");
    structure
        .expect_identifier_chain_absent(
            "nested_t9_regression_same_access_Consumer",
            &["p", "child", "request"],
        )
        .expect("same-inherited assignment must not retain a stale chain");
}
