use super::collection_work::{counted, record_collection_work as record_binding_work};
use super::emission_handle_freeze::freeze_source_bindings;
use super::emission_index::*;
use super::errors::NestedModportAnalysisInvariant;
use super::identity::NestedModportLoweringKey;
use super::record_resolution::FrozenSemanticRecords;
use super::semantic_records::InstantiationContextKey;
use super::specialization_context::{
    EmissionOwnerKind, EmissionSpecializationContext, LoweringAvailability,
};
use crate::{HashMap, HashSet};
use std::sync::Arc;
use veryl_parser::resource_table::{PathId, TokenId};

#[cfg(test)]
thread_local! {
    static EXTRA_FULL_INDEX_SCAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) struct ExtraFullIndexScanGuard;

#[cfg(test)]
impl Drop for ExtraFullIndexScanGuard {
    fn drop(&mut self) {
        EXTRA_FULL_INDEX_SCAN.set(false);
    }
}

#[cfg(test)]
pub(super) fn inject_extra_full_index_scan() -> ExtraFullIndexScanGuard {
    EXTRA_FULL_INDEX_SCAN.set(true);
    ExtraFullIndexScanGuard
}

#[cfg(test)]
fn run_extra_full_index_scan(bindings: &[PendingEmissionBinding]) {
    EXTRA_FULL_INDEX_SCAN.with(|enabled| {
        if enabled.get() {
            for _ in counted(bindings) {
                for _ in counted(bindings) {}
            }
        }
    });
}

#[cfg(not(test))]
fn run_extra_full_index_scan(_bindings: &[PendingEmissionBinding]) {}

impl FrozenEmissionIndex {
    pub(super) fn build(
        bindings: Vec<PendingEmissionBinding>,
        empty_owners: HashSet<(PathId, TokenId, EmissionOwnerKind)>,
        records: &FrozenSemanticRecords,
        instantiation_contexts: &HashMap<InstantiationContextKey, EmissionSpecializationContext>,
    ) -> Result<Self, NestedModportAnalysisInvariant> {
        let mut contexts_by_owner: HashMap<_, Vec<_>> = HashMap::default();
        for (key, context) in instantiation_contexts {
            contexts_by_owner
                .entry(key.owner.clone())
                .or_default()
                .push((key.token, context.clone()));
        }
        let mut sources: HashMap<PathId, Vec<PendingEmissionBinding>> = HashMap::default();
        for binding in bindings {
            record_binding_work(3);
            sources.entry(binding.source).or_default().push(binding);
        }
        let mut empty_by_source: HashMap<PathId, HashSet<(TokenId, EmissionOwnerKind)>> =
            HashMap::default();
        for (source, declaration, kind) in empty_owners {
            record_binding_work(3);
            empty_by_source
                .entry(source)
                .or_default()
                .insert((declaration, kind));
        }
        let source_ids: HashSet<_> = sources
            .keys()
            .chain(empty_by_source.keys())
            .copied()
            .collect();
        record_binding_work(sources.len() + empty_by_source.len());
        let mut by_source = HashMap::default();
        for source in source_ids {
            record_binding_work(4);
            let table = build_source_table(
                source,
                sources.remove(&source).unwrap_or_default(),
                empty_by_source.remove(&source).unwrap_or_default(),
                records,
                &contexts_by_owner,
            )?;
            by_source.insert(SourceEmissionKey(source), Arc::new(table));
        }
        Ok(Self { by_source })
    }
}

