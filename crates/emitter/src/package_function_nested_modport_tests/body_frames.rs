#[test]
fn package_function_body_preserves_raw_paths_affixes_and_selects() {
    let code = r##"interface ChildIf {
    var clk: clock;
    var payload: logic<8>;
    modport sink {
        clk: input,
        payload: inout,
    }
}
interface ParentIf {
    inst r#same: ChildIf;
    modport sink { r#same.sink: modport, }
}
package Pkg {
    function mutate (
        p: modport ParentIf::sink,
    ) -> logic {
        p.r#same.payload[0] = p.r#same.payload[1];
        return p.r#same.clk;
    }
}
module Top {
    #[allow(unassign_variable)]
    inst parent: ParentIf;
    assign parent.r#same.clk = 0;
    let observed: logic = Pkg::mutate(parent);
}
"##;
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.build.clock_posedge_prefix = Some("cp_".to_owned());
    metadata.build.clock_posedge_suffix = Some("_cs".to_owned());
    let emitted = emit_artifacts(&metadata, code).0;

    assert!(
        emitted.contains("__p_same__payload[0] = __p_same__payload[1]"),
        "{emitted}"
    );
    assert!(emitted.contains("return __p_cp_same__clk_cs;"), "{emitted}");
    assert!(emitted.contains("parent.cp_same__clk_cs"), "{emitted}");
    assert!(!emitted.contains("p.r#same"), "{emitted}");
    assert!(!emitted.contains("p.same"), "{emitted}");
}

#[test]
fn missing_package_function_port_record_is_typed_and_commits_nothing() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    modport sink { child.sink: modport, }
}
package Pkg {
    function consume (p: modport ParentIf::sink,) -> logic {
        return p.child.payload;
    }
}
"#;
    symbol_table::clear();
    attribute_table::clear();
    let metadata = Metadata::create_default("prj").unwrap();
    let source = PathBuf::from("package_function_fault.veryl");
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
        &PathBuf::from("package_function_fault.sv"),
        &PathBuf::from("package_function_fault.sv.map"),
    );
    emitter.inject_function_expanded_port_fault();

    let error = emitter.emit(&parser.veryl, code, &analysis).unwrap_err();

    let crate::EmitterError::MissingExpandedPortResolution { binding, token } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_ne!(
        binding,
        veryl_analyzer::nested_modport::EmissionBindingId::new(0)
    );
    assert_ne!(token, veryl_parser::resource_table::TokenId(0));
    assert!(emitter.as_str().is_empty());
    assert!(emitter.source_map().to_bytes().is_err());
}

#[test]
fn function_frames_restore_enclosing_module_and_interface_frames() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    modport sink { child.sink: modport, }
}
interface WrapperIf {
    inst inner: ParentIf;
    function sample (p: modport ParentIf::sink,) -> logic {
        return p.child.payload;
    }
    let local: logic = inner.child.payload;
    modport sink { inner.sink: modport, }
}
module Consumer (
    p: modport ParentIf::sink,
) {
    function sample (q: modport ParentIf::sink,) -> logic {
        return q.child.payload;
    }
    let from_call: logic = sample(p);
    let after_function: logic = p.child.payload;
}
"#;
    let emitted = emit(code);

    assert!(emitted.contains("return __p_child__payload;"), "{emitted}");
    assert!(emitted.contains("return __q_child__payload;"), "{emitted}");
    assert!(emitted.contains("sample(p.child__payload)"), "{emitted}");
    assert!(emitted.contains("p.child__payload"), "{emitted}");
    assert!(emitted.contains("inner__child__payload"), "{emitted}");
    assert!(!emitted.contains(".child.payload"), "{emitted}");
}

#[test]
fn uninstantiated_generic_interface_import_does_not_publish_function_binding() {
    let code = r#"interface UnusedIf::<W: u32> {
    var payload: logic<W>;
    function ready () -> logic {
        return payload[0];
    }
    modport sink {
        payload: input,
        ready: import,
    }
}
module Top {}
"#;

    let emitted = emit(code);

    assert!(emitted.contains("module prj_Top;"), "{emitted}");
    assert!(!emitted.contains("prj_UnusedIf"), "{emitted}");
}

#[test]
fn generic_package_member_function_uses_each_package_specialization() {
    let code = r#"package WidthPkg::<W: u32> {
    type Data = logic<W>;
    function width () -> u32 {
        return W;
    }
}
module Top {
    let _data8 : WidthPkg::<8>::Data  = 0;
    let _data16: WidthPkg::<16>::Data = 0;
}
"#;

    let emitted = emit(code);

    assert!(emitted.contains("package prj___WidthPkg__8;"), "{emitted}");
    assert!(emitted.contains("package prj___WidthPkg__16;"), "{emitted}");
    assert_eq!(
        emitted
            .matches("function automatic int unsigned width()")
            .count(),
        2
    );
}

#[test]
fn generic_package_and_function_specializations_keep_exact_ownership() {
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
    type PackageData = logic<PW>;
    function consume::<FW: u32> (
        p: modport ParentIf::<FW>::sink,
    ) -> logic<FW> {
        return p.child.payload;
    }
}
module Top {
    #[allow(unassign_variable)]
    inst parent8 : ParentIf::<8>;
    #[allow(unassign_variable)]
    inst parent16: ParentIf::<16>;
    let _force8 : Combo::<8>::PackageData = 0;
    let _force16: Combo::<16>::PackageData = 0;
    let _observed8 : logic<8>  = Combo::<8>::consume::<8>(parent8);
    let _observed16: logic<16> = Combo::<16>::consume::<16>(parent16);
}
"#;

    // When
    let emitted = emit(code);

    // Then
    let package8 = emitted
        .split_once("package prj___Combo__8;")
        .and_then(|(_, tail)| tail.split_once("endpackage"))
        .map(|(body, _)| body)
        .expect("8-bit package must be emitted");
    let package16 = emitted
        .split_once("package prj___Combo__16;")
        .and_then(|(_, tail)| tail.split_once("endpackage"))
        .map(|(body, _)| body)
        .expect("16-bit package must be emitted");
    assert!(package8.contains("__consume__8("), "{emitted}");
    assert!(!package8.contains("__consume__16("), "{emitted}");
    assert!(package16.contains("__consume__16("), "{emitted}");
    assert!(!package16.contains("__consume__8("), "{emitted}");
    assert!(
        emitted.contains("prj___Combo__8::__consume__8("),
        "{emitted}"
    );
    assert!(
        emitted.contains("prj___Combo__16::__consume__16("),
        "{emitted}"
    );
}
