use super::lowering_records::PendingModportEntry;
use crate::HashMap;
use crate::ir::ModportMemberPath;
use std::cell::Cell;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub(super) struct EffectiveMemberSet {
    entries: Vec<PendingModportEntry>,
    index_by_path: HashMap<ModportMemberPath, usize>,
}

impl EffectiveMemberSet {
    pub(super) fn merge_explicit(&mut self, incoming: PendingModportEntry) {
        record_effective_member_membership_operation();
        match self.index_by_path.entry(incoming.path.clone()) {
            Entry::Occupied(entry) => self.entries[*entry.get()] = incoming,
            Entry::Vacant(entry) => {
                entry.insert(self.entries.len());
                self.entries.push(incoming);
            }
        }
    }

    pub(super) fn merge_default(&mut self, incoming: PendingModportEntry) {
        record_effective_member_membership_operation();
        if let Entry::Vacant(entry) = self.index_by_path.entry(incoming.path.clone()) {
            entry.insert(self.entries.len());
            self.entries.push(incoming);
        }
    }

    pub(super) fn into_entries(self) -> Vec<PendingModportEntry> {
        self.entries
    }
}

#[cfg(test)]
pub(super) fn reset_effective_member_membership_operations() {
    EFFECTIVE_MEMBER_MEMBERSHIP_OPERATIONS.set(0);
}

#[cfg(test)]
pub(super) fn effective_member_membership_operations() -> usize {
    EFFECTIVE_MEMBER_MEMBERSHIP_OPERATIONS.get()
}

thread_local! {
    static EFFECTIVE_MEMBER_MEMBERSHIP_OPERATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record_effective_member_membership_operation() {
    #[cfg(test)]
    EFFECTIVE_MEMBER_MEMBERSHIP_OPERATIONS.set(
        EFFECTIVE_MEMBER_MEMBERSHIP_OPERATIONS
            .get()
            .saturating_add(1),
    );
}
