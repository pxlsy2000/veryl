fn connected_fixture(scale: usize) -> String {
    let mut code = String::from(
        "interface GenericIf::<W: u32 = 8> {\n    var payload: logic<W>;\n    modport sink { payload: input, }\n}\n\n#[expand(modport)]\nmodule Sink (\n",
    );
    for index in 0..scale {
        let width = if index % 2 == 0 { 8 } else { 16 };
        code.push_str(&format!(
            "    port{index}: modport GenericIf::<{width}>::sink,\n"
        ));
    }
    code.push_str(") {}\n\nmodule Top {\n");
    for index in 0..scale {
        let width = if index % 2 == 0 { 8 } else { 16 };
        code.push_str(&format!("    inst iface{index}: GenericIf::<{width}>;\n"));
        code.push_str(&format!("    assign iface{index}.payload = '0;\n"));
    }
    code.push_str("    inst sink: Sink (\n");
    for index in 0..scale {
        code.push_str(&format!("        port{index}: iface{index},\n"));
    }
    code.push_str("    );\n}\n");
    code
}

fn member_fixture(scale: usize) -> String {
    let mut code = String::from("interface WideIf {\n");
    for index in 0..scale {
        code.push_str(&format!("    var v{index}: logic;\n"));
    }
    code.push_str("    modport sink {\n");
    for index in 0..scale {
        code.push_str(&format!("        v{index}: input,\n"));
    }
    code.push_str(
        "    }\n}\n\npackage Helpers {\n    function observe (p: modport WideIf::sink) -> logic {\n",
    );
    for index in 0..scale {
        code.push_str(&format!(
            "        let _observed{index}: logic = p.v{index};\n"
        ));
    }
    code.push_str("        return p.v0;\n    }\n}\n\nmodule Top {\n    inst p: WideIf;\n");
    for index in 0..scale {
        code.push_str(&format!("    assign p.v{index} = 0;\n"));
    }
    code.push_str("    let _observed: logic = Helpers::observe(p);\n}\n");
    code
}

fn nested_member_fixture() -> &'static str {
    r#"interface ChildIf {
    var payload: logic<8>;
    modport sink { payload: input, }
}

interface ParentIf {
    inst child: ChildIf;
    modport sink { child.sink: modport, }
}

package Helpers {
    function observe (p: modport ParentIf::sink) -> logic {
        return p.child.payload[0];
    }
}

module Top {
    inst parent: ParentIf;
    assign parent.child.payload = 0;
    let _observed: logic = Helpers::observe(parent);
}
"#
}

fn nested_member_scaling_fixture(scale: usize) -> String {
    let mut code = String::from(
        "package Data::<W: u32> {\n    struct Payload {\n        bits: logic<W>,\n    }\n}\n\ninterface GenericChild::<W: u32> {\n",
    );
    for index in 0..scale {
        code.push_str(&format!("    var v{index}: Data::<W>::Payload[2];\n"));
    }
    code.push_str("    modport sink {\n");
    for index in 0..scale {
        code.push_str(&format!("        v{index}: input,\n"));
    }
    code.push_str(
        "    }\n}\n\ninterface ParentIf::<W: u32> {\n    inst child: GenericChild::<W>;\n    modport sink { child.sink: modport, }\n}\n\npackage Unrelated {\n    const v0: logic = 0;\n}\n\npackage Helpers {\n    import Unrelated::*;\n",
    );
    for width in [8, 16] {
        code.push_str(&format!(
            "    function observe{width} (p: modport ParentIf::<{width}>::sink) -> logic {{\n        let _unrelated{width}: logic = v0;\n"
        ));
        for index in 0..scale {
            code.push_str(&format!(
                "        let _observed{width}_{index}: logic = p.child.v{index}[1].bits[0];\n"
            ));
        }
        code.push_str("        return p.child.v0[0].bits[0];\n    }\n");
    }
    code.push_str(
        "}\n\nmodule Top {\n    #[allow(unassign_variable)]\n    inst parent8: ParentIf::<8>;\n    #[allow(unassign_variable)]\n    inst parent16: ParentIf::<16>;\n    let _result8: logic = Helpers::observe8(parent8);\n    let _result16: logic = Helpers::observe16(parent16);\n}\n",
    );
    code
}

fn assert_affine(normal: &[Measurement; 3], mutation: &[Measurement; 3]) {
    for (normal, mutation) in normal.iter().zip(mutation) {
        assert_eq!(normal.text, mutation.text);
        assert_eq!(normal.source_map, mutation.source_map);
    }
    let normal_work = normal.each_ref().map(|value| value.work);
    let mutation_work = mutation.each_ref().map(|value| value.work);
    println!("indexed={normal_work:?} scan_mutation={mutation_work:?}");
    assert!(normal_work[0] > 0, "{normal_work:?}");
    assert!(normal_work[1] <= 3 * normal_work[0], "{normal_work:?}");
    assert!(normal_work[2] <= 3 * normal_work[1], "{normal_work:?}");
    assert!(mutation_work[1] > 3 * mutation_work[0], "{mutation_work:?}");
    assert!(mutation_work[2] > 3 * mutation_work[1], "{mutation_work:?}");
}

#[test]
fn connected_map_lookup_is_affine_when_ports_and_actuals_scale_together() {
    let normal = [64, 128, 256].map(|scale| emit_measured(&connected_fixture(scale)));
    let mutation = [64, 128, 256]
        .map(|scale| with_connected_map_scan_mutation(|| emit_measured(&connected_fixture(scale))));
    for (level, measurement) in normal.iter().enumerate() {
        assert!(measurement.text.contains("GenericIf__8"));
        assert!(measurement.text.contains("GenericIf__16"));
        for index in 0..(64 << level) {
            assert!(
                measurement.text.contains(&format!("__port{index}_payload")),
                "{}",
                measurement.text
            );
        }
    }
    assert_affine(&normal, &mutation);
}
