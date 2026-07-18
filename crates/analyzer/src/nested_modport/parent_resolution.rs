use super::binding_specialization_identity::BindingSpecializationIdentity;
use super::collection_work::{counted, record_collection_work as record_binding_work};
use super::emission_index::PendingEmissionBinding;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::SemanticGenericMap;
use super::semantic_work::record_semantic_work;
use super::specialization_context::EmissionOwnerKind;
use crate::HashMap;
use crate::symbol::SymbolId;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug)]
struct ParentResolutionKey {
    specialization: BindingSpecializationIdentity,
    generic_map: SemanticGenericMap,
}

#[derive(Clone, Debug)]
struct UnresolvedNamespaceParentKey {
    symbol: SymbolId,
    generic_map: SemanticGenericMap,
}

impl PartialEq for ParentResolutionKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.specialization == other.specialization && self.generic_map == other.generic_map
    }
}

impl Eq for ParentResolutionKey {}

impl Hash for ParentResolutionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.specialization.hash(state);
        self.generic_map.hash(state);
    }
}

impl PartialEq for UnresolvedNamespaceParentKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.symbol == other.symbol && self.generic_map == other.generic_map
    }
}

impl Eq for UnresolvedNamespaceParentKey {}

impl Hash for UnresolvedNamespaceParentKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.symbol.hash(state);
        self.generic_map.hash(state);
    }
}

enum ParentResolutionEntry {
    Unique(BindingSpecializationIdentity),
    Ambiguous,
}

pub(super) fn resolve_function_enclosing_owners(
    bindings: &mut [PendingEmissionBinding],
    retained: &[bool],
) -> Result<(), NestedModportAnalysisInvariant> {
    let mut parents = HashMap::default();
    let mut namespace_parents = HashMap::default();
    for (binding, keep) in counted(bindings.iter().zip(retained)) {
        if !keep || binding.kind == EmissionOwnerKind::Function {
            continue;
        }
        let identity = binding
            .specialization_identity
            .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?;
        record_binding_work(1);
        let key = ParentResolutionKey {
            specialization: identity,
            generic_map: SemanticGenericMap::from(&binding.emission_context.generic_map),
        };
        record_binding_work(1);
        match parents.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                record_binding_work(1);
                entry.insert(ParentResolutionEntry::Unique(identity));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                record_binding_work(1);
                entry.insert(ParentResolutionEntry::Ambiguous);
            }
        }
        record_binding_work(2);
        let namespace_key = UnresolvedNamespaceParentKey {
            symbol: binding.specialization.specialization.owner.symbol,
            generic_map: SemanticGenericMap::from(&binding.emission_context.generic_map),
        };
        match namespace_parents.entry(namespace_key) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                record_binding_work(1);
                entry.insert(ParentResolutionEntry::Unique(identity));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                record_binding_work(1);
                entry.insert(ParentResolutionEntry::Ambiguous);
            }
        }
    }
    let mut resolved_parents = Vec::with_capacity(bindings.len());
    for (binding, keep) in counted(bindings.iter().zip(retained)) {
        if !keep || binding.kind != EmissionOwnerKind::Function {
            resolved_parents.push(None);
            continue;
        }
        let (Some(parent), Some(parent_identity), Some(enclosing_map)) = (
            &binding.enclosing_owner,
            binding.enclosing_owner_identity,
            &binding.enclosing_generic_map,
        ) else {
            resolved_parents.push(None);
            continue;
        };
        let generic_map = SemanticGenericMap::from(enclosing_map);
        record_binding_work(2);
        let resolved = if binding.namespace_parent_fallback {
            namespace_parents.get(&UnresolvedNamespaceParentKey {
                symbol: parent.specialization.owner.symbol,
                generic_map,
            })
        } else {
            parents.get(&ParentResolutionKey {
                specialization: parent_identity,
                generic_map,
            })
        };
        match resolved {
            Some(ParentResolutionEntry::Unique(resolved)) => {
                record_binding_work(1);
                resolved_parents.push(Some(*resolved));
            }
            Some(ParentResolutionEntry::Ambiguous) | None => {
                return Err(NestedModportAnalysisInvariant::MismatchedEmissionContext);
            }
        }
    }
    for (binding, resolved) in bindings.iter_mut().zip(resolved_parents) {
        if let Some(resolved) = resolved {
            binding.enclosing_owner_identity = Some(resolved);
        }
    }
    Ok(())
}
