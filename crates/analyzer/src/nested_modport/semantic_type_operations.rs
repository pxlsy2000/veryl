use super::identity_semantics::{hash_signature, signature_eq};
use super::semantic_path_operations::*;
use super::semantic_work::record_semantic_work;
use crate::ir::{InstanceKind, Type, TypeKind, TypeKindMember, ValueVariant};
use std::hash::{Hash, Hasher};

pub(super) fn type_eq(left: &Type, right: &Type) -> bool {
    record_semantic_work(1);
    type_kind_eq(&left.kind, &right.kind)
        && left.signed == right.signed
        && left.is_positive == right.is_positive
        && shape_eq(&left.array, &right.array)
        && shape_eq(left.width(), right.width())
        && width_exprs_eq(left.width_expr(), right.width_expr())
        && width_exprs_eq(left.array_expr(), right.array_expr())
        && named_resolution_eq(left, right)
}

pub(super) fn hash_type<H: Hasher>(value: &Type, state: &mut H) {
    record_semantic_work(1);
    hash_type_kind(&value.kind, state);
    value.signed.hash(state);
    value.is_positive.hash(state);
    hash_shape(&value.array, state);
    hash_shape(value.width(), state);
    hash_width_exprs(value.width_expr(), state);
    hash_width_exprs(value.array_expr(), state);
    match value.named_path() {
        Some(path) => {
            1_u8.hash(state);
            hash_generic_path(path, state);
            let context = value.named_generic_context();
            record_semantic_work(1);
            context.len().hash(state);
            for (name, path) in context {
                record_semantic_work(1);
                name.hash(state);
                hash_generic_path(path, state);
            }
        }
        None => 0_u8.hash(state),
    }
}

fn named_resolution_eq(left: &Type, right: &Type) -> bool {
    record_semantic_work(1);
    match (left.named_path(), right.named_path()) {
        (Some(left_path), Some(right_path)) => {
            generic_path_eq(left_path, right_path)
                && left.named_generic_context().len() == right.named_generic_context().len()
                && left
                    .named_generic_context()
                    .iter()
                    .zip(right.named_generic_context())
                    .all(|((left_name, left), (right_name, right))| {
                        record_semantic_work(1);
                        left_name == right_name && generic_path_eq(left, right)
                    })
        }
        (None, None) => true,
        _ => false,
    }
}

fn members_eq(left: &[TypeKindMember], right: &[TypeKindMember]) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            record_semantic_work(1);
            left.name == right.name && type_eq(&left.r#type, &right.r#type)
        })
}

