use super::semantic_work::{reset_semantic_work, semantic_work};
use super::*;
use crate::ir::Signature;
use crate::ir::{ModportMemberPath, Shape, Type, TypeKind, VarId};
use crate::namespace::DefineContext;
use crate::scope::ScopeId;
use crate::symbol::{Direction, GenericTables, SymbolId};
use crate::symbol_path::{GenericSymbol, GenericSymbolPath, GenericSymbolPathKind};
use crate::HashMap;
use std::collections::HashSet;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use veryl_parser::resource_table;
use veryl_parser::veryl_token::Token;

#[derive(Default)]
pub(super) struct ConstantHasher;

impl Hasher for ConstantHasher {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, _bytes: &[u8]) {}
}

pub(super) type CollisionSet<T> = HashSet<T, BuildHasherDefault<ConstantHasher>>;

pub(super) fn generic_path(depth: usize, suffix: usize) -> GenericSymbolPath {
    let mut path = GenericSymbolPath {
        paths: vec![GenericSymbol {
            base: Token::from_external_text(&format!("value_{suffix}")),
            arguments: Vec::new(),
        }],
        kind: GenericSymbolPathKind::ValueLiteral,
        range: Default::default(),
    };
    for level in 1..depth {
        path = GenericSymbolPath {
            paths: vec![GenericSymbol {
                base: Token::from_external_text(&format!("layer_{level}")),
                arguments: vec![path],
            }],
            kind: GenericSymbolPathKind::ValueLiteral,
            range: Default::default(),
        };
    }
    path
}

fn specialization(width: usize) -> ComponentSpecializationIdentity {
    let owner = Signature::new(SymbolId(10));
    let connected = (0..width).rev().map(|index| {
        let mut actual = Signature::new(SymbolId(20));
        actual
            .full_path
            .push(resource_table::insert_str(&format!("actual_{index}")));
        actual.add_generic_parameter(
            resource_table::insert_str("W"),
            generic_path(if index == 0 { width } else { 1 }, index),
        );
        ConnectedInterfaceSpecialization {
            formal_port: resource_table::insert_str(&format!("port_{index}")),
            actual,
        }
    });
    ComponentSpecializationIdentity::new(owner, connected).expect("unique connected actuals")
}

pub(super) fn lowering(width: usize) -> NestedModportLowering {
    let mut table = HashMap::default();
    for index in 0..width {
        table.insert(
            resource_table::insert_str(&format!("T{index}")),
            generic_path(2, index),
        );
    }
    let mut generic_tables = GenericTables::default();
    generic_tables.insert((ScopeId(1), DefineContext::default()), table);
    let named = ResolvedNamedType {
        symbol: SymbolId(30),
        path: generic_path(width, 100),
        full_path: (0..width).map(|index| SymbolId(40 + index)).collect(),
        generic_tables,
        token: Default::default(),
    };
    let mut ir = Type::default();
    ir.kind = TypeKind::Logic;
    ir.array = Shape::new(vec![Some(2); width]);
    let resolved_type = ResolvedTerminalType {
        ir,
        declaration: ResolvedDeclarationType {
            kind: ResolvedDeclarationKind::Struct(named),
            signed: false,
            packed: Shape::new(vec![Some(8); width]),
            packed_expr: Vec::new(),
            unpacked: Shape::new(vec![Some(2); width]),
            unpacked_expr: Vec::new(),
        },
    };
    let terminal = ResolvedNestedTerminal {
        id: ResolvedNestedTerminalId(0),
        identifier: flatten_identifier_segments(
            &(0..width)
                .map(|index| resource_table::insert_str(&format!("segment_{index}")))
                .collect::<Vec<_>>(),
        ),
        emitted_identifier: EmittedIdentifierIdentity::from_logical(
            resource_table::insert_str("flattened"),
        ),
        variable: VarId::from_raw(1),
        symbol: SymbolId(31),
        token: Default::default(),
        resolved_type: resolved_type.clone(),
    };
    let entry = ResolvedModportEntry {
        path: ModportMemberPath::from_slice(&terminal.identifier.source_segments),
        direction: Direction::Input,
        terminal: Some(ResolvedModportTerminal::DirectVariable {
            variable: terminal.variable,
            symbol: terminal.symbol,
            emitted_identifier: terminal.emitted_identifier,
            resolved_type,
        }),
        terminal_site: None,
    };
    NestedModportLowering::new(
        HashMap::from_iter([(
            resource_table::insert_str("view"),
            ResolvedModport {
                entries: Arc::from([entry]),
            },
        )]),
        Arc::from([terminal]),
        HashMap::default(),
    )
}

