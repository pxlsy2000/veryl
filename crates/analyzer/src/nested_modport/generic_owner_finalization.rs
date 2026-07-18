use super::collection_work::{counted, record_collection_work};
use super::emission_index::*;
use super::errors::NestedModportAnalysisInvariant;
use super::generic_owner_work::owner_membership_scan_mutation_enabled;
use super::identity::*;
use super::pending_records::PendingNestedModportAnalysis;
use super::specialization_context::*;
use crate::ir::Signature;
use crate::symbol::GenericMap;
use crate::{HashMap, HashSet, symbol_table};
use std::hash::{Hash, Hasher};
use veryl_parser::resource_table::{PathId, TokenId};

impl PendingNestedModportAnalysis {
    pub(super) fn complete_generic_emission_owners(
        &mut self,
        session: AnalysisSessionId,
    ) -> Result<HashSet<(PathId, TokenId, EmissionOwnerKind)>, NestedModportAnalysisInvariant> {
        let mut empty = HashSet::default();
        let owners = std::mem::take(&mut self.generic_emission_owners);
        for owner in counted(owners) {
            let symbol = symbol_table::get(owner.symbol)
                .ok_or(NestedModportAnalysisInvariant::MissingLowering)?;
            let namespace_owner = symbol_table::get_namespace_symbol(&symbol.namespace)
                .filter(|namespace_owner| namespace_owner.has_generic_paramters());
            let uses_namespace_owner =
                symbol.generic_parameters().is_empty() && namespace_owner.is_some();
            let expansion_symbol = if uses_namespace_owner {
                namespace_owner.as_ref().unwrap_or(&symbol)
            } else {
                &symbol
            };
            let parameters: Vec<_> = counted(expansion_symbol.generic_parameters())
                .map(|(name, _)| name)
                .collect();
            let parameter_index = (owner.kind == EmissionOwnerKind::Function).then(|| {
                let index: HashSet<_> = counted(&parameters)
                    .copied()
                    .map(CountedParameterName)
                    .collect();
                index
            });
            let declaration = EmissionDeclarationKey {
                source: owner.source,
                declaration: owner.declaration,
                kind: owner.kind,
            };
            if owner.kind == EmissionOwnerKind::Function
                && !uses_namespace_owner
                && self.has_emission_declaration(&declaration)
            {
                continue;
            }
            let generic_maps = expansion_symbol.generic_maps();
            if generic_maps.is_empty() {
                empty.insert((owner.source, owner.declaration, owner.kind));
                continue;
            }
            let signature_parameters: Vec<_> = if owner.kind == EmissionOwnerKind::Function {
                let mut names: Vec<_> = counted(
                    generic_maps
                        .first()
                        .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?
                        .map
                        .keys(),
                )
                .copied()
                .collect();
                names.sort_by(|left, right| {
                    record_collection_work(1);
                    left.cmp(right)
                });
                names
            } else {
                counted(&parameters).copied().collect()
            };
            for generic_map in counted(generic_maps) {
                let fills_namespace_scopes =
                    owner.kind != EmissionOwnerKind::Function || uses_namespace_owner;
                if fills_namespace_scopes
                    && self.has_emission_scope(&EmissionDeclarationScopeKey {
                        declaration: declaration.clone(),
                        scope: SemanticGenericMap::from(&generic_map),
                    })
                {
                    continue;
                }
                let mut signature = Signature::new(owner.symbol);
                let mut emission_map = GenericMap {
                    id: generic_map.id,
                    map: HashMap::default(),
                };
                let value_index: HashMap<_, _> = counted(&generic_map.map)
                    .map(|(name, value)| (CountedParameterName(*name), value))
                    .collect();
                if value_index.len() != signature_parameters.len() {
                    return Err(NestedModportAnalysisInvariant::MismatchedEmissionContext);
                }
                for name in counted(&signature_parameters) {
                    let value = value_index
                        .get(&CountedParameterName(*name))
                        .ok_or(NestedModportAnalysisInvariant::MismatchedEmissionContext)?;
                    record_collection_work(1);
                    signature.add_generic_parameter(*name, (*value).clone());
                    let owns_parameter = owner.kind != EmissionOwnerKind::Function
                        || if owner_membership_scan_mutation_enabled() {
                            counted(&parameters).any(|parameter| parameter == name)
                        } else {
                            parameter_index
                                .as_ref()
                                .is_some_and(|index| index.contains(&CountedParameterName(*name)))
                        };
                    if owns_parameter {
                        record_collection_work(1);
                        emission_map.map.insert(*name, (*value).clone());
                    }
                }
                expansion_symbol.eval_generic_consts(&mut emission_map);
                if owner.kind != EmissionOwnerKind::Function {
                    signature.generic_parameters.sort_by(|left, right| {
                        record_collection_work(1);
                        left.cmp(right)
                    });
                }
                let specialization = NestedModportLoweringKey {
                    session,
                    specialization: ComponentSpecializationIdentity {
                        owner: signature,
                        connected_actuals: Vec::new(),
                    }
                    .into(),
                };
                if self.has_declaration_specialization(&declaration, &specialization) {
                    continue;
                }
                record_collection_work(1);
                self.record_lowering(specialization.clone(), LoweringAvailability::NotNested);
                let emission_context = EmissionSpecializationContext::new(
                    specialization.specialization.as_ref().clone(),
                    emission_map,
                    None,
                    Vec::new(),
                );
                record_collection_work(1);
                self.record_emission_owner(
                    owner.source,
                    owner.declaration,
                    owner.kind,
                    specialization.clone(),
                    emission_context,
                    LoweringAvailability::NotNested,
                )?;
                record_collection_work(1);
                self.set_emission_owner_scope(&specialization, &generic_map);
            }
        }
        Ok(empty)
    }
}

#[derive(Clone, Copy, Eq)]
struct CountedParameterName(veryl_parser::resource_table::StrId);

impl PartialEq for CountedParameterName {
    fn eq(&self, other: &Self) -> bool {
        super::collection_work::record_collection_work(1);
        self.0 == other.0
    }
}

impl Hash for CountedParameterName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        super::collection_work::record_collection_work(1);
        self.0.hash(state);
    }
}