fn build_source_table(
    source: PathId,
    bindings: Vec<PendingEmissionBinding>,
    empty_owners: HashSet<(TokenId, EmissionOwnerKind)>,
    records: &FrozenSemanticRecords,
    contexts_by_owner: &HashMap<
        NestedModportLoweringKey,
        Vec<(TokenId, EmissionSpecializationContext)>,
    >,
) -> Result<SourceEmissionTable, NestedModportAnalysisInvariant> {
    run_extra_full_index_scan(&bindings);
    let bindings = freeze_source_bindings(bindings, records, contexts_by_owner)?;
    let mut grouped: Vec<(EmissionOwnerGroupKey, Vec<EmissionOwnerBinding>)> = Vec::new();
    let mut positions = HashMap::default();
    for binding in bindings {
        record_binding_work(3);
        let key = EmissionOwnerGroupKey {
            declaration: binding.declaration,
            kind: binding.kind,
            scope: binding_scope(&binding),
        };
        let position = *positions.entry(key.clone()).or_insert_with(|| {
            record_binding_work(2);
            grouped.push((key, Vec::new()));
            grouped.len() - 1
        });
        record_binding_work(1);
        grouped[position].1.push(binding);
    }
    let mut bindings = Vec::new();
    let mut groups = Vec::new();
    let mut group_index = HashMap::default();
    let mut ordered: HashMap<EmissionScopeKey, Vec<usize>> = HashMap::default();
    for (key, values) in grouped {
        record_binding_work(5 + values.len());
        let start = bindings.len();
        bindings.extend(values);
        let index = groups.len();
        group_index.insert(key.clone(), index);
        ordered.entry(key.scope.clone()).or_default().push(index);
        groups.push(EmissionOwnerGroup {
            key,
            binding_range: start..bindings.len(),
        });
    }
    let package_scope_index = build_package_scope_index(&bindings);
    let nonempty_owners = groups
        .iter()
        .map(|group| {
            record_binding_work(2);
            (group.key.declaration, group.key.kind)
        })
        .collect();
    record_binding_work(2 * ordered.len() + 1);
    Ok(SourceEmissionTable {
        source,
        bindings: bindings.into(),
        groups: groups.into(),
        group_index,
        ordered_groups_by_scope: ordered
            .into_iter()
            .map(|(key, groups)| (key, groups.into()))
            .collect(),
        package_scope_index,
        nonempty_owners,
        empty_owners,
    })
}

fn build_package_scope_index(
    bindings: &[EmissionOwnerBinding],
) -> HashMap<PackageScopeLookupKey, PackageScopeIndex> {
    let mut ret = HashMap::default();
    for binding in bindings {
        record_binding_work(1);
        let Some(scope) = binding.package_scope.as_ref() else {
            continue;
        };
        record_binding_work(3);
        let key = PackageScopeLookupKey {
            declaration: scope.declaration,
            symbol: scope.symbol,
        };
        let semantic = scope.semantic_generic_map().clone();
        let index = ret.entry(key).or_insert_with(|| {
            record_binding_work(1);
            PackageScopeIndex {
                expected: semantic.clone(),
                entries: HashMap::default(),
            }
        });
        record_binding_work(1);
        match index.entries.entry(semantic) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                record_binding_work(1);
                entry.insert(PackageScopeEntry::Unique(scope.clone()));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                record_binding_work(1);
                if !matches!(entry.get(), PackageScopeEntry::Unique(previous) if previous == scope)
                {
                    entry.insert(PackageScopeEntry::Ambiguous);
                }
            }
        }
    }
    ret
}

pub(super) fn validate_bindings<'a>(
    bindings: impl IntoIterator<Item = &'a PendingEmissionBinding>,
    records: &FrozenSemanticRecords,
) -> Result<(), NestedModportAnalysisInvariant> {
    let mut ids = HashSet::default();
    for binding in bindings {
        record_binding_work(4);
        if !ids.insert(binding.id) {
            return Err(NestedModportAnalysisInvariant::DuplicateBindingId);
        }
        if !binding
            .emission_context
            .matches(binding.kind, &binding.specialization.specialization)
        {
            return Err(NestedModportAnalysisInvariant::MismatchedEmissionContext);
        }
        match (
            &binding.lowering,
            records.availability.get(&binding.specialization),
        ) {
            (LoweringAvailability::NotNested, Some(LoweringAvailability::NotNested)) => {}
            (LoweringAvailability::Found(bound), Some(LoweringAvailability::Found(known)))
                if Arc::ptr_eq(bound, known) => {}
            _ => return Err(NestedModportAnalysisInvariant::MissingLowering),
        }
        for key in counted(&*binding.required_rewrites) {
            if key.owner != *binding.specialization {
                return Err(NestedModportAnalysisInvariant::CrossOwnerRecord);
            }
            if !records.rewrites.contains_key(key) {
                return Err(NestedModportAnalysisInvariant::MissingRewrite);
            }
        }
        for key in counted(&*binding.required_expanded_ports) {
            if key.owner != *binding.specialization {
                return Err(NestedModportAnalysisInvariant::CrossOwnerRecord);
            }
            if !records.expanded_ports.contains_key(key) {
                return Err(NestedModportAnalysisInvariant::MissingExpandedPort);
            }
        }
    }
    Ok(())
}
