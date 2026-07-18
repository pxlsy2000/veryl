use crate::analyzer_error::{
    ExceedLimitKind, InvalidNestedModportKind, NestedModportDiagnosticSite,
};
use crate::conv::checker::alias::{AliasType, check_alias_target};
use crate::conv::checker::clock_domain::check_clock_domain;
use crate::conv::checker::generic::check_generic_bound;
use crate::conv::checker::proto::check_proto;
use crate::conv::utils::{check_module_with_unevaluable_generic_parameters, get_component};
use crate::conv::{Affiliation, Context, Conv};
use crate::ir::{self, IrResult, VarPath};
use crate::nested_modport::{
    EmissionOwnerKind, LoweringAvailability, NestedLoweringResolveError, NestedModportLoweringKey,
    PendingGenericEmissionOwner, resolve_direct_modport_effective_with_index,
    resolve_pending_nested_modport_lowering_for_owner_with_index,
};
use crate::symbol::SymbolKind;
use crate::symbol_table;
use crate::{AnalyzerError, HashMap, ir_error};
use std::sync::Arc;
use veryl_parser::token_range::TokenRange;
use veryl_parser::veryl_grammar_trait::*;

/// Memo key for a top-level component's direct conversion, or `None` when
/// sharing must not apply: a non-`veryl test` run (instantiation clears bodies)
/// or a generic template.
fn top_level_memo_sig(context: &Context, identifier: &Identifier) -> Option<ir::Signature> {
    if context.config.retain_component_body && !context.in_generic {
        symbol_table::resolve(identifier).ok().map(|s| {
            let mut sig = ir::Signature::new(s.found.id);
            sig.normalize();
            sig
        })
    } else {
        None
    }
}

impl Conv<&Veryl> for ir::Ir {
    fn conv(context: &mut Context, value: &Veryl) -> IrResult<Self> {
        let mut components = vec![];

        for x in &value.veryl_list {
            let items: Vec<_> = x.description_group.as_ref().into();
            for item in &items {
                // ignore IrError of generic top-level components
                let in_generic = context.in_generic;
                if item.is_generic() {
                    context.in_generic = true;
                }

                match item {
                    DescriptionItem::DescriptionItemOptPublicDescriptionItem(x) => {
                        match x.public_description_item.as_ref() {
                            PublicDescriptionItem::ModuleDeclaration(x) => {
                                let decl = x.module_declaration.as_ref();
                                if let Some(sig) =
                                    top_level_memo_sig(context, decl.identifier.as_ref())
                                {
                                    let token: TokenRange = decl.identifier.as_ref().into();
                                    if let Ok(component) = get_component(context, &sig, token)
                                        && let ir::Component::Module(m) = component.as_ref()
                                    {
                                        let mut component = m.clone();
                                        if check_module_with_unevaluable_generic_parameters(
                                            &decl.identifier,
                                        ) {
                                            component.suppress_unassigned = true;
                                        }
                                        components.push(ir::Component::Module(component));
                                    }
                                } else {
                                    let ret: IrResult<ir::Module> = Conv::conv(context, decl);
                                    if let Ok(mut component) = ret {
                                        // suppress unassigned check for modules with unevaluable generic parameters
                                        if check_module_with_unevaluable_generic_parameters(
                                            &decl.identifier,
                                        ) {
                                            component.suppress_unassigned = true;
                                        }
                                        components.push(ir::Component::Module(component));
                                    }
                                }
                            }
                            PublicDescriptionItem::InterfaceDeclaration(x) => {
                                let decl = x.interface_declaration.as_ref();
                                if let Some(sig) =
                                    top_level_memo_sig(context, decl.identifier.as_ref())
                                {
                                    let token: TokenRange = decl.identifier.as_ref().into();
                                    let _ = get_component(context, &sig, token);
                                } else {
                                    let _: IrResult<ir::Interface> = Conv::conv(context, decl);
                                }
                            }
                            PublicDescriptionItem::PackageDeclaration(x) => {
                                let _: IrResult<()> =
                                    Conv::conv(context, x.package_declaration.as_ref());
                            }
                            PublicDescriptionItem::ProtoDeclaration(x) => {
                                match x.proto_declaration.proto_declaration_group.as_ref() {
                                    ProtoDeclarationGroup::ProtoModuleDeclaration(x) => {
                                        let _: IrResult<ir::Module> = Conv::conv(
                                            context,
                                            x.proto_module_declaration.as_ref(),
                                        );
                                    }
                                    ProtoDeclarationGroup::ProtoInterfaceDeclaration(x) => {
                                        let _: IrResult<()> = Conv::conv(
                                            context,
                                            x.proto_interface_declaration.as_ref(),
                                        );
                                    }
                                    ProtoDeclarationGroup::ProtoPackageDeclaration(x) => {
                                        let _: IrResult<()> = Conv::conv(
                                            context,
                                            x.proto_package_declaration.as_ref(),
                                        );
                                    }
                                }
                            }
                            PublicDescriptionItem::AliasDeclaration(x) => {
                                let _: IrResult<()> =
                                    Conv::conv(context, x.alias_declaration.as_ref());
                            }
                            PublicDescriptionItem::FunctionDeclaration(x) => {
                                conv_global_function(context, x.function_declaration.as_ref());
                            }
                        }
                    }
                    DescriptionItem::BindDeclaration(x) => {
                        let _: IrResult<()> = Conv::conv(context, x.bind_declaration.as_ref());
                    }
                    DescriptionItem::EmbedDeclaration(x) => {
                        let _: IrResult<()> = Conv::conv(context, x.embed_declaration.as_ref());
                    }
                    DescriptionItem::ImportDeclaration(x) => {
                        let _: IrResult<ir::DeclarationBlock> =
                            Conv::conv(context, x.import_declaration.as_ref());
                    }
                    _ => (),
                }

                if item.is_generic() {
                    context.in_generic = in_generic;
                }
            }
        }

        Ok(ir::Ir { components })
    }
}

