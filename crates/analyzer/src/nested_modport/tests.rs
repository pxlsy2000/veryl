pub use super::*;
use super::{collection_work, effective_member_set, prepared_emission};

mod binding_scaling_tests {
    include!("binding_scaling_tests.rs");
}

mod function_index_scaling_tests {
    include!("function_index_scaling_tests.rs");
}

mod index_locality_tests {
    include!("index_locality_tests.rs");
}

mod owner_index_scaling_tests {
    include!("owner_index_scaling_tests.rs");
}

mod parent_identity_tests {
    include!("parent_identity_tests.rs");
}

mod material_parent_staging_tests {
    include!("material_parent_staging_tests.rs");
}

mod production_path_scaling_tests {
    include!("production_path_scaling_tests.rs");
}

mod occurrence_lookup_scaling_tests {
    include!("occurrence_lookup_scaling_tests.rs");
}

mod semantic_work_scaling_tests {
    include!("semantic_work_scaling_tests.rs");
}

mod binding_key_semantic_scaling_tests {
    include!("binding_key_semantic_scaling_tests.rs");
}

mod production_binding_generic_tables_tests {
    include!("production_binding_generic_tables_tests.rs");
}

mod default_expansion_scaling_tests {
    include!("default_expansion_scaling_tests.rs");
}

mod generic_owner_scaling_tests {
    include!("generic_owner_scaling_tests.rs");
}

mod collision_tests {
    include!("collision_tests.rs");
}

mod lowering_tests {
    include!("../nested_modport_lowering_tests.rs");
}
