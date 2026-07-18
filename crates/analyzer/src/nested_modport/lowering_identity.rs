use super::identity::AnalysisSessionId;
use super::lowering_records::NestedModportLowering;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct SemanticLoweringIdentity {
    session: AnalysisSessionId,
    ordinal: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum LoweringEquivalenceIdentity {
    NotNested,
    Found(SemanticLoweringIdentity),
}

pub(super) struct SessionLoweringInterner<S = std::hash::RandomState> {
    identities: HashMap<Arc<NestedModportLowering>, SemanticLoweringIdentity, S>,
    next_ordinal: usize,
}

impl Default for SessionLoweringInterner {
    fn default() -> Self {
        Self {
            identities: HashMap::default(),
            next_ordinal: 0,
        }
    }
}

impl<S: BuildHasher> SessionLoweringInterner<S> {
    #[cfg(test)]
    pub(super) fn with_hasher(hasher: S) -> Self {
        Self {
            identities: HashMap::with_hasher(hasher),
            next_ordinal: 0,
        }
    }

    pub(super) fn intern(
        &mut self,
        session: AnalysisSessionId,
        lowering: Arc<NestedModportLowering>,
    ) -> SemanticLoweringIdentity {
        if let Some(identity) = self.identities.get(&lowering) {
            return *identity;
        }
        let identity = SemanticLoweringIdentity {
            session,
            ordinal: self.next_ordinal,
        };
        self.next_ordinal += 1;
        self.identities.insert(lowering, identity);
        identity
    }
}

pub(super) const fn rehash_on_identity_query() -> bool {
    false
}

#[cfg(test)]
impl SemanticLoweringIdentity {
    pub(super) const fn fixture(session: AnalysisSessionId, ordinal: usize) -> Self {
        Self { session, ordinal }
    }
}
