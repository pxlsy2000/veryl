#[cfg(test)]
thread_local! {
    static COLLECTION_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_collection_work() {
    COLLECTION_WORK.set(0);
}

#[cfg(test)]
pub(super) fn collection_work() -> usize {
    COLLECTION_WORK.get()
}

#[cfg(test)]
pub(super) fn record_collection_work(units: usize) {
    COLLECTION_WORK.with(|count| count.set(count.get().saturating_add(units)));
}

#[cfg(not(test))]
pub(super) fn record_collection_work(_units: usize) {}

pub(super) struct Counted<I>(I);

impl<I: Iterator> Iterator for Counted<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.0.next();
        record_collection_work(usize::from(value.is_some()));
        value
    }
}

pub(super) fn counted<I: IntoIterator>(values: I) -> Counted<I::IntoIter> {
    Counted(values.into_iter())
}
