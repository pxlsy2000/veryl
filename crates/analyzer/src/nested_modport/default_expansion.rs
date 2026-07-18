use crate::ir::{Function, ModportMemberPath, VarId, Variable};
use crate::symbol::Direction;
use crate::{HashMap, HashSet};
use veryl_parser::resource_table::StrId;
use veryl_parser::token_range::TokenRange;

use super::default_expansion_work::{
    member_rescan_mutation_enabled, record_default_expansion_work,
};
use super::default_member_order::{
    DefaultMemberOrder, member_order_build_budget, ordered_merge_budget, rescanned_entries,
};
use super::effective_member_set::EffectiveMemberSet;
use super::errors::NestedLoweringResolveError;
use super::lowering_records::*;

pub(super) fn expand_effective_modports(
    pending: &PendingComponentLowering,
    variables: &HashMap<VarId, Variable>,
    functions: &HashMap<VarId, Function>,
    expansion_limit: usize,
) -> Result<HashMap<StrId, Vec<PendingModportEntry>>, NestedLoweringResolveError> {
    EffectiveModportExpander::new(pending, variables, functions, expansion_limit).expand_all()
}

struct EffectiveModportExpander<'a> {
    declarations: HashMap<StrId, &'a PendingModportDeclaration>,
    declaration_order: Vec<StrId>,
    variables: &'a HashMap<VarId, Variable>,
    functions: &'a HashMap<VarId, Function>,
    member_order: Option<DefaultMemberOrder>,
    memo: HashMap<StrId, Vec<PendingModportEntry>>,
    visiting_set: HashSet<StrId>,
    requested: usize,
    expansion_limit: usize,
}

impl<'a> EffectiveModportExpander<'a> {
    fn new(
        pending: &'a PendingComponentLowering,
        variables: &'a HashMap<VarId, Variable>,
        functions: &'a HashMap<VarId, Function>,
        expansion_limit: usize,
    ) -> Self {
        Self {
            declarations: pending
                .declarations
                .iter()
                .map(|declaration| (declaration.name, declaration))
                .collect(),
            declaration_order: pending
                .declarations
                .iter()
                .map(|declaration| declaration.name)
                .collect(),
            variables,
            functions,
            member_order: None,
            memo: HashMap::default(),
            visiting_set: HashSet::default(),
            requested: 0,
            expansion_limit,
        }
    }

    fn expand_all(
        mut self,
    ) -> Result<HashMap<StrId, Vec<PendingModportEntry>>, NestedLoweringResolveError> {
        let names = self.declaration_order.clone();
        for name in names {
            self.expand(name)?;
        }
        Ok(self.memo)
    }

    fn expand(
        &mut self,
        name: StrId,
    ) -> Result<Vec<PendingModportEntry>, NestedLoweringResolveError> {
        if let Some(entries) = self.memo.get(&name) {
            return Ok(entries.clone());
        }
        self.declarations
            .get(&name)
            .ok_or(NestedLoweringResolveError::MissingModport {
                name,
                origin: TokenRange::default(),
            })?;
        let mut stack = vec![EffectiveExpansionFrame {
            name,
            next_target: 0,
        }];
        self.visiting_set.insert(name);

        while let Some(frame) = stack.last_mut() {
            let current = frame.name;
            let declaration = self.declarations[&current].clone();
            let targets = match &declaration.default {
                Some(PendingModportDefault::Same(targets))
                | Some(PendingModportDefault::Converse(targets)) => targets.as_slice(),
                Some(PendingModportDefault::Input) | Some(PendingModportDefault::Output) | None => {
                    &[]
                }
            };
            if let Some((target, origin)) = targets.get(frame.next_target).copied() {
                frame.next_target += 1;
                self.charge_work(1)?;
                if self.memo.contains_key(&target) {
                    continue;
                }
                if self.visiting_set.contains(&target) {
                    return Err(NestedLoweringResolveError::DefaultCycle { target, origin });
                }
                if !self.declarations.contains_key(&target) {
                    return Err(NestedLoweringResolveError::MissingModport {
                        name: target,
                        origin,
                    });
                }
                self.visiting_set.insert(target);
                stack.push(EffectiveExpansionFrame {
                    name: target,
                    next_target: 0,
                });
                continue;
            }

            let entries = self.materialize(&declaration)?;
            if declaration.contains_nested_item && entries.is_empty() {
                return Err(NestedLoweringResolveError::EmptyModport {
                    name: current,
                    origin: declaration.origin,
                });
            }
            self.memo.insert(current, entries);
            stack.pop();
            self.visiting_set.remove(&current);
        }
        self.memo
            .get(&name)
            .cloned()
            .ok_or(NestedLoweringResolveError::MissingModport {
                name,
                origin: TokenRange::default(),
            })
    }

