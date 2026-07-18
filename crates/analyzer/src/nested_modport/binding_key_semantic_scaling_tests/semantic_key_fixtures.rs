use super::super::binding_key_model::{
    BindingBaseEquivalenceKey, BindingEquivalenceKey, BindingVariantGroupKey, ConnectedClass,
    ExpandedPortEquivalenceKey, ExpandedPortEquivalenceValue, RewriteEquivalenceKey,
};
use super::super::lowering_identity::{
    LoweringEquivalenceIdentity, SemanticLoweringIdentity, SessionLoweringInterner,
};
use super::super::binding_specialization_identity::{
    BindingSpecializationIdentity, BindingSpecializationInterner,
};
use super::semantic_work_scaling_tests::{CollisionSet, generic_path, lowering};
use super::super::semantic_work::{reset_semantic_work, semantic_work};
use super::*;
use crate::symbol::{GenericMap, SymbolId};
use std::sync::Arc;
use std::hash::BuildHasherDefault;
use veryl_parser::resource_table::{self, PathId, TokenId};

pub(super) fn material_parameter(index: usize) -> crate::ir::ValueVariant {
    match index % 3 {
        0 => crate::ir::ValueVariant::Numeric(crate::value::Value::new(
            index as u64,
            96 + index,
            false,
        )),
        1 => crate::ir::ValueVariant::NumericArray(vec![
            crate::value::Value::new(index as u64, 65 + index, false),
            crate::value::Value::new((index + 1) as u64, 97 + index, true),
        ]),
        _ => {
            let mut value = crate::ir::Type::new(crate::ir::TypeKind::Logic);
            value.array = crate::ir::Shape::new(vec![Some(index + 1), Some(index + 2)]);
            value.set_concrete_width(crate::ir::Shape::new(vec![
                Some(index + 3),
                Some(index + 4),
            ]));
            crate::ir::ValueVariant::Type(value)
        }
    }
}

fn semantic_map(width: usize, offset: usize) -> SemanticGenericMap {
    let mut map = GenericMap::default();
    for index in 0..width {
        map.map.insert(
            resource_table::insert_str(&format!("G{index}")),
            generic_path(2, offset + index),
        );
    }
    SemanticGenericMap::from(&map)
}

fn complete_binding_key(width: usize, discriminator: usize) -> BindingEquivalenceKey<'static> {
    let session = AnalysisSessionId::new();
    let lowering = LoweringEquivalenceIdentity::Found(SemanticLoweringIdentity::fixture(session, 0));
    let mut interface_map = GenericMap::default();
    for index in 0..width {
        interface_map.map.insert(
            resource_table::insert_str(&format!("I{index}")),
            generic_path(2, index),
        );
    }
    BindingEquivalenceKey {
        base: BindingBaseEquivalenceKey {
            variant_group: BindingVariantGroupKey {
                source: PathId(1),
                declaration: TokenId(10),
                kind: EmissionOwnerKind::Interface,
                owner: SymbolId(10),
                generic_map: Box::leak(Box::new(semantic_map(width, 0))),
            },
            enclosing_owner: None,
            namespace_parent_fallback: false,
            package_scope: None,
            lowering,
            rewrites: vec![RewriteEquivalenceKey {
                kind: OccurrenceKind::ExpressionIdentifier,
                token: TokenId(20),
                semantic_segments: Box::leak(
                    (0..width)
                        .map(|index| resource_table::insert_str(&format!("rewrite_{index}")))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                replace_from_segment: 0,
                consumed_segments: width,
                terminal: 0,
                target_lowering: lowering,
            }],
            expanded_ports: vec![ExpandedPortEquivalenceKey {
                token: TokenId(30),
                value: ExpandedPortEquivalenceValue::Nested {
                    modport: resource_table::insert_str("view"),
                    interface: Box::leak(Box::new(ResolvedExpandedPortInterface {
                        symbol: SymbolId(30),
                        generic_map: interface_map,
                    })),
                    target_lowering: lowering,
                },
            }],
        },
        connected: ConnectedClass::Concrete(BindingSpecializationIdentity::fixture(
            session,
            discriminator,
        )),
    }
}

fn measured_work(bindings: usize, width: usize) -> usize {
    reset_semantic_work();
    let keys: Vec<_> = (0..bindings)
        .map(|index| complete_binding_key(width, index))
        .collect();
    let mut set = CollisionSet::default();
    for key in &keys {
        assert!(set.insert(key.clone()));
    }
    let last = keys.last().expect("nonempty binding set");
    assert!(set.contains(last));
    assert!(!set.insert(last.clone()));
    let late_miss = complete_binding_key(width, bindings);
    assert_ne!(last, &late_miss);
    assert!(!set.contains(&late_miss));
    let mut inner_miss = last.clone();
    inner_miss.base.lowering = LoweringEquivalenceIdentity::Found(
        SemanticLoweringIdentity::fixture(AnalysisSessionId::new(), 1),
    );
    assert_ne!(last, &inner_miss);
    assert!(!set.contains(&inner_miss));
    semantic_work()
}

#[test]
fn complete_binding_key_collision_work_reaches_every_nested_semantic_value() {
    let by_width = [1, 2, 4].map(|width| measured_work(16, width));
    assert!(by_width[0] > 0);
    assert!(by_width[1] > by_width[0]);
    assert!(by_width[2] > by_width[1]);
    assert!(by_width[1] <= 4 * by_width[0]);
    assert!(by_width[2] <= 4 * by_width[1]);
    let by_bindings = [8, 16, 32].map(|bindings| measured_work(bindings, 2));
    assert!(by_bindings[1] > 2 * by_bindings[0], "{by_bindings:?}");
    assert!(by_bindings[2] > 2 * by_bindings[1], "{by_bindings:?}");
    assert!(by_bindings[1] <= 5 * by_bindings[0]);
    assert!(by_bindings[2] <= 5 * by_bindings[1]);
}
