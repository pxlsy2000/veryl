use std::fmt::{self, Display};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComponentKind {
    Interface,
    Module,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PortDirection {
    Inout,
    Input,
    Output,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SvStructureError {
    Parse { detail: String },
    Invariant { detail: String },
}

impl Display for SvStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { detail } => write!(formatter, "SystemVerilog parse failed: {detail}"),
            Self::Invariant { detail } => {
                write!(formatter, "SystemVerilog invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SvStructureError {}

#[derive(Clone, Copy)]
pub(crate) struct DeclarationExpectation<'a> {
    pub owner: &'a str,
    pub name: &'a str,
    pub type_text: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) struct ModportMemberExpectation<'a> {
    pub owner: &'a str,
    pub modport: &'a str,
    pub member: &'a str,
    pub direction: PortDirection,
}

#[derive(Clone, Copy)]
pub(crate) struct InstanceExpectation<'a> {
    pub owner: &'a str,
    pub component: &'a str,
    pub instance: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Component {
    pub(super) kind: ComponentKind,
    pub(super) name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Declaration {
    pub(super) owner: String,
    pub(super) name: String,
    pub(super) type_tokens: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ModportMember {
    pub(super) owner: String,
    pub(super) modport: String,
    pub(super) member: String,
    pub(super) direction: PortDirection,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Instance {
    pub(super) owner: String,
    pub(super) component: String,
    pub(super) instance: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct IdentifierChain {
    pub(super) owner: String,
    pub(super) segments: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PackageFunction {
    pub(super) package: String,
    pub(super) function: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct QualifiedFunctionCall {
    pub(super) package: String,
    pub(super) function: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SvStructure {
    pub(super) components: Vec<Component>,
    pub(super) declarations: Vec<Declaration>,
    pub(super) modport_members: Vec<ModportMember>,
    pub(super) instances: Vec<Instance>,
    pub(super) identifier_chains: Vec<IdentifierChain>,
    pub(super) package_functions: Vec<PackageFunction>,
    pub(super) qualified_function_calls: Vec<QualifiedFunctionCall>,
}
