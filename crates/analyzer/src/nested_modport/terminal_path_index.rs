use super::collection_work::{counted, record_collection_work};
use super::lowering_records::ResolvedNestedTerminal;
use crate::HashMap;
use veryl_parser::resource_table::StrId;

#[derive(Clone, Debug, Default)]
struct TerminalPathNode {
    terminal_index: Option<usize>,
    children: HashMap<StrId, TerminalPathNode>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ResolvedNestedTerminalPathIndex {
    root: TerminalPathNode,
}

impl ResolvedNestedTerminalPathIndex {
    pub(super) fn build(terminals: &[ResolvedNestedTerminal]) -> Self {
        let mut index = Self::default();
        for (terminal_index, terminal) in counted(terminals).enumerate() {
            let mut node = &mut index.root;
            for segment in counted(&terminal.identifier.source_segments) {
                node = node.children.entry(*segment).or_default();
            }
            node.terminal_index = Some(terminal_index);
        }
        index
    }

    pub(super) fn longest_prefix(&self, segments: &[StrId]) -> Option<usize> {
        let mut node = &self.root;
        let mut terminal_index = node.terminal_index;
        for segment in counted(segments) {
            let Some(child) = node.children.get(segment) else {
                break;
            };
            record_collection_work(1);
            node = child;
            terminal_index = node.terminal_index.or(terminal_index);
        }
        terminal_index
    }

    pub(super) fn removes_root(&self, root: StrId) -> bool {
        record_collection_work(1);
        self.root.children.contains_key(&root)
    }
}

#[cfg(test)]
thread_local! {
    static TERMINAL_SCAN_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) fn terminal_scan_mutation_enabled() -> bool {
    TERMINAL_SCAN_MUTATION.get()
}

#[cfg(not(test))]
pub(super) const fn terminal_scan_mutation_enabled() -> bool {
    false
}

#[cfg(test)]
pub(crate) fn with_terminal_scan_mutation<T>(f: impl FnOnce() -> T) -> T {
    struct ResetMutation(bool);

    impl Drop for ResetMutation {
        fn drop(&mut self) {
            TERMINAL_SCAN_MUTATION.set(self.0);
        }
    }

    let previous = TERMINAL_SCAN_MUTATION.replace(true);
    let _reset = ResetMutation(previous);
    f()
}
