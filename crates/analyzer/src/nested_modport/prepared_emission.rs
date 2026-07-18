use super::analysis_snapshot::NestedModportAnalysis;
use super::emission_frame::{EmissionFrame, EmissionOwnerBatch};
use super::emission_index::{
    EmissionOwnerGroupKey, EmissionScopeKey, PackageScopeEntry, PackageScopeLookupKey,
    SourceEmissionTable,
};
use super::emission_scope::EmissionScopeHandle;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::SemanticGenericMap;
use super::semantic_work::record_semantic_work;
use super::specialization_context::{EmissionOwnerKind, EmissionPhase};
use crate::HashMap;
use crate::symbol::{GenericMap, SymbolId};
use veryl_parser::resource_table::TokenId;

#[cfg(test)]
thread_local! {
    static PREPARE_EMISSION_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_prepare_emission_work() {
    PREPARE_EMISSION_WORK.set(0);
}

#[cfg(test)]
pub(super) fn prepare_emission_work() -> usize {
    PREPARE_EMISSION_WORK.get()
}

#[cfg(test)]
pub(super) fn record_prepare_emission_work(units: usize) {
    PREPARE_EMISSION_WORK.with(|count| count.set(count.get().saturating_add(units)));
}

#[cfg(not(test))]
pub(super) fn record_prepare_emission_work(_units: usize) {}

pub struct PreparedEmission<'a> {
    pub(super) analysis: &'a NestedModportAnalysis,
    pub(super) phase: EmissionPhase,
    pub(super) table: Option<&'a SourceEmissionTable>,
    pub(super) cursor: usize,
    pub(super) scope_cursors: HashMap<EmissionScopeKey, usize>,
    pub(super) consumed: Vec<bool>,
}

impl<'a> PreparedEmission<'a> {
    fn is_empty_owner(&self, declaration: TokenId, kind: EmissionOwnerKind) -> bool {
        record_prepare_emission_work(1);
        self.table
            .is_some_and(|table| table.empty_owners.contains(&(declaration, kind)))
    }

    pub fn phase(&self) -> EmissionPhase {
        self.phase
    }

    pub fn package_scope(
        &self,
        declaration: TokenId,
        symbol: SymbolId,
        generic_map: &GenericMap,
    ) -> Result<Option<EmissionScopeHandle>, NestedModportAnalysisInvariant> {
        let Some(table) = self.table else {
            return Ok(None);
        };
        record_prepare_emission_work(1);
        let key = PackageScopeLookupKey {
            declaration,
            symbol,
        };
        let Some(index) = table.package_scope_index.get(&key) else {
            return Ok(None);
        };
        record_prepare_emission_work(3);
        let semantic = SemanticGenericMap::from_matching_keys(generic_map, &index.expected);
        match index.entries.get(&semantic) {
            Some(PackageScopeEntry::Unique(scope))
                if scope.matches_package_semantic(table.source, declaration, symbol, &semantic) =>
            {
                Ok(Some(EmissionScopeHandle(scope.clone())))
            }
            Some(PackageScopeEntry::Unique(_)) | Some(PackageScopeEntry::Ambiguous) | None => {
                Err(NestedModportAnalysisInvariant::PackageScopeMismatch)
            }
        }
    }

