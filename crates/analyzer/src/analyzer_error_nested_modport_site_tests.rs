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
fn invalid_nested_modport_terminal_in_another_source_falls_back_to_item_path() {
    let parent = file_source("parent.veryl", SOURCE_TEXT);
    let child = file_source("child.veryl", "interface Child { var signal: logic; }");
    let site = NestedModportDiagnosticSite {
        item_path: token_range(parent, "child.sink", 52, 10),
        offending: token_range(child, "signal", 22, 6),
        first_conflict: None,
    };

    for kind in [
        InvalidNestedModportKind::UnsupportedMemberDirection {
            direction: "ref".into(),
        },
        InvalidNestedModportKind::NonVariableTerminal {
            name: "signal".into(),
            actual_kind: "function".into(),
        },
        InvalidNestedModportKind::UnemittableTerminalType {
            name: "signal".into(),
            actual_type: "interface".into(),
        },
    ] {
        let error = AnalyzerError::invalid_nested_modport("child.sink", kind, &site);
        assert_eq!(
            labels(&error),
            vec![(Some("nested modport lowering failure".into()), 52, 10)]
        );
        assert_eq!(error.input_sources().sources[0].path, "parent.veryl");
    }
}

#[test]
fn invalid_nested_modport_collision_omits_foreign_secondary_label() {
    let parent = file_source("parent_collision.veryl", SOURCE_TEXT);
    let child = file_source(
        "child_collision.veryl",
        "interface Child { var signal: logic; }",
    );
    let site = NestedModportDiagnosticSite {
        item_path: token_range(parent, "child.sink", 52, 10),
        offending: token_range(parent, "sink", 58, 4),
        first_conflict: Some(token_range(child, "signal", 22, 6)),
    };
    let error = AnalyzerError::invalid_nested_modport(
        "child.sink",
        InvalidNestedModportKind::FlatNameCollision {
            flat: "child_signal".into(),
            first_path: "child.signal".into(),
        },
        &site,
    );

    assert_eq!(
        labels(&error),
        vec![(Some("nested modport lowering failure".into()), 52, 10)]
    );
    assert_eq!(
        error.input_sources().sources[0].path,
        "parent_collision.veryl"
    );
}

#[test]
fn invalid_nested_modport_survives_cache_and_graphical_rendering() {
    let source = file_source("rendered_nested_diagnostic.veryl", SOURCE_TEXT);
    let site = NestedModportDiagnosticSite {
        item_path: token_range(source, "child.sink", 52, 10),
        offending: token_range(source, "sink", 58, 4),
        first_conflict: None,
    };
    let error = AnalyzerError::invalid_nested_modport(
        "child.sink",
        InvalidNestedModportKind::MissingModport {
            name: "sink".into(),
        },
        &site,
    );

    let cached = CachedDiagnostic::from_error(&error);
    assert_eq!(
        cached,
        CachedDiagnostic {
            severity: Some(CachedSeverity::Error),
            code: Some("invalid_nested_modport".into()),
            url: Some(
                "https://doc.veryl-lang.org/book/07_appendix/02_semantic_error.html#invalid_nested_modport"
                    .into(),
            ),
            help: Some(String::new()),
            message: "cannot lower nested modport \"child.sink\": child modport \"sink\" was not found"
                .into(),
            sources: MultiSources {
                sources: vec![Source {
                    path: "rendered_nested_diagnostic.veryl".into(),
                    text: SOURCE_TEXT.into(),
                }],
            },
            labels: vec![(
                Some("nested modport lowering failure".into()),
                58,
                4,
            )],
            token_path: Some("rendered_nested_diagnostic.veryl".into()),
        }
    );

    let mut rendered = String::new();
    miette::GraphicalReportHandler::new()
        .render_report(&mut rendered, &cached)
        .expect("diagnostic should render");
    assert!(rendered.contains("invalid_nested_modport"));
    assert!(rendered.contains("rendered_nested_diagnostic.veryl"));
    assert!(rendered.contains("child modport \"sink\" was not found"));
    assert!(rendered.contains("nested modport lowering failure"));
}
