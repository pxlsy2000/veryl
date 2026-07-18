#[cfg(test)]
thread_local! {
    static DEFAULT_EXPANSION_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MEMBER_RESCAN_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) fn reset_default_expansion_work() {
    DEFAULT_EXPANSION_WORK.set(0);
}

#[cfg(test)]
pub(super) fn default_expansion_work() -> usize {
    DEFAULT_EXPANSION_WORK.get()
}

pub(super) fn record_default_expansion_work(units: usize) {
    #[cfg(test)]
    DEFAULT_EXPANSION_WORK.with(|work| work.set(work.get().saturating_add(units)));
    #[cfg(not(test))]
    let _ = units;
}

#[cfg(test)]
pub(super) struct MemberRescanMutationGuard;

#[cfg(test)]
impl Drop for MemberRescanMutationGuard {
    fn drop(&mut self) {
        MEMBER_RESCAN_MUTATION.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_member_rescan_mutation() -> MemberRescanMutationGuard {
    MEMBER_RESCAN_MUTATION.set(true);
    MemberRescanMutationGuard
}

#[cfg(test)]
pub(super) fn member_rescan_mutation_enabled() -> bool {
    MEMBER_RESCAN_MUTATION.get()
}

#[cfg(not(test))]
pub(super) const fn member_rescan_mutation_enabled() -> bool {
    false
}
