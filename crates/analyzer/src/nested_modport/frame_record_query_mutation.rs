thread_local! {
    #[cfg(test)]
    static REQUIRED_RECORD_SCAN_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    #[cfg(test)]
    static OWNER_RECORD_LOOKUP_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TERMINAL_AVAILABILITY_LOOKUP_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static INSTANTIATION_OWNER_LOOKUP_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) struct RequiredRecordScanMutationGuard;

#[cfg(test)]
impl Drop for RequiredRecordScanMutationGuard {
    fn drop(&mut self) {
        REQUIRED_RECORD_SCAN_MUTATION.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_required_record_scan_mutation() -> RequiredRecordScanMutationGuard {
    REQUIRED_RECORD_SCAN_MUTATION.set(true);
    RequiredRecordScanMutationGuard
}

#[cfg(test)]
pub(super) fn required_record_scan_mutation_enabled() -> bool {
    REQUIRED_RECORD_SCAN_MUTATION.get()
}

#[cfg(test)]
pub(super) struct OwnerRecordLookupMutationGuard;

#[cfg(test)]
impl Drop for OwnerRecordLookupMutationGuard {
    fn drop(&mut self) {
        OWNER_RECORD_LOOKUP_MUTATION.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_owner_record_lookup_mutation() -> OwnerRecordLookupMutationGuard {
    OWNER_RECORD_LOOKUP_MUTATION.set(true);
    OwnerRecordLookupMutationGuard
}

#[cfg(test)]
pub(super) fn owner_record_lookup_mutation_enabled() -> bool {
    OWNER_RECORD_LOOKUP_MUTATION.get()
}

pub(super) struct TerminalAvailabilityLookupMutationGuard;

impl Drop for TerminalAvailabilityLookupMutationGuard {
    fn drop(&mut self) {
        TERMINAL_AVAILABILITY_LOOKUP_MUTATION.set(false);
    }
}

pub(super) fn inject_terminal_availability_lookup_mutation()
-> TerminalAvailabilityLookupMutationGuard {
    TERMINAL_AVAILABILITY_LOOKUP_MUTATION.set(true);
    TerminalAvailabilityLookupMutationGuard
}

pub(super) fn terminal_availability_lookup_mutation_enabled() -> bool {
    TERMINAL_AVAILABILITY_LOOKUP_MUTATION.get()
}

pub(super) struct InstantiationOwnerLookupMutationGuard;

impl Drop for InstantiationOwnerLookupMutationGuard {
    fn drop(&mut self) {
        INSTANTIATION_OWNER_LOOKUP_MUTATION.set(false);
    }
}

pub(super) fn inject_instantiation_owner_lookup_mutation() -> InstantiationOwnerLookupMutationGuard
{
    INSTANTIATION_OWNER_LOOKUP_MUTATION.set(true);
    InstantiationOwnerLookupMutationGuard
}

pub(super) fn instantiation_owner_lookup_mutation_enabled() -> bool {
    INSTANTIATION_OWNER_LOOKUP_MUTATION.get()
}
