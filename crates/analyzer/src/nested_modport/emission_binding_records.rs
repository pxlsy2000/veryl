use super::collection_work::record_collection_work;
use super::emission_index::*;
use super::emission_scope::EmissionScopeIdentity;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::*;
use super::pending_records::PendingNestedModportAnalysis;
use super::specialization_context::*;
use crate::symbol::GenericMap;
use std::sync::Arc;
use veryl_parser::resource_table::{PathId, TokenId};

impl PendingNestedModportAnalysis {
    pub fn record_emission_binding(&mut self, mut value: PendingEmissionBinding) {
        self.record_session(value.specialization.session);
        let (specialization, specialization_identity) = self
            .binding_specialization_interner
            .intern(Arc::clone(&value.specialization));
        value.specialization = specialization;
        value.specialization_identity = Some(specialization_identity);
        if let Some(parent) = value.enclosing_owner.take() {
            let (parent, identity) = self.binding_specialization_interner.intern(parent);
            value.enclosing_owner = Some(parent);
            value.enclosing_owner_identity = Some(identity);
        }
        let index = self.emission_bindings.len();
        let declaration = EmissionDeclarationKey {
            source: value.source,
            declaration: value.declaration,
            kind: value.kind,
        };
        let record = EmissionBindingRecordKey {
            declaration: declaration.clone(),
            specialization: specialization_identity,
        };
        let scope = semantic_scope(&value).map(|scope| EmissionDeclarationScopeKey {
            declaration: declaration.clone(),
            scope,
        });
        self.max_emission_binding_id = Some(
            self.max_emission_binding_id
                .map_or(value.id.0, |current| current.max(value.id.0)),
        );
        self.emission_bindings.push(value);
        self.emission_binding_records.entry(record).or_insert(index);
        self.emission_binding_specializations
            .entry(specialization_identity)
            .or_insert(index);
        self.emission_binding_declarations
            .insert(declaration.clone());
        self.emission_declaration_specializations
            .insert((declaration, specialization_identity));
        if let Some(scope) = scope {
            self.emission_binding_scopes.insert(scope);
        }
        record_collection_work(8);
    }

    pub fn record_generic_emission_owner(&mut self, value: PendingGenericEmissionOwner) {
        self.record_session(value.session);
        record_collection_work(1);
        if self.generic_emission_owner_set.insert(value.clone()) {
            self.generic_emission_owners.push(value);
            record_collection_work(1);
        }
    }

    pub fn record_emission_owner(
        &mut self,
        source: PathId,
        declaration: TokenId,
        kind: EmissionOwnerKind,
        specialization: NestedModportLoweringKey,
        emission_context: EmissionSpecializationContext,
        lowering: LoweringAvailability,
    ) -> Result<EmissionBindingId, NestedModportAnalysisInvariant> {
        let declaration_key = EmissionDeclarationKey {
            source,
            declaration,
            kind,
        };
        let specialization = Arc::new(specialization);
        let (specialization, specialization_identity) =
            self.binding_specialization_interner.intern(specialization);
        let record_key = EmissionBindingRecordKey {
            declaration: declaration_key,
            specialization: specialization_identity,
        };
        record_collection_work(1);
        if let Some(index) = self.emission_binding_records.get(&record_key).copied() {
            return Ok(self.emission_bindings[index].id);
        }
        let raw_id = self
            .max_emission_binding_id
            .map_or(Some(0), |id| id.checked_add(1))
            .ok_or(NestedModportAnalysisInvariant::EmissionBindingOverflow)?;
        let id = EmissionBindingId::new(raw_id);
        self.record_emission_binding(PendingEmissionBinding {
            id,
            source,
            declaration,
            kind,
            enclosing_owner: None,
            enclosing_owner_identity: None,
            namespace_parent_fallback: false,
            enclosing_generic_map: None,
            package_scope: None,
            specialization,
            specialization_identity: None,
            emission_context,
            lowering,
            required_rewrites: Arc::from([]),
            required_expanded_ports: Arc::from([]),
        });
        self.inferred_binding_ids.insert(id);
        record_collection_work(1);
        Ok(id)
    }

