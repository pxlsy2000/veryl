use super::connected_generic_map_index::ConnectedGenericMapIndex;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::*;
use super::lowering_records::*;
use crate::HashMap;
use crate::ir::Signature;
use crate::symbol::GenericMap;
use crate::symbol_table;
use std::cell::RefCell;
use std::sync::Arc;
use veryl_parser::resource_table::StrId;

thread_local! {
    static CANONICAL_GENERIC_MAPS: RefCell<HashMap<crate::symbol::SymbolId, CanonicalGenericMaps>> =
        RefCell::new(HashMap::default());
}

#[derive(Clone)]
struct CanonicalGenericMaps {
    instances: Vec<crate::symbol::SymbolId>,
    maps: HashMap<SemanticGenericMap, GenericMap>,
}

pub(crate) fn clear_canonical_generic_map_cache() {
    CANONICAL_GENERIC_MAPS.with(|cache| cache.borrow_mut().clear());
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LoweringAvailability {
    NotNested,
    Found(Arc<NestedModportLowering>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EmissionBindingId(pub(super) u32);

impl EmissionBindingId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EmissionOwnerKind {
    Module,
    Interface,
    Function,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmissionPhase {
    Align,
    Build,
}

#[derive(Clone, Debug)]
pub struct EmissionSpecializationContext {
    pub owner: Signature,
    pub generic_map: GenericMap,
    pub enclosing_generic_map: Option<GenericMap>,
    pub connected_generic_maps: Vec<(StrId, GenericMap)>,
    pub emit_connected_variant: bool,
    frozen: FrozenEmissionSpecializationContext,
}

#[derive(Clone, Debug)]
struct FrozenEmissionSpecializationContext {
    specialization: ComponentSpecializationIdentity,
    generic_map: SemanticGenericMap,
    enclosing_generic_map: Option<SemanticGenericMap>,
    connected_generic_maps: Vec<(StrId, SemanticGenericMap)>,
    connected_generic_map_index: ConnectedGenericMapIndex,
}

impl EmissionSpecializationContext {
    pub(super) fn semantic_generic_map(&self) -> &SemanticGenericMap {
        &self.frozen.generic_map
    }

    pub(crate) fn new(
        specialization: ComponentSpecializationIdentity,
        generic_map: GenericMap,
        enclosing_generic_map: Option<GenericMap>,
        connected_generic_maps: Vec<(StrId, GenericMap)>,
    ) -> Self {
        let connected_generic_map_index = ConnectedGenericMapIndex::build(&connected_generic_maps);
        let frozen = FrozenEmissionSpecializationContext {
            specialization: specialization.clone(),
            generic_map: SemanticGenericMap::from(&generic_map),
            enclosing_generic_map: enclosing_generic_map.as_ref().map(SemanticGenericMap::from),
            connected_generic_maps: connected_generic_maps
                .iter()
                .map(|(port, map)| (*port, SemanticGenericMap::from(map)))
                .collect(),
            connected_generic_map_index,
        };
        Self {
            owner: specialization.owner,
            generic_map,
            enclosing_generic_map,
            connected_generic_maps,
            emit_connected_variant: false,
            frozen,
        }
    }

    pub(super) fn connected_generic_map(&self, formal_port: StrId) -> Option<&GenericMap> {
        self.frozen
            .connected_generic_map_index
            .get(&self.connected_generic_maps, formal_port)
    }

    #[cfg(test)]
    pub(crate) fn test_set_enclosing_generic_map(&mut self, map: GenericMap) {
        self.enclosing_generic_map = Some(map.clone());
        self.frozen.enclosing_generic_map = Some(SemanticGenericMap::from(&map));
    }

    fn canonical_generic_map(
        signature: &Signature,
        kind: EmissionOwnerKind,
    ) -> Result<GenericMap, NestedModportAnalysisInvariant> {
        let function_parameters = (kind == EmissionOwnerKind::Function)
            .then(|| symbol_table::get(signature.symbol))
            .flatten()
            .map(|symbol| {
                symbol
                    .generic_parameters()
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect::<crate::HashSet<_>>()
            });
        let mut expected = GenericMap::default();
        expected.map.extend(
            signature
                .generic_parameters
                .iter()
                .filter(|(name, _)| {
                    function_parameters
                        .as_ref()
                        .is_none_or(|parameters| parameters.contains(name))
                })
                .cloned(),
        );
        if let Some(symbol) = symbol_table::get(signature.symbol) {
            symbol.eval_generic_consts(&mut expected);
        }
        let expected_semantic = SemanticGenericMap::from(&expected);
        let Some(symbol) = symbol_table::get(signature.symbol) else {
            return Ok(expected);
        };
        let instances = symbol.generic_instances.clone();
        if let Some(found) = CANONICAL_GENERIC_MAPS.with(|cache| {
            let cache = cache.borrow();
            let entry = cache.get(&signature.symbol)?;
            (entry.instances == instances)
                .then(|| entry.maps.get(&expected_semantic).cloned())
                .flatten()
        }) {
            return Ok(found);
        }
        let maps: HashMap<_, _> = super::collection_work::counted(symbol.generic_maps())
            .map(|map| (SemanticGenericMap::from(&map), map))
            .collect();
        let found = maps.get(&expected_semantic).cloned().unwrap_or(expected);
        CANONICAL_GENERIC_MAPS.with(|cache| {
            cache
                .borrow_mut()
                .insert(signature.symbol, CanonicalGenericMaps { instances, maps });
        });
        Ok(found)
    }

    pub(crate) fn from_specialization(
        kind: EmissionOwnerKind,
        specialization: &ComponentSpecializationIdentity,
    ) -> Result<Self, NestedModportAnalysisInvariant> {
        let owner = specialization.owner.clone();
        let generic_map = Self::canonical_generic_map(&owner, kind)?;
        let enclosing_generic_map = (kind == EmissionOwnerKind::Function).then(|| {
            let function_names: crate::HashSet<_> = symbol_table::get(owner.symbol)
                .into_iter()
                .flat_map(|symbol| symbol.generic_parameters())
                .map(|(name, _)| name)
                .collect();
            let mut map = GenericMap::default();
            map.map.extend(
                owner
                    .generic_parameters
                    .iter()
                    .filter(|(name, _)| !function_names.contains(name))
                    .cloned(),
            );
            map
        });
        let connected_generic_maps = specialization
            .connected_actuals
            .iter()
            .map(|connected| {
                Ok((
                    connected.formal_port,
                    Self::canonical_generic_map(&connected.actual, EmissionOwnerKind::Interface)?,
                ))
            })
            .collect::<Result<Vec<_>, NestedModportAnalysisInvariant>>()?;
        Ok(Self::new(
            specialization.clone(),
            generic_map,
            enclosing_generic_map,
            connected_generic_maps,
        ))
    }

    pub(super) fn matches(
        &self,
        _kind: EmissionOwnerKind,
        specialization: &ComponentSpecializationIdentity,
    ) -> bool {
        if self.owner != specialization.owner || self.frozen.specialization != *specialization {
            return false;
        }
        if SemanticGenericMap::from(&self.generic_map) != self.frozen.generic_map
            || self
                .enclosing_generic_map
                .as_ref()
                .map(SemanticGenericMap::from)
                != self.frozen.enclosing_generic_map
        {
            return false;
        }

        self.connected_generic_maps.len() == self.frozen.connected_generic_maps.len()
            && self
                .frozen
                .connected_generic_map_index
                .matches(&self.connected_generic_maps)
            && self
                .connected_generic_maps
                .iter()
                .zip(&self.frozen.connected_generic_maps)
                .all(
                    |((formal_port, actual_map), (expected_port, expected_map))| {
                        formal_port == expected_port
                            && SemanticGenericMap::from(actual_map) == *expected_map
                    },
                )
    }
}
