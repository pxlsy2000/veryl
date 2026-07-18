use super::*;
use miette::Diagnostic;
use std::path::Path;
use veryl_parser::{
    resource_table,
    text_table::{self, TextInfo},
    token_range::TokenRange,
    veryl_token::{Token, TokenSource},
};

const SOURCE_TEXT: &str =
    "interface Parent { inst child: Child; modport mp { child.sink: modport, } }";

fn file_source(path: &str, text: &str) -> TokenSource {
    let path = resource_table::insert_path(Path::new(path));
    let text = text_table::set_current_text(TextInfo {
        text: text.to_string(),
        path,
    });
    TokenSource::File { path, text }
}

fn token_range(source: TokenSource, text: &str, offset: u32, length: u32) -> TokenRange {
    Token::new(text, 1, offset + 1, length, offset, source).into()
}

fn labels(error: &AnalyzerError) -> Vec<(Option<String>, usize, usize)> {
    error
        .labels()
        .expect("nested diagnostics always carry labels")
        .map(|label| {
            (
                label.label().map(ToOwned::to_owned),
                label.offset(),
                label.len(),
            )
        })
        .collect()
}

#[test]
fn ordinary_invalid_modport_item_matches_runtime_start_head_contract() {
    let source = file_source("ordinary_modport_diagnostic.veryl", "a");
    let token = token_range(source, "a", 0, 1);
    let error = AnalyzerError::invalid_modport_item(InvalidModportItemKind::Function, "a", &token);

    assert_eq!(
        error.to_string(),
        "\"a\" is not a valid modport item: function"
    );
    assert_eq!(
        [
            InvalidModportItemKind::ArrayedInterface.to_string(),
            InvalidModportItemKind::Function.to_string(),
            InvalidModportItemKind::Modport.to_string(),
            InvalidModportItemKind::Variable.to_string(),
        ],
        [
            "unsupported nested modport because interface arrays are unsupported",
            "function",
            "unsupported nested modport item because the terminal is not a plain variable",
            "variable",
        ]
    );
}

#[test]
fn invalid_nested_modport_constructor_preserves_every_payload_and_span() {
    let source = file_source("nested_diagnostic.veryl", SOURCE_TEXT);
    let item_path = token_range(source, "child.sink", 52, 10);
    let offending = token_range(source, "sink", 58, 4);
    let first_conflict = token_range(source, "other.sink", 12, 10);

    let cases = [
        (
            InvalidNestedModportKind::MissingInterfaceSegment {
                segment: "child".into(),
            },
            "cannot lower nested modport \"child.sink\": interface segment \"child\" was not found",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::NonInterfaceSegment {
                segment: "child".into(),
                actual_kind: "module instance".into(),
            },
            "cannot lower nested modport \"child.sink\": segment \"child\" is module instance, not a scalar interface instance",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::ArrayedInterfaceSegment {
                segment: "child".into(),
            },
            "cannot lower nested modport \"child.sink\": interface segment \"child\" is arrayed",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::MissingModport {
                name: "sink".into(),
            },
            "cannot lower nested modport \"child.sink\": child modport \"sink\" was not found",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::UnsupportedMemberDirection {
                direction: "ref".into(),
            },
            "cannot lower nested modport \"child.sink\": member direction \"ref\" cannot be flattened",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::NonVariableTerminal {
                name: "sink".into(),
                actual_kind: "function".into(),
            },
            "cannot lower nested modport \"child.sink\": terminal \"sink\" is function, not a plain signal variable",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::UnemittableTerminalType {
                name: "sink".into(),
                actual_type: "interface".into(),
            },
            "cannot lower nested modport \"child.sink\": terminal \"sink\" has unsupported emitted type \"interface\"",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::EmptyModport {
                name: "sink".into(),
            },
            "cannot lower nested modport \"child.sink\": child modport \"sink\" has no flattenable members",
            52,
            10,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::FlatNameCollision {
                flat: "child_sink".into(),
                first_path: "child.sink".into(),
            },
            "cannot lower nested modport \"child.sink\": flattened name \"child_sink\" also represents \"child.sink\"",
            52,
            10,
            "nested modport lowering failure",
            true,
        ),
        (
            InvalidNestedModportKind::DefaultCycle {
                target: "sink".into(),
            },
            "cannot lower nested modport \"child.sink\": default modport cycle reaches \"sink\"",
            58,
            4,
            "nested modport lowering failure",
            false,
        ),
        (
            InvalidNestedModportKind::UnloweredLocalReference {
                path: "child.hidden".into(),
            },
            "cannot lower nested modport \"child.sink\": local reference \"child.hidden\" is not forwarded by the nested modport",
            52,
            10,
            "nested modport lowering failure",
            false,
        ),
    ];

    for (kind, message, offset, length, primary_label, has_conflict) in cases {
        let site = NestedModportDiagnosticSite {
            item_path,
            offending,
            first_conflict: has_conflict.then_some(first_conflict),
        };
        let error = AnalyzerError::invalid_nested_modport("child.sink", kind.clone(), &site);
        let expected_labels = if has_conflict {
            vec![
                (Some(primary_label.into()), offset, length),
                (Some("first conflicting item".into()), 12, 10),
            ]
        } else {
            vec![(Some(primary_label.into()), offset, length)]
        };

        assert_eq!(
            error,
            AnalyzerError::InvalidNestedModport {
                path: "child.sink".into(),
                kind,
                input: MultiSources {
                    sources: vec![Source {
                        path: "nested_diagnostic.veryl".into(),
                        text: SOURCE_TEXT.into(),
                    }],
                },
                error_location: (offset, length).into(),
                first_conflict_location: if has_conflict {
                    vec![(12, 10).into()]
                } else {
                    Vec::new()
                },
                token_source: source,
            }
        );
        assert_eq!(
            error.code().expect("diagnostic code").to_string(),
            "invalid_nested_modport"
        );
        assert_eq!(error.to_string(), message);
        assert_eq!(
            error.input_sources().sources[0].path,
            "nested_diagnostic.veryl"
        );
        assert_eq!(labels(&error), expected_labels);
    }
}
