#[test]
fn member_lookup_is_affine_when_references_and_members_scale_together() {
    let normal = [16, 32, 64].map(|scale| emit_measured(&member_fixture(scale)));
    let mutation = [16, 32, 64]
        .map(|scale| with_expanded_member_scan_mutation(|| emit_measured(&member_fixture(scale))));
    for (level, measurement) in normal.iter().enumerate() {
        assert_eq!(measurement.resolved_member_lookups, 0);
        for index in 0..(16 << level) {
            assert!(
                measurement.text.contains(&format!("__p_v{index}")),
                "{}",
                measurement.text
            );
        }
    }
    assert_affine(&normal, &mutation);
}

#[test]
fn nested_member_lookup_is_affine_at_the_resolved_longest_prefix_seam() {
    let normal = [16, 32, 64].map(|scale| emit_measured(&nested_member_scaling_fixture(scale)));
    let mutation = [16, 32, 64].map(|scale| {
        with_expanded_member_scan_mutation(|| emit_measured(&nested_member_scaling_fixture(scale)))
    });
    println!(
        "resolved lookups indexed={:?} scan_mutation={:?}",
        normal.each_ref().map(|value| value.resolved_member_lookups),
        mutation
            .each_ref()
            .map(|value| value.resolved_member_lookups)
    );
    for (level, (normal, mutation)) in normal.iter().zip(&mutation).enumerate() {
        let members = 16 << level;
        assert!(
            normal.resolved_member_lookups >= 2 * members,
            "resolved branch count {} did not cover {members} members in both generic functions",
            normal.resolved_member_lookups
        );
        assert_eq!(
            normal.resolved_member_lookups,
            mutation.resolved_member_lookups
        );
        assert!(normal.text.contains("Data__8"), "{}", normal.text);
        assert!(normal.text.contains("Data__16"), "{}", normal.text);
        assert!(normal.text.contains("[1].bits[0]"), "{}", normal.text);
        assert!(!normal.text.contains("p.child.v"), "{}", normal.text);
        for index in 0..members {
            assert!(
                normal.text.contains(&format!("__p_child__v{index}")),
                "{}",
                normal.text
            );
        }
    }
    assert_affine(&normal, &mutation);
}

#[test]
fn emitter_indexes_are_collision_safe_for_generic_and_nested_semantic_keys() {
    for code in [connected_fixture(16), nested_member_fixture().to_owned()] {
        let normal = emit_measured(&code);
        let collided = with_emitter_index_hash_collision(|| emit_measured(&code));
        assert_eq!(normal.text, collided.text);
        assert_eq!(normal.source_map, collided.source_map);
    }
}
