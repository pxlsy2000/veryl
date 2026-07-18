use super::*;
use std::collections::HashMap;
use std::path::Path;
use sv_parser::{Defines, NodeEvent, RefNode, parse_sv_str, unwrap_node};

impl SvStructure {
    pub(crate) fn parse(source: &str, path: &Path) -> Result<Self, SvStructureError> {
        let defines: Defines<std::collections::hash_map::RandomState> = HashMap::new();
        let include_paths: Vec<std::path::PathBuf> = Vec::new();
        let (tree, _) = parse_sv_str(source, path, &defines, &include_paths, false, false)
            .map_err(|error| SvStructureError::Parse {
                detail: format!("{error:?}"),
            })?;

        let mut structure = Self::default();
        for node in &tree {
            match node {
                RefNode::ModuleDeclarationAnsi(_) => {
                    structure.inspect_component(&node, source, ComponentKind::Module)
                }
                RefNode::InterfaceDeclarationAnsi(_) => {
                    structure.inspect_component(&node, source, ComponentKind::Interface)
                }
                _ => {}
            }
        }
        structure.inspect_package_functions_and_calls(source);
        Ok(structure)
    }

    fn inspect_package_functions_and_calls(&mut self, source: &str) {
        let mut package = None;
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed
                .strip_prefix("package ")
                .and_then(|value| value.strip_suffix(';'))
            {
                package = Some(name.trim().to_owned());
            } else if trimmed == "endpackage" {
                package = None;
            } else if let Some(package) = package.as_ref()
                && trimmed.starts_with("function automatic ")
                && let Some(function) = identifier_before_call_parenthesis(trimmed)
            {
                self.package_functions.push(PackageFunction {
                    package: package.clone(),
                    function,
                });
            }
        }
        self.qualified_function_calls
            .extend(qualified_function_calls(source));
    }

    fn inspect_component(&mut self, node: &RefNode<'_>, source: &str, kind: ComponentKind) {
        let name = component_name(node, source, kind);
        self.components.push(Component {
            kind,
            name: name.clone(),
        });
        self.inspect_component_declarations(node, source, &name);
        for descendant in node.clone().into_iter() {
            match descendant {
                RefNode::ModportItem(_) => self.inspect_modport(&descendant, source, &name),
                RefNode::InterfaceInstantiation(_) | RefNode::ModuleInstantiation(_) => {
                    self.inspect_instantiation(&descendant, source, &name)
                }
                RefNode::HierarchicalIdentifier(_) => {
                    self.identifier_chains.push(IdentifierChain {
                        owner: name.clone(),
                        segments: identifier_segments(&descendant, source),
                    });
                }
                _ => {}
            }
        }
    }

    fn inspect_component_declarations(&mut self, node: &RefNode<'_>, source: &str, owner: &str) {
        let mut event_depth = 0;
        let mut nested_scope_depth = 0;
        for event in node.clone().into_iter().event() {
            match event {
                NodeEvent::Enter(descendant) => {
                    let is_nested = event_depth > 0;
                    event_depth += 1;
                    if is_nested && is_declaration_owner_boundary(&descendant) {
                        nested_scope_depth += 1;
                    } else if nested_scope_depth == 0
                        && matches!(descendant, RefNode::DataDeclaration(_))
                    {
                        self.inspect_declaration(&descendant, source, owner);
                    }
                }
                NodeEvent::Leave(descendant) => {
                    let is_nested = event_depth > 1;
                    if is_nested && is_declaration_owner_boundary(&descendant) {
                        nested_scope_depth -= 1;
                    }
                    event_depth -= 1;
                }
            }
        }
    }

    fn inspect_declaration(&mut self, node: &RefNode<'_>, source: &str, owner: &str) {
        let type_tokens = unwrap_node!(node.clone(), DataTypeOrImplicit)
            .map(|data_type| super::type_tokens::from_node(&data_type, source))
            .unwrap_or_default();
        for assignment in node.clone().into_iter() {
            if let RefNode::VariableDeclAssignmentVariable(_) = assignment {
                let name = unwrap_node!(assignment.clone(), VariableIdentifier)
                    .map(|identifier| normalize_text(node_text(&identifier, source)))
                    .unwrap_or_default();
                let dimension_tokens: Vec<String> = assignment
                    .clone()
                    .into_iter()
                    .filter(|node| matches!(node, RefNode::VariableDimension(_)))
                    .flat_map(|dimension| super::type_tokens::from_node(&dimension, source))
                    .collect();
                self.declarations.push(Declaration {
                    owner: owner.to_string(),
                    name,
                    type_tokens: type_tokens
                        .iter()
                        .cloned()
                        .chain(dimension_tokens)
                        .collect(),
                });
            }
        }
    }

    fn inspect_modport(&mut self, node: &RefNode<'_>, source: &str, owner: &str) {
        let modport = unwrap_node!(node.clone(), ModportIdentifier)
            .map(|identifier| normalize_text(node_text(&identifier, source)))
            .unwrap_or_default();
        for declaration in node.clone().into_iter() {
            if let RefNode::ModportSimplePortsDeclaration(_) = declaration {
                let Some(direction) = unwrap_node!(declaration.clone(), PortDirection)
                    .and_then(|node| parse_direction(node_text(&node, source)))
                else {
                    continue;
                };
                for port in declaration.clone().into_iter() {
                    if let RefNode::ModportSimplePortOrdered(_) = port {
                        self.modport_members.push(ModportMember {
                            owner: owner.to_string(),
                            modport: modport.clone(),
                            member: normalize_text(node_text(&port, source)),
                            direction,
                        });
                    }
                }
            }
        }
    }

    fn inspect_instantiation(&mut self, node: &RefNode<'_>, source: &str, owner: &str) {
        let component = match node {
            RefNode::InterfaceInstantiation(_) => {
                unwrap_node!(node.clone(), InterfaceIdentifier)
            }
            RefNode::ModuleInstantiation(_) => unwrap_node!(node.clone(), ModuleIdentifier),
            _ => None,
        }
        .map(|identifier| normalize_text(node_text(&identifier, source)))
        .unwrap_or_default();
        for instance in node.clone().into_iter() {
            if let RefNode::HierarchicalInstance(_) = instance {
                let name = unwrap_node!(instance.clone(), InstanceIdentifier)
                    .map(|identifier| normalize_text(node_text(&identifier, source)))
                    .unwrap_or_default();
                self.instances.push(Instance {
                    owner: owner.to_string(),
                    component: component.clone(),
                    instance: name,
                });
            }
        }
    }
}

mod text;
use text::*;
