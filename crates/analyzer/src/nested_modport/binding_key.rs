use super::binding_key_model::*;
use super::semantic_work::{hash_slice, record_semantic_work, slice_eq};
use std::hash::{Hash, Hasher};

impl PartialEq for BindingVariantGroupKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.source == other.source
            && self.declaration == other.declaration
            && self.kind == other.kind
            && self.owner == other.owner
            && self.generic_map == other.generic_map
    }
}

impl Eq for BindingVariantGroupKey<'_> {}

impl Hash for BindingVariantGroupKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(4);
        self.source.hash(state);
        self.declaration.hash(state);
        self.kind.hash(state);
        self.owner.hash(state);
        self.generic_map.hash(state);
    }
}

impl PartialEq for BindingBaseEquivalenceKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.variant_group == other.variant_group
            && self.enclosing_owner == other.enclosing_owner
            && self.namespace_parent_fallback == other.namespace_parent_fallback
            && self.package_scope == other.package_scope
            && self.lowering == other.lowering
            && slice_eq(&self.rewrites, &other.rewrites)
            && slice_eq(&self.expanded_ports, &other.expanded_ports)
    }
}

impl Eq for BindingBaseEquivalenceKey<'_> {}

impl Hash for BindingBaseEquivalenceKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(5);
        self.variant_group.hash(state);
        self.enclosing_owner.hash(state);
        self.namespace_parent_fallback.hash(state);
        self.package_scope.hash(state);
        self.lowering.hash(state);
        hash_slice(&self.rewrites, state);
        hash_slice(&self.expanded_ports, state);
    }
}

impl PartialEq for BindingEquivalenceKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.base == other.base && connected_eq(&self.connected, &other.connected)
    }
}

impl Eq for BindingEquivalenceKey<'_> {}

impl Hash for BindingEquivalenceKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.base.hash(state);
        hash_connected(&self.connected, state);
    }
}

fn connected_eq(left: &ConnectedClass, right: &ConnectedClass) -> bool {
    record_semantic_work(1);
    match (left, right) {
        (ConnectedClass::Template, ConnectedClass::Template) => true,
        (ConnectedClass::Concrete(left), ConnectedClass::Concrete(right)) => left == right,
        _ => false,
    }
}

fn hash_connected<H: Hasher>(connected: &ConnectedClass, state: &mut H) {
    record_semantic_work(1);
    match connected {
        ConnectedClass::Template => 0_u8.hash(state),
        ConnectedClass::Concrete(values) => {
            1_u8.hash(state);
            values.hash(state);
        }
    }
}

impl PartialEq for RewriteEquivalenceKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.kind == other.kind
            && self.token == other.token
            && slice_eq(&self.semantic_segments, &other.semantic_segments)
            && self.replace_from_segment == other.replace_from_segment
            && self.consumed_segments == other.consumed_segments
            && self.terminal == other.terminal
            && self.target_lowering == other.target_lowering
    }
}

impl Eq for RewriteEquivalenceKey<'_> {}

impl Hash for RewriteEquivalenceKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(6);
        self.kind.hash(state);
        self.token.hash(state);
        hash_slice(&self.semantic_segments, state);
        self.replace_from_segment.hash(state);
        self.consumed_segments.hash(state);
        self.terminal.hash(state);
        self.target_lowering.hash(state);
    }
}

impl PartialEq for ExpandedPortEquivalenceValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        match (self, other) {
            (Self::DirectLegacy, Self::DirectLegacy) => true,
            (
                Self::Nested {
                    modport: left_modport,
                    interface: left_interface,
                    target_lowering: left_lowering,
                },
                Self::Nested {
                    modport: right_modport,
                    interface: right_interface,
                    target_lowering: right_lowering,
                },
            ) => {
                left_modport == right_modport
                    && left_interface == right_interface
                    && left_lowering == right_lowering
            }
            _ => false,
        }
    }
}

impl Eq for ExpandedPortEquivalenceValue<'_> {}

impl Hash for ExpandedPortEquivalenceValue<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        match self {
            Self::DirectLegacy => 0_u8.hash(state),
            Self::Nested {
                modport,
                interface,
                target_lowering,
            } => {
                1_u8.hash(state);
                modport.hash(state);
                interface.hash(state);
                target_lowering.hash(state);
            }
        }
    }
}

impl PartialEq for ExpandedPortEquivalenceKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.token == other.token && self.value == other.value
    }
}

impl Eq for ExpandedPortEquivalenceKey<'_> {}

impl Hash for ExpandedPortEquivalenceKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.token.hash(state);
        self.value.hash(state);
    }
}
