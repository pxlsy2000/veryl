use crate::HashMap;
use crate::ir::{Shape, Type, TypeKind, WidthExpr};
use crate::symbol::{GenericTables, SymbolId};
use crate::symbol_path::GenericSymbolPath;
use crate::symbol_table;
use veryl_parser::token_range::TokenRange;

#[derive(Clone, Debug)]
pub enum ResolvedDeclarationKind {
    Clock,
    ClockPosedge,
    ClockNegedge,
    Reset,
    ResetAsyncHigh,
    ResetAsyncLow,
    ResetSyncHigh,
    ResetSyncLow,
    Bit,
    F32,
    F64,
    Logic,
    Struct(ResolvedNamedType),
    Union(ResolvedNamedType),
    Enum(ResolvedNamedType),
    String,
}

#[derive(Clone, Debug)]
pub struct ResolvedNamedType {
    pub symbol: SymbolId,
    pub path: GenericSymbolPath,
    pub full_path: Vec<SymbolId>,
    pub generic_tables: GenericTables,
    pub token: TokenRange,
}

#[derive(Clone, Debug)]
pub struct ResolvedDeclarationType {
    pub kind: ResolvedDeclarationKind,
    pub signed: bool,
    pub packed: Shape,
    pub packed_expr: Vec<WidthExpr>,
    pub unpacked: Shape,
    pub unpacked_expr: Vec<WidthExpr>,
}

#[derive(Clone, Debug)]
pub struct ResolvedTerminalType {
    pub ir: Type,
    pub declaration: ResolvedDeclarationType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnemittableResolvedType {
    pub actual_type: String,
}

impl ResolvedTerminalType {
    pub fn try_from_ir(r#type: &Type) -> Result<Self, UnemittableResolvedType> {
        let kind = match &r#type.kind {
            TypeKind::Clock => ResolvedDeclarationKind::Clock,
            TypeKind::ClockPosedge => ResolvedDeclarationKind::ClockPosedge,
            TypeKind::ClockNegedge => ResolvedDeclarationKind::ClockNegedge,
            TypeKind::Reset => ResolvedDeclarationKind::Reset,
            TypeKind::ResetAsyncHigh => ResolvedDeclarationKind::ResetAsyncHigh,
            TypeKind::ResetAsyncLow => ResolvedDeclarationKind::ResetAsyncLow,
            TypeKind::ResetSyncHigh => ResolvedDeclarationKind::ResetSyncHigh,
            TypeKind::ResetSyncLow => ResolvedDeclarationKind::ResetSyncLow,
            TypeKind::Bit => ResolvedDeclarationKind::Bit,
            TypeKind::F32 => ResolvedDeclarationKind::F32,
            TypeKind::F64 => ResolvedDeclarationKind::F64,
            TypeKind::Logic => ResolvedDeclarationKind::Logic,
            TypeKind::Struct(value) => {
                ResolvedDeclarationKind::Struct(resolve_named_type(r#type, value.id)?)
            }
            TypeKind::Union(value) => {
                ResolvedDeclarationKind::Union(resolve_named_type(r#type, value.id)?)
            }
            TypeKind::Enum(value) => {
                ResolvedDeclarationKind::Enum(resolve_named_type(r#type, value.id)?)
            }
            TypeKind::String => ResolvedDeclarationKind::String,
            TypeKind::Module(_)
            | TypeKind::Interface(_)
            | TypeKind::Modport(_, _)
            | TypeKind::Package(_)
            | TypeKind::Instance(_, _)
            | TypeKind::AbstractInterface(_)
            | TypeKind::Type
            | TypeKind::SystemVerilog
            | TypeKind::Void
            | TypeKind::Unknown => {
                return Err(UnemittableResolvedType {
                    actual_type: r#type.kind.to_string(),
                });
            }
        };

        Ok(Self {
            ir: r#type.clone(),
            declaration: ResolvedDeclarationType {
                kind,
                signed: r#type.signed,
                packed: r#type.width().to_owned(),
                packed_expr: r#type.width_expr().to_vec(),
                unpacked: r#type.array.clone(),
                unpacked_expr: r#type.array_expr().to_vec(),
            },
        })
    }
}

fn resolve_named_type(
    r#type: &Type,
    symbol: SymbolId,
) -> Result<ResolvedNamedType, UnemittableResolvedType> {
    let path = r#type
        .named_path()
        .cloned()
        .ok_or_else(|| UnemittableResolvedType {
            actual_type: r#type.kind.to_string(),
        })?;
    let resolution_scope = crate::scope::token_scope(path.paths[0].base.id).ok_or_else(|| {
        UnemittableResolvedType {
            actual_type: r#type.kind.to_string(),
        }
    })?;
    let resolved =
        symbol_table::resolve_generic_structural(&path, resolution_scope).map_err(|_| {
            UnemittableResolvedType {
                actual_type: r#type.kind.to_string(),
            }
        })?;
    let generic_tables = resolved_named_generic_tables(r#type, &path, symbol)?;
    Ok(ResolvedNamedType {
        symbol,
        token: path.range,
        path,
        full_path: resolved.full_path.clone(),
        generic_tables,
    })
}

fn resolved_named_generic_tables(
    r#type: &Type,
    path: &GenericSymbolPath,
    symbol: SymbolId,
) -> Result<GenericTables, UnemittableResolvedType> {
    let mut tables = GenericTables::default();
    for (index, generic_map) in path.to_generic_maps().into_iter().enumerate() {
        if generic_map.map.is_empty() {
            continue;
        }
        let prefix = path.slice(index);
        let resolved = symbol_table::resolve(&prefix).map_err(|_| UnemittableResolvedType {
            actual_type: path.to_string(),
        })?;
        tables.insert(
            (
                crate::scope::inner_scope(resolved.found.scope, resolved.found.token.text),
                resolved.found.namespace.define_context.clone(),
            ),
            generic_map.map,
        );
    }
    let named_symbol = symbol_table::get(symbol).ok_or_else(|| UnemittableResolvedType {
        actual_type: r#type.kind.to_string(),
    })?;
    if let Some(package) = named_symbol.get_parent_package() {
        let context: HashMap<_, _> = r#type.named_generic_context().iter().cloned().collect();
        let table: HashMap<_, _> = package
            .generic_parameters()
            .into_iter()
            .filter_map(|(name, _)| context.get(&name).cloned().map(|value| (name, value)))
            .collect();
        if !table.is_empty() {
            tables.insert(
                (
                    crate::scope::inner_scope(package.scope, package.token.text),
                    package.namespace.define_context.clone(),
                ),
                table,
            );
        }
    }
    Ok(tables)
}
