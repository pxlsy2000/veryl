use super::*;
use crate::ir::SystemVerilog;
use crate::nested_modport::{ComponentSpecializationIdentity, ConnectedInterfaceSpecialization};
use crate::symbol::SymbolId;
use veryl_parser::resource_table::StrId;

fn signature(symbol: usize, full_path: usize) -> Signature {
    let mut signature = Signature::new(SymbolId(symbol));
    signature.full_path.push(StrId(full_path));
    signature
}

fn cache_key(owner: &Signature, width: usize) -> ComponentCacheKey {
    ComponentCacheKey(
        ComponentSpecializationIdentity::new(
            owner.clone(),
            [ConnectedInterfaceSpecialization {
                formal_port: StrId(4),
                actual: signature(2, width),
            }],
        )
        .expect("one connected actual is valid"),
    )
}

fn component(name: usize) -> Arc<Component> {
    Arc::new(Component::SystemVerilog(SystemVerilog {
        name: StrId(name),
        connects: vec![],
    }))
}

#[test]
fn completed_cache_distinguishes_alternating_connected_specializations() {
    let owner = signature(1, 1);
    let key8 = cache_key(&owner, 8);
    let key16 = cache_key(&owner, 16);
    let mut history = InstanceHistory::default();
    let config = Config::default();

    assert_eq!(history.push(owner.clone(), key8.clone(), &config), Ok(true));
    history.set(&key8, component(8), false);
    history.pop();
    assert_eq!(
        history.push(owner.clone(), key16.clone(), &config),
        Ok(true)
    );
    history.set(&key16, component(16), false);
    history.pop();

    for (key, expected_name) in [(&key8, StrId(8)), (&key16, StrId(16)), (&key8, StrId(8))] {
        let (cached, _) = history.get(key).expect("specialization should be cached");
        let Component::SystemVerilog(cached) = cached.as_ref() else {
            panic!("expected synthetic SystemVerilog cache entry");
        };
        assert_eq!(cached.name, expected_name);
    }
}

#[test]
fn total_limit_counts_distinct_connected_specializations_of_one_owner() {
    let owner = signature(1, 1);
    let keys = [
        cache_key(&owner, 8),
        cache_key(&owner, 16),
        cache_key(&owner, 32),
    ];
    let mut history = InstanceHistory::default();
    let config = Config {
        instance_total_limit: 1,
        ..Config::default()
    };

    for (key, width) in keys[..2].iter().zip([8, 16]) {
        assert_eq!(history.push(owner.clone(), key.clone(), &config), Ok(true));
        history.set(key, component(width), false);
        history.pop();
    }

    assert_eq!(
        history.push(owner, keys[2].clone(), &config),
        Err(InstanceHistoryError::ExceedTotalLimit(2))
    );

    for (key, expected_name) in keys[..2].iter().zip([StrId(8), StrId(16)]) {
        let (cached, _) = history.get(key).expect("specialization should be cached");
        let Component::SystemVerilog(cached) = cached.as_ref() else {
            panic!("expected synthetic SystemVerilog cache entry");
        };
        assert_eq!(cached.name, expected_name);
    }
    assert_eq!(history.full.len(), 2);
}

#[test]
fn total_limit_allows_same_owner_specializations_below_existing_boundary() {
    let owner = signature(1, 1);
    let keys = [cache_key(&owner, 8), cache_key(&owner, 16)];
    let mut history = InstanceHistory::default();
    let config = Config {
        instance_total_limit: 1,
        ..Config::default()
    };

    for (key, width) in keys.iter().zip([8, 16]) {
        assert_eq!(history.push(owner.clone(), key.clone(), &config), Ok(true));
        history.set(key, component(width), false);
        history.pop();
    }

    assert_eq!(history.full.len(), 2);
}

#[test]
fn total_limit_retains_existing_distinct_owner_boundary() {
    let owners = [signature(1, 1), signature(2, 2), signature(3, 3)];
    let keys = owners.each_ref().map(|owner| cache_key(owner, 8));
    let mut history = InstanceHistory::default();
    let config = Config {
        instance_total_limit: 1,
        ..Config::default()
    };

    for index in 0..2 {
        assert_eq!(
            history.push(owners[index].clone(), keys[index].clone(), &config),
            Ok(true)
        );
        history.set(&keys[index], component(index), false);
        history.pop();
    }

    assert_eq!(
        history.push(owners[2].clone(), keys[2].clone(), &config),
        Err(InstanceHistoryError::ExceedTotalLimit(2))
    );
}

#[test]
fn removing_specialization_releases_its_total_slot() {
    let first_owner = signature(1, 1);
    let key8 = cache_key(&first_owner, 8);
    let key16 = cache_key(&first_owner, 16);
    let next_owner = signature(2, 2);
    let next_key = cache_key(&next_owner, 8);
    let final_owner = signature(3, 3);
    let final_key = cache_key(&final_owner, 8);
    let mut history = InstanceHistory::default();
    let config = Config {
        instance_total_limit: 1,
        ..Config::default()
    };

    for (key, width) in [(&key8, 8), (&key16, 16)] {
        assert_eq!(
            history.push(first_owner.clone(), key.clone(), &config),
            Ok(true)
        );
        history.set(key, component(width), false);
        history.pop();
    }

    history.remove(&key8);
    assert_eq!(
        history.push(next_owner.clone(), next_key.clone(), &config),
        Ok(true)
    );
    history.pop();
    assert_eq!(
        history.push(final_owner.clone(), final_key.clone(), &config),
        Err(InstanceHistoryError::ExceedTotalLimit(2))
    );

    history.remove(&key16);
    assert_eq!(history.push(final_owner, final_key, &config), Ok(true));
}

#[test]
fn recursion_tracking_uses_owner_even_when_connected_actual_changes() {
    let owner = signature(1, 1);
    let key8 = cache_key(&owner, 8);
    let key16 = cache_key(&owner, 16);
    let mut history = InstanceHistory::default();

    assert_eq!(
        history.push(owner.clone(), key8, &Config::default()),
        Ok(true)
    );
    assert!(matches!(
        history.push(owner, key16, &Config::default()),
        Err(InstanceHistoryError::InfiniteRecursion)
    ));
}

#[test]
fn dropping_connected_actuals_reproduces_the_rejected_cache_alias() {
    let owner = signature(1, 1);
    let key8 = cache_key(&owner, 8);
    let key16 = cache_key(&owner, 16);
    let owner_only8 = ComponentCacheKey(
        ComponentSpecializationIdentity::new(owner.clone(), [])
            .expect("empty connected actuals are valid"),
    );
    let owner_only16 = ComponentCacheKey(
        ComponentSpecializationIdentity::new(owner, []).expect("empty connected actuals are valid"),
    );

    assert_eq!(owner_only8, owner_only16);
    assert_ne!(key8, key16);
}
