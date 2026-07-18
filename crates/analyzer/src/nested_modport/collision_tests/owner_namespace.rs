#[test]
fn two_nested_roots_collide_after_terminal_affix_semantics() {
    let code = r#"interface ClockIf {
    var clk: clock;
    modport source { clk: output, }
}
interface LogicIf {
    var clk: logic;
    modport source { clk: output, }
}
interface ParentIf {
    inst child: ClockIf;
    inst cp_child: LogicIf;
    modport source {
        child.source: modport,
        cp_child.source: modport,
    }
}"#;
    let mut metadata = Metadata::create_default("prj").expect("valid metadata");
    metadata.build.clock_posedge_prefix = Some("cp_".to_owned());
    let errors = analyze(code, &metadata);
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "cp_child.clk",
            flat: "cp_child__clk",
            first_path: "child.clk",
            primary: (
                code.find("cp_child.source").expect("second nested item"),
                "cp_child.source".len(),
            ),
            secondary: (
                code.find("child.source").expect("first nested item"),
                "child.source".len(),
            ),
        },
    );
}

#[test]
fn raw_and_affixed_noncolliding_identifiers_remain_legal() {
    let code = r#"interface ChildIf {
    var clk: clock;
    modport source { clk: output, }
}
interface ParentIf {
    inst r#child: ChildIf;
    var child__clk: logic;
    modport source {
        r#child.source: modport,
        child__clk: input,
    }
}"#;
    let mut metadata = Metadata::create_default("prj").expect("valid metadata");
    metadata.build.clock_posedge_prefix = Some("cp_".to_owned());
    let errors = analyze(code, &metadata);
    assert!(errors.is_empty(), "noncolliding control failed: {errors:?}");
}

#[test]
fn retained_interface_typedef_collides_with_flattened_terminal() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    type child__payload = logic;
    modport sink { child.sink: modport, }
}"#;
    let errors = analyze(code, &Metadata::create_default("prj").unwrap());
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "child.payload",
            flat: "child__payload",
            first_path: "child__payload",
            primary: (code.find("child.sink").unwrap(), "child.sink".len()),
            secondary: (code.find("child__payload").unwrap(), "child__payload".len()),
        },
    );
}

#[test]
fn retained_interface_parameter_and_instance_names_collide_with_flattened_terminal() {
    for declaration in [
        "const child__payload: u32 = 1;",
        "inst child__payload: MarkerIf;",
    ] {
        let code = format!(
            r#"interface ChildIf {{
    var payload: logic;
    modport sink {{ payload: input, }}
}}
interface MarkerIf {{}}
interface ParentIf {{
    inst child: ChildIf;
    {declaration}
    modport sink {{ child.sink: modport, }}
}}"#
        );
        let errors = analyze(&code, &Metadata::create_default("prj").unwrap());
        assert!(!errors.is_empty(), "expected collision for `{declaration}`");
        assert_collision(
            &errors,
            ExpectedCollision {
                path: "child.payload",
                flat: "child__payload",
                first_path: "child__payload",
                primary: (code.find("child.sink").unwrap(), "child.sink".len()),
                secondary: (code.find("child__payload").unwrap(), "child__payload".len()),
            },
        );
    }
}

#[test]
fn retained_owner_genvar_collides_with_flattened_terminal_at_exact_sites() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    for child__payload in 0..1 : generated {
        var _marker: logic;
    }
    modport sink { child.sink: modport, }
}"#;
    let errors = analyze(
        code,
        &Metadata::create_default("prj").expect("valid metadata"),
    );
    assert_collision(
        &errors,
        ExpectedCollision {
            path: "child.payload",
            flat: "child__payload",
            first_path: "child__payload",
            primary: (
                code.find("child.sink").expect("nested item"),
                "child.sink".len(),
            ),
            secondary: (
                code.find("child__payload").expect("owner genvar"),
                "child__payload".len(),
            ),
        },
    );
}

#[test]
fn nested_generate_genvar_stays_outside_the_emitted_owner_namespace() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    for outer in 0..1 : outer_generated {
        for child__payload in 0..1 : inner_generated {
            var _marker: logic;
        }
    }
    modport sink { child.sink: modport, }
}"#;
    let errors = analyze(
        code,
        &Metadata::create_default("prj").expect("valid metadata"),
    );
    assert!(
        errors.is_empty(),
        "a genvar emitted inside a nested generate scope must remain legal: {errors:?}"
    );
}

#[test]
fn differently_named_owner_genvar_remains_legal() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    for item_index in 0..1 : generated {
        var _marker: logic;
    }
    modport sink { child.sink: modport, }
}"#;
    let errors = analyze(
        code,
        &Metadata::create_default("prj").expect("valid metadata"),
    );
    assert!(
        errors.is_empty(),
        "a differently named owner genvar must remain legal: {errors:?}"
    );
}

#[test]
fn inner_function_local_and_modport_name_do_not_seed_component_declaration_collisions() {
    let code = r#"interface ChildIf {
    var payload: logic;
    modport sink { payload: input, }
}
interface ParentIf {
    inst child: ChildIf;
    function helper() -> logic {
        let child__payload: logic = 0;
        return child__payload;
    }
    modport child__payload { child.sink: modport, }
}"#;
    let errors = analyze(code, &Metadata::create_default("prj").unwrap());
    assert!(
        errors.is_empty(),
        "separate/inner namespaces must remain legal: {errors:?}"
    );
}
