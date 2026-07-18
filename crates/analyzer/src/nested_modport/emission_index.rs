use super::binding_specialization_identity::BindingSpecializationIdentity;
use super::emission_scope::EmissionScopeIdentity;
use super::frame_record_index::FrozenFrameRecords;
use super::identity::*;
use super::semantic_records::*;
use super::semantic_work::record_semantic_work;
use super::specialization_context::{
    EmissionBindingId, EmissionOwnerKind, EmissionSpecializationContext, LoweringAvailability,
};
use crate::symbol::{GenericMap, SymbolId};
use crate::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use veryl_parser::resource_table::{PathId, TokenId};

#[derive(Clone, Debug)]
pub struct PendingEmissionBinding {
    pub id: EmissionBindingId,
    pub source: PathId,
    pub declaration: TokenId,
    pub kind: EmissionOwnerKind,
    pub enclosing_owner: Option<Arc<NestedModportLoweringKey>>,
    pub(crate) enclosing_owner_identity: Option<BindingSpecializationIdentity>,
    pub namespace_parent_fallback: bool,
    pub enclosing_generic_map: Option<GenericMap>,
    pub package_scope: Option<EmissionScopeIdentity>,
    pub specialization: Arc<NestedModportLoweringKey>,
    pub(crate) specialization_identity: Option<BindingSpecializationIdentity>,
    pub emission_context: EmissionSpecializationContext,
    pub lowering: LoweringAvailability,
    pub required_rewrites: Arc<[OccurrenceRewriteKey]>,
    pub required_expanded_ports: Arc<[ExpandedPortKey]>,
}

#[derive(Clone, Debug)]
pub struct EmissionOwnerBinding {
    record: PendingEmissionBinding,
    pub(super) frame_records: FrozenFrameRecords,
    pub(super) owner_handle: FrozenOwnerHandle,
    pub(super) enclosing_owner_handle: Option<FrozenOwnerHandle>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct FrozenOwnerHandle {
    session: AnalysisSessionId,
    ordinal: usize,
}

impl FrozenOwnerHandle {
    pub(super) const fn new(session: AnalysisSessionId, ordinal: usize) -> Self {
        Self { session, ordinal }
    }
}

impl EmissionOwnerBinding {
    pub(super) fn freeze(
        record: PendingEmissionBinding,
        owner_handle: FrozenOwnerHandle,
        enclosing_owner_handle: Option<FrozenOwnerHandle>,
        instantiation_contexts: &[(TokenId, EmissionSpecializationContext)],
        records: &super::record_resolution::FrozenSemanticRecords,
    ) -> Result<Self, super::errors::NestedModportAnalysisInvariant> {
        let frame_records = FrozenFrameRecords::freeze(
            &record.required_rewrites,
            &record.required_expanded_ports,
            instantiation_contexts,
            records,
        )?;
        Ok(Self {
            record,
            frame_records,
            owner_handle,
            enclosing_owner_handle,
        })
    }
}

impl std::ops::Deref for EmissionOwnerBinding {
    type Target = PendingEmissionBinding;

