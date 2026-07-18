#[cfg(any(test, feature = "nested-modport-test-utils"))]
thread_local! {
    static MATERIAL_FUNCTION_OWNER_MUTATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) struct MaterialFunctionOwnerMutationGuard;

#[cfg(any(test, feature = "nested-modport-test-utils"))]
impl Drop for MaterialFunctionOwnerMutationGuard {
    fn drop(&mut self) {
        MATERIAL_FUNCTION_OWNER_MUTATION.set(false);
    }
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) fn inject_material_function_owner_mutation() -> MaterialFunctionOwnerMutationGuard {
    MATERIAL_FUNCTION_OWNER_MUTATION.set(true);
    MaterialFunctionOwnerMutationGuard
}

#[cfg(any(test, feature = "nested-modport-test-utils"))]
pub(super) fn material_function_owner_mutation_enabled() -> bool {
    MATERIAL_FUNCTION_OWNER_MUTATION.get()
}
