use super::semantic_work::record_semantic_work;
use crate::ir::{Shape, WidthExpr};
use crate::symbol_path::{GenericSymbolPath, GenericSymbolPathKind};
use std::hash::{Hash, Hasher};

pub(super) fn generic_path_eq(left: &GenericSymbolPath, right: &GenericSymbolPath) -> bool {
    record_semantic_work(1);
    generic_kind_eq(&left.kind, &right.kind)
        && left.range == right.range
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            record_semantic_work(1);
            left.base == right.base
                && left.arguments.len() == right.arguments.len()
                && left
                    .arguments
                    .iter()
                    .zip(&right.arguments)
                    .all(|(left, right)| generic_path_eq(left, right))
        })
}

pub(super) fn hash_generic_path<H: Hasher>(path: &GenericSymbolPath, state: &mut H) {
    record_semantic_work(1);
    hash_generic_kind(&path.kind, state);
    path.range.hash(state);
    path.paths.len().hash(state);
    for segment in &path.paths {
        record_semantic_work(1);
        segment.base.hash(state);
        segment.arguments.len().hash(state);
        for argument in &segment.arguments {
            hash_generic_path(argument, state);
        }
    }
}

pub(super) fn generic_kind_eq(left: &GenericSymbolPathKind, right: &GenericSymbolPathKind) -> bool {
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

pub(super) fn hash_generic_kind<H: Hasher>(kind: &GenericSymbolPathKind, state: &mut H) {
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

pub(super) fn shape_eq(left: &Shape, right: &Shape) -> bool {
    record_semantic_work(1);
    left.iter().len() == right.iter().len()
        && left.iter().zip(right.iter()).all(|(left, right)| {
            record_semantic_work(1);
            left == right
        })
}

pub(super) fn hash_shape<H: Hasher>(shape: &Shape, state: &mut H) {
    record_semantic_work(1);
    shape.iter().len().hash(state);
    for dimension in shape.iter() {
        record_semantic_work(1);
        dimension.hash(state);
    }
}

pub(super) fn width_expr_eq(left: &WidthExpr, right: &WidthExpr) -> bool {
    record_semantic_work(1);
    match (left, right) {
        (WidthExpr::Concrete(left), WidthExpr::Concrete(right)) => left == right,
        (WidthExpr::Param(left), WidthExpr::Param(right)) => left == right,
        (WidthExpr::BinOp(ll, lo, lr), WidthExpr::BinOp(rl, ro, rr)) => {
            lo == ro && width_expr_eq(ll, rl) && width_expr_eq(lr, rr)
        }
        _ => false,
    }
}

pub(super) fn hash_width_expr<H: Hasher>(expr: &WidthExpr, state: &mut H) {
    record_semantic_work(1);
    match expr {
        WidthExpr::Concrete(value) => {
            0_u8.hash(state);
            value.hash(state);
        }
        WidthExpr::Param(value) => {
            1_u8.hash(state);
            value.hash(state);
        }
        WidthExpr::BinOp(left, op, right) => {
            2_u8.hash(state);
            op.hash(state);
            hash_width_expr(left, state);
            hash_width_expr(right, state);
        }
    }
}

pub(super) fn width_exprs_eq(left: &[WidthExpr], right: &[WidthExpr]) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| width_expr_eq(left, right))
}

pub(super) fn hash_width_exprs<H: Hasher>(values: &[WidthExpr], state: &mut H) {
    record_semantic_work(1);
    values.len().hash(state);
    for value in values {
        hash_width_expr(value, state);
    }
}

fn slice_eq<T: PartialEq>(left: &[T], right: &[T]) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            record_semantic_work(1);
            left == right
        })
}

fn hash_slice<T: Hash, H: Hasher>(values: &[T], state: &mut H) {
    record_semantic_work(1);
    values.len().hash(state);
    for value in values {
        record_semantic_work(1);
        value.hash(state);
    }
}
