use super::*;

impl SvStructure {
    pub(crate) fn expect_component(
        &self,
        kind: ComponentKind,
        name: &str,
    ) -> Result<(), SvStructureError> {
        let count = self
            .components
            .iter()
            .filter(|component| component.kind == kind && component.name == name)
            .count();
        expect_one(count, &format!("component {kind:?} {name}"))
    }

    pub(crate) fn expect_declaration(
        &self,
        expected: DeclarationExpectation<'_>,
    ) -> Result<(), SvStructureError> {
        let expected_tokens = type_tokens::from_text(expected.type_text);
        let count = self
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.owner == expected.owner
                    && declaration.name == expected.name
                    && declaration.type_tokens == expected_tokens
            })
            .count();
        expect_one(
            count,
            &format!(
                "declaration {}.{} with type {}",
                expected.owner, expected.name, expected.type_text
            ),
        )
    }

    pub(crate) fn expect_modport_member(
        &self,
        expected: ModportMemberExpectation<'_>,
    ) -> Result<(), SvStructureError> {
        let count = self
            .modport_members
            .iter()
            .filter(|member| {
                member.owner == expected.owner
                    && member.modport == expected.modport
                    && member.member == expected.member
                    && member.direction == expected.direction
            })
            .count();
        expect_one(
            count,
            &format!(
                "modport member {}.{}.{} as {:?}",
                expected.owner, expected.modport, expected.member, expected.direction
            ),
        )
    }

    pub(crate) fn expect_instance_absent(
        &self,
        expected: InstanceExpectation<'_>,
    ) -> Result<(), SvStructureError> {
        let count = self.instance_count(expected);
        if count == 0 {
            Ok(())
        } else {
            Err(invariant(format!(
                "instance {}.{} of {} must be absent, found {count}",
                expected.owner, expected.instance, expected.component
            )))
        }
    }

    pub(crate) fn expect_instance(
        &self,
        expected: InstanceExpectation<'_>,
    ) -> Result<(), SvStructureError> {
        expect_one(
            self.instance_count(expected),
            &format!(
                "instance {}.{} of {}",
                expected.owner, expected.instance, expected.component
            ),
        )
    }

    pub(crate) fn expect_identifier_chain_absent(
        &self,
        owner: &str,
        segments: &[&str],
    ) -> Result<(), SvStructureError> {
        if self.identifier_chain_count(owner, segments) > 0 {
            Err(invariant(format!(
                "identifier chain {owner}.{} must be absent",
                segments.join(".")
            )))
        } else {
            Ok(())
        }
    }

    pub(crate) fn expect_identifier_chain(
        &self,
        owner: &str,
        segments: &[&str],
    ) -> Result<(), SvStructureError> {
        let count = self.identifier_chain_count(owner, segments);
        expect_one(
            count,
            &format!("identifier chain {owner}.{}", segments.join(".")),
        )
    }

    pub(crate) fn expect_qualified_function_calls_declared(&self) -> Result<(), SvStructureError> {
        for call in &self.qualified_function_calls {
            let declarations = self
                .package_functions
                .iter()
                .filter(|declaration| {
                    declaration.package == call.package && declaration.function == call.function
                })
                .count();
            if declarations != 1 {
                return Err(invariant(format!(
                    "qualified call {}::{} must resolve to exactly one declaration in the same package, found {declarations}",
                    call.package, call.function
                )));
            }
        }
        Ok(())
    }

    fn identifier_chain_count(&self, owner: &str, segments: &[&str]) -> usize {
        self.identifier_chains
            .iter()
            .filter(|chain| {
                chain.owner == owner
                    && chain
                        .segments
                        .iter()
                        .map(String::as_str)
                        .eq(segments.iter().copied())
            })
            .count()
    }

    fn instance_count(&self, expected: InstanceExpectation<'_>) -> usize {
        self.instances
            .iter()
            .filter(|instance| {
                instance.owner == expected.owner
                    && instance.component == expected.component
                    && instance.instance == expected.instance
            })
            .count()
    }
}

fn expect_one(count: usize, description: &str) -> Result<(), SvStructureError> {
    if count == 1 {
        Ok(())
    } else {
        Err(invariant(format!(
            "expected one {description}, found {count}"
        )))
    }
}

fn invariant(detail: String) -> SvStructureError {
    SvStructureError::Invariant { detail }
}
