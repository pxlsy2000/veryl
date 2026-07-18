use crate::HashMap;
use crate::ir::{Function, ModportMemberPath, VarId, Variable};
use std::hash::{Hash, Hasher};
use veryl_parser::resource_table::StrId;
use veryl_parser::token_range::TokenRange;

#[cfg(test)]
thread_local! {
    static TERMINAL_RESOLUTION_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FORCE_TERMINAL_RESOLUTION_SCAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn reset_terminal_resolution_work() {
    TERMINAL_RESOLUTION_WORK.set(0);
}

#[cfg(test)]
pub(crate) fn terminal_resolution_work() -> usize {
    TERMINAL_RESOLUTION_WORK.get()
}

#[cfg(test)]
pub(super) fn record_terminal_resolution_work(units: usize) {
    TERMINAL_RESOLUTION_WORK.with(|work| work.set(work.get().saturating_add(units)));
}

#[cfg(not(test))]
pub(super) fn record_terminal_resolution_work(_units: usize) {}

#[cfg(test)]
struct TerminalResolutionScanGuard(bool);

#[cfg(test)]
impl Drop for TerminalResolutionScanGuard {
    fn drop(&mut self) {
        FORCE_TERMINAL_RESOLUTION_SCAN.set(self.0);
    }
}

#[cfg(test)]
pub(crate) fn with_terminal_resolution_scan_mutation<T>(f: impl FnOnce() -> T) -> T {
    let previous = FORCE_TERMINAL_RESOLUTION_SCAN.replace(true);
    let guard = TerminalResolutionScanGuard(previous);
    let result = f();
    drop(guard);
    result
}

#[cfg(test)]
fn scan_mutation_enabled() -> bool {
    FORCE_TERMINAL_RESOLUTION_SCAN.get()
}

#[derive(Clone, Debug)]
struct SemanticTerminalPath(ModportMemberPath);

impl SemanticTerminalPath {
    fn new(path: &[StrId]) -> Self {
        Self(ModportMemberPath::from_slice(path))
    }
}

impl PartialEq for SemanticTerminalPath {
    fn eq(&self, other: &Self) -> bool {
        let left = self.0.as_slice();
        let right = other.0.as_slice();
        record_terminal_resolution_work(1);
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                record_terminal_resolution_work(1);
                left == right
            })
    }
}

impl Eq for SemanticTerminalPath {}

impl Hash for SemanticTerminalPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let path = self.0.as_slice();
        record_terminal_resolution_work(1 + path.len());
        path.hash(state);
    }
}

#[derive(Clone, Copy, Debug)]
struct SemanticTerminalName(StrId);

impl PartialEq for SemanticTerminalName {
    fn eq(&self, other: &Self) -> bool {
        record_terminal_resolution_work(1);
        self.0 == other.0
    }
}

impl Eq for SemanticTerminalName {}

impl Hash for SemanticTerminalName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_terminal_resolution_work(1);
        self.0.hash(state);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InstantiatedTerminalIndex {
    variables: HashMap<SemanticTerminalPath, Vec<VarId>>,
    variable_paths: HashMap<VarId, SemanticTerminalPath>,
    functions: HashMap<SemanticTerminalName, Vec<VarId>>,
    function_names: HashMap<VarId, SemanticTerminalName>,
}

impl InstantiatedTerminalIndex {
    pub(crate) fn from_records(
        variables: &HashMap<VarId, Variable>,
        functions: &HashMap<VarId, Function>,
    ) -> Self {
        let mut index = Self::default();
        for (id, variable) in variables {
            index.insert_variable(*id, variable);
        }
        for (id, function) in functions {
            index.insert_function(*id, function);
        }
        index
    }

    pub(crate) fn insert_variable(&mut self, id: VarId, variable: &Variable) {
        self.remove_variable(id);
        let path = SemanticTerminalPath::new(&variable.path.0);
        self.variables.entry(path.clone()).or_default().push(id);
        self.variable_paths.insert(id, path);
        record_terminal_resolution_work(2);
    }

    pub(crate) fn remove_variable(&mut self, id: VarId) {
        let Some(path) = self.variable_paths.remove(&id) else {
            return;
        };
        let empty = self.variables.get_mut(&path).is_some_and(|ids| {
            ids.retain(|candidate| *candidate != id);
            ids.is_empty()
        });
        if empty {
            self.variables.remove(&path);
        }
        record_terminal_resolution_work(1);
    }

    pub(crate) fn insert_function(&mut self, id: VarId, function: &Function) {
        self.remove_function(id);
        let name = SemanticTerminalName(function.name);
        self.functions.entry(name).or_default().push(id);
        self.function_names.insert(id, name);
        record_terminal_resolution_work(2);
    }

    pub(crate) fn remove_function(&mut self, id: VarId) {
        let Some(name) = self.function_names.remove(&id) else {
            return;
        };
        let empty = self.functions.get_mut(&name).is_some_and(|ids| {
            ids.retain(|candidate| *candidate != id);
            ids.is_empty()
        });
        if empty {
            self.functions.remove(&name);
        }
        record_terminal_resolution_work(1);
    }

    pub(crate) fn retain_records(
        &mut self,
        variables: &HashMap<VarId, Variable>,
        functions: &HashMap<VarId, Function>,
    ) {
        let stale_variables: Vec<_> = self
            .variable_paths
            .keys()
            .filter(|id| !variables.contains_key(id))
            .copied()
            .collect();
        let stale_functions: Vec<_> = self
            .function_names
            .keys()
            .filter(|id| !functions.contains_key(id))
            .copied()
            .collect();
        for id in stale_variables {
            self.remove_variable(id);
        }
        for id in stale_functions {
            self.remove_function(id);
        }
    }

    pub(crate) fn variable<'a>(
        &self,
        path: &ModportMemberPath,
        variables: &'a HashMap<VarId, Variable>,
    ) -> Option<&'a Variable> {
        #[cfg(test)]
        if scan_mutation_enabled() {
            return variables.values().find(|variable| {
                record_terminal_resolution_work(1 + variable.path.0.len());
                variable.path.0.as_slice() == path.as_slice()
            });
        }
        record_terminal_resolution_work(1);
        self.variables
            .get(&SemanticTerminalPath::new(path.as_slice()))?
            .iter()
            .find_map(|id| variables.get(id))
    }

    pub(crate) fn function<'a>(
        &self,
        name: StrId,
        functions: &'a HashMap<VarId, Function>,
    ) -> Option<(VarId, &'a Function)> {
        #[cfg(test)]
        if scan_mutation_enabled() {
            return functions.iter().find_map(|(id, function)| {
                record_terminal_resolution_work(1);
                (function.name == name).then_some((*id, function))
            });
        }
        record_terminal_resolution_work(1);
        self.functions
            .get(&SemanticTerminalName(name))?
            .iter()
            .find_map(|id| functions.get(id).map(|function| (*id, function)))
    }

    pub(crate) fn terminal_site(
        &self,
        name: StrId,
        variables: &HashMap<VarId, Variable>,
        functions: &HashMap<VarId, Function>,
    ) -> Option<TokenRange> {
        let path = ModportMemberPath::from_slice(&[name]);
        self.variable(&path, variables)
            .map(|variable| variable.token)
            .or_else(|| {
                self.function(name, functions)
                    .map(|(_, function)| function.token)
            })
    }
}
