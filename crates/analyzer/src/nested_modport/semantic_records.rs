use crate::symbol::{GenericMap, SymbolId};
use std::hash::{Hash, Hasher};
use veryl_parser::resource_table::{StrId, TokenId};

use super::expanded_member_index::ResolvedExpandedMemberSet;
use super::identity::*;
use super::lowering_identity::SemanticLoweringIdentity;
use super::lowering_records::NestedModportLowering;
use super::semantic_work::record_semantic_work;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OccurrenceKind {
    ExpressionIdentifier,
    HierarchicalIdentifier,
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub struct OccurrenceRewriteKey {
    pub owner: NestedModportLoweringKey,
    pub kind: OccurrenceKind,
    pub token: TokenId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResolvedNestedTerminalId(pub u32);

#[derive(Clone, Debug)]
pub struct ResolvedTerminalRef {
    pub terminal: ResolvedNestedTerminalId,
    pub(super) lowering: Arc<NestedModportLowering>,
    pub(super) lowering_identity: SemanticLoweringIdentity,
    #[cfg(any(test, feature = "nested-modport-test-utils"))]
    pub(super) legacy_target: NestedModportLoweringKey,
}

impl PartialEq for ResolvedTerminalRef {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.lowering_identity == other.lowering_identity && self.terminal == other.terminal
    }
}

impl Eq for ResolvedTerminalRef {}

impl Hash for ResolvedTerminalRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.lowering_identity.hash(state);
        self.terminal.hash(state);
    }
}

impl ResolvedTerminalRef {
    pub(super) fn new(
        target: &NestedModportLoweringKey,
        lowering: Arc<NestedModportLowering>,
        lowering_identity: SemanticLoweringIdentity,
        terminal: ResolvedNestedTerminalId,
    ) -> Self {
        #[cfg(not(any(test, feature = "nested-modport-test-utils")))]
        let _ = target;
        Self {
            terminal,
            lowering,
            lowering_identity,
            #[cfg(any(test, feature = "nested-modport-test-utils"))]
            legacy_target: target.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(
        target: NestedModportLoweringKey,
        ordinal: usize,
        lowering: Arc<NestedModportLowering>,
        terminal: ResolvedNestedTerminalId,
    ) -> Self {
        Self::new(
            &target,
            lowering,
            SemanticLoweringIdentity::fixture(target.session, ordinal),
            terminal,
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedPathRewrite {
    pub terminal: ResolvedTerminalRef,
    pub semantic_segments: Vec<StrId>,
    pub replace_from_segment: usize,
    pub consumed_segments: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingPathRewriteCandidate {
    pub target: NestedModportLoweringKey,
    pub semantic_segments: Vec<StrId>,
    pub replace_from_segment: usize,
    pub consumed_segments: usize,
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub struct ExpandedPortKey {
    pub owner: NestedModportLoweringKey,
    pub token: TokenId,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ExpandedPortResolution {
    DirectLegacy,
    Nested {
        target: NestedModportLoweringKey,
        modport: StrId,
        interface: ResolvedExpandedPortInterface,
        members: std::sync::Arc<ResolvedExpandedMemberSet>,
    },
}

#[derive(Clone, Debug)]
pub struct ResolvedExpandedPortInterface {
    pub symbol: SymbolId,
    pub generic_map: GenericMap,
}

impl PartialEq for ResolvedExpandedPortInterface {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && SemanticGenericMap::from(&self.generic_map)
                == SemanticGenericMap::from(&other.generic_map)
    }
}

impl Eq for ResolvedExpandedPortInterface {}

impl Hash for ResolvedExpandedPortInterface {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
        SemanticGenericMap::from(&self.generic_map).hash(state);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingExpandedPortCandidate {
    pub target: NestedModportLoweringKey,
    pub modport: StrId,
}

#[derive(Clone, Debug, Ord, PartialOrd)]
pub struct InstantiationContextKey {
    pub owner: NestedModportLoweringKey,
    pub token: TokenId,
}

macro_rules! counted_owner_token_key {
    ($key:ty, $($extra:ident),* $(,)?) => {
        impl PartialEq for $key {
            fn eq(&self, other: &Self) -> bool {
                record_semantic_work(1);
                self.owner == other.owner $(&& self.$extra == other.$extra)*
            }
        }

        impl Eq for $key {}

        impl Hash for $key {
            fn hash<H: Hasher>(&self, state: &mut H) {
                record_semantic_work(1);
                self.owner.hash(state);
                $(self.$extra.hash(state);)*
            }
        }
    };
}

counted_owner_token_key!(OccurrenceRewriteKey, kind, token);
counted_owner_token_key!(ExpandedPortKey, token);
counted_owner_token_key!(InstantiationContextKey, token);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingInstantiationContextCandidate {
    pub target: NestedModportLoweringKey,
}
