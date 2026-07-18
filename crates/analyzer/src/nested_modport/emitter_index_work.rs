#[cfg(any(test, feature = "nested-modport-test-utils"))]
thread_local! {
    static WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CONNECTED_SCAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static MEMBER_SCAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FORCE_HASH_COLLISION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static RESOLVED_MEMBER_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) fn record_emitter_index_work(units: usize) {
    WORK.with(|work| work.set(work.get().saturating_add(units)));
}

#[cfg(not(any(test, feature = "nested-modport-test-utils")))]
pub(super) const fn record_emitter_index_work(_units: usize) {}

#[cfg(feature = "nested-modport-test-utils")]
pub fn record_external_emitter_index_work(units: usize) {
    record_emitter_index_work(units);
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn reset_emitter_index_work() {
    WORK.set(0);
    RESOLVED_MEMBER_LOOKUPS.set(0);
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn emitter_index_work() -> usize {
    WORK.get()
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn record_resolved_member_lookup() {
    RESOLVED_MEMBER_LOOKUPS.set(RESOLVED_MEMBER_LOOKUPS.get().saturating_add(1));
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn resolved_member_lookups() -> usize {
    RESOLVED_MEMBER_LOOKUPS.get()
}

#[cfg(feature = "nested-modport-test-utils")]
struct MutationGuard {
    target: &'static std::thread::LocalKey<std::cell::Cell<bool>>,
}

#[cfg(feature = "nested-modport-test-utils")]
impl Drop for MutationGuard {
    fn drop(&mut self) {
        self.target.set(false);
    }
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn with_connected_map_scan_mutation<T>(operation: impl FnOnce() -> T) -> T {
    CONNECTED_SCAN.set(true);
    let guard = MutationGuard {
        target: &CONNECTED_SCAN,
    };
    let result = operation();
    drop(guard);
    result
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn with_expanded_member_scan_mutation<T>(operation: impl FnOnce() -> T) -> T {
    MEMBER_SCAN.set(true);
    let guard = MutationGuard {
        target: &MEMBER_SCAN,
    };
    let result = operation();
    drop(guard);
    result
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn with_emitter_index_hash_collision<T>(operation: impl FnOnce() -> T) -> T {
    FORCE_HASH_COLLISION.set(true);
    let guard = MutationGuard {
        target: &FORCE_HASH_COLLISION,
    };
    let result = operation();
    drop(guard);
    result
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) fn connected_map_scan_mutation_enabled() -> bool {
    CONNECTED_SCAN.get()
}

#[cfg(not(any(test, feature = "nested-modport-test-utils")))]
pub(super) const fn connected_map_scan_mutation_enabled() -> bool {
    false
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub fn expanded_member_scan_mutation_enabled() -> bool {
    MEMBER_SCAN.get()
}

#[cfg(not(any(test, feature = "nested-modport-test-utils")))]
pub const fn expanded_member_scan_mutation_enabled() -> bool {
    false
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) fn emitter_index_hash_collision_forced() -> bool {
    FORCE_HASH_COLLISION.get()
}

#[cfg(not(any(test, feature = "nested-modport-test-utils")))]
pub(super) const fn emitter_index_hash_collision_forced() -> bool {
    false
}
