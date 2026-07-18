use sv_parser::{NodeEvent, RefNode, WhiteSpace};

pub(super) fn from_node(node: &RefNode<'_>, source: &str) -> Vec<String> {
    let mut trivia_depth = 0usize;
    let mut tokens = Vec::new();
    for event in node.clone().into_iter().event() {
        match event {
            NodeEvent::Enter(node) if is_lexical_trivia(&node) => trivia_depth += 1,
            NodeEvent::Leave(node) if is_lexical_trivia(&node) => trivia_depth -= 1,
            NodeEvent::Enter(RefNode::Locate(location)) if trivia_depth == 0 => {
                if let Some(token) = source.get(location.offset..location.offset + location.len) {
                    tokens.push(token.to_owned());
                }
            }
            NodeEvent::Enter(_) | NodeEvent::Leave(_) => {}
        }
    }
    tokens
}

fn is_lexical_trivia(node: &RefNode<'_>) -> bool {
    matches!(
        node,
        RefNode::WhiteSpace(WhiteSpace::Newline(_) | WhiteSpace::Space(_) | WhiteSpace::Comment(_))
    )
}

pub(super) fn from_text(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index += 2;
            while index + 1 < bytes.len() && !bytes[index..].starts_with(b"*/") {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        let start = index;
        if bytes[index] == b'\\' {
            index += 1;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
        } else if bytes[index].is_ascii_alphanumeric()
            || matches!(bytes[index], b'_' | b'$' | b'\'')
        {
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || matches!(bytes[index], b'_' | b'$' | b'\''))
            {
                index += 1;
            }
        } else {
            index += longest_operator(&bytes[index..]);
        }
        if let Some(token) = text.get(start..index) {
            tokens.push(token.to_owned());
        }
    }
    tokens
}

fn longest_operator(input: &[u8]) -> usize {
    const OPERATORS: [&[u8]; 22] = [
        b"<<<", b">>>", b"===", b"!==", b"==?", b"!=?", b"&&", b"||", b"<<", b">>", b"<=", b">=",
        b"==", b"!=", b"+:", b"-:", b"**", b"~&", b"~|", b"~^", b"^~", b"::",
    ];
    OPERATORS
        .iter()
        .find(|operator| input.starts_with(operator))
        .map_or(1, |operator| operator.len())
}
