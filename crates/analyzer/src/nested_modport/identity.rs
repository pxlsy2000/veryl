use crate::ir::Signature;
use crate::symbol::GenericMap;
use crate::symbol_path::{GenericSymbolPath, GenericSymbolPathKind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use veryl_parser::resource_table::{self, StrId};

static NEXT_ANALYSIS_SESSION_ID: AtomicU64 = AtomicU64::new(1);

use super::identity_semantics::{normalize_signature, signature_eq};
use super::semantic_work::{counted_sort_by, record_semantic_work};

#[derive(Clone, Debug, Ord, PartialOrd)]
pub(super) struct SemanticGenericMap(pub(super) Vec<(StrId, SemanticGenericPath)>);

impl From<&GenericMap> for SemanticGenericMap {
    fn from(value: &GenericMap) -> Self {
        let mut entries: Vec<_> = value
            .map
            .iter()
            .map(|(name, path)| {
                record_semantic_work(1);
                (
                    resource_table::canonical_str_id(*name),
                    SemanticGenericPath::from(path),
                )
            })
            .collect();
        counted_sort_by(&mut entries, |(left, _), (right, _)| left.cmp(right));
        Self(entries)
    }
}

impl SemanticGenericMap {
    pub(super) fn from_matching_keys(value: &GenericMap, expected: &Self) -> Self {
        let mut matched = Vec::with_capacity(expected.0.len());
        for (name, _) in &expected.0 {
            record_semantic_work(1);
            if let Some(path) = value.map.get(name) {
                record_semantic_work(1);
                matched.push((*name, SemanticGenericPath::from(path)));
            }
        }
        Self(matched)
    }

    #[cfg(test)]
    pub(super) fn from_signature(signature: &Signature) -> Self {
        let mut entries: Vec<_> = signature
            .generic_parameters
            .iter()
            .map(|(name, path)| {
                record_semantic_work(1);
                (
                    resource_table::canonical_str_id(*name),
                    SemanticGenericPath::from(path),
                )
            })
            .collect();
        counted_sort_by(&mut entries, |(left, _), (right, _)| left.cmp(right));
        Self(entries)
    }
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub(super) struct SemanticGenericPath {
    pub(super) segments: Vec<SemanticGenericSegment>,
    pub(super) kind: GenericSymbolPathKind,
}

impl From<&GenericSymbolPath> for SemanticGenericPath {
    fn from(value: &GenericSymbolPath) -> Self {
        Self {
            segments: value
                .paths
                .iter()
                .map(|segment| SemanticGenericSegment {
                    name: {
                        record_semantic_work(1);
                        segment.base.text
                    },
                    arguments: segment
                        .arguments
                        .iter()
                        .map(|argument| {
                            record_semantic_work(1);
                            SemanticGenericPath::from(argument)
                        })
                        .collect(),
                })
                .collect(),
            kind: value.kind.clone(),
        }
    }
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub(super) struct SemanticGenericSegment {
    pub(super) name: StrId,
    pub(super) arguments: Vec<SemanticGenericPath>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnalysisSessionId(u64);

impl Default for AnalysisSessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisSessionId {
    pub fn new() -> Self {
        Self(NEXT_ANALYSIS_SESSION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AnalysisSessionHandle(Arc<AnalysisSessionState>);

#[derive(Debug, Default)]
struct AnalysisSessionState {
    id: OnceLock<AnalysisSessionId>,
    allocations: AtomicUsize,
}

impl AnalysisSessionHandle {
    pub(crate) fn id(&self) -> AnalysisSessionId {
        *self.0.id.get_or_init(|| {
            self.0.allocations.fetch_add(1, Ordering::Relaxed);
            AnalysisSessionId::new()
        })
    }

    #[cfg(test)]
    pub(crate) fn allocation_count(&self) -> usize {
        self.0.allocations.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub struct ConnectedInterfaceSpecialization {
    pub formal_port: StrId,
    pub actual: Signature,
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub struct ComponentSpecializationIdentity {
    pub owner: Signature,
    pub connected_actuals: Vec<ConnectedInterfaceSpecialization>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SpecializationIdentityError {
    #[error("formal interface port {formal_port} has conflicting actual specializations")]
    ConflictingConnectedActual { formal_port: StrId },
}

impl ComponentSpecializationIdentity {
    pub fn new(
        owner: Signature,
        connected_actuals: impl IntoIterator<Item = ConnectedInterfaceSpecialization>,
    ) -> Result<Self, SpecializationIdentityError> {
        let (owner, connected_actuals) = normalize_specialization(owner, connected_actuals);

        let mut normalized: Vec<ConnectedInterfaceSpecialization> =
            Vec::with_capacity(connected_actuals.len());
        for connected in connected_actuals {
            if let Some(previous) = normalized.last()
                && previous.formal_port == connected.formal_port
            {
                if !signature_eq(&previous.actual, &connected.actual) {
                    return Err(SpecializationIdentityError::ConflictingConnectedActual {
                        formal_port: connected.formal_port,
                    });
                }
                continue;
            }
            normalized.push(connected);
        }

        Ok(Self {
            owner,
            connected_actuals: normalized,
        })
    }

    pub(crate) fn from_unique_connected_actuals(
        owner: Signature,
        connected_actuals: impl IntoIterator<Item = ConnectedInterfaceSpecialization>,
    ) -> Self {
        let (owner, connected_actuals) = normalize_specialization(owner, connected_actuals);
        Self {
            owner,
            connected_actuals,
        }
    }
}

fn normalize_specialization(
    mut owner: Signature,
    connected_actuals: impl IntoIterator<Item = ConnectedInterfaceSpecialization>,
) -> (Signature, Vec<ConnectedInterfaceSpecialization>) {
    normalize_signature(&mut owner);
    let mut connected_actuals: Vec<_> = connected_actuals
        .into_iter()
        .map(|mut connected| {
            normalize_signature(&mut connected.actual);
            connected
        })
        .collect();
    counted_sort_by(&mut connected_actuals, |left, right| {
        left.formal_port.cmp(&right.formal_port)
    });
    (owner, connected_actuals)
}

#[derive(Debug, Ord, PartialOrd)]
pub struct NestedModportLoweringKey {
    pub session: AnalysisSessionId,
    pub specialization: Arc<ComponentSpecializationIdentity>,
}

impl Clone for NestedModportLoweringKey {
    fn clone(&self) -> Self {
        #[cfg(test)]
        super::identity_clone_work::record_shared_key_clone();
        Self {
            session: self.session,
            specialization: Arc::clone(&self.specialization),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentCacheKey(pub ComponentSpecializationIdentity);
