use super::lowering_records::*;
use super::semantic_work::{hash_slice, record_semantic_work, slice_eq};
use std::hash::{Hash, Hasher};

impl PartialEq for FlattenedIdentifier {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        self.logical == other.logical && slice_eq(&self.source_segments, &other.source_segments)
    }
}

impl Eq for FlattenedIdentifier {}

impl Hash for FlattenedIdentifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        self.logical.hash(state);
        hash_slice(&self.source_segments, state);
    }
}

impl PartialEq for ResolvedNestedTerminal {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(6);
        self.id == other.id
            && self.identifier == other.identifier
            && self.emitted_identifier == other.emitted_identifier
            && self.variable == other.variable
            && self.symbol == other.symbol
            && self.token == other.token
            && self.resolved_type == other.resolved_type
    }
}

impl Eq for ResolvedNestedTerminal {}

impl Hash for ResolvedNestedTerminal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(6);
        self.id.hash(state);
        self.identifier.hash(state);
        self.emitted_identifier.hash(state);
        self.variable.hash(state);
        self.symbol.hash(state);
        self.token.hash(state);
        self.resolved_type.hash(state);
    }
}

impl PartialEq for ResolvedModportTerminal {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(1);
        match (self, other) {
            (
                Self::DirectVariable {
                    variable: left_variable,
                    symbol: left_symbol,
                    emitted_identifier: left_identifier,
                    resolved_type: left_type,
                },
                Self::DirectVariable {
                    variable: right_variable,
                    symbol: right_symbol,
                    emitted_identifier: right_identifier,
                    resolved_type: right_type,
                },
            ) => {
                record_semantic_work(3);
                left_variable == right_variable
                    && left_symbol == right_symbol
                    && left_identifier == right_identifier
                    && left_type == right_type
            }
            (Self::DirectFunction { function: left }, Self::DirectFunction { function: right }) => {
                left == right
            }
            (
                Self::FlattenedVariable { terminal: left },
                Self::FlattenedVariable { terminal: right },
            ) => left == right,
            _ => false,
        }
    }
}

impl Eq for ResolvedModportTerminal {}

impl Hash for ResolvedModportTerminal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(1);
        std::mem::discriminant(self).hash(state);
        match self {
            Self::DirectVariable {
                variable,
                symbol,
                emitted_identifier,
                resolved_type,
            } => {
                record_semantic_work(3);
                variable.hash(state);
                symbol.hash(state);
                emitted_identifier.hash(state);
                resolved_type.hash(state);
            }
            Self::DirectFunction { function } => function.hash(state),
            Self::FlattenedVariable { terminal } => terminal.hash(state),
        }
    }
}

impl PartialEq for ResolvedModportEntry {
    fn eq(&self, other: &Self) -> bool {
        record_semantic_work(3);
        slice_eq(self.path.as_slice(), other.path.as_slice())
            && self.direction == other.direction
            && self.terminal == other.terminal
            && self.terminal_site == other.terminal_site
    }
}

impl Eq for ResolvedModportEntry {}

impl Hash for ResolvedModportEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        record_semantic_work(3);
        hash_slice(self.path.as_slice(), state);
        self.direction.hash(state);
        self.terminal.hash(state);
        self.terminal_site.hash(state);
    }
}

impl PartialEq for ResolvedModport {
    fn eq(&self, other: &Self) -> bool {
        slice_eq(&self.entries, &other.entries)
    }
}

impl Eq for ResolvedModport {}

impl Hash for ResolvedModport {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_slice(&self.entries, state);
    }
}
