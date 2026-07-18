use super::errors::NestedModportAnalysisInvariant;
use super::record_resolution::FrozenSemanticRecords;
use super::semantic_records::*;
use super::semantic_work::record_semantic_work;
use super::specialization_context::EmissionSpecializationContext;
use crate::HashMap;
#[cfg(test)]
use crate::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use veryl_parser::resource_table::TokenId;

#[cfg(test)]
thread_local! {
    static FORCE_FRAME_RECORD_HASH_COLLISION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) struct FrameRecordHashCollisionGuard;

#[cfg(test)]
impl Drop for FrameRecordHashCollisionGuard {
    fn drop(&mut self) {
        FORCE_FRAME_RECORD_HASH_COLLISION.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_frame_record_hash_collision() -> FrameRecordHashCollisionGuard {
    FORCE_FRAME_RECORD_HASH_COLLISION.set(true);
    FrameRecordHashCollisionGuard
}

#[cfg(test)]
fn force_hash_collision() -> bool {
    FORCE_FRAME_RECORD_HASH_COLLISION.get()
}

#[cfg(not(test))]
const fn force_hash_collision() -> bool {
    false
}

#[derive(Clone, Copy, Debug)]
struct FrameOccurrenceKey {
    kind: OccurrenceKind,
    token: TokenId,
}

impl PartialEq for FrameOccurrenceKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.kind == other.kind && self.token == other.token
    }
}

impl Eq for FrameOccurrenceKey {}

impl Hash for FrameOccurrenceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        if !force_hash_collision() {
            self.kind.hash(state);
            self.token.hash(state);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FrameExpandedPortKey(TokenId);

impl PartialEq for FrameExpandedPortKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.0 == other.0
    }
}

impl Eq for FrameExpandedPortKey {}

impl Hash for FrameExpandedPortKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        if !force_hash_collision() {
            self.0.hash(state);
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct FrozenFrameRecords {
    rewrites: HashMap<FrameOccurrenceKey, Arc<ResolvedPathRewrite>>,
    expanded_ports: HashMap<FrameExpandedPortKey, Arc<ExpandedPortResolution>>,
    instantiation_contexts: HashMap<TokenId, EmissionSpecializationContext>,
    #[cfg(test)]
    legacy_rewrites: HashSet<OccurrenceRewriteKey>,
    #[cfg(test)]
    legacy_expanded_ports: HashSet<ExpandedPortKey>,
}

impl FrozenFrameRecords {
    pub(super) fn freeze(
        required_rewrites: &Arc<[OccurrenceRewriteKey]>,
        required_expanded_ports: &Arc<[ExpandedPortKey]>,
        instantiation_contexts: &[(TokenId, EmissionSpecializationContext)],
        records: &FrozenSemanticRecords,
    ) -> Result<Self, NestedModportAnalysisInvariant> {
        let mut rewrites = HashMap::default();
        for key in required_rewrites.iter() {
            let value = records
                .rewrites
                .get(key)
                .map(Arc::clone)
                .ok_or(NestedModportAnalysisInvariant::MissingRewrite)?;
            rewrites.insert(
                FrameOccurrenceKey {
                    kind: key.kind,
                    token: key.token,
                },
                value,
            );
        }
        let mut expanded_ports = HashMap::default();
        for key in required_expanded_ports.iter() {
            let value = records
                .expanded_ports
                .get(key)
                .ok_or(NestedModportAnalysisInvariant::MissingExpandedPort)?;
            expanded_ports.insert(FrameExpandedPortKey(key.token), Arc::clone(value));
        }
        Ok(Self {
            rewrites,
            expanded_ports,
            instantiation_contexts: instantiation_contexts.iter().cloned().collect(),
            #[cfg(test)]
            legacy_rewrites: required_rewrites.iter().cloned().collect(),
            #[cfg(test)]
            legacy_expanded_ports: required_expanded_ports.iter().cloned().collect(),
        })
    }

    pub(super) fn rewrite(
        &self,
        kind: OccurrenceKind,
        token: TokenId,
    ) -> Option<&ResolvedPathRewrite> {
        self.rewrites
            .get(&FrameOccurrenceKey { kind, token })
            .map(Arc::as_ref)
    }

    pub(super) fn expanded_port(&self, token: TokenId) -> Option<&ExpandedPortResolution> {
        self.expanded_ports
            .get(&FrameExpandedPortKey(token))
            .map(Arc::as_ref)
    }

    pub(super) fn instantiation_context(
        &self,
        token: TokenId,
    ) -> Option<&EmissionSpecializationContext> {
        record_semantic_work(1);
        self.instantiation_contexts.get(&token)
    }

    #[cfg(test)]
    pub(super) fn legacy_requires_rewrite(&self, key: &OccurrenceRewriteKey) -> bool {
        self.legacy_rewrites.contains(key)
    }

    #[cfg(test)]
    pub(super) fn legacy_requires_expanded_port(&self, key: &ExpandedPortKey) -> bool {
        self.legacy_expanded_ports.contains(key)
    }
}
