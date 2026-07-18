use super::*;

#[test]
fn component_declaration_excludes_same_named_function_task_and_block_locals() {
    // Given
    let source = r#"
interface ParentIf;
    logic child__fatal;
    function automatic logic observe();
        logic child__fatal;
        return child__fatal;
    endfunction
    task automatic inspect();
        logic child__fatal;
    endtask
    initial begin : local_block
        logic child__fatal;
        child__fatal = 1'b0;
    end
endinterface
"#;

    // When
    let structure = parse(source).expect("function-local shadow fixture must parse");
    let result = structure.expect_declaration(DeclarationExpectation {
        owner: "ParentIf",
        name: "child__fatal",
        type_text: "logic",
    });

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn outer_declarations_exclude_same_named_nested_interface_declaration() {
    // Given
    let source = r#"
interface Outer;
    logic direct_signal;
    interface Inner;
        logic direct_signal;
    endinterface
    logic trailing_signal;
endinterface
"#;

    // When
    let structure = parse(source).expect("nested interface fixture must parse");
    structure
        .expect_declaration(DeclarationExpectation {
            owner: "Outer",
            name: "trailing_signal",
            type_text: "logic",
        })
        .expect("direct declaration after nested owner must remain visible");
    let result = structure.expect_declaration(DeclarationExpectation {
        owner: "Outer",
        name: "direct_signal",
        type_text: "logic",
    });

    // Then
    assert_eq!(result, Ok(()));
}

#[test]
fn outer_declarations_exclude_nested_module_and_program_declarations() {
    // Given
    let source = r#"
module OuterModule;
    logic module_signal;
    module InnerModule;
        logic module_signal;
    endmodule
    logic trailing_signal;
endmodule
module OuterProgramHost;
    logic program_signal;
    program InnerProgram;
        logic program_signal;
    endprogram
    logic trailing_signal;
endmodule
"#;

    // When
    let structure = parse(source).expect("nested declaration-owner fixture must parse");

    // Then
    for (owner, name) in [
        ("OuterModule", "module_signal"),
        ("OuterModule", "trailing_signal"),
        ("OuterProgramHost", "program_signal"),
        ("OuterProgramHost", "trailing_signal"),
    ] {
        structure
            .expect_declaration(DeclarationExpectation {
                owner,
                name,
                type_text: "logic",
            })
            .expect("outer declaration must remain uniquely owned");
    }
}
