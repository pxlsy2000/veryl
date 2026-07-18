#[test]
fn mismatched_package_scope_is_typed_and_commits_nothing() {
    // Given
    let code = r#"interface ChildIf::<W: u32> {
    var payload: logic<W>;
    modport sink { payload: input, }
}

interface ParentIf::<W: u32> {
    inst child: ChildIf::<W>;
    modport sink { child.sink: modport, }
}
package Combo::<PW: u32> {
    function consume::<FW: u32> (
        p: modport ParentIf::<FW>::sink,
    ) -> logic<FW> {
        return p.child.payload;
    }
}
module Top {
    inst parent: ParentIf::<8>;
    assign parent.child.payload = '0;
    let observed: logic<8> = Combo::<8>::consume::<8>(parent);
}
"#;
    symbol_table::clear();
    attribute_table::clear();
    let metadata = Metadata::create_default("prj").unwrap();
    let source = PathBuf::from("package_scope_fault.veryl");
    let parser = Parser::parse(code, &source).unwrap();
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    analyzer.analyze_pass1("prj", &parser.veryl);
    Analyzer::analyze_post_pass1();
    assert!(
        analyzer
            .analyze_pass2(&parser.veryl, &mut context, None)
            .is_empty()
    );
    let analysis = context.finish_nested_modport_analysis().unwrap();
    let mut emitter = Emitter::new(
        &metadata,
        &source,
        &PathBuf::from("package_scope_fault.sv"),
        &PathBuf::from("package_scope_fault.sv.map"),
    );
    emitter.inject_package_scope_fault();

    // When
    let error = emitter.emit(&parser.veryl, code, &analysis).unwrap_err();

    // Then
    let crate::EmitterError::ConflictingEmissionBinding {
        source_path,
        declaration,
        invariant:
            veryl_analyzer::nested_modport::NestedModportAnalysisInvariant::PackageScopeMismatch,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_ne!(
        source_path,
        veryl_parser::resource_table::PathId(usize::MAX)
    );
    assert_ne!(declaration, veryl_parser::resource_table::TokenId(0));
    assert!(emitter.as_str().is_empty());
    assert!(emitter.source_map().to_bytes().is_err());
}

#[test]
fn flattened_named_type_uses_instantiated_child_package_specialization() {
    // Given
    let code = r#"package Types::<W: u32> {
    struct Payload {
        data: logic<W>,
    }
}
interface ChildIf::<W: u32> {
    var payload: Types::<W>::Payload;
    modport sink { payload: input, }
}
interface ParentIf::<W: u32> {
    inst child: ChildIf::<16>;
    modport sink { child.sink: modport, }
}
module Top {
    #[allow(unassign_variable)]
    inst parent: ParentIf::<8>;
}
"#;

    // When
    let emitted = emit(code);

    // Then
    let parent = emitted
        .split_once("interface prj___ParentIf__8;")
        .and_then(|(_, tail)| tail.split_once("endinterface"))
        .map(|(body, _)| body)
        .expect("ParentIf::<8> must be emitted");
    assert!(
        parent.contains("prj___Types__16::Payload child__payload;"),
        "flattened type must retain ChildIf::<16> package specialization:\n{emitted}"
    );
    assert!(!parent.contains("prj_Types::Payload"), "{emitted}");
    assert!(!parent.contains("prj___Types__8::Payload"), "{emitted}");
}

#[test]
fn nested_interface_member_function_is_emitted_with_enclosing_rewrite() {
    // Given
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    function sample () -> logic {
        return child.payload;
    }
    modport sink {
        child.sink: modport,
        sample: import,
    }
}
module Top {
    #[allow(unassign_variable)]
    inst parent: ParentIf;
    let _observed: logic = parent.sample();
}
"#;

    // When
    let emitted = emit(code);

    // Then
    let parent = emitted
        .split_once("interface prj_ParentIf;")
        .and_then(|(_, tail)| tail.split_once("endinterface"))
        .map(|(body, _)| body)
        .expect("ParentIf must be emitted");
    assert!(
        parent.contains("function automatic logic sample()"),
        "member function declaration must be emitted:\n{emitted}"
    );
    assert!(parent.contains("return child__payload;"), "{emitted}");
    assert!(parent.contains("import sample"), "{emitted}");
    assert!(emitted.contains("parent.sample()"), "{emitted}");
    assert!(!emitted.contains("child.payload"), "{emitted}");
}

#[test]
fn flattened_named_type_matrix_preserves_each_child_specialization() {
    let code = r#"package Types::<W: u32> {
    struct StructValue { bits: logic<W>, }
    union UnionValue { bits: logic<W>, }
    enum EnumValue: logic<W> { Zero = 0, One = 1, }
    type StructAlias = StructValue;
    type UnionAlias = UnionValue;
    type EnumAlias = EnumValue;
    type PrimitiveAlias = logic<W>;
}
interface ChildIf::<W: u32> {
    var struct_value: Types::<W>::StructValue;
    var union_value: Types::<W>::UnionValue;
    var enum_value: Types::<W>::EnumValue;
    var struct_alias: Types::<W>::StructAlias;
    var union_alias: Types::<W>::UnionAlias;
    var enum_alias: Types::<W>::EnumAlias;
    var primitive_alias: Types::<W>::PrimitiveAlias;
    modport sink {
        struct_value: input,
        union_value: input,
        enum_value: input,
        struct_alias: input,
        union_alias: input,
        enum_alias: input,
        primitive_alias: input,
    }
}
interface ParentIf {
    inst child8: ChildIf::<8>;
    inst child16: ChildIf::<16>;
    modport sink {
        child8.sink: modport,
        child16.sink: modport,
    }
}
module Top {
    #[allow(unassign_variable)]
    inst parent: ParentIf;
}
"#;

    let emitted = emit(code);

    for (width, root) in [(8, "child8"), (16, "child16")] {
        for (field, named) in [
            ("struct_value", "StructValue"),
            ("union_value", "UnionValue"),
            ("enum_value", "EnumValue"),
            ("struct_alias", "StructValue"),
            ("union_alias", "UnionValue"),
            ("enum_alias", "EnumValue"),
        ] {
            let expected = format!("prj___Types__{width}::{named} {root}__{field};");
            assert!(
                emitted.contains(&expected),
                "missing `{expected}`:\n{emitted}"
            );
        }
        let primitive = format!("logic[{width}-1:0] {root}__primitive_alias;");
        assert!(
            emitted.contains(&primitive),
            "missing `{primitive}`:\n{emitted}"
        );
    }
}
