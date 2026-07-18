use super::*;
use crate::HashMap;
use crate::ir::{
    Comptime, FuncPath, Function, ModportMemberPath, Shape, Type, TypeKind, VarId, VarKind,
    VarPath, Variable, WidthExpr,
};
use crate::symbol::{Affiliation, Direction, SymbolId};
use veryl_parser::resource_table::{self, StrId};
use veryl_parser::token_range::TokenRange;

fn id(text: &str) -> StrId {
    resource_table::insert_str(text)
}

fn path(segments: &[&str]) -> ModportMemberPath {
    ModportMemberPath::from_slice(
        &segments
            .iter()
            .map(|segment| id(segment))
            .collect::<Vec<_>>(),
    )
}

fn variable(var_id: u32, segments: &[&str], width: usize, array: &[usize]) -> Variable {
    let mut r#type = Type::new(TypeKind::Logic);
    r#type.set_concrete_width(Shape::new(vec![Some(width)]));
    r#type.array = Shape::new(array.iter().copied().map(Some).collect());
    r#type.set_array_expr(WidthExpr::from_shape(&r#type.array));
    Variable {
        id: VarId::from_raw(var_id),
        symbol: Some(SymbolId(var_id as usize)),
        path: VarPath::from_slice(
            &segments
                .iter()
                .map(|segment| id(segment))
                .collect::<Vec<_>>(),
        ),
        kind: VarKind::Variable,
        r#type,
        value: Vec::new(),
        assigned: Vec::new(),
        affiliation: Affiliation::Interface,
        declaration_token: TokenRange::default(),
        token: TokenRange::default(),
    }
}

fn function(var_id: u32, name: &str) -> Function {
    Function {
        name: id(name),
        id: VarId::from_raw(var_id),
        path: FuncPath::new(SymbolId(var_id as usize)),
        r#type: Comptime::default(),
        array: Shape::default(),
        arity: 0,
        args: Vec::new(),
        is_const: false,
        functions: Vec::new(),
        token: TokenRange::default(),
    }
}

fn pending_from_effective(
    modports: &HashMap<StrId, Vec<(ModportMemberPath, Direction)>>,
) -> PendingComponentLowering {
    PendingComponentLowering {
        declarations: modports
            .iter()
            .map(|(name, entries)| PendingModportDeclaration {
                name: *name,
                explicit: entries
                    .iter()
                    .map(|(path, direction)| PendingModportEntry {
                        path: path.clone(),
                        direction: *direction,
                        origin: TokenRange::default(),
                    })
                    .collect(),
                default: None,
                origin: TokenRange::default(),
                contains_nested_item: entries.iter().any(|(path, _)| path.as_slice().len() > 1),
            })
            .collect(),
    }
}

#[test]
fn resolved_terminal_type_preserves_instantiated_packed_and_unpacked_dimensions() {
    let signal = variable(1, &["child", "payload"], 8, &[3, 2]);
    let resolved = ResolvedTerminalType::try_from_ir(&signal.r#type)
        .expect("plain instantiated logic type should be renderable");

    assert_eq!(resolved.ir.width().as_slice(), &[Some(8)]);
    assert_eq!(resolved.ir.array.as_slice(), &[Some(3), Some(2)]);
    assert_eq!(resolved.declaration.packed.as_slice(), &[Some(8)]);
    assert_eq!(
        resolved.declaration.unpacked.as_slice(),
        &[Some(3), Some(2)]
    );
    assert_eq!(resolved.declaration.packed_expr, signal.r#type.width_expr());
    assert_eq!(
        resolved.declaration.unpacked_expr,
        signal.r#type.array_expr()
    );
}

#[test]
fn nested_lowering_keeps_first_position_and_applies_later_direction() {
    let sink = id("sink");
    let z = path(&["child", "z"]);
    let a = path(&["child", "a"]);
    let modports = HashMap::from_iter([(
        sink,
        vec![
            (z.clone(), Direction::Input),
            (a.clone(), Direction::Inout),
            (z.clone(), Direction::Output),
        ],
    )]);
    let variables = HashMap::from_iter([
        (VarId::from_raw(1), variable(1, &["child", "z"], 8, &[])),
        (VarId::from_raw(2), variable(2, &["child", "a"], 8, &[])),
    ]);

    super::effective_member_set::reset_effective_member_membership_operations();
    let lowering = resolve_pending_nested_modport_lowering(
        &pending_from_effective(&modports),
        &variables,
        &HashMap::default(),
        16,
    )
    .expect("effective set should resolve")
    .expect("dotted members require nested lowering");
    let entries = lowering.modports[&sink].entries.as_ref();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].path, z);
    assert_eq!(entries[0].direction, Direction::Output);
    assert_eq!(entries[1].path, a);
    assert_eq!(entries[1].direction, Direction::Inout);
    assert_eq!(
        super::effective_member_set::effective_member_membership_operations(),
        3,
        "the repeated semantic identity should replace in place with one indexed lookup",
    );
}

#[test]
fn unique_explicit_members_use_linear_membership_operations() {
    let member_count = std::env::var("VERYL_TEST_EFFECTIVE_MEMBER_COUNT")
        .map(|value| {
            value
                .parse::<usize>()
                .expect("VERYL_TEST_EFFECTIVE_MEMBER_COUNT must be a usize integer")
        })
        .unwrap_or(1_024);
    let explicit = (0..member_count)
        .map(|index| PendingModportEntry {
            path: path(&[&format!("member_{index}")]),
            direction: Direction::Input,
            origin: TokenRange::default(),
        })
        .collect();
    let pending = PendingComponentLowering {
        declarations: vec![PendingModportDeclaration {
            name: id("many_unique"),
            explicit,
            default: None,
            origin: TokenRange::default(),
            contains_nested_item: false,
        }],
    };

    super::effective_member_set::reset_effective_member_membership_operations();
    let effective = resolve_direct_modport_effective(
        &pending,
        &HashMap::default(),
        &HashMap::default(),
        member_count,
    )
    .expect("the exact member budget should admit every unique entry");

    assert_eq!(effective[&id("many_unique")].len(), member_count);
    assert_eq!(
        super::effective_member_set::effective_member_membership_operations(),
        member_count,
        "each incoming semantic identity should require exactly one indexed membership operation",
    );
}