    fn charge_work(&mut self, units: usize) -> Result<(), NestedLoweringResolveError> {
        record_default_expansion_work(units);
        self.requested = self.requested.saturating_add(units);
        if self.requested > self.expansion_limit {
            Err(NestedLoweringResolveError::ExpansionBudget {
                requested: self.requested,
                limit: self.expansion_limit,
            })
        } else {
            Ok(())
        }
    }

    fn materialize(
        &mut self,
        declaration: &PendingModportDeclaration,
    ) -> Result<Vec<PendingModportEntry>, NestedLoweringResolveError> {
        let mut entries = EffectiveMemberSet::default();
        for entry in &declaration.explicit {
            self.charge_work(1)?;
            entries.merge_explicit(entry.clone());
        }
        match &declaration.default {
            Some(PendingModportDefault::Input) | Some(PendingModportDefault::Output) => {
                let direction = if matches!(declaration.default, Some(PendingModportDefault::Input))
                {
                    Direction::Input
                } else {
                    Direction::Output
                };
                let mut members: Vec<_> = self.variables.values().collect();
                members.sort_by_key(|variable| variable.id);
                for variable in members {
                    self.charge_work(1)?;
                    entries.merge_default(PendingModportEntry {
                        path: ModportMemberPath::from_slice(variable.path.0.as_slice()),
                        direction,
                        origin: declaration.origin,
                    });
                }
            }
            Some(PendingModportDefault::Same(targets))
            | Some(PendingModportDefault::Converse(targets)) => {
                let converse = matches!(
                    declaration.default,
                    Some(PendingModportDefault::Converse(_))
                );
                if self.member_order.is_none() && !member_rescan_mutation_enabled() {
                    let count = self.variables.len().saturating_add(self.functions.len());
                    self.charge_work(member_order_build_budget(count))?;
                    self.member_order =
                        Some(DefaultMemberOrder::build(self.variables, self.functions));
                }
                let mut directions = HashMap::default();
                for (target, _) in targets {
                    let target_entry_count = self.memo[target].len();
                    for entry_index in 0..target_entry_count {
                        self.charge_work(1)?;
                        let target_entry = &self.memo[target][entry_index];
                        let direction = if converse {
                            match target_entry.direction {
                                Direction::Input => Some(Direction::Output),
                                Direction::Output => Some(Direction::Input),
                                Direction::Inout => Some(Direction::Inout),
                                Direction::Import | Direction::Modport | Direction::Interface => {
                                    None
                                }
                            }
                        } else {
                            Some(target_entry.direction)
                        };
                        if let Some(direction) = direction {
                            directions.insert(
                                target_entry.path.clone(),
                                (direction, target_entry.origin),
                            );
                        }
                    }
                }
                if member_rescan_mutation_enabled() {
                    let count = self.variables.len().saturating_add(self.functions.len());
                    self.charge_work(member_order_build_budget(count))?;
                    for entry in rescanned_entries(self.variables, self.functions, &directions) {
                        entries.merge_default(entry);
                    }
                } else {
                    self.charge_work(ordered_merge_budget(directions.len()))?;
                    if let Some(order) = self.member_order.as_ref() {
                        for entry in order.ordered_entries(directions) {
                            entries.merge_default(entry);
                        }
                    }
                }
            }
            None => {}
        }
        Ok(entries.into_entries())
    }
}

struct EffectiveExpansionFrame {
    name: StrId,
    next_target: usize,
}
