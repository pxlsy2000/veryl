use super::*;

fn declaration_source(type_text: &str) -> String {
    format!("interface ParentIf; {type_text} payload; endinterface")
}

#[test]
fn declaration_type_comparison_ignores_only_systemverilog_lexical_trivia() {
    for spelling in [
        "logic[8-1:0]",
        "logic [8-1:0]",
        "logic /* packed width */ [8 - 1 : 0]",
        "logic // packed width\n [8-1:0]",
    ] {
        let source = declaration_source(spelling);
        let structure = parse(&source).expect("each trivia spelling must parse");
        structure
            .expect_declaration(DeclarationExpectation {
                owner: "ParentIf",
                name: "payload",
                type_text: "logic [8-1:0]",
            })
            .unwrap_or_else(|error| panic!("{spelling} must be token-equivalent: {error}"));
    }
}

#[test]
fn declaration_type_tokenizer_preserves_maximal_operator_identity() {
    assert_ne!(
        super::super::type_tokens::from_text("logic[8&&1:0]"),
        super::super::type_tokens::from_text("logic[8& &1:0]")
    );
}

#[test]
fn declaration_type_comparison_keeps_width_and_operator_tokens_strict() {
    for mutation in ["logic[16-1:0]", "logic[8*1-1:0]", "bit[8-1:0]"] {
        let source = declaration_source(mutation);
        let structure = parse(&source).expect("type mutation remains valid syntax");
        let error = structure
            .expect_declaration(DeclarationExpectation {
                owner: "ParentIf",
                name: "payload",
                type_text: "logic [8-1:0]",
            })
            .expect_err("width/operator mutation must violate selected structure");
        assert!(matches!(error, SvStructureError::Invariant { .. }));
    }
}
