use crate::Emitter;
use std::path::PathBuf;
use veryl_analyzer::nested_modport::{
    emission_query_work, emitter_index_work, reset_emission_query_work, reset_emitter_index_work,
    resolved_member_lookups, with_connected_map_scan_mutation, with_emitter_index_hash_collision,
    with_expanded_member_scan_mutation, with_instantiation_owner_lookup_mutation,
    with_material_function_owner_mutation, with_terminal_availability_lookup_mutation,
};
use veryl_analyzer::{Analyzer, Context, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::Parser;

struct Measurement {
    work: usize,
    resolved_member_lookups: usize,
    text: String,
    source_map: Vec<u8>,
    query_work: usize,
}

fn emit_measured(code: &str) -> Measurement {
    symbol_table::clear();
    attribute_table::clear();
    reset_emitter_index_work();
    let metadata = Metadata::create_default("scale").expect("test metadata");
    let source = PathBuf::from("emitter_index_scale.veryl");
    let parser = Parser::parse(code, &source).expect("scaling fixture parses");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let pass1 = analyzer.analyze_pass1("scale", &parser.veryl);
    assert!(pass1.is_empty(), "{pass1:?}");
    let post_pass1 = Analyzer::analyze_post_pass1();
    assert!(post_pass1.is_empty(), "{post_pass1:?}");
    let pass2 = analyzer.analyze_pass2(&parser.veryl, &mut context, None);
    assert!(pass2.is_empty(), "{pass2:?}");
    let analysis = context
        .finish_nested_modport_analysis()
        .expect("scaling fixture finalizes");
    let mut emitter = Emitter::new(
        &metadata,
        &source,
        &PathBuf::from("emitter_index_scale.sv"),
        &PathBuf::from("emitter_index_scale.sv.map"),
    );
    reset_emission_query_work();
    emitter
        .emit(&parser.veryl, code, analysis.as_ref())
        .expect("scaling fixture emits");
    Measurement {
        work: emitter_index_work(),
        query_work: emission_query_work(),
        resolved_member_lookups: resolved_member_lookups(),
        text: emitter.as_str().to_owned(),
        source_map: emitter
            .source_map()
            .to_bytes()
            .expect("scaling fixture source map"),
    }
}

fn material_parameters(scale: usize) -> String {
    (0..scale)
        .map(|index| format!("P{index}: u32 = {}", index + 8))
        .collect::<Vec<_>>()
        .join(", ")
}

fn terminal_handle_fixture(scale: usize) -> String {
    let mut code = format!(
        "interface ChildIf::<W: u32 = 8> {{\n    var payload: logic<W>;\n    modport sink {{ payload: input, }}\n}}\n\ninterface ParentIf::<{}> {{\n    inst child: ChildIf::<P0>;\n    modport sink {{ child.sink: modport, }}\n",
        material_parameters(scale)
    );
    for index in 0..scale {
        code.push_str(&format!(
            "    let _observed{index}: logic = child.payload[{index} % P0];\n"
        ));
    }
    code.push_str(
        "}\n\nmodule Top {\n    #[allow(unassign_variable)]\n    inst parent: ParentIf;\n}\n",
    );
    code
}

fn function_handle_fixture(scale: usize) -> String {
    let mut code = format!(
        "interface FunctionOwner::<{}> {{\n",
        material_parameters(scale)
    );
    for index in 0..scale {
        code.push_str(&format!(
            "    function observe{index}() -> logic {{ return P0 == P0; }}\n"
        ));
    }
    code.push_str("}\n\nmodule Top {\n    inst owner: FunctionOwner;\n}\n");
    code
}

fn instantiation_handle_fixture(scale: usize) -> String {
    let mut code = format!(
        "interface link_if::<W: u32 = 8> {{\n    var payload: logic<W>;\n}}\n\nmodule Leaf::<W: u32 = 8> (link: interface) {{}}\n\nmodule InstOwner::<{}> {{\n    #[allow(unassign_variable)]\n    inst link: link_if::<P0>;\n",
        material_parameters(scale)
    );
    for index in 0..scale {
        code.push_str(&format!("    inst leaf{index}: Leaf::<P0> (link: link);\n"));
    }
    code.push_str("}\n\nmodule Top {\n    inst owner: InstOwner;\n}\n");
    code
}

fn assert_query_affine(normal: &[Measurement; 3], mutation: &[Measurement; 3]) {
    for (normal, mutation) in normal.iter().zip(mutation) {
        assert_eq!(normal.text, mutation.text);
        assert_eq!(normal.source_map, mutation.source_map);
    }
    let normal = normal.each_ref().map(|measurement| measurement.query_work);
    let mutation = mutation
        .each_ref()
        .map(|measurement| measurement.query_work);
    println!("query compact={normal:?} mutation={mutation:?}");
    assert!(normal[0] > 0, "{normal:?}");
    assert!(normal[1] <= 3 * normal[0], "{normal:?}");
    assert!(normal[2] <= 3 * normal[1], "{normal:?}");
    assert!(mutation[1] > 3 * mutation[0], "{mutation:?}");
    assert!(mutation[2] > 3 * mutation[1], "{mutation:?}");
}

#[test]
fn terminal_handles_are_affine_through_real_parse_analysis_and_emission() {
    let normal = [4, 8, 16].map(|scale| emit_measured(&terminal_handle_fixture(scale)));
    let mutation = [4, 8, 16].map(|scale| {
        with_terminal_availability_lookup_mutation(|| {
            emit_measured(&terminal_handle_fixture(scale))
        })
    });
    assert_query_affine(&normal, &mutation);
}

#[test]
fn function_owner_handles_are_affine_through_real_parse_analysis_and_emission() {
    let normal = [4, 8, 16].map(|scale| emit_measured(&function_handle_fixture(scale)));
    let mutation = [4, 8, 16].map(|scale| {
        with_material_function_owner_mutation(|| emit_measured(&function_handle_fixture(scale)))
    });
    assert_query_affine(&normal, &mutation);
}

#[test]
fn instantiation_handles_are_affine_through_real_parse_analysis_and_emission() {
    let normal = [4, 8, 16].map(|scale| emit_measured(&instantiation_handle_fixture(scale)));
    let mutation = [4, 8, 16].map(|scale| {
        with_instantiation_owner_lookup_mutation(|| {
            emit_measured(&instantiation_handle_fixture(scale))
        })
    });
    assert_query_affine(&normal, &mutation);
}