fn conv_global_function(context: &mut Context, value: &FunctionDeclaration) {
    let upper_context = context;
    let mut context = upper_context.child();
    context.inherit(upper_context);

    context.in_global_func = Some(value.identifier.identifier_token.token);
    let _: IrResult<()> = Conv::conv(&mut context, value);
    context.in_global_func = None;

    upper_context.inherit(&mut context);
}

impl Conv<&ModuleDeclaration> for ir::Module {
    fn conv(context: &mut Context, value: &ModuleDeclaration) -> IrResult<Self> {
        Conv::conv(context, (value, false))
    }
}

impl Conv<(&ModuleDeclaration, bool)> for ir::Module {
    fn conv(context: &mut Context, value: (&ModuleDeclaration, bool)) -> IrResult<Self> {
        let (module_declaration, header_only) = value;

        let mut declarations = vec![];

        // each top-level component has independent context
        let upper_context = context;
        let mut context = upper_context.child();
        context.inherit(upper_context);

        let _guard = context.conv_profile_guard(module_declaration.identifier.text());

        // pop_affiliation is not necessary because the local `context` will be dropped
        context.push_affiliation(Affiliation::Module);

        if let Ok(symbol) = symbol_table::resolve(module_declaration.identifier.as_ref())
            && let SymbolKind::Module(x) = &symbol.found.kind
            && !x.is_proto
        {
            let owner = context
                .get_current_signature()
                .cloned()
                .unwrap_or_else(|| ir::Signature::new(symbol.found.id));
            context.set_component_emission_specialization(
                context.component_specialization_identity(&owner),
            );
            context.set_emission_owner_signature(owner);
            context.push_namespace(symbol.found.inner_namespace());
            context.in_test_module = x.test.is_some();
            if let Some(x) = x.default_clock {
                let path = VarPath::new(symbol_table::get(x).unwrap().token.text);
                context.set_default_clock(path, x);
            }
            if let Some(x) = x.default_reset {
                let path = VarPath::new(symbol_table::get(x).unwrap().token.text);
                context.set_default_reset(path, x);
            }
        } else {
            let token: TokenRange = module_declaration.identifier.as_ref().into();
            return Err(ir_error!(token));
        }

        if let Some(x) = &module_declaration.module_declaration_opt {
            check_generic_bound(&mut context, &x.with_generic_parameter);
            let items: Vec<_> = x
                .with_generic_parameter
                .with_generic_parameter_list
                .as_ref()
                .into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        if let Some(x) = &module_declaration.module_declaration_opt0 {
            check_proto(
                &mut context,
                &module_declaration.identifier,
                &x.scoped_identifier,
            );
        }

        if let Some(x) = &module_declaration.module_declaration_opt1
            && let Some(x) = &x.with_parameter.with_parameter_opt
        {
            let items: Vec<_> = x.with_parameter_list.as_ref().into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        if let Some(x) = &module_declaration.module_declaration_opt2
            && let Some(x) = &x.port_declaration.port_declaration_opt
        {
            let items: Vec<_> = x.port_declaration_list.as_ref().into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        if !header_only {
            for x in &module_declaration.module_declaration_list {
                let items: Vec<_> = x.module_group.as_ref().into();
                for item in &items {
                    let ret: IrResult<ir::DeclarationBlock> =
                        Conv::conv(&mut context, item.generate_item.as_ref());

                    if let Ok(mut block) = ret {
                        declarations.append(&mut block.0);
                    }
                }
            }
        }

        // This check must be after default clock/reset are registered in context
        if let (Some(clock), Some(reset)) =
            (context.get_default_clock(), context.get_default_reset())
        {
            check_clock_domain(
                &mut context,
                &clock.0.comptime,
                &reset.0.comptime,
                &module_declaration.module.module_token.token,
            );
        }

        declarations.retain(|x| !x.is_null());
        let port_types = context.drain_port_types();
        let variables = context.drain_variables();
        let functions = context.drain_functions();

        let mut ports = HashMap::default();

        for (id, var) in &variables {
            if var.kind.is_port() {
                ports.insert(var.path.clone(), *id);
            }
        }

        let declaration = module_declaration.identifier.identifier_token.token;
        let resolved_symbol = symbol_table::resolve(module_declaration.identifier.as_ref())
            .map_err(|_| ir_error!(TokenRange::from(module_declaration.identifier.as_ref())))?;
        let current_signature = context.get_current_signature().cloned();
        if context.in_generic
            && current_signature.is_none()
            && let Some(source) = declaration.source.get_path()
            && let Err(error) =
                context.record_nested_generic_emission_owner(PendingGenericEmissionOwner {
                    session: context.analysis_session_id(),
                    source,
                    declaration: declaration.id,
                    kind: EmissionOwnerKind::Module,
                    symbol: resolved_symbol.found.id,
                })
        {
            context.insert_error(AnalyzerError::from(error));
            return Err(ir_error!(TokenRange::from(
                module_declaration.identifier.as_ref()
            )));
        }
        if !context.in_generic
            && (current_signature.is_some() || !resolved_symbol.found.has_generic_paramters())
            && let Some(source) = declaration.source.get_path()
        {
            let owner =
                current_signature.unwrap_or_else(|| ir::Signature::new(resolved_symbol.found.id));
            let specialization = NestedModportLoweringKey {
                session: context.analysis_session_id(),
                specialization: (context.component_specialization_identity(&owner)).into(),
            };
            if let Err(error) = context
                .record_nested_lowering(specialization.clone(), LoweringAvailability::NotNested)
            {
                context.insert_error(AnalyzerError::from(error));
                return Err(ir_error!(TokenRange::from(
                    module_declaration.identifier.as_ref()
                )));
            }
            if let Err(error) = context.record_nested_emission_owner(
                source,
                declaration.id,
                EmissionOwnerKind::Module,
                specialization,
                LoweringAvailability::NotNested,
            ) {
                context.insert_error(AnalyzerError::from(error));
                return Err(ir_error!(TokenRange::from(
                    module_declaration.identifier.as_ref()
                )));
            }
        }

        context.pop_namespace();
        upper_context.inherit(&mut context);

        Ok(ir::Module {
            name: module_declaration.identifier.text(),
            token: module_declaration.identifier.as_ref().into(),
            ports,
            port_types,
            variables,
            functions,
            declarations,
            suppress_unassigned: false,
            per_decl_refs: HashMap::default(),
            assign_tokens: HashMap::default(),
            ff_table: ir::FfTable::default(),
        })
    }
}

impl Conv<&InterfaceDeclaration> for ir::Interface {
    fn conv(context: &mut Context, value: &InterfaceDeclaration) -> IrResult<Self> {
        // each top-level component has independent context
        let upper_context = context;
        let mut context = upper_context.child();
        context.inherit(upper_context);

        let _guard = context.conv_profile_guard(value.identifier.text());

        // pop_affiliation is not necessary because the local `context` will be dropped
        context.push_affiliation(Affiliation::Interface);

        if let Ok(symbol) = symbol_table::resolve(value.identifier.as_ref())
            && matches!(symbol.found.kind, SymbolKind::Interface(ref x) if !x.is_proto)
        {
            let owner = context
                .get_current_signature()
                .cloned()
                .unwrap_or_else(|| ir::Signature::new(symbol.found.id));
            context.set_component_emission_specialization(
                context.component_specialization_identity(&owner),
            );
            context.set_emission_owner_signature(owner);
            context.push_namespace(symbol.found.inner_namespace());
        } else {
            let token: TokenRange = value.identifier.as_ref().into();
            return Err(ir_error!(token));
        }

        if let Some(x) = &value.interface_declaration_opt {
            check_generic_bound(&mut context, &x.with_generic_parameter);
            let items: Vec<_> = x
                .with_generic_parameter
                .with_generic_parameter_list
                .as_ref()
                .into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        if let Some(x) = &value.interface_declaration_opt0 {
            check_proto(&mut context, &value.identifier, &x.scoped_identifier);
        }

        if let Some(x) = &value.interface_declaration_opt1
            && let Some(x) = &x.with_parameter.with_parameter_opt
        {
            let items: Vec<_> = x.with_parameter_list.as_ref().into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        for x in &value.interface_declaration_list {
            let items: Vec<_> = x.interface_group.as_ref().into();
            for item in items {
                match item {
                    InterfaceItem::GenerateItem(x) => {
                        let _: IrResult<ir::DeclarationBlock> =
                            Conv::conv(&mut context, x.generate_item.as_ref());
                    }
                    InterfaceItem::ModportDeclaration(x) => {
                        let _: IrResult<()> =
                            Conv::conv(&mut context, x.modport_declaration.as_ref());
                    }
                }
            }
        }

        let var_paths = context.drain_var_paths();
        let func_paths = context.drain_func_paths();
        let mut variables = context.drain_variables();
        let functions = context.drain_functions();
        let modports = context.drain_modports();
        let pending_component_lowering = context.drain_pending_component_lowering();

        let variables = variables
            .extract_if(|_, v| v.affiliation != Affiliation::Function)
            .collect();
        let terminal_index = context.drain_instantiated_terminal_index(&variables, &functions);

        let diagnostic_token: TokenRange = value.identifier.as_ref().into();
        let owner_signature = if let Some(signature) = context.get_current_signature() {
            signature.clone()
        } else if let Ok(symbol) = symbol_table::resolve(value.identifier.as_ref()) {
            ir::Signature::new(symbol.found.id)
        } else {
            return Err(ir_error!(diagnostic_token));
        };
        let lowering_key = NestedModportLoweringKey {
            session: context.analysis_session_id(),
            specialization: (context.component_specialization_identity(&owner_signature)).into(),
        };
        let requires_nested_lowering = pending_component_lowering
            .declarations
            .iter()
            .any(|declaration| declaration.contains_nested_item);
        let availability;
        let interface_modports = if requires_nested_lowering {
            let lowering = match resolve_pending_nested_modport_lowering_for_owner_with_index(
                &pending_component_lowering,
                &variables,
                &functions,
                &terminal_index,
                Some(owner_signature.symbol),
                context.config.evaluate_size_limit,
            ) {
                Ok(Some(lowering)) => lowering,
                Ok(None) => {
                    context.insert_error(AnalyzerError::from(
                        crate::nested_modport::NestedModportAnalysisInvariant::MissingLowering,
                    ));
                    context.pop_namespace();
                    upper_context.inherit(&mut context);
                    return Err(ir_error!(diagnostic_token));
                }
                Err(error) => {
                    insert_nested_lowering_error(&mut context, error, diagnostic_token);
                    context.pop_namespace();
                    upper_context.inherit(&mut context);
                    return Err(ir_error!(diagnostic_token));
                }
            };
            let lowering =
                match context.record_nested_interface_lowering(lowering_key.clone(), lowering) {
                    Ok(lowering) => lowering,
                    Err(error) => {
                        context.insert_error(AnalyzerError::from(error));
                        context.pop_namespace();
                        upper_context.inherit(&mut context);
                        return Err(ir_error!(diagnostic_token));
                    }
                };
            availability = LoweringAvailability::Found(Arc::clone(&lowering));
            ir::InterfaceModports::Nested(lowering)
        } else {
            let effective = match resolve_direct_modport_effective_with_index(
                &pending_component_lowering,
                &variables,
                &functions,
                &terminal_index,
                context.config.evaluate_size_limit,
            ) {
                Ok(effective) => effective,
                Err(error) => {
                    insert_nested_lowering_error(&mut context, error, diagnostic_token);
                    context.pop_namespace();
                    upper_context.inherit(&mut context);
                    return Err(ir_error!(diagnostic_token));
                }
            };
            if let Err(error) = context
                .record_nested_lowering(lowering_key.clone(), LoweringAvailability::NotNested)
            {
                context.insert_error(AnalyzerError::from(error));
                context.pop_namespace();
                upper_context.inherit(&mut context);
                return Err(ir_error!(diagnostic_token));
            }
            availability = LoweringAvailability::NotNested;
            ir::InterfaceModports::direct_legacy_with_effective(modports, effective)
        };

        let declaration = value.identifier.identifier_token.token;
        let resolved_symbol = symbol_table::resolve(value.identifier.as_ref())
            .map_err(|_| ir_error!(diagnostic_token))?;
        let current_signature = context.get_current_signature().cloned();
        if context.in_generic
            && current_signature.is_none()
            && let Some(source) = declaration.source.get_path()
            && let Err(error) =
                context.record_nested_generic_emission_owner(PendingGenericEmissionOwner {
                    session: context.analysis_session_id(),
                    source,
                    declaration: declaration.id,
                    kind: EmissionOwnerKind::Interface,
                    symbol: resolved_symbol.found.id,
                })
        {
            context.insert_error(AnalyzerError::from(error));
            context.pop_namespace();
            upper_context.inherit(&mut context);
            return Err(ir_error!(diagnostic_token));
        }
        if !context.in_generic
            && (current_signature.is_some() || !resolved_symbol.found.has_generic_paramters())
            && let Some(source) = declaration.source.get_path()
            && let Err(error) = context.record_nested_emission_owner(
                source,
                declaration.id,
                EmissionOwnerKind::Interface,
                lowering_key,
                availability,
            )
        {
            context.insert_error(AnalyzerError::from(error));
            return Err(ir_error!(diagnostic_token));
        }

        context.pop_namespace();
        upper_context.inherit(&mut context);

        Ok(ir::Interface {
            name: value.identifier.text(),
            var_paths,
            func_paths,
            variables,
            functions,
            modports: interface_modports,
        })
    }
}

fn insert_nested_lowering_error(
    context: &mut Context,
    error: NestedLoweringResolveError,
    token: TokenRange,
) {
    let (path, kind, site) = match error {
        NestedLoweringResolveError::ExpansionBudget { requested, .. } => {
            context.insert_error(AnalyzerError::exceed_limit(
                ExceedLimitKind::EvaluateSize,
                requested,
                &token,
            ));
            return;
        }
        NestedLoweringResolveError::MissingTerminal { path, origin } => {
            let name = path.to_string();
            (
                name.clone(),
                InvalidNestedModportKind::NonVariableTerminal {
                    name,
                    actual_kind: "missing terminal".to_string(),
                },
                NestedModportDiagnosticSite {
                    item_path: origin,
                    offending: origin,
                    first_conflict: None,
                },
            )
        }
        NestedLoweringResolveError::MissingModport { name, origin } => (
            name.to_string(),
            InvalidNestedModportKind::MissingModport {
                name: name.to_string(),
            },
            NestedModportDiagnosticSite {
                item_path: token,
                offending: origin,
                first_conflict: None,
            },
        ),
        NestedLoweringResolveError::DefaultCycle { target, origin } => (
            target.to_string(),
            InvalidNestedModportKind::DefaultCycle {
                target: target.to_string(),
            },
            NestedModportDiagnosticSite {
                item_path: token,
                offending: origin,
                first_conflict: None,
            },
        ),
        NestedLoweringResolveError::EmptyModport { name, origin } => (
            name.to_string(),
            InvalidNestedModportKind::EmptyModport {
                name: name.to_string(),
            },
            NestedModportDiagnosticSite {
                item_path: origin,
                offending: origin,
                first_conflict: None,
            },
        ),
        NestedLoweringResolveError::UnsupportedMemberDirection {
            path,
            direction,
            origin,
        } => (
            path.to_string(),
            InvalidNestedModportKind::UnsupportedMemberDirection {
                direction: direction.to_string(),
            },
            NestedModportDiagnosticSite {
                item_path: origin,
                offending: origin,
                first_conflict: None,
            },
        ),
        NestedLoweringResolveError::NonVariableTerminal {
            path,
            actual_kind,
            origin,
            terminal,
        } => {
            let name = path.to_string();
            (
                name.clone(),
                InvalidNestedModportKind::NonVariableTerminal { name, actual_kind },
                NestedModportDiagnosticSite {
                    item_path: origin,
                    offending: if terminal.beg.source == origin.beg.source {
                        terminal
                    } else {
                        origin
                    },
                    first_conflict: None,
                },
            )
        }
        NestedLoweringResolveError::UnemittableTerminal {
            path,
            actual_type,
            origin,
            terminal,
        } => {
            let name = path.to_string();
            (
                name.clone(),
                InvalidNestedModportKind::UnemittableTerminalType { name, actual_type },
                NestedModportDiagnosticSite {
                    item_path: origin,
                    offending: if terminal.beg.source == origin.beg.source {
                        terminal
                    } else {
                        origin
                    },
                    first_conflict: None,
                },
            )
        }
        NestedLoweringResolveError::FlatNameCollision {
            flat,
            first_path,
            second_path,
            first_origin,
            second_origin,
        } => (
            second_path.to_string(),
            InvalidNestedModportKind::FlatNameCollision {
                flat: flat.to_string(),
                first_path: first_path.to_string(),
            },
            NestedModportDiagnosticSite {
                item_path: second_origin,
                offending: second_origin,
                first_conflict: Some(first_origin),
            },
        ),
    };
    context.insert_error(AnalyzerError::invalid_nested_modport(&path, kind, &site));
}

impl Conv<&PackageDeclaration> for () {
    fn conv(context: &mut Context, value: &PackageDeclaration) -> IrResult<Self> {
        // each top-level component has independent context
        let upper_context = context;
        let mut context = upper_context.child();
        context.inherit(upper_context);

        let _guard = context.conv_profile_guard(value.identifier.text());

        // pop_affiliation is not necessary because the local `context` will be dropped
        context.push_affiliation(Affiliation::Package);

        if let Ok(symbol) = symbol_table::resolve(value.identifier.as_ref())
            && matches!(symbol.found.kind, SymbolKind::Package(ref x) if !x.is_proto)
        {
            context.push_namespace(symbol.found.inner_namespace());
        } else {
            let token: TokenRange = value.identifier.as_ref().into();
            return Err(ir_error!(token));
        }

        if let Some(x) = &value.package_declaration_opt {
            check_generic_bound(&mut context, &x.with_generic_parameter);
        }

        if let Some(x) = &value.package_declaration_opt0 {
            check_proto(&mut context, &value.identifier, &x.scoped_identifier);
        }

        for x in &value.package_declaration_list {
            let items: Vec<_> = x.package_group.as_ref().into();
            for item in items {
                match item {
                    PackageItem::ConstDeclaration(x) => {
                        let _: IrResult<ir::Declaration> =
                            Conv::conv(&mut context, x.const_declaration.as_ref());
                    }
                    PackageItem::FunctionDeclaration(x) => {
                        let _: IrResult<()> =
                            Conv::conv(&mut context, x.function_declaration.as_ref());
                    }
                    PackageItem::StructUnionDeclaration(x) => {
                        let _: IrResult<()> =
                            Conv::conv(&mut context, x.struct_union_declaration.as_ref());
                    }
                    _ => (),
                }
            }
        }

        context.pop_namespace();
        upper_context.inherit(&mut context);

        Ok(())
    }
}

impl Conv<&ProtoModuleDeclaration> for ir::Module {
    fn conv(context: &mut Context, value: &ProtoModuleDeclaration) -> IrResult<Self> {
        // each top-level component has independent context
        let upper_context = context;
        let mut context = upper_context.child();
        context.inherit(upper_context);

        // pop_affiliation is not necessary because the local `context` will be dropped
        context.push_affiliation(Affiliation::ProtoModule);

        if let Ok(symbol) = symbol_table::resolve(value.identifier.as_ref())
            && matches!(symbol.found.kind, SymbolKind::Module(ref x) if x.is_proto)
        {
            context.push_namespace(symbol.found.inner_namespace());
        } else {
            let token: TokenRange = value.identifier.as_ref().into();
            return Err(ir_error!(token));
        }

        if let Some(x) = &value.proto_module_declaration_opt
            && let Some(x) = &x.with_parameter.with_parameter_opt
        {
            let items: Vec<_> = x.with_parameter_list.as_ref().into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        if let Some(x) = &value.proto_module_declaration_opt0
            && let Some(x) = &x.port_declaration.port_declaration_opt
        {
            let items: Vec<_> = x.port_declaration_list.as_ref().into();
            for item in items {
                let _ret: IrResult<()> = Conv::conv(&mut context, item);
            }
        }

        let port_types = context.drain_port_types();
        let variables = context.drain_variables();

        let mut ports = HashMap::default();

        for (id, var) in &variables {
            if var.kind.is_port() {
                ports.insert(var.path.clone(), *id);
            }
        }

        context.pop_namespace();
        upper_context.inherit(&mut context);

        Ok(ir::Module {
            name: value.identifier.text(),
            token: value.identifier.as_ref().into(),
            ports,
            port_types,
            variables,
            functions: HashMap::default(),
            declarations: vec![],
            suppress_unassigned: false,
            per_decl_refs: HashMap::default(),
            assign_tokens: HashMap::default(),
            ff_table: ir::FfTable::default(),
        })
    }
}

impl Conv<&ProtoInterfaceDeclaration> for () {
    fn conv(context: &mut Context, value: &ProtoInterfaceDeclaration) -> IrResult<Self> {
        context.push_affiliation(Affiliation::ProtoInterface);

        if let Ok(symbol) = symbol_table::resolve(value.identifier.as_ref())
            && matches!(symbol.found.kind, SymbolKind::Interface(ref x) if x.is_proto)
        {
            context.push_namespace(symbol.found.inner_namespace());
        } else {
            let token: TokenRange = value.identifier.as_ref().into();
            return Err(ir_error!(token));
        }

        for x in &value.proto_interface_declaration_list {
            if let ProtoInterfaceItem::ProtoAliasDeclaration(x) = x.proto_interface_item.as_ref() {
                let r#type = match x
                    .proto_alias_declaration
                    .proto_alias_declaration_group
                    .as_ref()
                {
                    ProtoAliasDeclarationGroup::Module(_) => AliasType::ProtoModule,
                    ProtoAliasDeclarationGroup::Interface(_) => AliasType::ProtoInterface,
                    ProtoAliasDeclarationGroup::Package(_) => AliasType::ProtoPackage,
                };
                check_alias_target(
                    context,
                    &x.proto_alias_declaration.scoped_identifier,
                    r#type,
                );
            }
        }

        context.pop_affiliation();
        context.pop_namespace();
        Ok(())
    }
}

impl Conv<&ProtoPackageDeclaration> for () {
    fn conv(context: &mut Context, value: &ProtoPackageDeclaration) -> IrResult<Self> {
        context.push_affiliation(Affiliation::ProtoPackage);

        if let Ok(symbol) = symbol_table::resolve(value.identifier.as_ref())
            && matches!(symbol.found.kind, SymbolKind::Package(ref x) if x.is_proto)
        {
            context.push_namespace(symbol.found.inner_namespace());
        } else {
            let token: TokenRange = value.identifier.as_ref().into();
            return Err(ir_error!(token));
        }

        for x in &value.proto_package_declaration_list {
            if let ProtoPacakgeItem::ProtoAliasDeclaration(x) = x.proto_pacakge_item.as_ref() {
                let r#type = match x
                    .proto_alias_declaration
                    .proto_alias_declaration_group
                    .as_ref()
                {
                    ProtoAliasDeclarationGroup::Module(_) => AliasType::ProtoModule,
                    ProtoAliasDeclarationGroup::Interface(_) => AliasType::ProtoInterface,
                    ProtoAliasDeclarationGroup::Package(_) => AliasType::ProtoPackage,
                };
                check_alias_target(
                    context,
                    &x.proto_alias_declaration.scoped_identifier,
                    r#type,
                );
            }
        }

        context.pop_affiliation();
        context.pop_namespace();
        Ok(())
    }
}
