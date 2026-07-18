use crate::Context;
use crate::ir::{Ir, Signature, Type, TypeKind, ValueVariant, VarId};
use crate::namespace::Namespace;
use crate::nested_modport::*;
use crate::symbol::{DocComment, GenericMap, Symbol, SymbolId, SymbolKind};
use crate::symbol_path::{GenericSymbol, GenericSymbolPath, GenericSymbolPathKind};
use crate::{Analyzer, attribute_table, symbol_table};
use miette::Diagnostic;
use std::sync::Arc;
use veryl_metadata::Metadata;
use veryl_parser::Parser;
use veryl_parser::resource_table::{self, PathId, StrId, TokenId};
use veryl_parser::token_range::TokenRange;
use veryl_parser::veryl_token::Token;

#[test]
fn child_and_inherited_contexts_share_session_without_allocating_another_id() {
    let mut parent = Context::default();
    assert_eq!(parent.analysis_session_allocation_count(), 0);
    let parent_session = parent.analysis_session_id();
    assert_eq!(parent.analysis_session_allocation_count(), 1);
    let parent_pending = parent.pending_nested_modport_analysis();
    let mut child = parent.child();

    assert_eq!(child.analysis_session_allocation_count(), 1);
    assert_eq!(child.analysis_session_id(), parent_session);
    assert_eq!(child.analysis_session_allocation_count(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &child.pending_nested_modport_analysis(),
        &parent_pending
    ));

    child.inherit(&mut parent);
    assert_eq!(child.analysis_session_id(), parent_session);
    assert_eq!(child.analysis_session_allocation_count(), 1);
}

#[test]
fn independent_contexts_have_distinct_sessions_and_pending_owners() -> Result<(), &'static str> {
    let first = Context::default();
    let second = std::thread::spawn(Context::default)
        .join()
        .map_err(|_| "context worker should not panic")?;

    assert_ne!(first.analysis_session_id(), second.analysis_session_id());
    assert!(!std::sync::Arc::ptr_eq(
        &first.pending_nested_modport_analysis(),
        &second.pending_nested_modport_analysis()
    ));
    Ok(())
}

#[test]
fn poisoned_pending_state_is_not_reported_as_already_finalized() -> Result<(), &'static str> {
    let mut context = Context::default();
    let pending = context.pending_nested_modport_analysis();
    let poisoner = std::thread::spawn(move || {
        let _guard = pending.lock().map_err(|_| "fresh mutex should lock")?;
        panic!("intentional pending-state poison");
        #[allow(unreachable_code)]
        Ok::<(), &'static str>(())
    });
    assert!(poisoner.join().is_err());

    let error = context
        .finish_nested_modport_analysis()
        .expect_err("poisoned pending state must be rejected");
    assert!(matches!(
        error,
        crate::AnalyzerError::NestedModportAnalysisInvariant {
            kind: NestedModportAnalysisInvariant::PendingStatePoisoned,
            ..
        }
    ));
    Ok(())
}

#[test]
fn active_connected_actuals_define_the_component_specialization() -> Result<(), &'static str> {
    let owner = Signature::new(SymbolId(1));
    let mut child8 = Signature::new(SymbolId(2));
    child8.full_path.push(StrId(8));
    let mut child16 = Signature::new(SymbolId(2));
    child16.full_path.push(StrId(16));
    let mut context = Context::default();
    context.modport_signatures.push(Default::default());
    let connected_actuals = context
        .modport_signatures
        .last_mut()
        .ok_or("active connected actual table should exist")?;
    connected_actuals.insert(StrId(4), child8);
    let identity8 = context.component_specialization_identity(&owner);
    context.modport_signatures.pop();
    context.modport_signatures.push(Default::default());
    let connected_actuals = context
        .modport_signatures
        .last_mut()
        .ok_or("active connected actual table should exist")?;
    connected_actuals.insert(StrId(4), child16);
    let identity16 = context.component_specialization_identity(&owner);

    assert_ne!(identity8, identity16);
    assert_eq!(identity8.connected_actuals[0].formal_port, StrId(4));
    assert_eq!(
        identity16.connected_actuals[0].actual.full_path,
        [StrId(16)]
    );
    Ok(())
}

fn signature(symbol: usize, width: usize) -> Signature {
    let mut signature = Signature::new(SymbolId(symbol));
    signature
        .parameters
        .push((StrId(10), ValueVariant::Unknown));
    signature.full_path.push(StrId(width));
    signature
}

fn connected(formal_port: usize, width: usize) -> ConnectedInterfaceSpecialization {
    ConnectedInterfaceSpecialization {
        formal_port: StrId(formal_port),
        actual: signature(2, width),
    }
}

fn valid_identity(
    owner: Signature,
    connected_actuals: Vec<ConnectedInterfaceSpecialization>,
) -> Result<ComponentSpecializationIdentity, SpecializationIdentityError> {
    ComponentSpecializationIdentity::new(owner, connected_actuals)
}

#[test]
fn analysis_session_ids_are_isolated_across_interleaved_and_parallel_creation()
-> Result<(), &'static str> {
    let first = AnalysisSessionId::new();
    let middle = std::thread::spawn(AnalysisSessionId::new)
        .join()
        .map_err(|_| "session worker should not panic")?;
    let last = AnalysisSessionId::new();

    assert_ne!(first, middle);
    assert_ne!(middle, last);
    assert_ne!(first, last);
    Ok(())
}

#[test]
fn specialization_identity_normalizes_order_and_rejects_conflicts()
-> Result<(), SpecializationIdentityError> {
    let mut owner_forward = signature(1, 1);
    owner_forward
        .parameters
        .push((StrId(9), ValueVariant::Unknown));
    let mut owner_reverse = owner_forward.clone();
    owner_reverse.parameters.reverse();
    let child8 = connected(8, 8);
    let child16 = connected(16, 16);

    let forward = valid_identity(owner_forward, vec![child16.clone(), child8.clone()])?;
    let reverse = valid_identity(owner_reverse, vec![child8.clone(), child16.clone()])?;
    assert_eq!(forward, reverse);

    let duplicate = valid_identity(signature(1, 1), vec![child8.clone(), child8])?;
    assert_eq!(duplicate.connected_actuals.len(), 1);

    let conflict =
        ComponentSpecializationIdentity::new(signature(1, 1), [child16, connected(16, 8)]);
    assert_eq!(
        conflict,
        Err(SpecializationIdentityError::ConflictingConnectedActual {
            formal_port: StrId(16)
        })
    );
    Ok(())
}

#[test]
fn specialization_identity_distinguishes_owner_and_connected_actual_signatures()
-> Result<(), SpecializationIdentityError> {
    let base = valid_identity(signature(1, 1), vec![connected(3, 8)])?;
    let owner_path = valid_identity(signature(1, 2), vec![connected(3, 8)])?;
    let mut parameter_owner = signature(1, 1);
    parameter_owner
        .parameters
        .push((StrId(11), ValueVariant::Unknown));
    let owner_parameter = valid_identity(parameter_owner, vec![connected(3, 8)])?;
    let connected16 = valid_identity(signature(1, 1), vec![connected(3, 16)])?;

    assert_ne!(base, owner_path);
    assert_ne!(base, owner_parameter);
    assert_ne!(base, connected16);
    Ok(())
}
