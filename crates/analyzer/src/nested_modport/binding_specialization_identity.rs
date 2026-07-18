use super::identity::{AnalysisSessionId, NestedModportLoweringKey};
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct BindingSpecializationIdentity {
    session: AnalysisSessionId,
    ordinal: usize,
}

#[derive(Debug)]
pub(super) struct BindingSpecializationInterner<S = std::hash::RandomState> {
    identities: HashMap<Arc<NestedModportLoweringKey>, BindingSpecializationIdentity, S>,
    next_ordinal: usize,
}

impl Default for BindingSpecializationInterner {
    fn default() -> Self {
        Self {
            identities: HashMap::default(),
            next_ordinal: 0,
        }
    }
}

impl<S: BuildHasher> BindingSpecializationInterner<S> {
    #[cfg(test)]
    pub(super) fn with_hasher(hasher: S) -> Self {
        Self {
            identities: HashMap::with_hasher(hasher),
            next_ordinal: 0,
        }
    }

    pub(super) fn intern(
        &mut self,
        key: Arc<NestedModportLoweringKey>,
    ) -> (Arc<NestedModportLoweringKey>, BindingSpecializationIdentity) {
        record_registration_work(1);
        clone_at_registration_seam_if_injected(&key);
        if let Some((canonical, identity)) = self.identities.get_key_value(key.as_ref()) {
            let canonical = Arc::clone(canonical);
            return (canonical, *identity);
        }
        let identity = BindingSpecializationIdentity {
            session: key.session,
            ordinal: self.next_ordinal,
        };
        self.next_ordinal += 1;
        self.identities.insert(Arc::clone(&key), identity);
        record_registration_work(1);
        (key, identity)
    }

    pub(super) fn identity(
        &self,
        key: &NestedModportLoweringKey,
    ) -> Option<BindingSpecializationIdentity> {
        self.identities.get(key).copied()
    }
}

#[cfg(test)]
thread_local! {
    static REGISTRATION_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DEEP_CLONE_REGISTRATION_KEY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn record_registration_work(units: usize) {
    REGISTRATION_WORK.with(|work| work.set(work.get().saturating_add(units)));
}

#[cfg(not(test))]
fn record_registration_work(_units: usize) {}

#[cfg(test)]
pub(super) fn reset_registration_work() {
    REGISTRATION_WORK.set(0);
}

#[cfg(test)]
pub(super) fn registration_work() -> usize {
    REGISTRATION_WORK.get()
}

#[cfg(test)]
pub(super) struct DeepCloneRegistrationKeyGuard;

#[cfg(test)]
impl Drop for DeepCloneRegistrationKeyGuard {
    fn drop(&mut self) {
        DEEP_CLONE_REGISTRATION_KEY.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_deep_clone_registration_key() -> DeepCloneRegistrationKeyGuard {
    DEEP_CLONE_REGISTRATION_KEY.set(true);
    DeepCloneRegistrationKeyGuard
}

#[cfg(test)]
fn clone_at_registration_seam_if_injected(key: &NestedModportLoweringKey) {
    if !DEEP_CLONE_REGISTRATION_KEY.get() {
        return;
    }
    let copy = super::identity_clone_work::deep_clone_key(key);
    std::hint::black_box(copy);
}

#[cfg(not(test))]
fn clone_at_registration_seam_if_injected(_key: &NestedModportLoweringKey) {}

#[cfg(test)]
impl BindingSpecializationIdentity {
    pub(super) const fn fixture(session: AnalysisSessionId, ordinal: usize) -> Self {
        Self { session, ordinal }
    }
}