fn measured_work(bindings: usize, semantic_width: usize) -> (usize, Duration) {
    reset_semantic_work();
    let started = Instant::now();
    for _ in 0..bindings {
        let key = specialization(semantic_width);
        let mut collision_set = CollisionSet::default();
        assert!(collision_set.insert(key.clone()));
        assert!(!collision_set.insert(key.clone()));
        assert!(collision_set.contains(&key));
        let mut late_miss = key.clone();
        late_miss
            .connected_actuals
            .last_mut()
            .expect("nonempty connected set")
            .actual
            .full_path
            .push(resource_table::insert_str("late_miss"));
        assert!(!collision_set.contains(&late_miss));

        let lowering = lowering(semantic_width);
        let mut lowering_set = CollisionSet::default();
        assert!(lowering_set.insert(lowering.clone()));
        assert!(!lowering_set.insert(lowering.clone()));
        assert!(lowering_set.contains(&lowering));
        let mut late_miss = lowering.clone();
        let terminals = Arc::make_mut(&mut late_miss.terminals);
        let ResolvedDeclarationKind::Struct(named) =
            &mut terminals[0].resolved_type.declaration.kind
        else {
            unreachable!()
        };
        let table = named.generic_tables.values_mut().next().expect("generic table");
        let value = table.values_mut().next().expect("generic argument");
        *value = generic_path(3, semantic_width + 1000);
        assert_ne!(lowering, late_miss);
        assert!(!lowering_set.contains(&late_miss));
    }
    (semantic_work(), started.elapsed())
}

#[test]
fn semantic_hash_eq_and_normalization_work_is_bounded_by_total_input() {
    let by_width = [1, 2, 4].map(|width| (width, measured_work(32, width)));
    for pair in by_width.windows(2) {
        let (left_width, (left_work, _)) = pair[0];
        let (right_width, (right_work, _)) = pair[1];
        assert!(right_work > left_work);
        assert!(right_work <= 4 * left_work);
        assert_eq!(right_width, 2 * left_width);
    }

    let by_bindings = [32, 64, 128].map(|bindings| (bindings, measured_work(bindings, 2)));
    for pair in by_bindings.windows(2) {
        let (left_bindings, (left_work, left_time)) = pair[0];
        let (right_bindings, (right_work, right_time)) = pair[1];
        assert_eq!(right_bindings, 2 * left_bindings);
        assert!(right_work >= 2 * left_work);
        assert!(right_work <= 3 * left_work);
        assert!(right_time <= left_time.saturating_mul(100));
    }
}

#[test]
fn terminal_type_equality_counts_only_the_dimension_prefix_it_visits() {
    fn mismatch_work(index: usize) -> usize {
        let left = lowering(8);
        let mut right = left.clone();
        let terminals = Arc::make_mut(&mut right.terminals);
        *terminals[0]
            .resolved_type
            .declaration
            .packed
            .get_mut(index)
            .expect("dimension") = Some(99);
        reset_semantic_work();
        assert_ne!(left, right);
        semantic_work()
    }

    let first = mismatch_work(0);
    let last = mismatch_work(7);
    assert!(first > 0);
    assert!(last > first, "first={first}, last={last}");
}
