use super::identifier::EmittedIdentifierIdentity;
use crate::HashMap;
use crate::ir::{ModportMemberPath, VarId};
use crate::symbol::{Direction, SymbolId};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use veryl_parser::resource_table::{self, StrId, TokenId};
use veryl_parser::token_range::TokenRange;

use super::collection_work::counted;
use super::semantic_records::ResolvedNestedTerminalId;
use super::semantic_type::*;
use super::semantic_work::{counted_sort_by, record_semantic_work, slice_eq};
use super::terminal_path_index::{ResolvedNestedTerminalPathIndex, terminal_scan_mutation_enabled};

#[derive(Clone, Debug, Ord, PartialOrd)]
pub struct FlattenedIdentifier {
    pub logical: StrId,
    pub source_segments: Vec<StrId>,
}

pub fn flatten_identifier_segments(segments: &[StrId]) -> FlattenedIdentifier {
    let logical = segments
        .iter()
        .map(ToString::to_string)
        .map(|segment| segment.strip_prefix("r#").unwrap_or(&segment).to_owned())
        .collect::<Vec<_>>()
        .join("__");
    FlattenedIdentifier {
        logical: resource_table::insert_str(&logical),
        source_segments: segments.to_vec(),
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedNestedTerminal {
    pub id: ResolvedNestedTerminalId,
    pub identifier: FlattenedIdentifier,
    pub emitted_identifier: EmittedIdentifierIdentity,
    pub variable: VarId,
    pub symbol: SymbolId,
    pub token: TokenRange,
    pub resolved_type: ResolvedTerminalType,
}

#[derive(Clone, Debug)]
pub enum ResolvedModportTerminal {
    DirectVariable {
        variable: VarId,
        symbol: SymbolId,
        emitted_identifier: EmittedIdentifierIdentity,
        resolved_type: ResolvedTerminalType,
    },
    DirectFunction {
        function: VarId,
    },
    FlattenedVariable {
        terminal: ResolvedNestedTerminalId,
    },
}

#[derive(Clone, Debug)]
pub struct ResolvedModportEntry {
    pub path: ModportMemberPath,
    pub direction: Direction,
    pub terminal: Option<ResolvedModportTerminal>,
    pub terminal_site: Option<TokenRange>,
}

#[derive(Clone, Debug, Default)]
pub struct ResolvedModport {
    pub entries: Arc<[ResolvedModportEntry]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedModportEntryRef {
    pub modport: StrId,
    pub entry_index: u32,
}

#[derive(Clone, Debug)]
pub struct NestedModportLowering {
    pub modports: HashMap<StrId, ResolvedModport>,
    pub terminals: Arc<[ResolvedNestedTerminal]>,
    pub entries_by_origin: HashMap<TokenId, Arc<[ResolvedModportEntryRef]>>,
    terminal_path_index: ResolvedNestedTerminalPathIndex,
    terminals_by_root: HashMap<StrId, Arc<[usize]>>,
}

impl PartialEq for NestedModportLowering {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.modports.len() == other.modports.len()
            && self.modports.iter().all(|(name, left)| {
                record_semantic_work(1);
                other
                    .modports
                    .get(name)
                    .is_some_and(|right| slice_eq(&left.entries, &right.entries))
            })
            && slice_eq(&self.terminals, &other.terminals)
            && self.entries_by_origin.len() == other.entries_by_origin.len()
            && self.entries_by_origin.iter().all(|(origin, left)| {
                record_semantic_work(1);
                other
                    .entries_by_origin
                    .get(origin)
                    .is_some_and(|right| slice_eq(left, right))
            })
    }
}

impl Eq for NestedModportLowering {}

impl Default for NestedModportLowering {
    fn default() -> Self {
        Self::new(HashMap::default(), Arc::from([]), HashMap::default())
    }
}

impl NestedModportLowering {
    pub fn new(
        modports: HashMap<StrId, ResolvedModport>,
        terminals: Arc<[ResolvedNestedTerminal]>,
        entries_by_origin: HashMap<TokenId, Arc<[ResolvedModportEntryRef]>>,
    ) -> Self {
        let terminal_path_index = ResolvedNestedTerminalPathIndex::build(&terminals);
        let mut root_indices: HashMap<StrId, Vec<usize>> = HashMap::default();
        for (index, terminal) in terminals.iter().enumerate() {
            if let Some(root) = terminal.identifier.source_segments.first() {
                root_indices.entry(*root).or_default().push(index);
            }
        }
        Self {
            modports,
            terminals,
            entries_by_origin,
            terminal_path_index,
            terminals_by_root: root_indices
                .into_iter()
                .map(|(root, indices)| (root, indices.into()))
                .collect(),
        }
    }

    pub(super) fn longest_terminal_prefix(
        &self,
        semantic_segments: &[StrId],
    ) -> Option<&ResolvedNestedTerminal> {
        if terminal_scan_mutation_enabled() {
            return counted(&*self.terminals)
                .filter(|terminal| {
                    semantic_segments.starts_with(&terminal.identifier.source_segments)
                })
                .max_by_key(|terminal| terminal.identifier.source_segments.len());
        }
        self.terminal_path_index
            .longest_prefix(semantic_segments)
            .and_then(|index| self.terminals.get(index))
    }

    pub fn removes_instance_root(&self, root: StrId) -> bool {
        if terminal_scan_mutation_enabled() {
            return counted(&*self.terminals).any(|terminal| {
                terminal.identifier.source_segments.first().copied() == Some(root)
            });
        }
        self.terminal_path_index.removes_root(root)
    }

    pub fn terminals_for_root(&self, root: StrId) -> impl Iterator<Item = &ResolvedNestedTerminal> {
        self.terminals_by_root
            .get(&root)
            .into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|index| self.terminals.get(*index))
    }
}

impl Hash for NestedModportLowering {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut modports: Vec<_> = self.modports.iter().collect();
        counted_sort_by(&mut modports, |(left, _), (right, _)| left.cmp(right));
        record_semantic_work(1);
        modports.len().hash(state);
        for modport in modports {
            record_semantic_work(1);
            modport.hash(state);
        }
        record_semantic_work(1);
        self.terminals.len().hash(state);
        for terminal in self.terminals.iter() {
            record_semantic_work(1);
            terminal.hash(state);
        }
        let mut origins: Vec<_> = self.entries_by_origin.iter().collect();
        counted_sort_by(&mut origins, |(left, _), (right, _)| left.cmp(right));
        record_semantic_work(1);
        origins.len().hash(state);
        for origin in origins {
            record_semantic_work(1);
            origin.hash(state);
        }
    }
}

#[derive(Clone, Debug)]
pub struct PendingModportEntry {
    pub path: ModportMemberPath,
    pub direction: Direction,
    pub origin: TokenRange,
}

#[derive(Clone, Debug)]
pub enum PendingModportDefault {
    Input,
    Output,
    Same(Vec<(StrId, TokenRange)>),
    Converse(Vec<(StrId, TokenRange)>),
}

#[derive(Clone, Debug)]
pub struct PendingModportDeclaration {
    pub name: StrId,
    pub explicit: Vec<PendingModportEntry>,
    pub default: Option<PendingModportDefault>,
    pub origin: TokenRange,
    pub contains_nested_item: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PendingComponentLowering {
    pub declarations: Vec<PendingModportDeclaration>,
}
