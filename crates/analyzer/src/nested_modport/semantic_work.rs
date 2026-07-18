#[cfg(any(test, feature = "nested-modport-test-utils"))]
thread_local! {
    static SEMANTIC_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SEMANTIC_WORK_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    #[cfg(feature = "nested-modport-test-utils")]
    static EMISSION_QUERY_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_semantic_work() {
    SEMANTIC_WORK.set(0);
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(crate) fn semantic_work() -> usize {
    SEMANTIC_WORK.get()
}

#[cfg(feature = "nested-modport-test-utils")]
pub(crate) fn reset_emission_query_work() {
    EMISSION_QUERY_WORK.set(0);
}

#[cfg(feature = "nested-modport-test-utils")]
pub(crate) fn emission_query_work() -> usize {
    EMISSION_QUERY_WORK.get()
}

#[cfg(feature = "nested-modport-test-utils")]
pub(super) fn record_emission_query_work(units: usize) {
    EMISSION_QUERY_WORK.with(|count| count.set(count.get().saturating_add(units)));
}

#[cfg(not(feature = "nested-modport-test-utils"))]
pub(super) fn record_emission_query_work(_units: usize) {}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) fn record_semantic_work(units: usize) {
    if SEMANTIC_WORK_ENABLED.get() {
        SEMANTIC_WORK.with(|count| count.set(count.get().saturating_add(units)));
    }
}

#[cfg(test)]
pub(super) fn without_semantic_work<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SEMANTIC_WORK_ENABLED.set(self.0);
        }
    }
    let restore = Restore(SEMANTIC_WORK_ENABLED.replace(false));
    let result = operation();
    drop(restore);
    result
}

#[cfg(not(any(test, feature = "nested-modport-test-utils")))]
pub(super) fn record_semantic_work(_units: usize) {}

pub(super) fn hash_slice<T: std::hash::Hash, H: std::hash::Hasher>(values: &[T], state: &mut H) {
    use std::hash::Hash;
    record_semantic_work(1);
    values.len().hash(state);
    for value in values {
        record_semantic_work(1);
        value.hash(state);
    }
}

pub(super) fn slice_eq<T: PartialEq>(left: &[T], right: &[T]) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            record_semantic_work(1);
            left == right
        })
}

pub(super) fn counted_sort_by<T>(
    values: &mut [T],
    mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
) {
    values.sort_by(|left, right| {
        record_semantic_work(1);
        compare(left, right)
    });
}
