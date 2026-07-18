use super::collection_work::{counted, record_collection_work};
use super::errors::*;
use super::identity::*;
use super::lowering_identity::{
    LoweringEquivalenceIdentity, SemanticLoweringIdentity, SessionLoweringInterner,
    rehash_on_identity_query,
};
use super::pending_records::PendingNestedModportAnalysis;
use super::semantic_records::*;
use super::specialization_context::*;
use crate::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub(super) struct FrozenSemanticRecords {
    pub(super) availability: HashMap<NestedModportLoweringKey, LoweringAvailability>,
    pub(super) rewrites: HashMap<OccurrenceRewriteKey, Arc<ResolvedPathRewrite>>,
    pub(super) expanded_ports: HashMap<ExpandedPortKey, Arc<ExpandedPortResolution>>,
    lowering_identities: HashMap<NestedModportLoweringKey, SemanticLoweringIdentity>,
}

impl FrozenSemanticRecords {
    pub(super) fn terminal_lowering_identity(
        &self,
        terminal: &ResolvedTerminalRef,
    ) -> LoweringEquivalenceIdentity {
        if rehash_on_identity_query() {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            terminal.lowering.hash(&mut hasher);
            std::hint::black_box(hasher.finish());
        }
        LoweringEquivalenceIdentity::Found(terminal.lowering_identity)
    }

    pub(super) fn lowering_identity(
        &self,
        key: &NestedModportLoweringKey,
        availability: &LoweringAvailability,
    ) -> Result<LoweringEquivalenceIdentity, NestedModportAnalysisInvariant> {
        if rehash_on_identity_query()
            && let LoweringAvailability::Found(lowering) = availability
        {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            lowering.hash(&mut hasher);
            std::hint::black_box(hasher.finish());
        }
        match availability {
            LoweringAvailability::NotNested => Ok(LoweringEquivalenceIdentity::NotNested),
            LoweringAvailability::Found(_) => self
                .lowering_identities
                .get(key)
                .copied()
                .map(LoweringEquivalenceIdentity::Found)
                .ok_or(NestedModportAnalysisInvariant::MissingLowering),
        }
    }
}

impl PendingNestedModportAnalysis {
    pub(super) fn resolve_semantic_records(
        &self,
    ) -> Result<FrozenSemanticRecords, NestedModportFinalizeError> {
        let mut availability: HashMap<_, _> = counted(&self.lowerings)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for (key, lowering) in counted(&self.interface_lowerings) {
            record_collection_work(1);
            availability.insert(
                key.clone(),
                LoweringAvailability::Found(Arc::clone(lowering)),
            );
        }
        let mut interners: HashMap<AnalysisSessionId, SessionLoweringInterner> = HashMap::default();
        let mut lowering_identities = HashMap::default();
        for (key, availability) in counted(&availability) {
            let LoweringAvailability::Found(lowering) = availability else {
                continue;
            };
            let identity = interners
                .entry(key.session)
                .or_default()
                .intern(key.session, Arc::clone(lowering));
            lowering_identities.insert(key.clone(), identity);
        }
        let rewrites = self.resolve_rewrites(&availability, &lowering_identities)?;
        let expanded_ports = self.resolve_expanded_ports(&availability)?;
        Ok(FrozenSemanticRecords {
            availability,
            rewrites,
            expanded_ports,
            lowering_identities,
        })
    }

