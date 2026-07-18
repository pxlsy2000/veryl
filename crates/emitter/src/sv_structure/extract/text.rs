use super::*;

pub(super) fn identifier_before_call_parenthesis(line: &str) -> Option<String> {
    let open = line.find('(')?;
    let prefix = line.get(..open)?.trim_end();
    let start = prefix
        .char_indices()
        .rev()
        .find(|(_, character)| !is_identifier_character(*character))
        .map_or(0, |(index, character)| index + character.len_utf8());
    let identifier = prefix.get(start..)?;
    (!identifier.is_empty()).then(|| identifier.to_owned())
}

pub(super) fn qualified_function_calls(source: &str) -> Vec<QualifiedFunctionCall> {
    let mut calls = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source.get(offset..).and_then(|tail| tail.find("::")) {
        let separator = offset + relative;
        let owner_start = source
            .get(..separator)
            .and_then(|prefix| {
                prefix
                    .char_indices()
                    .rev()
                    .find(|(_, character)| !is_identifier_character(*character))
                    .map(|(index, character)| index + character.len_utf8())
            })
            .unwrap_or(0);
        let function_start = separator + 2;
        let function_end = source
            .get(function_start..)
            .and_then(|tail| {
                tail.char_indices()
                    .find(|(_, character)| !is_identifier_character(*character))
                    .map(|(index, _)| function_start + index)
            })
            .unwrap_or(source.len());
        let is_call = source
            .get(function_end..)
            .is_some_and(|tail| tail.trim_start().starts_with('('));
        if is_call
            && let (Some(package), Some(function)) = (
                source.get(owner_start..separator),
                source.get(function_start..function_end),
            )
        {
            calls.push(QualifiedFunctionCall {
                package: package.to_owned(),
                function: function.to_owned(),
            });
        }
        offset = function_end.max(separator + 2);
    }
    calls
}

pub(super) fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
}

pub(super) fn is_declaration_owner_boundary(node: &RefNode<'_>) -> bool {
    matches!(
        node,
        RefNode::AlwaysConstruct(_)
            | RefNode::CheckerDeclaration(_)
            | RefNode::ClassDeclaration(_)
            | RefNode::ConditionalGenerateConstruct(_)
            | RefNode::FinalConstruct(_)
            | RefNode::FunctionDeclaration(_)
            | RefNode::GenerateRegion(_)
            | RefNode::InitialConstruct(_)
            | RefNode::InterfaceDeclaration(_)
            | RefNode::InterfaceDeclarationAnsi(_)
            | RefNode::InterfaceDeclarationNonansi(_)
            | RefNode::InterfaceDeclarationWildcard(_)
            | RefNode::LoopGenerateConstruct(_)
            | RefNode::ModuleDeclaration(_)
            | RefNode::ModuleDeclarationAnsi(_)
            | RefNode::ModuleDeclarationNonansi(_)
            | RefNode::ModuleDeclarationWildcard(_)
            | RefNode::PackageDeclaration(_)
            | RefNode::ProgramDeclaration(_)
            | RefNode::ProgramDeclarationAnsi(_)
            | RefNode::ProgramDeclarationNonansi(_)
            | RefNode::ProgramDeclarationWildcard(_)
            | RefNode::TaskDeclaration(_)
    )
}

pub(super) fn component_name(node: &RefNode<'_>, source: &str, kind: ComponentKind) -> String {
    match kind {
        ComponentKind::Interface => unwrap_node!(node.clone(), InterfaceIdentifier),
        ComponentKind::Module => unwrap_node!(node.clone(), ModuleIdentifier),
    }
    .map(|identifier| normalize_text(node_text(&identifier, source)))
    .unwrap_or_default()
}

pub(super) fn parse_direction(text: &str) -> Option<PortDirection> {
    match normalize_text(text).as_str() {
        "inout" => Some(PortDirection::Inout),
        "output" => Some(PortDirection::Output),
        "input" => Some(PortDirection::Input),
        _ => None,
    }
}

pub(super) fn identifier_segments(node: &RefNode<'_>, source: &str) -> Vec<String> {
    let RefNode::HierarchicalIdentifier(identifier) = node else {
        return Vec::new();
    };
    let mut segments: Vec<String> = identifier
        .nodes
        .1
        .iter()
        .map(|(segment, _, _)| normalize_text(node_text(&RefNode::Identifier(segment), source)))
        .collect();
    segments.push(normalize_text(node_text(
        &RefNode::Identifier(&identifier.nodes.2),
        source,
    )));
    segments
}

pub(super) fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn node_text<'a>(node: &RefNode<'_>, source: &'a str) -> &'a str {
    let mut start = None;
    let mut end = 0;
    for descendant in node.clone().into_iter() {
        if let RefNode::Locate(location) = descendant {
            start = Some(start.map_or(location.offset, |value: usize| value.min(location.offset)));
            end = end.max(location.offset + location.len);
        }
    }
    match start {
        Some(start) if start <= end && end <= source.len() => &source[start..end],
        Some(_) | None => "",
    }
}
