use crate::HashMap;
use crate::ir::{Comptime, FuncPath, Function, VarId, VarPath, Variable};
use crate::nested_modport::{NestedModportLowering, ResolvedModportEntry};
use crate::symbol::Direction;
use indent::indent_all_by;
use std::fmt;
use std::sync::Arc;
use veryl_parser::resource_table::StrId;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ModportMemberPath(Vec<StrId>);

impl ModportMemberPath {
    pub fn new(head: StrId, tail: impl IntoIterator<Item = StrId>) -> Self {
        let mut path = vec![head];
        path.extend(tail);
        Self(path)
    }

    pub fn from_slice(path: &[StrId]) -> Self {
        Self(path.to_vec())
    }

    pub fn as_slice(&self) -> &[StrId] {
        &self.0
    }

    pub fn strip_prefix<'a>(&self, path: &'a [StrId]) -> Option<&'a [StrId]> {
        path.strip_prefix(&self.0[..])
    }
}

impl fmt::Display for ModportMemberPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((head, tail)) = self.0.split_first() else {
            return Ok(());
        };

        write!(f, "{head}")?;
        for id in tail {
            write!(f, ".{id}")?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct Interface {
    pub name: StrId,
    pub var_paths: HashMap<VarPath, (VarId, Comptime)>,
    pub func_paths: HashMap<FuncPath, VarId>,
    pub variables: HashMap<VarId, Variable>,
    pub functions: HashMap<VarId, Function>,
    pub modports: InterfaceModports,
}

impl Interface {
    pub fn get_modport(&self, name: &StrId) -> Option<ModportView<'_>> {
        self.modports.get_modport(name)
    }
}

#[derive(Clone, Debug)]
pub enum InterfaceModports {
    DirectLegacy {
        declared: HashMap<StrId, Vec<(ModportMemberPath, Direction)>>,
        effective: HashMap<StrId, Arc<[ResolvedModportEntry]>>,
    },
    Nested(Arc<NestedModportLowering>),
}

impl InterfaceModports {
    pub fn direct_legacy(declared: HashMap<StrId, Vec<(ModportMemberPath, Direction)>>) -> Self {
        let effective = declared
            .iter()
            .map(|(name, entries)| (*name, reduce_direct_entries(entries)))
            .collect();
        Self::direct_legacy_with_effective(declared, effective)
    }

    pub(crate) fn direct_legacy_with_effective(
        declared: HashMap<StrId, Vec<(ModportMemberPath, Direction)>>,
        effective: HashMap<StrId, Arc<[ResolvedModportEntry]>>,
    ) -> Self {
        Self::DirectLegacy {
            declared,
            effective,
        }
    }

    pub fn declared(&self) -> Option<&HashMap<StrId, Vec<(ModportMemberPath, Direction)>>> {
        match self {
            Self::DirectLegacy { declared, .. } => Some(declared),
            Self::Nested(_) => None,
        }
    }

    pub(crate) fn get_modport(&self, name: &StrId) -> Option<ModportView<'_>> {
        match self {
            Self::DirectLegacy { effective, .. } => effective
                .get(name)
                .map(|entries| ModportView::Direct(entries.as_ref())),
            Self::Nested(lowering) => lowering
                .modports
                .get(name)
                .map(|modport| ModportView::Nested(modport.entries.as_ref())),
        }
    }
}

fn reduce_direct_entries(
    entries: &[(ModportMemberPath, Direction)],
) -> Arc<[ResolvedModportEntry]> {
    let mut effective: Vec<ResolvedModportEntry> = Vec::with_capacity(entries.len());
    for (path, direction) in entries {
        if let Some(existing) = effective.iter_mut().find(|entry| entry.path == *path) {
            existing.direction = *direction;
        } else {
            effective.push(ResolvedModportEntry {
                path: path.clone(),
                direction: *direction,
                terminal: None,
                terminal_site: None,
            });
        }
    }
    effective.into()
}

#[derive(Clone, Copy, Debug)]
pub enum ModportView<'a> {
    Direct(&'a [ResolvedModportEntry]),
    Nested(&'a [ResolvedModportEntry]),
}

impl<'a> ModportView<'a> {
    pub fn entries(self) -> &'a [ResolvedModportEntry] {
        match self {
            Self::Direct(entries) | Self::Nested(entries) => entries,
        }
    }

    pub fn iter_paths_directions(
        self,
    ) -> impl Iterator<Item = (&'a ModportMemberPath, &'a Direction)> {
        self.entries()
            .iter()
            .map(|entry| (&entry.path, &entry.direction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_modport_view_preserves_first_position_and_later_direction() {
        let z = ModportMemberPath::new(StrId(1), []);
        let a = ModportMemberPath::new(StrId(2), []);
        let observe = ModportMemberPath::new(StrId(3), []);
        let name = StrId(9);
        let declared = HashMap::from_iter([(
            name,
            vec![
                (z.clone(), Direction::Input),
                (a.clone(), Direction::Inout),
                (z.clone(), Direction::Output),
                (observe.clone(), Direction::Import),
            ],
        )]);
        let modports = InterfaceModports::direct_legacy(declared);

        let first: Vec<_> = modports
            .get_modport(&name)
            .expect("declared modport should exist")
            .iter_paths_directions()
            .map(|(path, direction)| (path.clone(), *direction))
            .collect();
        let second: Vec<_> = modports
            .get_modport(&name)
            .expect("repeated borrowed query should exist")
            .iter_paths_directions()
            .map(|(path, direction)| (path.clone(), *direction))
            .collect();
        let first_view = modports
            .get_modport(&name)
            .expect("declared modport should remain borrowed");
        let second_view = modports
            .get_modport(&name)
            .expect("repeated query should remain borrowed");

        assert_eq!(
            first,
            vec![
                (z, Direction::Output),
                (a, Direction::Inout),
                (observe, Direction::Import),
            ]
        );
        assert_eq!(second, first);
        assert!(std::ptr::eq(
            first_view.entries().as_ptr(),
            second_view.entries().as_ptr()
        ));
    }
}

impl fmt::Display for Interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ret = format!("interface {} {{\n", self.name);

        let mut variables: Vec<_> = self.variables.iter().collect();
        variables.sort_by(|a, b| a.0.cmp(b.0));

        let mut functions: Vec<_> = self.functions.iter().collect();
        functions.sort_by(|a, b| a.0.cmp(b.0));

        for (_, x) in variables {
            let text = format!("{}\n", x);
            ret.push_str(&indent_all_by(2, text));
        }

        for (_, x) in functions {
            let text = format!("{}\n", x);
            ret.push_str(&indent_all_by(2, text));
        }

        ret.push('}');
        ret.fmt(f)
    }
}
