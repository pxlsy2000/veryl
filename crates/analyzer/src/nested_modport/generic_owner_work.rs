#[cfg(test)]
thread_local! {
    static OWNER_MEMBERSHIP_SCAN_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) struct OwnerMembershipScanMutationGuard;

#[cfg(test)]
impl Drop for OwnerMembershipScanMutationGuard {
    fn drop(&mut self) {
        OWNER_MEMBERSHIP_SCAN_MUTATION.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_owner_membership_scan_mutation() -> OwnerMembershipScanMutationGuard {
    OWNER_MEMBERSHIP_SCAN_MUTATION.set(true);
    OwnerMembershipScanMutationGuard
}

#[cfg(test)]
pub(super) fn owner_membership_scan_mutation_enabled() -> bool {
    OWNER_MEMBERSHIP_SCAN_MUTATION.get()
}

#[cfg(not(test))]
pub(super) const fn owner_membership_scan_mutation_enabled() -> bool {
    false
}