    fn resolve_rewrites(
        &self,
        availability: &HashMap<NestedModportLoweringKey, LoweringAvailability>,
        lowering_identities: &HashMap<NestedModportLoweringKey, SemanticLoweringIdentity>,
    ) -> Result<HashMap<OccurrenceRewriteKey, Arc<ResolvedPathRewrite>>, NestedModportFinalizeError>
    {
        let mut rewrites: HashMap<_, _> = counted(&self.rewrites)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for (key, candidate) in counted(&self.rewrite_candidates) {
            record_collection_work(1);
            let Some(LoweringAvailability::Found(lowering)) = availability.get(&candidate.target)
            else {
                if self.optional_rewrite_candidates.contains_key(key) {
                    continue;
                }
                return Err(NestedModportAnalysisInvariant::UnresolvedRewrite.into());
            };
            let terminal = lowering.longest_terminal_prefix(&candidate.semantic_segments);
            let Some(terminal) = terminal else {
                if let Some(occurrence) = self.optional_rewrite_candidates.get(key) {
                    let removed_root = candidate
                        .semantic_segments
                        .first()
                        .is_some_and(|root| lowering.removes_instance_root(*root));
                    if removed_root {
                        return Err(NestedModportFinalizeError::UnloweredLocalReference {
                            semantic_segments: candidate.semantic_segments.clone(),
                            occurrence: *occurrence,
                        });
                    }
                    continue;
                }
                return Err(NestedModportAnalysisInvariant::UnresolvedRewrite.into());
            };
            let leading_segments = candidate
                .consumed_segments
                .checked_sub(candidate.semantic_segments.len())
                .ok_or(NestedModportAnalysisInvariant::UnresolvedRewrite)?;
            let consumed_segments = leading_segments
                .checked_add(terminal.identifier.source_segments.len())
                .ok_or(NestedModportAnalysisInvariant::UnresolvedRewrite)?;
            let resolved = ResolvedPathRewrite {
                terminal: ResolvedTerminalRef::new(
                    &candidate.target,
                    Arc::clone(lowering),
                    lowering_identities
                        .get(&candidate.target)
                        .copied()
                        .ok_or(NestedModportAnalysisInvariant::MissingLowering)?,
                    terminal.id,
                ),
                semantic_segments: terminal.identifier.source_segments.clone(),
                replace_from_segment: candidate.replace_from_segment,
                consumed_segments,
            };
            match rewrites.get(key) {
                Some(previous) if previous.as_ref() != &resolved => {
                    return Err(NestedModportAnalysisInvariant::ConflictingRewrite.into());
                }
                Some(_) => {}
                None => {
                    rewrites.insert(key.clone(), Arc::new(resolved));
                }
            }
        }
        Ok(rewrites)
    }

    fn resolve_expanded_ports(
        &self,
        availability: &HashMap<NestedModportLoweringKey, LoweringAvailability>,
    ) -> Result<HashMap<ExpandedPortKey, Arc<ExpandedPortResolution>>, NestedModportFinalizeError>
    {
        let mut expanded_ports: HashMap<_, _> = counted(&self.expanded_ports)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for (key, candidate) in counted(&self.expanded_port_candidates) {
            record_collection_work(1);
            let resolved = match availability.get(&candidate.target) {
                Some(LoweringAvailability::Found(lowering)) => {
                    let target_context = EmissionSpecializationContext::from_specialization(
                        EmissionOwnerKind::Interface,
                        &candidate.target.specialization,
                    )?;
                    ExpandedPortResolution::Nested {
                        target: candidate.target.clone(),
                        modport: candidate.modport,
                        interface: ResolvedExpandedPortInterface {
                            symbol: candidate.target.specialization.owner.symbol,
                            generic_map: target_context.generic_map,
                        },
                        members: Arc::new(
                            super::expanded_member_index::ResolvedExpandedMemberSet::from_lowering(
                                lowering,
                                candidate.modport,
                            )?,
                        ),
                    }
                }
                Some(LoweringAvailability::NotNested) => ExpandedPortResolution::DirectLegacy,
                None => {
                    return Err(NestedModportAnalysisInvariant::UnresolvedExpandedPort.into());
                }
            };
            match expanded_ports.get(key) {
                Some(previous) if previous.as_ref() != &resolved => {
                    return Err(NestedModportAnalysisInvariant::ConflictingExpandedPort.into());
                }
                Some(_) => {}
                None => {
                    expanded_ports.insert(key.clone(), Arc::new(resolved));
                }
            }
        }
        Ok(expanded_ports)
    }
}
