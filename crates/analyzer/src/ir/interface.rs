use crate::HashMap;
use crate::ir::{Comptime, FuncPath, Function, VarId, VarPath, Variable};
use crate::symbol::Direction;
use indent::indent_all_by;
use std::fmt;
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
    pub has_imports: bool,
    pub var_paths: HashMap<VarPath, (VarId, Comptime)>,
    pub func_paths: HashMap<FuncPath, VarId>,
    pub variables: HashMap<VarId, Variable>,
    pub functions: HashMap<VarId, Function>,
    pub modports: HashMap<StrId, Vec<(ModportMemberPath, Direction)>>,
}

impl Interface {
    pub fn get_modport(&self, name: &StrId) -> HashMap<ModportMemberPath, Direction> {
        let mut ret = HashMap::default();
        if let Some(x) = self.modports.get(name) {
            for x in x {
                ret.insert(x.0.clone(), x.1);
            }
        }
        ret
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