    pub(crate) fn set_emission_owner_parent(
        &mut self,
        specialization: &NestedModportLoweringKey,
        parent: Option<Arc<ComponentSpecializationIdentity>>,
        namespace_parent_fallback: bool,
        enclosing_generic_map: Option<GenericMap>,
    ) {
        record_collection_work(1);
        if let Some(index) = self
            .binding_specialization_interner
            .identity(specialization)
            .and_then(|identity| {
                self.emission_binding_specializations
                    .get(&identity)
                    .copied()
            })
        {
            let (parent, parent_identity) = parent.map_or((None, None), |parent| {
                let key = Arc::new(NestedModportLoweringKey {
                    session: specialization.session,
                    specialization: parent,
                });
                let (key, identity) = self.binding_specialization_interner.intern(key);
                (Some(key), Some(identity))
            });
            let binding = &mut self.emission_bindings[index];
            binding.enclosing_owner = parent;
            binding.enclosing_owner_identity = parent_identity;
            binding.namespace_parent_fallback = namespace_parent_fallback;
            binding.enclosing_generic_map = enclosing_generic_map;
            if binding.enclosing_owner.is_none() {
                let scope_map = binding
                    .enclosing_generic_map
                    .as_ref()
                    .or(binding.emission_context.enclosing_generic_map.as_ref())
                    .cloned()
                    .unwrap_or_default();
                binding.package_scope = EmissionScopeIdentity::for_function(
                    specialization.session,
                    &binding.specialization.specialization.owner,
                    &scope_map,
                );
            }
            self.index_binding_scope(index);
        }
    }

    pub(crate) fn set_emission_owner_scope(
        &mut self,
        specialization: &NestedModportLoweringKey,
        map: &GenericMap,
    ) {
        record_collection_work(1);
        if let Some(index) = self
            .binding_specialization_interner
            .identity(specialization)
            .and_then(|identity| {
                self.emission_binding_specializations
                    .get(&identity)
                    .copied()
            })
        {
            let binding = &mut self.emission_bindings[index];
            binding.package_scope = EmissionScopeIdentity::for_function(
                specialization.session,
                &binding.specialization.specialization.owner,
                map,
            );
            self.index_binding_scope(index);
        }
    }

    pub fn emission_bindings(&self) -> &[PendingEmissionBinding] {
        &self.emission_bindings
    }

    pub(super) fn has_emission_declaration(&self, key: &EmissionDeclarationKey) -> bool {
        record_collection_work(1);
        self.emission_binding_declarations.contains(key)
    }

    pub(super) fn has_emission_scope(&self, key: &EmissionDeclarationScopeKey) -> bool {
        record_collection_work(1);
        self.emission_binding_scopes.contains(key)
    }

    pub(super) fn has_declaration_specialization(
        &self,
        declaration: &EmissionDeclarationKey,
        specialization: &NestedModportLoweringKey,
    ) -> bool {
        record_collection_work(1);
        self.binding_specialization_interner
            .identity(specialization)
            .is_some_and(|identity| {
                self.emission_declaration_specializations
                    .contains(&(declaration.clone(), identity))
            })
    }

    fn index_binding_scope(&mut self, index: usize) {
        let binding = &self.emission_bindings[index];
        if let Some(scope) = semantic_scope(binding) {
            self.emission_binding_scopes
                .insert(EmissionDeclarationScopeKey {
                    declaration: EmissionDeclarationKey {
                        source: binding.source,
                        declaration: binding.declaration,
                        kind: binding.kind,
                    },
                    scope,
                });
            record_collection_work(1);
        }
    }
}

fn semantic_scope(binding: &PendingEmissionBinding) -> Option<SemanticGenericMap> {
    binding
        .package_scope
        .as_ref()
        .map(|scope| scope.semantic_generic_map().clone())
        .or_else(|| {
            binding
                .enclosing_generic_map
                .as_ref()
                .map(SemanticGenericMap::from)
        })
        .or_else(|| {
            (binding.kind != EmissionOwnerKind::Function)
                .then(|| SemanticGenericMap::from(&binding.emission_context.generic_map))
        })
}