    pub fn take_owners(
        &mut self,
        declaration: TokenId,
        kind: EmissionOwnerKind,
    ) -> Result<EmissionOwnerBatch<'a>, NestedModportAnalysisInvariant> {
        if kind == EmissionOwnerKind::Function {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        }
        if self.is_empty_owner(declaration, kind) {
            return Ok(EmissionOwnerBatch {
                analysis: self.analysis,
                bindings: Vec::new(),
            });
        }
        let Some(table) = self.table else {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        };
        loop {
            record_prepare_emission_work(1);
            match table.groups.get(self.cursor) {
                Some(group) if group.key.kind == EmissionOwnerKind::Function => self.cursor += 1,
                Some(_) | None => break,
            }
        }
        let key = EmissionOwnerGroupKey {
            declaration,
            kind,
            scope: EmissionScopeKey::Unscoped,
        };
        record_prepare_emission_work(1);
        let Some(group_index) = table.group_index.get(&key).copied() else {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        };
        record_prepare_emission_work(1);
        if group_index != self.cursor {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        }
        let group = &table.groups[group_index];
        for index in group.binding_range.clone() {
            record_prepare_emission_work(1);
            self.consumed[index] = true;
        }
        self.cursor += 1;
        Ok(EmissionOwnerBatch {
            analysis: self.analysis,
            bindings: table.bindings[group.binding_range.clone()].iter().collect(),
        })
    }

    pub fn take_function_owners(
        &mut self,
        declaration: TokenId,
        enclosing: Option<EmissionFrame<'a>>,
        package: Option<&EmissionScopeHandle>,
    ) -> Result<EmissionOwnerBatch<'a>, NestedModportAnalysisInvariant> {
        super::semantic_work::record_emission_query_work(1);
        if enclosing.is_some() == package.is_some() {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        }
        let Some(table) = self.table else {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        };
        #[cfg(any(test, feature = "nested-modport-test-utils"))]
        if super::function_owner_query_mutation::material_function_owner_mutation_enabled()
            && let Some(enclosing) = enclosing
        {
            use std::hash::{Hash, Hasher};
            let before = super::semantic_work::semantic_work();
            let material = enclosing.binding.specialization.specialization.clone();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            material.hash(&mut hasher);
            std::hint::black_box(hasher.finish());
            super::semantic_work::record_emission_query_work(
                super::semantic_work::semantic_work().saturating_sub(before),
            );
        }
        let scope_keys = match (enclosing, package) {
            (Some(enclosing), None) => vec![
                EmissionScopeKey::Enclosing(enclosing.binding.owner_handle),
                EmissionScopeKey::Package(enclosing.scope_handle().0),
            ],
            (None, Some(package)) => vec![EmissionScopeKey::Package(package.0.clone())],
            (Some(_), Some(_)) | (None, None) => {
                return Err(NestedModportAnalysisInvariant::DeclarationOrder);
            }
        };
        let mut candidates = Vec::new();
        for scope in scope_keys {
            record_prepare_emission_work(1);
            let Some(ordered) = table.ordered_groups_by_scope.get(&scope) else {
                continue;
            };
            record_prepare_emission_work(1);
            let cursor = self.scope_cursors.get(&scope).copied().unwrap_or(0);
            if let Some(group) = ordered.get(cursor).copied() {
                record_prepare_emission_work(1);
                candidates.push((scope, cursor, group));
            }
        }
        if candidates.is_empty() {
            record_prepare_emission_work(1);
            if !table
                .nonempty_owners
                .contains(&(declaration, EmissionOwnerKind::Function))
                && self.is_empty_owner(declaration, EmissionOwnerKind::Function)
            {
                return Ok(EmissionOwnerBatch {
                    analysis: self.analysis,
                    bindings: Vec::new(),
                });
            }
        }
        let first = candidates.iter().min_by_key(|(_, _, group)| {
            record_prepare_emission_work(1);
            table.groups[*group].binding_range.start
        });
        record_prepare_emission_work(usize::from(first.is_some()));
        if first.is_some_and(|(_, _, group)| table.groups[*group].key.declaration != declaration) {
            return Err(NestedModportAnalysisInvariant::DeclarationOrder);
        }
        let mut matching_groups: Vec<_> = candidates
            .into_iter()
            .filter(|(_, _, group)| {
                record_prepare_emission_work(1);
                table.groups[*group].key.declaration == declaration
            })
            .collect();
        matching_groups.sort_by_key(|(_, _, group)| table.groups[*group].binding_range.start);
        if matching_groups.is_empty() {
            return Err(if package.is_some() {
                NestedModportAnalysisInvariant::PackageScopeMismatch
            } else {
                NestedModportAnalysisInvariant::DeclarationOrder
            });
        }
        let mut bindings = Vec::new();
        for (scope, cursor, group_index) in matching_groups {
            record_prepare_emission_work(2);
            self.scope_cursors.insert(scope, cursor + 1);
            let group = &table.groups[group_index];
            for index in group.binding_range.clone() {
                record_prepare_emission_work(1);
                self.consumed[index] = true;
                bindings.push(&table.bindings[index]);
            }
        }
        Ok(EmissionOwnerBatch {
            analysis: self.analysis,
            bindings,
        })
    }

    pub fn finish(self) -> Result<(), NestedModportAnalysisInvariant> {
        if self.consumed.iter().any(|consumed| {
            record_prepare_emission_work(1);
            record_semantic_work(1);
            !consumed
        }) {
            return Err(NestedModportAnalysisInvariant::UnconsumedBindings);
        }
        Ok(())
    }
}
