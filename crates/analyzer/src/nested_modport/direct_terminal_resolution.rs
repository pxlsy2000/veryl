use super::identifier::EmittedIdentifierIdentity;
use crate::HashMap;
use crate::ir::{Function, ModportMemberPath, VarId, Variable};
use crate::scope;
use crate::symbol::Direction;
use crate::symbol::SymbolId;
use crate::symbol_table;
use std::sync::Arc;
use veryl_parser::resource_table::StrId;
use veryl_parser::token_range::TokenRange;

use super::collection_work::record_collection_work;
use super::default_expansion::expand_effective_modports;
use super::errors::NestedLoweringResolveError;
use super::instantiated_terminal_index::InstantiatedTerminalIndex;
use super::lowering_records::*;
use super::semantic_type::ResolvedTerminalType;

pub(super) fn collect_emitted_owner_paths(
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    owner: Option<SymbolId>,
) -> HashMap<EmittedIdentifierIdentity, (ModportMemberPath, TokenRange)> {
    let mut paths: HashMap<_, _> = variables
        .values()
        .filter_map(|variable| {
            let [identifier] = variable.path.0.as_slice() else {
                return None;
            };
            let symbol = variable.symbol.and_then(symbol_table::get)?;
            Some((
                EmittedIdentifierIdentity::for_symbol(*identifier, &symbol),
                (
                    ModportMemberPath::from_slice(&[*identifier]),
                    variable.token,
                ),
            ))
        })
        .chain(functions.values().map(|function| {
            (
                EmittedIdentifierIdentity::from_logical(function.name),
                (
                    ModportMemberPath::from_slice(&[function.name]),
                    function.token,
                ),
            )
        }))
        .collect();
    let owner_scope = owner.and_then(symbol_table::get).map(|symbol| {
        let namespace = symbol.inner_namespace();
        (scope::intern_namespace(&namespace), namespace)
    });
    if let Some((owner_scope, namespace)) = owner_scope {
        for id in symbol_table::owner_emission_candidates(owner_scope) {
            record_collection_work(1);
            let Some(symbol) = symbol_table::get(id) else {
                continue;
            };
            if !symbol.introduces_emitted_identifier_in_owner_scope(owner_scope, &namespace) {
                continue;
            }
            let identifier = symbol.token.text;
            paths.insert(
                EmittedIdentifierIdentity::for_symbol(identifier, &symbol),
                (
                    ModportMemberPath::from_slice(&[identifier]),
                    symbol.token.into(),
                ),
            );
        }
    }
    paths
}

pub(super) fn resolve_direct_terminal(
    entry: &PendingModportEntry,
    terminal_index: &InstantiatedTerminalIndex,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    path_by_flat: &mut HashMap<EmittedIdentifierIdentity, (ModportMemberPath, TokenRange)>,
) -> Result<Option<ResolvedModportTerminal>, NestedLoweringResolveError> {
    let path = &entry.path;
    let Some(&identifier) = path.as_slice().first() else {
        return Err(NestedLoweringResolveError::MissingTerminal {
            path: path.clone(),
            origin: entry.origin,
        });
    };
    let direct_path = ModportMemberPath::from_slice(&[identifier]);
    let variable = terminal_index.variable(&direct_path, variables);
    let symbol = variable
        .and_then(|variable| variable.symbol)
        .and_then(symbol_table::get);
    let emitted_identifier = symbol.as_ref().map_or_else(
        || EmittedIdentifierIdentity::from_logical(identifier),
        |symbol| EmittedIdentifierIdentity::for_symbol(identifier, symbol),
    );
    if let Some((first_path, first_origin)) = path_by_flat.get(&emitted_identifier)
        && first_path != path
    {
        return Err(NestedLoweringResolveError::FlatNameCollision {
            flat: emitted_identifier.logical(),
            first_path: first_path.clone(),
            second_path: path.clone(),
            first_origin: *first_origin,
            second_origin: entry.origin,
        });
    }
    path_by_flat.insert(emitted_identifier, (path.clone(), entry.origin));
    if entry.direction == Direction::Import {
        return Ok(terminal_index
            .function(identifier, functions)
            .map(|(id, _)| ResolvedModportTerminal::DirectFunction { function: id }));
    }

    let Some(variable) = variable else {
        return Ok(None);
    };
    let resolved_type = ResolvedTerminalType::try_from_ir(&variable.r#type).map_err(|error| {
        NestedLoweringResolveError::UnemittableTerminal {
            path: path.clone(),
            actual_type: error.actual_type,
            origin: entry.origin,
            terminal: variable.declaration_token,
        }
    })?;
    Ok(Some(ResolvedModportTerminal::DirectVariable {
        variable: variable.id,
        symbol: symbol.as_ref().map(|symbol| symbol.id).ok_or_else(|| {
            NestedLoweringResolveError::MissingTerminal {
                path: path.clone(),
                origin: entry.origin,
            }
        })?,
        emitted_identifier,
        resolved_type,
    }))
}

pub fn resolve_direct_modport_effective(
    pending: &PendingComponentLowering,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    expansion_limit: usize,
) -> Result<HashMap<StrId, Arc<[ResolvedModportEntry]>>, NestedLoweringResolveError> {
    let terminal_index = InstantiatedTerminalIndex::from_records(variables, functions);
    resolve_direct_modport_effective_with_index(
        pending,
        variables,
        functions,
        &terminal_index,
        expansion_limit,
    )
}

pub(crate) fn resolve_direct_modport_effective_with_index(
    pending: &PendingComponentLowering,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    terminal_index: &InstantiatedTerminalIndex,
    expansion_limit: usize,
) -> Result<HashMap<StrId, Arc<[ResolvedModportEntry]>>, NestedLoweringResolveError> {
    Ok(
        expand_effective_modports(pending, variables, functions, expansion_limit)?
            .into_iter()
            .map(|(name, entries)| {
                let entries = entries
                    .into_iter()
                    .map(|entry| ResolvedModportEntry {
                        terminal_site: entry.path.as_slice().first().and_then(|identifier| {
                            terminal_index.terminal_site(*identifier, variables, functions)
                        }),
                        path: entry.path,
                        direction: entry.direction,
                        terminal: None,
                    })
                    .collect::<Vec<_>>()
                    .into();
                (name, entries)
            })
            .collect(),
    )
}
