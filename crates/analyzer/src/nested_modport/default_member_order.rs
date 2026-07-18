use super::default_expansion_work::record_default_expansion_work;
use super::lowering_records::PendingModportEntry;
use crate::HashMap;
use crate::ir::{Function, ModportMemberPath, VarId, Variable};
use crate::symbol::Direction;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use veryl_parser::token_range::TokenRange;

pub(super) struct DefaultMemberOrder {
    rank_by_path: HashMap<CountedMemberPath, usize>,
}

#[derive(Clone, Eq)]
struct CountedMemberPath(ModportMemberPath);

impl PartialEq for CountedMemberPath {
    fn eq(&self, other: &Self) -> bool {
        record_default_expansion_work(1);
        self.0 == other.0
    }
}

impl Hash for CountedMemberPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_default_expansion_work(1);
        self.0.hash(state);
    }
}

impl DefaultMemberOrder {
    pub(super) fn build(
        variables: &HashMap<VarId, Variable>,
        functions: &HashMap<VarId, Function>,
    ) -> Self {
        let mut members = members(variables, functions);
        members.sort_by(|left, right| {
            record_default_expansion_work(1);
            left.0.cmp(&right.0)
        });
        Self {
            rank_by_path: members
                .into_iter()
                .enumerate()
                .map(|(rank, (_, path))| (CountedMemberPath(path), rank))
                .collect(),
        }
    }

    pub(super) fn ordered_entries(
        &self,
        directions: HashMap<ModportMemberPath, (Direction, TokenRange)>,
    ) -> Vec<PendingModportEntry> {
        let mut ordered = BTreeMap::new();
        for (path, (direction, origin)) in directions {
            if let Some(rank) = self.rank_by_path.get(&CountedMemberPath(path.clone())) {
                ordered.insert(
                    *rank,
                    PendingModportEntry {
                        path,
                        direction,
                        origin,
                    },
                );
            }
        }
        ordered.into_values().collect()
    }
}

pub(super) fn rescanned_entries(
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    directions: &HashMap<ModportMemberPath, (Direction, TokenRange)>,
) -> Vec<PendingModportEntry> {
    let mut members = members(variables, functions);
    members.sort_by(|left, right| {
        record_default_expansion_work(1);
        left.0.cmp(&right.0)
    });
    members
        .into_iter()
        .filter_map(|(_, path)| {
            directions
                .get(&path)
                .map(|(direction, origin)| PendingModportEntry {
                    path,
                    direction: *direction,
                    origin: *origin,
                })
        })
        .collect()
}

pub(super) fn member_order_build_budget(count: usize) -> usize {
    count
        .saturating_mul(2)
        .saturating_add(count.saturating_mul(logarithmic_bound(count)))
}

pub(super) fn ordered_merge_budget(output_bound: usize) -> usize {
    output_bound.saturating_mul(1 + logarithmic_bound(output_bound))
}

fn members(
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
) -> Vec<(VarId, ModportMemberPath)> {
    variables
        .values()
        .map(|variable| {
            (
                variable.id,
                ModportMemberPath::from_slice(variable.path.0.as_slice()),
            )
        })
        .chain(
            functions
                .values()
                .map(|function| (function.id, ModportMemberPath::from_slice(&[function.name]))),
        )
        .collect()
}

fn logarithmic_bound(count: usize) -> usize {
    usize::BITS as usize - count.max(1).leading_zeros() as usize
}
