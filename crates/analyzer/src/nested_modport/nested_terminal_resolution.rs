use super::identifier::EmittedIdentifierIdentity;
use crate::HashMap;
use crate::ir::{Function, ModportMemberPath, VarId, VarKind, Variable};
use crate::symbol::Direction;
use crate::symbol_table;
use std::sync::Arc;
use veryl_parser::resource_table::TokenId;
use veryl_parser::token_range::TokenRange;

use super::default_expansion::expand_effective_modports;
use super::direct_terminal_resolution::{collect_emitted_owner_paths, resolve_direct_terminal};
use super::errors::NestedLoweringResolveError;
use super::instantiated_terminal_index::InstantiatedTerminalIndex;
use super::lowering_records::*;
use super::semantic_records::ResolvedNestedTerminalId;
use super::semantic_type::ResolvedTerminalType;

pub fn resolve_pending_nested_modport_lowering(
    pending: &PendingComponentLowering,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    expansion_limit: usize,
) -> Result<Option<Arc<NestedModportLowering>>, NestedLoweringResolveError> {
    resolve_pending_nested_modport_lowering_for_owner(
        pending,
        variables,
        functions,
        None,
        expansion_limit,
    )
}

pub fn resolve_pending_nested_modport_lowering_for_owner(
    pending: &PendingComponentLowering,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    owner: Option<crate::symbol::SymbolId>,
    expansion_limit: usize,
) -> Result<Option<Arc<NestedModportLowering>>, NestedLoweringResolveError> {
    let terminal_index = InstantiatedTerminalIndex::from_records(variables, functions);
    resolve_pending_nested_modport_lowering_for_owner_with_index(
        pending,
        variables,
        functions,
        &terminal_index,
        owner,
        expansion_limit,
    )
}

pub(crate) fn resolve_pending_nested_modport_lowering_for_owner_with_index(
    pending: &PendingComponentLowering,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    terminal_index: &InstantiatedTerminalIndex,
    owner: Option<crate::symbol::SymbolId>,
    expansion_limit: usize,
) -> Result<Option<Arc<NestedModportLowering>>, NestedLoweringResolveError> {
    let modports = expand_effective_modports(pending, variables, functions, expansion_limit)?;
    if !modports
        .values()
        .flatten()
        .any(|entry| entry.path.as_slice().len() > 1)
    {
        return Ok(None);
    }

    let mut terminals = Vec::<ResolvedNestedTerminal>::new();
    let mut terminal_by_path = HashMap::<ModportMemberPath, ResolvedNestedTerminalId>::default();
    let mut path_by_flat = collect_emitted_owner_paths(variables, functions, owner);
    let mut resolved_modports = HashMap::default();
    let names: Vec<_> = pending
        .declarations
        .iter()
        .map(|declaration| declaration.name)
        .collect();
    let mut entries_by_origin = HashMap::<TokenId, Vec<ResolvedModportEntryRef>>::default();

    for name in names {
        let entries = &modports[&name];
        let mut resolved_entries = Vec::with_capacity(entries.len());
        for entry in entries {
            let path = &entry.path;
            let direction = entry.direction;
            let terminal = if path.as_slice().len() > 1 {
                Some(resolve_flattened_terminal(
                    entry,
                    terminal_index,
                    variables,
                    &mut terminals,
                    &mut terminal_by_path,
                    &mut path_by_flat,
                )?)
            } else {
                resolve_direct_terminal(
                    entry,
                    terminal_index,
                    variables,
                    functions,
                    &mut path_by_flat,
                )?
            };
            let terminal_site = match terminal.as_ref() {
                Some(ResolvedModportTerminal::FlattenedVariable { terminal }) => terminals
                    .get(terminal.0 as usize)
                    .map(|terminal| terminal.token),
                Some(ResolvedModportTerminal::DirectVariable { variable, .. }) => {
                    variables.get(variable).map(|variable| variable.token)
                }
                Some(ResolvedModportTerminal::DirectFunction { function }) => {
                    functions.get(function).map(|function| function.token)
                }
                None => None,
            };
            resolved_entries.push(ResolvedModportEntry {
                path: path.clone(),
                direction,
                terminal,
                terminal_site,
            });
        }
        resolved_modports.insert(
            name,
            ResolvedModport {
                entries: resolved_entries.into(),
            },
        );
        for (entry_index, entry) in entries.iter().enumerate() {
            entries_by_origin
                .entry(entry.origin.beg.id)
                .or_default()
                .push(ResolvedModportEntryRef {
                    modport: name,
                    entry_index: entry_index as u32,
                });
        }
    }

    Ok(Some(Arc::new(NestedModportLowering::new(
        resolved_modports,
        terminals.into(),
        entries_by_origin
            .into_iter()
            .map(|(origin, entries)| (origin, entries.into()))
            .collect(),
    ))))
}

fn resolve_flattened_terminal(
    entry: &PendingModportEntry,
    terminal_index: &InstantiatedTerminalIndex,
    variables: &HashMap<VarId, Variable>,
    terminals: &mut Vec<ResolvedNestedTerminal>,
    terminal_by_path: &mut HashMap<ModportMemberPath, ResolvedNestedTerminalId>,
    path_by_flat: &mut HashMap<EmittedIdentifierIdentity, (ModportMemberPath, TokenRange)>,
) -> Result<ResolvedModportTerminal, NestedLoweringResolveError> {
    let path = &entry.path;
    if !matches!(
        entry.direction,
        Direction::Input | Direction::Output | Direction::Inout
    ) {
        return Err(NestedLoweringResolveError::UnsupportedMemberDirection {
            path: path.clone(),
            direction: entry.direction,
            origin: entry.origin,
        });
    }
    if let Some(terminal) = terminal_by_path.get(path) {
        return Ok(ResolvedModportTerminal::FlattenedVariable {
            terminal: *terminal,
        });
    }

    let variable = terminal_index.variable(path, variables).ok_or_else(|| {
        NestedLoweringResolveError::MissingTerminal {
            path: path.clone(),
            origin: entry.origin,
        }
    })?;
    if !matches!(
        variable.kind,
        VarKind::Input | VarKind::Output | VarKind::Inout | VarKind::Variable
    ) {
        return Err(NestedLoweringResolveError::NonVariableTerminal {
            path: path.clone(),
            actual_kind: variable.kind.description(),
            origin: entry.origin,
            terminal: variable.declaration_token,
        });
    }
    let resolved_type = ResolvedTerminalType::try_from_ir(&variable.r#type).map_err(|error| {
        NestedLoweringResolveError::UnemittableTerminal {
            path: path.clone(),
            actual_type: error.actual_type,
            origin: entry.origin,
            terminal: variable.declaration_token,
        }
    })?;
    let identifier = flatten_identifier_segments(path.as_slice());
    let symbol = variable.symbol.and_then(symbol_table::get).ok_or_else(|| {
        NestedLoweringResolveError::MissingTerminal {
            path: path.clone(),
            origin: entry.origin,
        }
    })?;
    let emitted_identifier = EmittedIdentifierIdentity::for_symbol(identifier.logical, &symbol);
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
    let id = ResolvedNestedTerminalId(terminals.len() as u32);
    terminals.push(ResolvedNestedTerminal {
        id,
        identifier,
        emitted_identifier,
        variable: variable.id,
        symbol: symbol.id,
        token: variable.declaration_token,
        resolved_type,
    });
    terminal_by_path.insert(path.clone(), id);
    Ok(ResolvedModportTerminal::FlattenedVariable { terminal: id })
}
