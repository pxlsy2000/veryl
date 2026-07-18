fn binding_key(
    session: AnalysisSessionId,
    actual_suffix: &str,
) -> NestedModportLoweringKey {
    let owner = crate::ir::Signature::new(SymbolId(81_000));
    let mut actual = crate::ir::Signature::new(SymbolId(82_000));
    actual
        .full_path
        .push(resource_table::insert_str(actual_suffix));
    NestedModportLoweringKey {
        session,
        specialization: ComponentSpecializationIdentity::new(
            owner,
            [ConnectedInterfaceSpecialization {
                formal_port: resource_table::insert_str("child"),
                actual,
            }],
        )
        .expect("unique connected actual")
        .into(),
    }
}

#[test]
fn semantic_lowering_interning_is_session_scoped_and_collision_safe() {
    let session = AnalysisSessionId::new();
    let other_session = AnalysisSessionId::new();
    let mut interner = SessionLoweringInterner::with_hasher(BuildHasherDefault::<
        super::semantic_work_scaling_tests::ConstantHasher,
    >::default());
    let first = Arc::new(lowering(4));
    let equal = Arc::new((*first).clone());
    let distinct = Arc::new(lowering(5));
    let first_identity = interner.intern(session, first);
    assert_eq!(first_identity, interner.intern(session, equal));
    assert_ne!(first_identity, interner.intern(session, distinct));

    let mut other_interner = SessionLoweringInterner::with_hasher(BuildHasherDefault::<
        super::semantic_work_scaling_tests::ConstantHasher,
    >::default());
    assert_ne!(
        first_identity,
        other_interner.intern(other_session, Arc::new(lowering(4)))
    );
}

#[test]
fn binding_specialization_interner_ignores_transient_allocation_addresses() {
    let session = AnalysisSessionId::new();
    let other_session = AnalysisSessionId::new();
    let mut interner = BindingSpecializationInterner::with_hasher(BuildHasherDefault::<
        super::semantic_work_scaling_tests::ConstantHasher,
    >::default());

    let (canonical, first_identity) = interner.intern(Arc::new(binding_key(session, "actual_a")));
    for _ in 0..1_024 {
        let candidate = Arc::new(binding_key(session, "actual_a"));
        let (returned, identity) = interner.intern(candidate);
        assert_eq!(identity, first_identity);
        assert!(Arc::ptr_eq(&returned, &canonical));
        drop(returned);
        let churn = (0..8)
            .map(|_| Arc::new(binding_key(session, "allocator_churn")))
            .collect::<Vec<_>>();
        drop(churn);
    }

    assert_ne!(
        first_identity,
        interner.intern(Arc::new(binding_key(session, "actual_b"))).1
    );
    assert_ne!(
        first_identity,
        interner
            .intern(Arc::new(binding_key(other_session, "actual_a")))
            .1
    );
}
