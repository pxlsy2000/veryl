//! Targeted SystemVerilog syntax and structure checks for emitter tests.
//!
//! This deliberately models only the invariants needed by nested-modport
//! regressions. Parsing proves syntax; the records below support selected
//! structural assertions, not complete SystemVerilog semantic validation.

mod expectations;
mod extract;
mod model;
mod type_tokens;

use model::{
    Component, Declaration, IdentifierChain, Instance, ModportMember, PackageFunction,
    QualifiedFunctionCall,
};
pub(crate) use model::{
    ComponentKind, DeclarationExpectation, InstanceExpectation, ModportMemberExpectation,
    PortDirection, SvStructure, SvStructureError,
};

#[cfg(test)]
mod tests;
