use crate::emitter::{SymbolContext, resolve_generic_path, symbol_string};
use crate::expanded_modport_index::ExpandedModportLookupIndex;
use std::collections::HashMap;
use veryl_analyzer::attribute::ExpandItem;
use veryl_analyzer::attribute_table;
use veryl_analyzer::conv::{Context, Conv};
use veryl_analyzer::ir;
use veryl_analyzer::namespace::Namespace;
use veryl_analyzer::nested_modport::{
    EmissionFrame, ExpandedPortResolution, ResolvedDeclarationType, ResolvedExpandedMember,
    ResolvedExpandedMemberSet, ResolvedExpandedPortInterface,
};
use veryl_analyzer::symbol::Direction as SymDirection;
use veryl_analyzer::symbol::Type as SymType;
use veryl_analyzer::symbol::{
    GenericMap, GenericTables, Port, Symbol, SymbolId, SymbolKind, VariableProperty,
};
use veryl_analyzer::symbol_table;
use veryl_parser::resource_table::StrId;
use veryl_parser::stringifier::Stringifier;
use veryl_parser::veryl_grammar_trait::*;
use veryl_parser::veryl_token::{Token, VerylToken};
use veryl_parser::veryl_walker::VerylWalker;

#[cfg(test)]
thread_local! {
    static FORCE_DIRECT_INTERFACE_RESOLUTION_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn force_direct_interface_resolution_failure(value: bool) {
    FORCE_DIRECT_INTERFACE_RESOLUTION_FAILURE.set(value);
}

#[cfg(test)]
fn direct_interface_resolution_failure_forced() -> bool {
    FORCE_DIRECT_INTERFACE_RESOLUTION_FAILURE.get()
}

#[cfg(not(test))]
fn direct_interface_resolution_failure_forced() -> bool {
    false
}

pub struct ExpandModportConnection {
    pub port_target: VerylToken,
    pub interface_target: VerylToken,
}

pub struct ExpandModportConnections {
    pub connections: Vec<ExpandModportConnection>,
}

impl ExpandModportConnections {
    fn new(
        port: &Port,
        modport: &Symbol,
        interface_name: &VerylToken,
        array_index: &[isize],
    ) -> Self {
        let connections: Vec<_> = collect_modport_member_variables(modport)
            .iter()
            .map(|(variable_token, _variable, _direction)| {
                let (port_target, interface_target) = if array_index.is_empty() {
                    (
                        format!("__{}_{}", port.name(), variable_token),
                        format!("{interface_name}.{variable_token}"),
                    )
                } else {
                    let index: Vec<_> = array_index.iter().map(|x| format!("{x}")).collect();
                    let select: Vec<_> = array_index.iter().map(|x| format!("[{x}]")).collect();
                    (
                        format!("__{}_{}_{}", port.name(), index.join("_"), variable_token),
                        format!("{}{}.{}", interface_name, select.join(""), variable_token),
                    )
                };
                ExpandModportConnection {
                    port_target: port.token.replace(&port_target),
                    interface_target: interface_name.replace(&interface_target),
                }
            })
            .collect();
        Self { connections }
    }

    fn new_resolved(
        port: &Port,
        members: &[ResolvedExpandedMember],
        interface_name: &VerylToken,
        array_index: &[isize],
    ) -> Self {
        let connections = members
            .iter()
            .map(|member| {
                let emitted_identifier = port.token.replace(&member.identifier.to_string());
                let (port_target, interface_target) = if array_index.is_empty() {
                    (
                        format!("__{}_{}", port.name(), emitted_identifier),
                        format!("{interface_name}.{emitted_identifier}"),
                    )
                } else {
                    let index: Vec<_> = array_index.iter().map(|x| format!("{x}")).collect();
                    let select: Vec<_> = array_index.iter().map(|x| format!("[{x}]")).collect();
                    (
                        format!(
                            "__{}_{}_{}",
                            port.name(),
                            index.join("_"),
                            emitted_identifier
                        ),
                        format!(
                            "{}{}.{}",
                            interface_name,
                            select.join(""),
                            emitted_identifier
                        ),
                    )
                };
                ExpandModportConnection {
                    port_target: port.token.replace(&port_target),
                    interface_target: interface_name.replace(&interface_target),
                }
            })
            .collect();
        Self { connections }
    }
}

pub struct ExpandModportConnectionsTableEntry {
    id: StrId,
    index: usize,
    pub connections: Vec<ExpandModportConnections>,
}

pub struct ExpandModportConnectionsTable {
    entries: Vec<ExpandModportConnectionsTableEntry>,
}

impl ExpandModportConnectionsTable {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn create_from_inst_ports(
        defined_ports: &[Port],
        inst_ports: &Vec<&InstPortItem>,
        generic_map: &[GenericMap],
        namespace: &Namespace,
        frame: Option<EmissionFrame<'_>>,
    ) -> Result<Self, veryl_analyzer::nested_modport::NestedModportAnalysisInvariant> {
        fn extract_connected_port(
            inst_port: &InstPortItem,
            defined_ports: &[Port],
            i: usize,
        ) -> Option<(StrId, VerylToken)> {
            if i >= defined_ports.len() {
                // Too much arguments are supplied.
                return None;
            }

            let (port_name, arg_token) = if let Some(opt_item) = &inst_port.inst_port_item_opt {
                let Some(arg_identifier) = opt_item.expression.unwrap_identifier() else {
                    // Given expression is an operation but not a simple identifier reference.
                    // Such expression is not reference to an interface.
                    return None;
                };

                let mut stringifier = Stringifier::new();
                stringifier.expression_identifier(arg_identifier);

                (
                    inst_port.identifier.identifier_token.token.text,
                    arg_identifier.identifier().replace(stringifier.as_str()),
                )
            } else {
                (
                    defined_ports[i].name(),
                    inst_port.identifier.identifier_token.clone(),
                )
            };
            Some((port_name, arg_token))
        }

        let connected_ports: HashMap<StrId, VerylToken> = inst_ports
            .iter()
            .enumerate()
            .filter_map(|(i, inst_port)| extract_connected_port(inst_port, defined_ports, i))
            .collect();

        let mut ret = ExpandModportConnectionsTable::new();
        ret.expand(
            defined_ports,
            &connected_ports,
            generic_map,
            namespace,
            false,
            frame,
        )?;
        Ok(ret)
    }

    pub fn create_from_argument_list(
        defined_ports: &[Port],
        argument_list: &ArgumentList,
        generic_map: &[GenericMap],
        namespace: &Namespace,
        frame: Option<EmissionFrame<'_>>,
    ) -> Result<Self, veryl_analyzer::nested_modport::NestedModportAnalysisInvariant> {
        fn extract_connected_port(
            arg: &ArgumentItem,
            defined_ports: &[Port],
            i: usize,
        ) -> Option<(StrId, VerylToken)> {
            if i >= defined_ports.len() {
                // Too much arguments are supplied.
                return None;
            }

            let Some(arg_identifier) = arg.argument_expression.expression.unwrap_identifier()
            else {
                // Given expression is an operation but not a simple identifier reference.
                // Such expression is not reference to an interface.
                return None;
            };

            if let Some(arg_opt_item) = &arg.argument_item_opt {
                let Some(arg_opt_identifier) = arg_opt_item.expression.unwrap_identifier() else {
                    // Given expression is an operation but not a simple identifier reference.
                    // Such expression is not reference to an interface.
                    return None;
                };

                let mut stringifier = Stringifier::new();
                stringifier.expression_identifier(arg_opt_identifier);

                let arg_token = arg_opt_identifier
                    .identifier()
                    .replace(stringifier.as_str());
                let param_name = arg_identifier.identifier().token.text;
                Some((param_name, arg_token))
            } else {
                let mut stringifier = Stringifier::new();
                stringifier.expression_identifier(arg_identifier);

                let arg_token = arg_identifier.identifier().replace(stringifier.as_str());
                let param_name = defined_ports[i].token.token.text;
                Some((param_name, arg_token))
            }
        }

        let mut list: Vec<_> = argument_list
            .argument_list_list
            .iter()
            .map(|x| x.argument_item.clone())
            .collect();
        list.insert(0, argument_list.argument_item.clone());

        let connected_ports: HashMap<StrId, VerylToken> = list
            .iter()
            .enumerate()
            .filter_map(|(i, arg)| extract_connected_port(arg, defined_ports, i))
            .collect();

        let mut ret = ExpandModportConnectionsTable::new();
        ret.expand(
            defined_ports,
            &connected_ports,
            generic_map,
            namespace,
            true,
            frame,
        )?;
        Ok(ret)
    }

    fn expand(
        &mut self,
        defined_ports: &[Port],
        connected_ports: &HashMap<StrId, VerylToken>,
        generic_map: &[GenericMap],
        namespace: &Namespace,
        in_function: bool,
        frame: Option<EmissionFrame<'_>>,
    ) -> Result<(), veryl_analyzer::nested_modport::NestedModportAnalysisInvariant> {
        for (modport, port, index) in collect_modports(defined_ports, namespace) {
            if !(in_function || attribute_table::is_expand(&port.token.token, ExpandItem::Modport))
            {
                continue;
            }

            let property = port.property();
            let array_size = evaluate_array_size(&property.r#type.array, generic_map);
            let mut array_index = expand_array_index(&array_size, &[]);
            if array_index.is_empty() {
                array_index.push(vec![]);
            }

            let Some(connected_port) = connected_ports.get(&port.name()) else {
                continue;
            };
            let resolved_members = match resolved_expanded_port(frame, &port.token.token)? {
                ResolvedExpandedPort::DirectLegacy => None,
                ResolvedExpandedPort::Nested { members, .. } => Some(members),
            };
            let connections: Vec<_> = array_index
                .iter()
                .map(|index| {
                    if let Some(members) = resolved_members.as_deref() {
                        ExpandModportConnections::new_resolved(
                            &port,
                            members.members(),
                            connected_port,
                            index,
                        )
                    } else {
                        ExpandModportConnections::new(&port, &modport, connected_port, index)
                    }
                })
                .collect();

            let entry = ExpandModportConnectionsTableEntry {
                id: port.name(),
                index,
                connections,
            };
            self.entries.push(entry);
        }
        Ok(())
    }

    pub fn remove(&mut self, token: &VerylToken) -> Option<ExpandModportConnectionsTableEntry> {
        let index = self.entries.iter().position(|x| x.id == token.token.text)?;
        Some(self.entries.remove(index))
    }

    pub fn pop_front(&mut self, port_index: usize) -> Option<ExpandModportConnectionsTableEntry> {
        if self
            .entries
            .first()
            .map(|x| x.index == port_index)
            .unwrap_or(false)
        {
            Some(self.entries.remove(0))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpandedModportPort {
    pub source_segments: Vec<StrId>,
    pub array_index: Vec<isize>,
    pub identifier: VerylToken,
    pub r#type: ExpandedModportPortType,
    pub interface_target: VerylToken,
    pub direction: SymDirection,
    pub direction_token: VerylToken,
}

#[derive(Clone, Debug)]
pub enum ExpandedModportPortType {
    Direct(SymType),
    Resolved(ResolvedDeclarationType),
}

#[derive(Clone, Debug)]
pub struct ExpandedModportPorts {
    pub ports: Vec<ExpandedModportPort>,
}

impl ExpandedModportPorts {
    fn new(port: &Port, modport: &Symbol, array_index: &[isize]) -> Self {
        let ports: Vec<_> = collect_modport_member_variables(modport)
            .iter()
            .map(|(variable_token, variable, direction)| {
                let (port_name, interface_target) = if array_index.is_empty() {
                    (
                        format!("__{}_{}", port.name(), variable_token),
                        format!("{}.{}", port.name(), variable_token),
                    )
                } else {
                    let index: Vec<_> = array_index.iter().map(|x| format!("{x}")).collect();
                    let select: Vec<_> = array_index.iter().map(|x| format!("[{x}]")).collect();
                    (
                        format!("__{}_{}_{}", port.name(), index.join("_"), variable_token),
                        format!("{}{}.{}", port.name(), select.join(""), variable_token),
                    )
                };
                let direction_token = if matches!(direction, SymDirection::Input) {
                    port.token.replace("input")
                } else {
                    port.token.replace("output")
                };
                ExpandedModportPort {
                    source_segments: vec![variable_token.text],
                    array_index: array_index.to_vec(),
                    identifier: port.token.replace(&port_name),
                    r#type: ExpandedModportPortType::Direct(variable.r#type.clone()),
                    interface_target: port.token.replace(&interface_target),
                    direction: *direction,
                    direction_token,
                }
            })
            .collect();
        Self { ports }
    }

    fn new_resolved(
        port: &Port,
        members: &[ResolvedExpandedMember],
        array_index: &[isize],
    ) -> Self {
        let ports = members
            .iter()
            .map(|member| {
                let emitted_identifier = port.token.replace(&member.identifier.to_string());
                let (port_name, interface_target) = if array_index.is_empty() {
                    (
                        format!("__{}_{}", port.name(), emitted_identifier),
                        format!("{}.{}", port.name(), emitted_identifier),
                    )
                } else {
                    let index: Vec<_> = array_index.iter().map(|x| format!("{x}")).collect();
                    let select: Vec<_> = array_index.iter().map(|x| format!("[{x}]")).collect();
                    (
                        format!(
                            "__{}_{}_{}",
                            port.name(),
                            index.join("_"),
                            emitted_identifier
                        ),
                        format!("{}{}.{}", port.name(), select.join(""), emitted_identifier),
                    )
                };
                let direction_text = match member.direction {
                    SymDirection::Input => "input",
                    SymDirection::Output => "output",
                    SymDirection::Inout => "inout",
                    SymDirection::Import | SymDirection::Modport | SymDirection::Interface => {
                        "input"
                    }
                };
                ExpandedModportPort {
                    source_segments: member.source_segments.clone(),
                    array_index: array_index.to_vec(),
                    identifier: port.token.replace(&port_name),
                    r#type: ExpandedModportPortType::Resolved(member.declaration.clone()),
                    interface_target: port.token.replace(&interface_target),
                    direction: member.direction,
                    direction_token: port.token.replace(direction_text),
                }
            })
            .collect();
        Self { ports }
    }
}

enum ResolvedExpandedPort {
    DirectLegacy,
    Nested {
        interface: ResolvedExpandedPortInterface,
        members: std::sync::Arc<ResolvedExpandedMemberSet>,
    },
}

fn resolved_expanded_port(
    frame: Option<EmissionFrame<'_>>,
    token: &Token,
) -> Result<ResolvedExpandedPort, veryl_analyzer::nested_modport::NestedModportAnalysisInvariant> {
    let Some(frame) = frame else {
        return Ok(ResolvedExpandedPort::DirectLegacy);
    };
    let Some(ExpandedPortResolution::Nested {
        interface, members, ..
    }) = frame.expanded_port_for_emission(token.id)?
    else {
        return Ok(ResolvedExpandedPort::DirectLegacy);
    };
    Ok(ResolvedExpandedPort::Nested {
        interface: interface.clone(),
        members: std::sync::Arc::clone(members),
    })
}

#[derive(Clone, Debug)]
pub struct ExpandedModportPortTableEntry {
    pub(super) id: StrId,
    pub identifier: VerylToken,
    pub interface_name: VerylToken,
    pub array_size: Vec<isize>,
    pub generic_maps: Vec<GenericMap>,
    pub ports: Vec<ExpandedModportPorts>,
    pub(super) resolved_members: Option<std::sync::Arc<ResolvedExpandedMemberSet>>,
}

pub struct ExpandedModportPortTable {
    entries: Vec<ExpandedModportPortTableEntry>,
    index: ExpandedModportLookupIndex,
}

impl ExpandedModportPortTable {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: ExpandedModportLookupIndex::default(),
        }
    }

    pub fn create(
        defined_ports: &[Port],
        generic_map: &[GenericMap],
        namespace_token: &VerylToken,
        namespace: &Namespace,
        in_function: bool,
        context: &SymbolContext,
        frame: Option<EmissionFrame<'_>>,
    ) -> Result<Self, veryl_analyzer::nested_modport::NestedModportAnalysisInvariant> {
        let mut ret = ExpandedModportPortTable::new();
        ret.expand(
            defined_ports,
            generic_map,
            namespace_token,
            namespace,
            in_function,
            context,
            frame,
        )?;
        ret.index = ExpandedModportLookupIndex::build(&ret.entries);
        Ok(ret)
    }

    fn expand(
        &mut self,
        defined_ports: &[Port],
        generic_map: &[GenericMap],
        namespace_token: &VerylToken,
        namespace: &Namespace,
        in_function: bool,
        context: &SymbolContext,
        frame: Option<EmissionFrame<'_>>,
    ) -> Result<(), veryl_analyzer::nested_modport::NestedModportAnalysisInvariant> {
        for (modport, port, _) in collect_modports(defined_ports, namespace) {
            if !(in_function || attribute_table::is_expand(&port.token.token, ExpandItem::Modport))
            {
                continue;
            }

            let property = port.property();
            let array_size = evaluate_array_size(&property.r#type.array, generic_map);
            let array_index = expand_array_index(&array_size, &[]);
            let resolved = resolved_expanded_port(frame, &port.token.token)?;
            let (interface_name, entry_generic_maps, resolved_members) =
                if let ResolvedExpandedPort::Nested { interface, members } = resolved {
                    let Some(interface_symbol) = symbol_table::get(interface.symbol) else {
                        return Err(veryl_analyzer::nested_modport::NestedModportAnalysisInvariant::MissingExpandedPort);
                    };
                    let base_name = symbol_string(
                        namespace_token,
                        &interface_symbol,
                        &interface_symbol.namespace,
                        &[],
                        &GenericTables::default(),
                        context,
                        1,
                    );
                    let text = if interface.generic_map.id.is_some() {
                        let raw_base_token = interface_symbol.token.to_string();
                        let base_token =
                            raw_base_token.strip_prefix("r#").unwrap_or(&raw_base_token);
                        let Some(namespace_prefix) = base_name.strip_suffix(&base_token) else {
                            return Err(veryl_analyzer::nested_modport::NestedModportAnalysisInvariant::MissingExpandedPort);
                        };
                        format!(
                            "{namespace_prefix}{}",
                            interface
                                .generic_map
                                .name(false, context.build_opt.hashed_mangled_name)
                        )
                    } else {
                        base_name
                    };
                    (
                        port.token.replace(&text),
                        vec![interface.generic_map],
                        Some(members),
                    )
                } else {
                    let connected_generic_map = frame
                        .and_then(|frame| frame.connected_generic_map(port.name()))
                        .map(std::slice::from_ref);
                    let interface_generic_maps = connected_generic_map.unwrap_or(generic_map);
                    let resolved_interface = (!direct_interface_resolution_failure_forced())
                        .then(|| resolve_interface(&port, namespace, interface_generic_maps))
                        .flatten();
                    let Some((interface_symbol, interface_path, interface_tables)) =
                        resolved_interface
                    else {
                        return Err(veryl_analyzer::nested_modport::NestedModportAnalysisInvariant::MissingExpandedPort);
                    };
                    let text = connected_generic_map
                        .and_then(|maps| maps.first())
                        .and_then(|map| map.id)
                        .and_then(symbol_table::get)
                        .map(|connected_interface| {
                            symbol_string(
                                namespace_token,
                                &connected_interface,
                                &connected_interface.namespace,
                                &[],
                                &GenericTables::default(),
                                context,
                                1,
                            )
                        })
                        .unwrap_or_else(|| {
                            symbol_string(
                                namespace_token,
                                &interface_symbol,
                                &interface_symbol.namespace,
                                &interface_path,
                                &interface_tables,
                                context,
                                1,
                            )
                        });
                    (
                        port.token.replace(&text),
                        connected_generic_map
                            .map(<[GenericMap]>::to_vec)
                            .unwrap_or_else(|| interface_symbol.generic_maps()),
                        None,
                    )
                };
            let ports = if array_index.is_empty() {
                vec![if let Some(members) = resolved_members.as_deref() {
                    ExpandedModportPorts::new_resolved(&port, members.members(), &[])
                } else {
                    ExpandedModportPorts::new(&port, &modport, &[])
                }]
            } else {
                array_index
                    .iter()
                    .map(|index| {
                        if let Some(members) = resolved_members.as_deref() {
                            ExpandedModportPorts::new_resolved(&port, members.members(), index)
                        } else {
                            ExpandedModportPorts::new(&port, &modport, index)
                        }
                    })
                    .collect()
            };

            let entry = ExpandedModportPortTableEntry {
                id: port.name(),
                identifier: port.token.clone(),
                interface_name,
                generic_maps: entry_generic_maps,
                array_size,
                ports,
                resolved_members,
            };
            self.entries.push(entry);
        }
        Ok(())
    }

    pub fn get(&self, token: &Token) -> Option<ExpandedModportPortTableEntry> {
        self.index
            .entry(token.text)
            .and_then(|index| self.entries.get(index))
            .cloned()
    }

    pub fn get_modport_member(
        &self,
        modport_token: &Token,
        member_token: &Token,
        array_index: &[isize],
    ) -> Option<ExpandedModportPort> {
        self.get_modport_member_path(modport_token, &[member_token.text], array_index)
    }

    pub fn get_modport_member_path(
        &self,
        modport_token: &Token,
        member_path: &[StrId],
        array_index: &[isize],
    ) -> Option<ExpandedModportPort> {
        self.index
            .member(&self.entries, modport_token.text, member_path, array_index)
            .cloned()
    }

    pub fn drain(&mut self) -> Vec<ExpandedModportPortTableEntry> {
        self.index = ExpandedModportLookupIndex::default();
        self.entries.drain(..).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn collect_modports(ports: &[Port], namespace: &Namespace) -> Vec<(Symbol, Port, usize)> {
    ports
        .iter()
        .enumerate()
        .filter_map(|(i, port)| {
            let property = port.property();
            if let Some((_, Some(symbol))) = property.r#type.trace_user_defined(Some(namespace)) {
                if matches!(symbol.kind, SymbolKind::Modport(_)) {
                    Some((symbol, port.clone(), i))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

fn evaluate_array_size(array_size: &[Expression], generic_map: &[GenericMap]) -> Vec<isize> {
    let mut context = Context::default();
    context.push_generic_map(generic_map.to_vec());
    array_size
        .iter()
        .filter_map(|x| {
            let mut expr: ir::Expression = Conv::conv(&mut context, x).ok()?;
            let comptime = expr.eval_comptime(&mut context, None);
            let value = comptime.get_value().ok()?;
            Some(value.to_usize().unwrap_or(0) as isize)
        })
        .collect()
}

fn expand_array_index(array_size: &[isize], array_index: &[Vec<isize>]) -> Vec<Vec<isize>> {
    if array_size.is_empty() {
        return array_index.to_vec();
    }

    let mut array_size = array_size.to_owned();
    let size = array_size.pop().unwrap();

    let mut ret: Vec<_> = Vec::new();
    for s in 0..size {
        if array_index.is_empty() {
            ret.push(vec![s]);
        } else {
            let mut index: Vec<_> = array_index
                .iter()
                .map(|x| {
                    let mut x = x.clone();
                    x.insert(0, s);
                    x
                })
                .collect();
            ret.append(&mut index);
        }
    }

    if array_size.is_empty() {
        ret
    } else {
        expand_array_index(&array_size, &ret)
    }
}

pub(crate) fn collect_modport_member_variables(
    symbol: &Symbol,
) -> Vec<(Token, VariableProperty, SymDirection)> {
    let SymbolKind::Modport(modport) = &symbol.kind else {
        unreachable!()
    };

    modport
        .members
        .iter()
        .filter_map(|member| {
            if let SymbolKind::ModportVariableMember(member) =
                symbol_table::get(*member).unwrap().kind
            {
                let variable_symbol = symbol_table::get(member.variable).unwrap();
                if let SymbolKind::Variable(variable) = variable_symbol.kind {
                    Some((variable_symbol.token, variable, member.direction))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

fn resolve_interface(
    port: &Port,
    namespace: &Namespace,
    generic_map: &[GenericMap],
) -> Option<(Symbol, Vec<SymbolId>, GenericTables)> {
    let property = port.property();
    let (user_defined, _) = property.r#type.trace_user_defined(Some(namespace))?;

    let mut path = user_defined.get_user_defined()?.path.clone();
    path.paths.pop(); // remove modport path

    let (result, _) = resolve_generic_path(
        &path,
        veryl_analyzer::scope::intern_namespace(namespace),
        &namespace.define_context,
        Some(&generic_map.to_vec()),
    );
    result.ok().map(|x| {
        (
            (*x.found).clone(),
            x.full_path.clone(),
            x.generic_tables.clone(),
        )
    })
}
