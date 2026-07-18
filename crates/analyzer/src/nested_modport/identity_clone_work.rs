use super::NestedModportLoweringKey;
use super::{ComponentSpecializationIdentity, ConnectedInterfaceSpecialization};
use crate::ir::{Signature, ValueVariant};
use crate::symbol_path::{GenericSymbol, GenericSymbolPath};
use crate::value::Value;
use std::cell::Cell;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct KeyCloneWork {
    pub events: usize,
    pub recursive_nodes: usize,
    pub allocations: usize,
}

thread_local! {
    static WORK: Cell<KeyCloneWork> = const { Cell::new(KeyCloneWork {
        events: 0,
        recursive_nodes: 0,
        allocations: 0,
    }) };
}

fn record(work: KeyCloneWork) {
    WORK.with(|slot| {
        let current = slot.get();
        slot.set(KeyCloneWork {
            events: current.events.saturating_add(work.events),
            recursive_nodes: current.recursive_nodes.saturating_add(work.recursive_nodes),
            allocations: current.allocations.saturating_add(work.allocations),
        });
    });
}

pub(super) fn record_shared_key_clone() {
    record(KeyCloneWork {
        events: 1,
        recursive_nodes: 1,
        allocations: 0,
    });
}

pub(super) fn reset_key_clone_work() {
    WORK.set(KeyCloneWork::default());
}

pub(super) fn key_clone_work() -> KeyCloneWork {
    WORK.get()
}

pub(super) fn deep_clone_key(key: &NestedModportLoweringKey) -> NestedModportLoweringKey {
    let (specialization, recursive_nodes, allocations) = clone_specialization(&key.specialization);
    record(KeyCloneWork {
        events: 1,
        recursive_nodes,
        allocations: allocations.saturating_add(1),
    });
    NestedModportLoweringKey {
        session: key.session,
        specialization: Arc::new(specialization),
    }
}

fn clone_specialization(
    specialization: &ComponentSpecializationIdentity,
) -> (ComponentSpecializationIdentity, usize, usize) {
    let (owner, mut nodes, mut allocations) = clone_signature(&specialization.owner);
    let mut connected_actuals = Vec::with_capacity(specialization.connected_actuals.len());
    allocations += usize::from(!specialization.connected_actuals.is_empty());
    nodes += 1;
    for connected in &specialization.connected_actuals {
        let (actual, actual_nodes, actual_allocations) = clone_signature(&connected.actual);
        connected_actuals.push(ConnectedInterfaceSpecialization {
            formal_port: connected.formal_port,
            actual,
        });
        nodes += actual_nodes + 1;
        allocations += actual_allocations;
    }
    (
        ComponentSpecializationIdentity {
            owner,
            connected_actuals,
        },
        nodes,
        allocations,
    )
}

fn clone_signature(signature: &Signature) -> (Signature, usize, usize) {
    let full_path = signature.full_path.clone();
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    let mut generic_parameters = Vec::with_capacity(signature.generic_parameters.len());
    let mut nodes = 1;
    let mut allocations = usize::from(!full_path.is_empty())
        + usize::from(!signature.parameters.is_empty())
        + usize::from(!signature.generic_parameters.is_empty());
    for (name, value) in &signature.parameters {
        let (value, value_nodes, value_allocations) = clone_value(value);
        parameters.push((*name, value));
        nodes += value_nodes;
        allocations += value_allocations;
    }
    for (name, path) in &signature.generic_parameters {
        let (path, path_nodes, path_allocations) = clone_path(path);
        generic_parameters.push((*name, path));
        nodes += path_nodes;
        allocations += path_allocations;
    }
    (
        Signature {
            symbol: signature.symbol,
            full_path,
            parameters,
            generic_parameters,
        },
        nodes,
        allocations,
    )
}

fn clone_value(value: &ValueVariant) -> (ValueVariant, usize, usize) {
    match value {
        ValueVariant::Numeric(value) => (
            ValueVariant::Numeric(value.clone()),
            1,
            value_allocations(value),
        ),
        ValueVariant::NumericArray(values) => (
            ValueVariant::NumericArray(values.clone()),
            values.len() + 1,
            usize::from(!values.is_empty()) + values.iter().map(value_allocations).sum::<usize>(),
        ),
        ValueVariant::Type(value) => {
            let (value, nodes, allocations) = value.clone_with_nested_modport_work();
            (ValueVariant::Type(value), nodes, allocations)
        }
        ValueVariant::Unknown => (ValueVariant::Unknown, 1, 0),
    }
}

fn value_allocations(value: &Value) -> usize {
    match value {
        Value::U64(_) => 0,
        Value::BigUint(_) => 2,
    }
}

fn clone_path(path: &GenericSymbolPath) -> (GenericSymbolPath, usize, usize) {
    let mut paths = Vec::with_capacity(path.paths.len());
    let mut nodes = 1;
    let mut allocations = usize::from(!path.paths.is_empty());
    for segment in &path.paths {
        let mut arguments = Vec::with_capacity(segment.arguments.len());
        allocations += usize::from(!segment.arguments.is_empty());
        nodes += 1;
        for argument in &segment.arguments {
            let (argument, argument_nodes, argument_allocations) = clone_path(argument);
            arguments.push(argument);
            nodes += argument_nodes;
            allocations += argument_allocations;
        }
        paths.push(GenericSymbol {
            base: segment.base,
            arguments,
        });
    }
    (
        GenericSymbolPath {
            paths,
            kind: path.kind.clone(),
            range: path.range,
        },
        nodes,
        allocations,
    )
}
