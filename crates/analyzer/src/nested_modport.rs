mod analysis_snapshot;
mod binding_deduplication;
mod binding_equivalence;
mod binding_key;
mod binding_key_model;
mod binding_order;
mod binding_specialization_identity;
mod collection_work;
mod connected_generic_map_index;
mod default_expansion;
mod default_expansion_work;
mod default_member_order;
mod direct_terminal_resolution;
mod effective_member_set;
mod emission_binding_records;
mod emission_frame;
mod emission_handle_freeze;
mod emission_handle_queries;
mod emission_index;
mod emission_index_build;
mod emission_scope;
mod emitter_index_work;
mod errors;
mod expanded_member_index;
mod finalization;
mod frame_record_index;
mod frame_record_queries;
#[cfg(any(test, feature = "nested-modport-test-utils"))]
mod frame_record_query_mutation;
mod function_owner_query_mutation;
mod generic_owner_finalization;
mod generic_owner_work;
mod identifier;
mod identity;
#[cfg(test)]
mod identity_clone_work;
mod identity_semantics;
mod instantiated_terminal_index;
pub(crate) use instantiated_terminal_index::InstantiatedTerminalIndex;
#[cfg(test)]
pub(crate) use instantiated_terminal_index::with_terminal_resolution_scan_mutation;
mod lowering_identity;
mod lowering_record_semantics;
mod lowering_records;
mod nested_terminal_resolution;
mod parent_resolution;
mod pending_records;
mod prepared_emission;
mod record_resolution;
mod semantic_generic_tables;
mod semantic_path_operations;
mod semantic_records;
mod semantic_type;
mod semantic_type_operations;
mod semantic_type_semantics;
mod semantic_work;
mod specialization_context;
mod terminal_path_index;

pub use analysis_snapshot::*;
pub use direct_terminal_resolution::*;
pub use emission_frame::*;
pub use emission_index::*;
pub use emission_scope::*;
pub use errors::*;
pub use expanded_member_index::*;
pub use identifier::*;
pub use identity::*;
pub use lowering_records::*;
pub use nested_terminal_resolution::*;
pub use pending_records::*;
pub use prepared_emission::*;
pub use semantic_records::*;
pub use semantic_type::*;
pub use specialization_context::*;

#[cfg(feature = "nested-modport-test-utils")]
pub use emitter_index_work::{
    emitter_index_work, expanded_member_scan_mutation_enabled, record_external_emitter_index_work,
    record_resolved_member_lookup, reset_emitter_index_work, resolved_member_lookups,
    with_connected_map_scan_mutation, with_emitter_index_hash_collision,
    with_expanded_member_scan_mutation,
};

#[cfg(feature = "nested-modport-test-utils")]
pub fn reset_emission_query_work() {
    semantic_work::reset_emission_query_work();
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn emission_query_work() -> usize {
    semantic_work::emission_query_work()
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn with_terminal_availability_lookup_mutation<T>(operation: impl FnOnce() -> T) -> T {
    let guard = frame_record_query_mutation::inject_terminal_availability_lookup_mutation();
    let result = operation();
    drop(guard);
    result
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn with_instantiation_owner_lookup_mutation<T>(operation: impl FnOnce() -> T) -> T {
    let guard = frame_record_query_mutation::inject_instantiation_owner_lookup_mutation();
    let result = operation();
    drop(guard);
    result
}

#[cfg(feature = "nested-modport-test-utils")]
pub fn with_material_function_owner_mutation<T>(operation: impl FnOnce() -> T) -> T {
    let guard = function_owner_query_mutation::inject_material_function_owner_mutation();
    let result = operation();
    drop(guard);
    result
}

#[cfg(test)]
pub(crate) use terminal_path_index::with_terminal_scan_mutation;

#[cfg(test)]
pub(crate) use semantic_work::{reset_semantic_work, semantic_work};

#[cfg(test)]
pub(crate) use instantiated_terminal_index::{
    reset_terminal_resolution_work, terminal_resolution_work,
};

#[cfg(test)]
#[path = "nested_modport/tests.rs"]
mod tests;
