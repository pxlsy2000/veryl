use super::identity::*;
use super::semantic_path_operations::{generic_kind_eq, hash_generic_kind};
use super::semantic_type_operations::{hash_value_variant, value_variant_eq};
use super::semantic_work::{counted_sort_by, hash_slice, record_semantic_work, slice_eq};
use crate::ir::Signature;
use crate::symbol_path::{GenericSymbolPath, GenericSymbolPathKind};
use std::hash::{Hash, Hasher};

impl PartialEq for SemanticGenericMap {
    fn eq(&self, other: &Self) -> bool {
        slice_eq(&self.0, &other.0)
    }
}

impl Eq for SemanticGenericMap {}

impl Hash for SemanticGenericMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_slice(&self.0, state);
    }
}

impl PartialEq for SemanticGenericPath {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        generic_kind_eq(&self.kind, &other.kind) && slice_eq(&self.segments, &other.segments)
    }
}

impl Eq for SemanticGenericPath {}

impl Hash for SemanticGenericPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        hash_generic_kind(&self.kind, state);
        hash_slice(&self.segments, state);
    }
}

impl PartialEq for SemanticGenericSegment {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.name == other.name && slice_eq(&self.arguments, &other.arguments)
    }
}

impl Eq for SemanticGenericSegment {}

impl Hash for SemanticGenericSegment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.name.hash(state);
        hash_slice(&self.arguments, state);
    }
}

impl PartialEq for ConnectedInterfaceSpecialization {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.formal_port == other.formal_port && signature_eq(&self.actual, &other.actual)
    }
}

impl Eq for ConnectedInterfaceSpecialization {}

impl Hash for ConnectedInterfaceSpecialization {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.formal_port.hash(state);
        hash_signature(&self.actual, state);
    }
}

impl PartialEq for ComponentSpecializationIdentity {
    fn eq(&self, other: &Self) -> bool {
        signature_eq(&self.owner, &other.owner)
            && slice_eq(&self.connected_actuals, &other.connected_actuals)
    }
}

impl Eq for ComponentSpecializationIdentity {}

impl Hash for ComponentSpecializationIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_signature(&self.owner, state);
        hash_slice(&self.connected_actuals, state);
    }
}

impl PartialEq for NestedModportLoweringKey {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.session == other.session && self.specialization == other.specialization
    }
}

impl Eq for NestedModportLoweringKey {}

impl Hash for NestedModportLoweringKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.session.hash(state);
        self.specialization.hash(state);
    }
}

pub(super) fn normalize_signature(signature: &mut Signature) {
    counted_sort_by(&mut signature.parameters, |left, right| {
        left.0.cmp(&right.0)
    });
    counted_sort_by(&mut signature.generic_parameters, |left, right| {
        left.0.cmp(&right.0)
    });
}

pub(super) fn signature_eq(left: &Signature, right: &Signature) -> bool {
    record_semantic_work(1);
    left.symbol == right.symbol
        && slice_eq(&left.full_path, &right.full_path)
        && parameters_eq(&left.parameters, &right.parameters)
        && generic_parameters_eq(&left.generic_parameters, &right.generic_parameters)
}

fn parameters_eq(
    left: &[(veryl_parser::resource_table::StrId, crate::ir::ValueVariant)],
    right: &[(veryl_parser::resource_table::StrId, crate::ir::ValueVariant)],
) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left.iter().zip(right).all(|((ln, lv), (rn, rv))| {
            record_semantic_work(1);
            ln == rn && value_variant_eq(lv, rv)
        })
}

fn generic_parameters_eq(
    left: &[(veryl_parser::resource_table::StrId, GenericSymbolPath)],
    right: &[(veryl_parser::resource_table::StrId, GenericSymbolPath)],
) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((left_name, left), (right_name, right))| {
                record_semantic_work(1);
                left_name == right_name && generic_path_eq(left, right)
            })
}

fn generic_path_eq(left: &GenericSymbolPath, right: &GenericSymbolPath) -> bool {
    record_semantic_work(1);
    semantic_kind_eq(&left.kind, &right.kind)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            record_semantic_work(1);
            left.base.text == right.base.text
                && left.arguments.len() == right.arguments.len()
                && left
                    .arguments
                    .iter()
                    .zip(&right.arguments)
                    .all(|(left, right)| generic_path_eq(left, right))
        })
}

pub(super) fn hash_signature<H: Hasher>(signature: &Signature, state: &mut H) {
    record_semantic_work(1);
    signature.symbol.hash(state);
    hash_slice(&signature.full_path, state);
    record_semantic_work(1);
    signature.parameters.len().hash(state);
    for (name, value) in &signature.parameters {
        record_semantic_work(1);
        name.hash(state);
        hash_value_variant(value, state);
    }
    signature.generic_parameters.len().hash(state);
    for (name, path) in &signature.generic_parameters {
        record_semantic_work(1);
        name.hash(state);
        hash_generic_path(path, state);
    }
}

fn hash_generic_path<H: Hasher>(path: &GenericSymbolPath, state: &mut H) {
    record_semantic_work(1);
    hash_semantic_kind(&path.kind, state);
    path.paths.len().hash(state);
    for segment in &path.paths {
        record_semantic_work(1);
        segment.base.text.hash(state);
        segment.arguments.len().hash(state);
        for argument in &segment.arguments {
            hash_generic_path(argument, state);
        }
    }
}

fn semantic_kind_eq(left: &GenericSymbolPathKind, right: &GenericSymbolPathKind) -> bool {
    record_semantic_work(1);
    match (left, right) {
        (GenericSymbolPathKind::Identifier, GenericSymbolPathKind::Identifier)
        | (GenericSymbolPathKind::TypeLiteral, GenericSymbolPathKind::TypeLiteral)
        | (GenericSymbolPathKind::ValueLiteral, GenericSymbolPathKind::ValueLiteral) => true,
        (GenericSymbolPathKind::VariableType(left), GenericSymbolPathKind::VariableType(right)) => {
            slice_eq(left, right)
        }
        _ => false,
    }
}

fn hash_semantic_kind<H: Hasher>(kind: &GenericSymbolPathKind, state: &mut H) {
    record_semantic_work(1);
    match kind {
        GenericSymbolPathKind::Identifier => 0_u8.hash(state),
        GenericSymbolPathKind::TypeLiteral => 1_u8.hash(state),
        GenericSymbolPathKind::VariableType(dimensions) => {
            2_u8.hash(state);
            hash_slice(dimensions, state);
        }
        GenericSymbolPathKind::ValueLiteral => 3_u8.hash(state),
    }
}
