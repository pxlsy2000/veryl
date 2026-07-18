use crate::symbol::{Symbol, SymbolKind};
use crate::symbol_table;
use veryl_parser::resource_table::{self, StrId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EmittedIdentifierIdentity(StrId);

impl EmittedIdentifierIdentity {
    pub fn from_logical(logical: StrId) -> Self {
        Self::from_affixes(logical, None, None)
    }

    pub fn for_symbol(logical: StrId, symbol: &Symbol) -> Self {
        match &symbol.kind {
            SymbolKind::Port(property) => Self::from_affixes(
                logical,
                property.prefix.as_deref(),
                property.suffix.as_deref(),
            ),
            SymbolKind::Variable(property) => Self::from_affixes(
                logical,
                property.prefix.as_deref(),
                property.suffix.as_deref(),
            ),
            SymbolKind::ModportVariableMember(property) => {
                symbol_table::get(property.variable).as_ref().map_or_else(
                    || Self::from_logical(logical),
                    |symbol| Self::for_symbol(logical, symbol),
                )
            }
            _ => Self::from_logical(logical),
        }
    }

    fn from_affixes(logical: StrId, prefix: Option<&str>, suffix: Option<&str>) -> Self {
        let logical = logical.to_string();
        let logical = logical.strip_prefix("r#").unwrap_or(&logical);
        let mut identity = String::with_capacity(
            prefix.map_or(0, str::len) + logical.len() + suffix.map_or(0, str::len),
        );
        identity.push_str(prefix.unwrap_or_default());
        identity.push_str(logical);
        identity.push_str(suffix.unwrap_or_default());
        Self(resource_table::insert_str(&identity))
    }

    pub fn logical(self) -> StrId {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> StrId {
        resource_table::insert_str(text)
    }

    #[test]
    fn raw_and_plain_logical_names_have_one_identity() {
        assert_eq!(
            EmittedIdentifierIdentity::from_logical(id("r#child__payload")),
            EmittedIdentifierIdentity::from_logical(id("child__payload"))
        );
    }

    #[test]
    fn affixes_participate_in_semantic_identity_without_storing_sv_syntax() {
        let affixed =
            EmittedIdentifierIdentity::from_affixes(id("child__clk"), Some("cp_"), Some("_cs"));
        let direct = EmittedIdentifierIdentity::from_logical(id("cp_child__clk_cs"));
        let noncolliding = EmittedIdentifierIdentity::from_logical(id("child__clk"));

        assert_eq!(affixed, direct);
        assert_ne!(affixed, noncolliding);
        assert_eq!(affixed.logical().to_string(), "cp_child__clk_cs");
    }
}
