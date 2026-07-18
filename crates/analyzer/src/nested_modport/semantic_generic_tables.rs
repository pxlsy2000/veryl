use super::semantic_path_operations::{generic_path_eq, hash_generic_path};
use super::semantic_work::{counted_sort_by, record_semantic_work};
use crate::HashMap;
use crate::namespace::DefineContext;
use crate::scope::ScopeId;
use crate::symbol::{GenericTable, GenericTables};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use veryl_parser::resource_table::StrId;

#[derive(Clone, Copy)]
struct GenericTableKey<'a> {
    scope: ScopeId,
    context: &'a DefineContext,
}

impl PartialEq for GenericTableKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.scope == other.scope && context_eq(self.context, other.context)
    }
}

impl Eq for GenericTableKey<'_> {}

impl Hash for GenericTableKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.scope.hash(state);
        hash_context(self.context, state);
    }
}

pub(super) fn generic_tables_eq(left: &GenericTables, right: &GenericTables) -> bool {
    record_semantic_work(1);
    if left.len() != right.len() {
        return false;
    }
    let right: HashMap<_, _> = right
        .iter()
        .map(|((scope, context), table)| {
            record_semantic_work(1);
            (
                GenericTableKey {
                    scope: *scope,
                    context,
                },
                table,
            )
        })
        .collect();
    left.iter().all(|((scope, context), table)| {
        record_semantic_work(1);
        right
            .get(&GenericTableKey {
                scope: *scope,
                context,
            })
            .is_some_and(|right| table_eq(table, right))
    })
}

pub(super) fn hash_generic_tables<H: Hasher>(tables: &GenericTables, state: &mut H) {
    let mut outer: Vec<_> = tables.iter().collect();
    counted_sort_by(&mut outer, |((ls, lc), _), ((rs, rc), _)| {
        ls.cmp(rs).then_with(|| context_cmp(lc, rc))
    });
    record_semantic_work(1);
    outer.len().hash(state);
    for ((scope, context), table) in outer {
        record_semantic_work(1);
        scope.hash(state);
        hash_context(context, state);
        let mut entries: Vec<_> = table.iter().collect();
        counted_sort_by(&mut entries, |(left, _), (right, _)| left.cmp(right));
        record_semantic_work(1);
        entries.len().hash(state);
        for (name, path) in entries {
            record_semantic_work(1);
            name.hash(state);
            hash_generic_path(path, state);
        }
    }
}

fn table_eq(left: &GenericTable, right: &GenericTable) -> bool {
    record_semantic_work(1);
    left.len() == right.len()
        && left.iter().all(|(name, path)| {
            record_semantic_work(1);
            right
                .get(name)
                .is_some_and(|right| generic_path_eq(path, right))
        })
}

fn context_eq(left: &DefineContext, right: &DefineContext) -> bool {
    iter_eq(left.positive_defines(), right.positive_defines())
        && iter_eq(left.negative_defines(), right.negative_defines())
}

fn context_cmp(left: &DefineContext, right: &DefineContext) -> Ordering {
    iter_cmp(left.positive_defines(), right.positive_defines())
        .then_with(|| iter_cmp(left.negative_defines(), right.negative_defines()))
}

fn hash_context<H: Hasher>(context: &DefineContext, state: &mut H) {
    hash_iter(context.positive_defines(), state);
    hash_iter(context.negative_defines(), state);
}

fn iter_eq<'a>(
    mut left: impl Iterator<Item = &'a StrId>,
    mut right: impl Iterator<Item = &'a StrId>,
) -> bool {
    loop {
        record_semantic_work(1);
        match (left.next(), right.next()) {
            (Some(left), Some(right)) if left == right => {}
            (None, None) => return true,
            (Some(_), Some(_)) | (None, Some(_)) | (Some(_), None) => return false,
        }
    }
}

fn iter_cmp<'a>(
    mut left: impl Iterator<Item = &'a StrId>,
    mut right: impl Iterator<Item = &'a StrId>,
) -> Ordering {
    loop {
        record_semantic_work(1);
        match (left.next(), right.next()) {
            (Some(left), Some(right)) => match left.cmp(right) {
                Ordering::Equal => {}
                ordering => return ordering,
            },
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

fn hash_iter<'a, H: Hasher>(values: impl Iterator<Item = &'a StrId>, state: &mut H) {
    let mut length = 0;
    for value in values {
        record_semantic_work(1);
        value.hash(state);
        length += 1;
    }
    length.hash(state);
}
