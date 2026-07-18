#[test]
fn flattened_named_types_preserve_metadata_raw_names_and_source_mappings() {
    let code = r##"package Types::<W: u32> {
    struct Payload { bits: logic<W>, }
    type PayloadAlias = Payload;
}
interface ChildIf::<W: u32> {
    var r#converse: Types::<W>::PayloadAlias;
    var clk: clock;
    modport sink { r#converse: input, clk: input, }
}
interface ParentIf {
    inst child8: ChildIf::<8>;
    inst child16: ChildIf::<16>;
    modport sink { child8.sink: modport, child16.sink: modport, }
}
module Top {
    #[allow(unassign_variable)]
    inst parent: ParentIf;
}
"##;
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.build.hashed_mangled_name = true;
    metadata.build.omit_project_prefix = true;
    metadata.build.clock_posedge_prefix = Some("cp_".to_owned());
    metadata.build.clock_posedge_suffix = Some("_cs".to_owned());

    let (emitted, source_map) = emit_artifacts(&metadata, code);
    let lines: Vec<_> = emitted.lines().collect();
    let package_name = |width| {
        let marker = format!("// __Types__{width}");
        lines
            .windows(2)
            .find(|pair| pair[0] == marker)
            .and_then(|pair| pair[1].strip_prefix("package "))
            .and_then(|line| line.strip_suffix(';'))
            .map(str::to_owned)
            .unwrap_or_else(|| panic!("missing hashed Types::<{width}> package:\n{emitted}"))
    };
    let types8 = package_name(8);
    let types16 = package_name(16);
    assert_ne!(types8, types16);
    assert!(!types8.starts_with("prj_"), "{emitted}");
    assert!(!types16.starts_with("prj_"), "{emitted}");

    let structure = SvStructure::parse(&emitted, Path::new("named_metadata.sv"))
        .unwrap_or_else(|error| panic!("generated SystemVerilog must parse: {error}:\n{emitted}"));
    for (name, type_text) in [
        ("child8__converse", format!("{types8}::Payload")),
        ("child16__converse", format!("{types16}::Payload")),
        ("cp_child8__clk_cs", "logic".to_owned()),
    ] {
        structure
            .expect_declaration(DeclarationExpectation {
                owner: "ParentIf",
                name,
                type_text: &type_text,
            })
            .unwrap_or_else(|error| panic!("{error}:\n{emitted}"));
    }

    let type8 = format!("{types8}::Payload");
    let type16 = format!("{types16}::Payload");
    assert_ne!(type8, type16, "instantiated package roots must be distinct");
    let serialized_map = std::str::from_utf8(&source_map).expect("source map must be UTF-8 JSON");
    for expected_name in [
        type8.as_str(),
        type16.as_str(),
        "child8__converse",
        "child16__converse",
        "cp_child8__clk_cs",
    ] {
        let quoted_name = format!("\"{expected_name}\"");
        assert!(
            serialized_map.contains(&quoted_name),
            "source-map names must contain exact JSON token {quoted_name}: {serialized_map}"
        );
    }

    let temp = CleanTempDir::new("exact");
    let source_path = temp.path().join("package_function.veryl");
    let emitted_path = temp.path().join("package_function.sv");
    let map_path = temp.path().join("package_function.sv.map");
    fs::write(&source_path, code).expect("source fixture must be written");
    fs::write(&emitted_path, &emitted).expect("emitted fixture must be written");
    fs::write(&map_path, &source_map).expect("source-map fixture must be written");
    let canonical_source = fs::canonicalize(&source_path).expect("source path must canonicalize");
    let public_map = SourceMap::from_src(&emitted_path).expect("linked source map must load");
    let fixture = MappingFixture {
        map: &public_map,
        canonical_source: &canonical_source,
        source: code,
        emitted: &emitted,
    };
    let alias_line = "    type PayloadAlias = Payload;";
    fixture.assert_exact(MappingExpectation {
        line_marker: "child8__converse",
        emitted_token: &type8,
        control_previous_lines: 0,
        source_position: (3, 25),
        source_line: alias_line,
        source_token: "Payload",
    });
    fixture.assert_exact(MappingExpectation {
        line_marker: "child16__converse",
        emitted_token: &type16,
        control_previous_lines: 0,
        source_position: (3, 25),
        source_line: alias_line,
        source_token: "Payload",
    });
    fixture.assert_exact(MappingExpectation {
        line_marker: "child8__converse",
        emitted_token: "child8__converse",
        control_previous_lines: 0,
        source_position: (11, 10),
        source_line: "    inst child8: ChildIf::<8>;",
        source_token: "child8",
    });
    fixture.assert_exact(MappingExpectation {
        line_marker: "child16__converse",
        emitted_token: "child16__converse",
        control_previous_lines: 0,
        source_position: (12, 10),
        source_line: "    inst child16: ChildIf::<16>;",
        source_token: "child16",
    });
    fixture.assert_exact(MappingExpectation {
        line_marker: "cp_child8__clk_cs",
        emitted_token: "cp_child8__clk_cs",
        control_previous_lines: 1,
        source_position: (11, 10),
        source_line: "    inst child8: ChildIf::<8>;",
        source_token: "child8",
    });
}

#[test]
fn public_source_map_loader_reports_missing_link_with_typed_error() {
    let temp = CleanTempDir::new("missing-link");
    let emitted_path = temp.path().join("missing-link.sv");
    fs::write(&emitted_path, "module missing_link; endmodule\n")
        .expect("unlinked emitted fixture must be written");

    assert!(matches!(
        SourceMap::from_src(&emitted_path),
        Err(SourceMapError::NotFound)
    ));
}
