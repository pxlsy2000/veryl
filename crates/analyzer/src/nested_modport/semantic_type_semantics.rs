use super::semantic_generic_tables::*;
use super::semantic_path_operations::*;
use super::semantic_type::*;
use super::semantic_type_operations::{hash_type, type_eq};
use super::semantic_work::{hash_slice, record_semantic_work, slice_eq};
use std::hash::{Hash, Hasher};

impl PartialEq for ResolvedNamedType {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(2);
        self.symbol == other.symbol
            && generic_path_eq(&self.path, &other.path)
            && slice_eq(&self.full_path, &other.full_path)
            && generic_tables_eq(&self.generic_tables, &other.generic_tables)
            && self.token == other.token
    }
}

impl Eq for ResolvedNamedType {}

impl Hash for ResolvedNamedType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(2);
        self.symbol.hash(state);
        hash_generic_path(&self.path, state);
        hash_slice(&self.full_path, state);
        hash_generic_tables(&self.generic_tables, state);
        self.token.hash(state);
    }
}

fn kind_tag(kind: &ResolvedDeclarationKind) -> u8 {
    match kind {
        ResolvedDeclarationKind::Clock => 0,
        ResolvedDeclarationKind::ClockPosedge => 1,
        ResolvedDeclarationKind::ClockNegedge => 2,
        ResolvedDeclarationKind::Reset => 3,
        ResolvedDeclarationKind::ResetAsyncHigh => 4,
        ResolvedDeclarationKind::ResetAsyncLow => 5,
        ResolvedDeclarationKind::ResetSyncHigh => 6,
        ResolvedDeclarationKind::ResetSyncLow => 7,
        ResolvedDeclarationKind::Bit => 8,
        ResolvedDeclarationKind::F32 => 9,
        ResolvedDeclarationKind::F64 => 10,
        ResolvedDeclarationKind::Logic => 11,
        ResolvedDeclarationKind::Struct(_) => 12,
        ResolvedDeclarationKind::Union(_) => 13,
        ResolvedDeclarationKind::Enum(_) => 14,
        ResolvedDeclarationKind::String => 15,
    }
}

fn named(kind: &ResolvedDeclarationKind) -> Option<&ResolvedNamedType> {
    match kind {
        ResolvedDeclarationKind::Struct(value)
        | ResolvedDeclarationKind::Union(value)
        | ResolvedDeclarationKind::Enum(value) => Some(value),
        _ => None,
    }
}

impl PartialEq for ResolvedDeclarationKind {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        kind_tag(self) == kind_tag(other) && named(self) == named(other)
    }
}

impl Eq for ResolvedDeclarationKind {}

impl Hash for ResolvedDeclarationKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        kind_tag(self).hash(state);
        named(self).hash(state);
    }
}

impl PartialEq for ResolvedDeclarationType {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.kind == other.kind
            && self.signed == other.signed
            && shape_eq(&self.packed, &other.packed)
            && width_exprs_eq(&self.packed_expr, &other.packed_expr)
            && shape_eq(&self.unpacked, &other.unpacked)
            && width_exprs_eq(&self.unpacked_expr, &other.unpacked_expr)
    }
}

impl Eq for ResolvedDeclarationType {}

impl Hash for ResolvedDeclarationType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.kind.hash(state);
        self.signed.hash(state);
        hash_shape(&self.packed, state);
        hash_width_exprs(&self.packed_expr, state);
        hash_shape(&self.unpacked, state);
        hash_width_exprs(&self.unpacked_expr, state);
    }
}

impl PartialEq for ResolvedTerminalType {
    fn eq(&self, other: &Self) -> bool {
        type_eq(&self.ir, &other.ir) && self.declaration == other.declaration
    }
}

impl Eq for ResolvedTerminalType {}

impl Hash for ResolvedTerminalType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_type(&self.ir, state);
        self.declaration.hash(state);
    }
}