fn hash_members<H: Hasher>(members: &[TypeKindMember], state: &mut H) {
    record_semantic_work(1);
    members.len().hash(state);
    for member in members {
        record_semantic_work(1);
        member.name.hash(state);
        hash_type(&member.r#type, state);
    }
}

fn type_kind_eq(left: &TypeKind, right: &TypeKind) -> bool {
    record_semantic_work(1);
    match (left, right) {
        (TypeKind::Clock, TypeKind::Clock)
        | (TypeKind::ClockPosedge, TypeKind::ClockPosedge)
        | (TypeKind::ClockNegedge, TypeKind::ClockNegedge)
        | (TypeKind::Reset, TypeKind::Reset)
        | (TypeKind::ResetAsyncHigh, TypeKind::ResetAsyncHigh)
        | (TypeKind::ResetAsyncLow, TypeKind::ResetAsyncLow)
        | (TypeKind::ResetSyncHigh, TypeKind::ResetSyncHigh)
        | (TypeKind::ResetSyncLow, TypeKind::ResetSyncLow)
        | (TypeKind::Bit, TypeKind::Bit)
        | (TypeKind::F32, TypeKind::F32)
        | (TypeKind::F64, TypeKind::F64)
        | (TypeKind::Logic, TypeKind::Logic)
        | (TypeKind::Type, TypeKind::Type)
        | (TypeKind::String, TypeKind::String)
        | (TypeKind::SystemVerilog, TypeKind::SystemVerilog)
        | (TypeKind::Void, TypeKind::Void)
        | (TypeKind::Unknown, TypeKind::Unknown) => true,
        (TypeKind::Struct(left), TypeKind::Struct(right)) => {
            left.id == right.id && members_eq(&left.members, &right.members)
        }
        (TypeKind::Union(left), TypeKind::Union(right)) => {
            left.id == right.id && members_eq(&left.members, &right.members)
        }
        (TypeKind::Enum(left), TypeKind::Enum(right)) => {
            left.id == right.id && type_eq(&left.r#type, &right.r#type)
        }
        (TypeKind::Module(left), TypeKind::Module(right))
        | (TypeKind::Interface(left), TypeKind::Interface(right))
        | (TypeKind::Package(left), TypeKind::Package(right)) => signature_eq(left, right),
        (TypeKind::Modport(ls, ln), TypeKind::Modport(rs, rn)) => signature_eq(ls, rs) && ln == rn,
        (TypeKind::Instance(ls, lk), TypeKind::Instance(rs, rk)) => {
            signature_eq(ls, rs) && instance_kind_eq(lk, rk)
        }
        (TypeKind::AbstractInterface(left), TypeKind::AbstractInterface(right)) => left == right,
        _ => false,
    }
}

fn type_kind_tag(kind: &TypeKind) -> u8 {
    match kind {
        TypeKind::Clock => 0,
        TypeKind::ClockPosedge => 1,
        TypeKind::ClockNegedge => 2,
        TypeKind::Reset => 3,
        TypeKind::ResetAsyncHigh => 4,
        TypeKind::ResetAsyncLow => 5,
        TypeKind::ResetSyncHigh => 6,
        TypeKind::ResetSyncLow => 7,
        TypeKind::Bit => 8,
        TypeKind::F32 => 9,
        TypeKind::F64 => 10,
        TypeKind::Logic => 11,
        TypeKind::Struct(_) => 12,
        TypeKind::Union(_) => 13,
        TypeKind::Enum(_) => 14,
        TypeKind::Module(_) => 15,
        TypeKind::Interface(_) => 16,
        TypeKind::Modport(_, _) => 17,
        TypeKind::Package(_) => 18,
        TypeKind::Instance(_, _) => 19,
        TypeKind::AbstractInterface(_) => 20,
        TypeKind::Type => 21,
        TypeKind::String => 22,
        TypeKind::SystemVerilog => 23,
        TypeKind::Void => 24,
        TypeKind::Unknown => 25,
    }
}

fn hash_type_kind<H: Hasher>(kind: &TypeKind, state: &mut H) {
    record_semantic_work(1);
    type_kind_tag(kind).hash(state);
    match kind {
        TypeKind::Struct(value) => {
            value.id.hash(state);
            hash_members(&value.members, state);
        }
        TypeKind::Union(value) => {
            value.id.hash(state);
            hash_members(&value.members, state);
        }
        TypeKind::Enum(value) => {
            value.id.hash(state);
            hash_type(&value.r#type, state);
        }
        TypeKind::Module(value) | TypeKind::Interface(value) | TypeKind::Package(value) => {
            hash_signature(value, state)
        }
        TypeKind::Modport(value, name) => {
            hash_signature(value, state);
            name.hash(state);
        }
        TypeKind::Instance(value, kind) => {
            hash_signature(value, state);
            hash_instance_kind(kind, state);
        }
        TypeKind::AbstractInterface(value) => value.hash(state),
        _ => {}
    }
}

fn instance_kind_eq(left: &InstanceKind, right: &InstanceKind) -> bool {
    record_semantic_work(1);
    matches!(
        (left, right),
        (InstanceKind::Module, InstanceKind::Module)
            | (InstanceKind::Interface, InstanceKind::Interface)
            | (InstanceKind::SystemVerilog, InstanceKind::SystemVerilog)
    )
}

fn hash_instance_kind<H: Hasher>(kind: &InstanceKind, state: &mut H) {
    record_semantic_work(1);
    match kind {
        InstanceKind::Module => 0_u8,
        InstanceKind::Interface => 1,
        InstanceKind::SystemVerilog => 2,
    }
    .hash(state);
}

pub(super) fn value_variant_eq(left: &ValueVariant, right: &ValueVariant) -> bool {
    record_semantic_work(1);
    match (left, right) {
        (ValueVariant::Numeric(left), ValueVariant::Numeric(right)) => left == right,
        (ValueVariant::NumericArray(left), ValueVariant::NumericArray(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    record_semantic_work(1);
                    left == right
                })
        }
        (ValueVariant::Type(left), ValueVariant::Type(right)) => type_eq(left, right),
        (ValueVariant::Unknown, ValueVariant::Unknown) => true,
        _ => false,
    }
}

pub(super) fn hash_value_variant<H: Hasher>(value: &ValueVariant, state: &mut H) {
    record_semantic_work(1);
    match value {
        ValueVariant::Numeric(value) => {
            0_u8.hash(state);
            value.hash(state);
        }
        ValueVariant::NumericArray(values) => {
            1_u8.hash(state);
            values.len().hash(state);
            for value in values {
                record_semantic_work(1);
                value.hash(state);
            }
        }
        ValueVariant::Type(value) => {
            2_u8.hash(state);
            hash_type(value, state);
        }
        ValueVariant::Unknown => 3_u8.hash(state),
    }
}
