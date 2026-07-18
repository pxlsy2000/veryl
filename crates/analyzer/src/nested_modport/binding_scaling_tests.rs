use super::collection_work::{
    collection_work as binding_finalization_work,
    reset_collection_work as reset_binding_finalization_work,
};
use super::prepared_emission::{prepare_emission_work, reset_prepare_emission_work};
use super::*;
use crate::ir::Signature;
use crate::symbol::{GenericMap, SymbolId};
use std::sync::Arc;
use veryl_parser::resource_table::{PathId, TokenId};

pub(super) fn direct_binding(
    session: AnalysisSessionId,
    id: u32,
    source: usize,
    declaration: usize,
) -> PendingEmissionBinding {
    let specialization = ComponentSpecializationIdentity::from_unique_connected_actuals(
        Signature::new(SymbolId(1)),
        [],
    );
    let key = NestedModportLoweringKey {
        session,
        specialization: specialization.clone().into(),
    };
    PendingEmissionBinding {
        id: EmissionBindingId::new(id),
        source: PathId(source),
        declaration: TokenId(declaration),
        kind: EmissionOwnerKind::Interface,
        enclosing_owner: None,
        enclosing_owner_identity: None,
        namespace_parent_fallback: false,
        enclosing_generic_map: None,
        package_scope: None,
        specialization: Arc::new(key),
        specialization_identity: None,
        emission_context: EmissionSpecializationContext::new(
            specialization,
            GenericMap::default(),
            None,
            Vec::new(),
        ),
        lowering: LoweringAvailability::NotNested,
        required_rewrites: Arc::from([]),
        required_expanded_ports: Arc::from([]),
    }
}

fn finalized_analysis(binding_count: usize, one_source: bool) -> Arc<NestedModportAnalysis> {
    let session = AnalysisSessionId::new();
    let mut pending = PendingNestedModportAnalysis::default();
    let key = NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::from_unique_connected_actuals(
            Signature::new(SymbolId(1)),
            [],
        )
        .into(),
    };
    pending.record_lowering(key, LoweringAvailability::NotNested);
    for index in 0..binding_count {
        let source = if one_source { 1 } else { index + 1 };
        pending.record_emission_binding(direct_binding(session, index as u32, source, index + 1));
    }
    pending
        .finalize(session)
        .expect("scaling fixture should finalize")
}

#[test]
fn binding_finalization_work_is_linear_at_1x_2x_4x() {
    for binding_count in [128, 256, 512] {
        reset_binding_finalization_work();

        let _analysis = finalized_analysis(binding_count, true);

        let work = binding_finalization_work();
        assert_eq!(work, 78 * binding_count + 2_317);
    }
}

#[test]
fn prepare_emission_work_is_source_local_at_1x_2x_4x() {
    for binding_count in [128, 256, 512] {
        let analysis = finalized_analysis(binding_count, false);
        reset_prepare_emission_work();

        for source in 1..=binding_count {
            let _align = analysis
                .prepare_emission(PathId(source), EmissionPhase::Align)
                .expect("align preparation should succeed");
            let _build = analysis
                .prepare_emission(PathId(source), EmissionPhase::Build)
                .expect("build preparation should succeed");
        }

        let work = prepare_emission_work();
        assert!(
            work <= 6 * binding_count,
            "preparation work {work} exceeded source-local bound {} for B={binding_count}",
            6 * binding_count
        );
    }
}
