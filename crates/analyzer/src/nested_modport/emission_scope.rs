use super::identity::*;
use super::semantic_work::{hash_slice, record_semantic_work, slice_eq};
use crate::ir::Signature;
use crate::symbol::{GenericMap, SymbolId, SymbolKind};
use crate::{HashSet, symbol_table};
use std::hash::{Hash, Hasher};
use veryl_parser::resource_table::{PathId, StrId, TokenId};
#[derive(Clone, Debug)]
pub struct EmissionScopeIdentity {
    pub(super) session: AnalysisSessionId,
    pub(super) source: PathId,
    pub(super) declaration: TokenId,
    pub(super) symbol: SymbolId,
    pub(super) semantic_path: Vec<StrId>,
    pub(super) generic_map: SemanticGenericMap,
}

impl PartialEq for EmissionScopeIdentity {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.session == other.session
            && self.source == other.source
            && self.declaration == other.declaration
            && self.symbol == other.symbol
            && slice_eq(&self.semantic_path, &other.semantic_path)
            && self.generic_map == other.generic_map
    }
}

impl Eq for EmissionScopeIdentity {}

impl Hash for EmissionScopeIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(4);
        self.session.hash(state);
        self.source.hash(state);
        self.declaration.hash(state);
        self.symbol.hash(state);
        hash_slice(&self.semantic_path, state);
        self.generic_map.hash(state);
    }
}

#[derive(Clone, Debug)]
pub struct EmissionScopeHandle(pub(super) EmissionScopeIdentity);

impl EmissionScopeIdentity {
    pub(crate) fn for_function(
        session: AnalysisSessionId,
        signature: &Signature,
        generic_map: &GenericMap,
    ) -> Option<Self> {
        let function = symbol_table::get(signature.symbol)?;
        let package = symbol_table::get_namespace_symbol(&function.namespace)?;
        if !matches!(package.kind, SymbolKind::Package(_)) {
            return None;
        }
        let mut scope_map = GenericMap::default();
        scope_map
            .map
            .extend(signature.generic_parameters.iter().cloned());
        scope_map.map.extend(
            generic_map
                .map
                .iter()
                .map(|(name, value)| (*name, value.clone())),
        );
        Some(Self::for_package(session, &package, &scope_map))
    }

    fn for_package(
        session: AnalysisSessionId,
        package: &crate::symbol::Symbol,
        generic_map: &GenericMap,
    ) -> Self {
        let parameter_names: HashSet<_> = package
            .generic_parameters()
            .into_iter()
            .map(|(name, _)| veryl_parser::resource_table::canonical_str_id(name))
            .collect();
        let mut normalized = GenericMap::default();
        normalized.map.extend(
            generic_map
                .map
                .iter()
                .filter(|(name, _)| {
                    parameter_names
                        .contains(&veryl_parser::resource_table::canonical_str_id(**name))
                })
                .map(|(name, value)| {
                    (
                        veryl_parser::resource_table::canonical_str_id(*name),
                        value.clone(),
                    )
                }),
        );
        let mut semantic_path = package.namespace.paths.to_vec();
        semantic_path.push(package.token.text);
        Self {
            session,
            source: package.token.source.get_path().unwrap_or_default(),
            declaration: package.token.id,
            symbol: package.id,
            semantic_path,
            generic_map: SemanticGenericMap::from(&normalized),
        }
    }

    pub(super) fn matches_package_semantic(
        &self,
        source: PathId,
        declaration: TokenId,
        symbol: SymbolId,
        generic_map: &SemanticGenericMap,
    ) -> bool {
        self.source == source
            && self.declaration == declaration
            && self.symbol == symbol
            && self.generic_map == *generic_map
    }

    pub(super) fn semantic_generic_map(&self) -> &SemanticGenericMap {
        &self.generic_map
    }

    #[cfg(test)]
    pub(crate) fn test_scope(
        session: AnalysisSessionId,
        source: PathId,
        declaration: TokenId,
        symbol: SymbolId,
        generic_map: &GenericMap,
    ) -> Self {
        Self {
            session,
            source,
            declaration,
            symbol,
            semantic_path: Vec::new(),
            generic_map: SemanticGenericMap::from(generic_map),
        }
    }
}