    fn deref(&self) -> &Self::Target {
        &self.record
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct EmissionDeclarationKey {
    pub(super) source: PathId,
    pub(super) declaration: TokenId,
    pub(super) kind: EmissionOwnerKind,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct EmissionBindingRecordKey {
    pub(super) declaration: EmissionDeclarationKey,
    pub(super) specialization: BindingSpecializationIdentity,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct EmissionDeclarationScopeKey {
    pub(super) declaration: EmissionDeclarationKey,
    pub(super) scope: SemanticGenericMap,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PendingGenericEmissionOwner {
    pub session: AnalysisSessionId,
    pub source: PathId,
    pub declaration: TokenId,
    pub kind: EmissionOwnerKind,
    pub symbol: SymbolId,
}

#[derive(Clone, Debug)]
pub(super) enum EmissionScopeKey {
    Unscoped,
    Enclosing(FrozenOwnerHandle),
    Package(EmissionScopeIdentity),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SourceEmissionKey(pub(super) PathId);

impl PartialEq for SourceEmissionKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.0 == other.0
    }
}

impl Eq for SourceEmissionKey {}

impl Hash for SourceEmissionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.0.hash(state);
    }
}

#[derive(Clone, Debug)]
pub(super) struct EmissionOwnerGroupKey {
    pub(super) declaration: TokenId,
    pub(super) kind: EmissionOwnerKind,
    pub(super) scope: EmissionScopeKey,
}

#[derive(Clone, Debug)]
pub(super) struct EmissionOwnerGroup {
    pub(super) key: EmissionOwnerGroupKey,
    pub(super) binding_range: Range<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct PackageScopeLookupKey {
    pub(super) declaration: TokenId,
    pub(super) symbol: SymbolId,
}

impl PartialEq for EmissionScopeKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        match (self, other) {
            (Self::Unscoped, Self::Unscoped) => true,
            (Self::Enclosing(left), Self::Enclosing(right)) => left == right,
            (Self::Package(left), Self::Package(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for EmissionScopeKey {}

impl Hash for EmissionScopeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        match self {
            Self::Unscoped => 0_u8.hash(state),
            Self::Enclosing(value) => {
                1_u8.hash(state);
                value.hash(state);
            }
            Self::Package(value) => {
                2_u8.hash(state);
                value.hash(state);
            }
        }
    }
}

impl PartialEq for EmissionOwnerGroupKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.declaration == other.declaration
            && self.kind == other.kind
            && self.scope == other.scope
    }
}

impl Eq for EmissionOwnerGroupKey {}

impl Hash for EmissionOwnerGroupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(2);
        self.declaration.hash(state);
        self.kind.hash(state);
        self.scope.hash(state);
    }
}

impl PartialEq for PackageScopeLookupKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.declaration == other.declaration && self.symbol == other.symbol
    }
}

impl Eq for PackageScopeLookupKey {}

impl Hash for PackageScopeLookupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.declaration.hash(state);
        self.symbol.hash(state);
    }
}

#[derive(Debug)]
pub(super) enum PackageScopeEntry {
    Unique(EmissionScopeIdentity),
    Ambiguous,
}

#[derive(Debug)]
pub(super) struct PackageScopeIndex {
    pub(super) expected: SemanticGenericMap,
    pub(super) entries: HashMap<SemanticGenericMap, PackageScopeEntry>,
}

#[derive(Debug, Default)]
pub(super) struct FrozenEmissionIndex {
    pub(super) by_source: HashMap<SourceEmissionKey, Arc<SourceEmissionTable>>,
}

#[derive(Debug)]
pub(super) struct SourceEmissionTable {
    pub(super) source: PathId,
    pub(super) bindings: Arc<[EmissionOwnerBinding]>,
    pub(super) groups: Arc<[EmissionOwnerGroup]>,
    pub(super) group_index: HashMap<EmissionOwnerGroupKey, usize>,
    pub(super) ordered_groups_by_scope: HashMap<EmissionScopeKey, Arc<[usize]>>,
    pub(super) package_scope_index: HashMap<PackageScopeLookupKey, PackageScopeIndex>,
    pub(super) nonempty_owners: HashSet<(TokenId, EmissionOwnerKind)>,
    pub(super) empty_owners: HashSet<(TokenId, EmissionOwnerKind)>,
}

impl FrozenEmissionIndex {
    pub(super) fn source(&self, source: PathId) -> Option<&SourceEmissionTable> {
        self.by_source
            .get(&SourceEmissionKey(source))
            .map(Arc::as_ref)
    }
}

pub(super) fn binding_scope(binding: &EmissionOwnerBinding) -> EmissionScopeKey {
    if binding.kind != EmissionOwnerKind::Function {
        EmissionScopeKey::Unscoped
    } else if let Some(enclosing) = binding.enclosing_owner_handle {
        EmissionScopeKey::Enclosing(enclosing)
    } else if let Some(package) = &binding.package_scope {
        EmissionScopeKey::Package(package.clone())
    } else {
        EmissionScopeKey::Unscoped
    }
}
