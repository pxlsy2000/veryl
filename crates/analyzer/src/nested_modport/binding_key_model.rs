use super::binding_specialization_identity::BindingSpecializationIdentity;
use super::collection_work::{counted, record_collection_work};
use super::emission_index::PendingEmissionBinding;
use super::emission_scope::EmissionScopeIdentity;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::{ComponentSpecializationIdentity, SemanticGenericMap};
use super::lowering_identity::LoweringEquivalenceIdentity;
use super::record_resolution::FrozenSemanticRecords;
use super::semantic_records::{
    ExpandedPortResolution, OccurrenceKind, ResolvedExpandedPortInterface,
};
use super::specialization_context::EmissionOwnerKind;
use crate::symbol::SymbolId;
use veryl_parser::resource_table::{PathId, StrId, TokenId};

#[derive(Clone, Copy, Debug)]
pub(super) struct BindingVariantGroupKey<'a> {
    pub(super) source: PathId,
    pub(super) declaration: TokenId,
    pub(super) kind: EmissionOwnerKind,
    pub(super) owner: SymbolId,
    pub(super) generic_map: &'a SemanticGenericMap,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ConnectedClass {
    Template,
    Concrete(BindingSpecializationIdentity),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RewriteEquivalenceKey<'a> {
    pub(super) kind: OccurrenceKind,
    pub(super) token: TokenId,
    pub(super) semantic_segments: &'a [StrId],
    pub(super) replace_from_segment: usize,
    pub(super) consumed_segments: usize,
    pub(super) terminal: u32,
    pub(super) target_lowering: LoweringEquivalenceIdentity,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ExpandedPortEquivalenceValue<'a> {
    DirectLegacy,
    Nested {
        modport: StrId,
        interface: &'a ResolvedExpandedPortInterface,
        target_lowering: LoweringEquivalenceIdentity,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ExpandedPortEquivalenceKey<'a> {
    pub(super) token: TokenId,
    pub(super) value: ExpandedPortEquivalenceValue<'a>,
}

#[derive(Clone, Debug)]
pub(super) struct BindingBaseEquivalenceKey<'a> {
    pub(super) variant_group: BindingVariantGroupKey<'a>,
    pub(super) enclosing_owner: Option<&'a ComponentSpecializationIdentity>,
    pub(super) namespace_parent_fallback: bool,
    pub(super) package_scope: Option<&'a EmissionScopeIdentity>,
    pub(super) lowering: LoweringEquivalenceIdentity,
    pub(super) rewrites: Vec<RewriteEquivalenceKey<'a>>,
    pub(super) expanded_ports: Vec<ExpandedPortEquivalenceKey<'a>>,
}

#[derive(Clone, Debug)]
pub(super) struct BindingEquivalenceKey<'a> {
    pub(super) base: BindingBaseEquivalenceKey<'a>,
    pub(super) connected: ConnectedClass,
}

impl<'a> BindingEquivalenceKey<'a> {
    pub(super) fn build(
        binding: &'a PendingEmissionBinding,
        specialization_identity: BindingSpecializationIdentity,
        records: &'a FrozenSemanticRecords,
    ) -> Result<(BindingVariantGroupKey<'a>, Self), NestedModportAnalysisInvariant> {
        let variant_group = BindingVariantGroupKey {
            source: binding.source,
            declaration: binding.declaration,
            kind: binding.kind,
            owner: binding.specialization.specialization.owner.symbol,
            generic_map: binding.emission_context.semantic_generic_map(),
        };
        let mut rewrites = Vec::with_capacity(binding.required_rewrites.len());
        for key in counted(binding.required_rewrites.iter()) {
            let rewrite = records
                .rewrites
                .get(key)
                .ok_or(NestedModportAnalysisInvariant::MissingRewrite)?;
            rewrites.push(RewriteEquivalenceKey {
                kind: key.kind,
                token: key.token,
                semantic_segments: &rewrite.semantic_segments,
                replace_from_segment: rewrite.replace_from_segment,
                consumed_segments: rewrite.consumed_segments,
                terminal: rewrite.terminal.terminal.0,
                target_lowering: records.terminal_lowering_identity(&rewrite.terminal),
            });
        }
        rewrites.sort_by(|left, right| {
            record_collection_work(1);
            (left.kind, left.token).cmp(&(right.kind, right.token))
        });
        let mut expanded_ports = Vec::with_capacity(binding.required_expanded_ports.len());
        for key in counted(binding.required_expanded_ports.iter()) {
            let value = match records
                .expanded_ports
                .get(key)
                .ok_or(NestedModportAnalysisInvariant::MissingExpandedPort)?
                .as_ref()
            {
                ExpandedPortResolution::DirectLegacy => ExpandedPortEquivalenceValue::DirectLegacy,
                ExpandedPortResolution::Nested {
                    target,
                    modport,
                    interface,
                    ..
                } => ExpandedPortEquivalenceValue::Nested {
                    modport: *modport,
                    interface,
                    target_lowering: records.lowering_identity(
                        target,
                        records
                            .availability
                            .get(target)
                            .ok_or(NestedModportAnalysisInvariant::MissingLowering)?,
                    )?,
                },
            };
            expanded_ports.push(ExpandedPortEquivalenceKey {
                token: key.token,
                value,
            });
        }
        expanded_ports.sort_by(|left, right| {
            record_collection_work(1);
            left.token.cmp(&right.token)
        });
        let base = BindingBaseEquivalenceKey {
            variant_group,
            enclosing_owner: (binding.kind == EmissionOwnerKind::Function)
                .then_some(binding.enclosing_owner.as_ref())
                .flatten()
                .map(|owner| &owner.specialization)
                .map(|v| &**v),
            namespace_parent_fallback: binding.kind == EmissionOwnerKind::Function
                && binding.namespace_parent_fallback,
            package_scope: (binding.kind == EmissionOwnerKind::Function)
                .then_some(binding.package_scope.as_ref())
                .flatten(),
            lowering: records.lowering_identity(&binding.specialization, &binding.lowering)?,
            rewrites,
            expanded_ports,
        };
        let connected = if binding
            .specialization
            .specialization
            .connected_actuals
            .is_empty()
        {
            ConnectedClass::Template
        } else {
            ConnectedClass::Concrete(specialization_identity)
        };
        Ok((variant_group, Self { base, connected }))
    }

    pub(super) fn is_template(&self) -> bool {
        matches!(self.connected, ConnectedClass::Template)
    }
}
