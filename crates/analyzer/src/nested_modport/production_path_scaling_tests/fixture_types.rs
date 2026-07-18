use super::collection_work::{collection_work, reset_collection_work};
use super::prepared_emission::{prepare_emission_work, reset_prepare_emission_work};
use super::semantic_work::{reset_semantic_work, semantic_work, without_semantic_work};
use super::*;
use crate::ir::{ModportMemberPath, Signature, Type, TypeKind, VarId};
use crate::symbol::{Direction, GenericMap, Symbol, SymbolId, SymbolKind};
use crate::{Analyzer, HashMap, HashSet, attribute_table, symbol_table};
use std::path::PathBuf;
use std::sync::Arc;
use veryl_metadata::Metadata;
use veryl_parser::Parser;
use veryl_parser::resource_table::{self, PathId, TokenId};

struct Fixture {
    pending: PendingNestedModportAnalysis,
    session: AnalysisSessionId,
    functions: Vec<Symbol>,
    generic_interfaces: Vec<Symbol>,
    parent_declarations: HashMap<SymbolId, TokenId>,
    sources: Vec<PathId>,
}

fn lowering(segments: &[veryl_parser::resource_table::StrId]) -> Arc<NestedModportLowering> {
    let identifier = flatten_identifier_segments(segments);
    let terminal = ResolvedNestedTerminal {
        id: ResolvedNestedTerminalId(0),
        emitted_identifier: EmittedIdentifierIdentity::from_logical(identifier.logical),
        identifier,
        variable: VarId::default(),
        symbol: SymbolId(0),
        token: Default::default(),
        resolved_type: ResolvedTerminalType::try_from_ir(&Type::new(TypeKind::Logic))
            .expect("logic terminal"),
    };
    let member = ResolvedModportEntry {
        path: ModportMemberPath::from_slice(segments),
        direction: Direction::Input,
        terminal: Some(ResolvedModportTerminal::FlattenedVariable {
            terminal: terminal.id,
        }),
        terminal_site: None,
    };
    let modport = *segments.last().expect("material terminal path");
    Arc::new(NestedModportLowering::new(
        HashMap::from_iter([(
            modport,
            ResolvedModport {
                entries: Arc::from([member]),
            },
        )]),
        Arc::from([terminal]),
        Default::default(),
    ))
}

fn source(package: &str, prefix: &str, count: usize) -> String {
    let interface = format!("{prefix}GenericIf");
    let user = format!("{prefix}GenericUse");
    let mut code = format!(
        "interface {interface}::<W: u32> {{ var _payload: logic<W>; }}\n\
         module {user} {{\n"
    );
    for index in 0..count {
        code.push_str(&format!("    inst _g{index}: {interface}::<{}>;\n", index + 1));
    }
    code.push_str(&format!("}}\npackage {package} {{\n"));
    for index in 0..count {
        code.push_str(&format!(
            "    function {prefix}{index}() -> logic {{ return 0; }}\n"
        ));
    }
    code.push_str("}\n");
    code
}
