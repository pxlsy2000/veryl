use crate::ir::Ir;
use crate::{Analyzer, Context, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::{Parser, doc_comment_table};

fn occurrence_source(variable_count: usize, reference_count: usize) -> String {
    let mut code = String::from(
        "interface ChildIf {\n\
             var forwarded: logic;\n\
             function get_forwarded() -> logic { return forwarded; }\n\
             modport sink { forwarded: input, }\n\
         }\n",
    );
    code.push_str("function Observe::<");
    for index in 0..variable_count {
        code.push_str(&format!("I{index}: inst ChildIf, "));
    }
    code.push_str("Target: inst ChildIf>() -> logic { return Target::get_forwarded(); }\n");
    for parent in 0..reference_count {
        code.push_str(&format!("interface ParentIf{parent} {{\n"));
        for index in 0..variable_count {
            code.push_str(&format!(
                "    inst noise_{parent}_{index}: ChildIf;\n"
            ));
        }
        code.push_str(&format!("    inst cpu_{parent}: ChildIf;\n"));
        code.push_str(&format!(
            "    #[allow(unused_variable)] let seen_{parent}: logic = Observe::<"
        ));
        for index in 0..variable_count {
            code.push_str(&format!("noise_{parent}_{index}, "));
        }
        code.push_str(&format!(
            "cpu_{parent}>();\n\
             modport sink {{ cpu_{parent}.sink: modport, }}\n\
             }}\n"
        ));
    }
    code
}

fn occurrence_lookup_work(variable_count: usize, reference_count: usize) -> (usize, usize) {
    symbol_table::clear();
    attribute_table::clear();
    doc_comment_table::clear();
    let metadata = Metadata::create_default("occurrence_scale").expect("test metadata");
    let parser = Parser::parse(
        &occurrence_source(variable_count, reference_count),
        &"occurrence_scale.veryl",
    )
    .expect("scaling source should parse");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();
    let mut errors = analyzer.analyze_pass1("occurrence_scale", &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    Context::reset_nested_signature_lookup_work();
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    if let Err(error) = context.finish_nested_modport_analysis() {
        errors.push(error);
    }
    assert!(
        errors
            .iter()
            .all(|error| matches!(error, crate::AnalyzerError::InvalidNestedModport { .. })),
        "production pass2 fixture had unrelated errors: {errors:?}"
    );
    Context::nested_signature_lookup_work()
}

#[test]
fn occurrence_discovery_uses_bounded_index_probes_per_reference() {
    let by_variables = [16, 64, 256].map(|variables| {
        let baseline = occurrence_lookup_work(variables, 0);
        let work = occurrence_lookup_work(variables, 4);
        (work.0 - baseline.0, work.1 - baseline.1)
    });
    assert_eq!(by_variables.map(|work| work.0), [12, 12, 12], "{by_variables:?}");
    assert_eq!(by_variables.map(|work| work.1), [0, 0, 0], "{by_variables:?}");

    let baseline = occurrence_lookup_work(128, 0);
    let by_references = [1, 2, 4].map(|references| {
        let work = occurrence_lookup_work(128, references);
        (work.0 - baseline.0, work.1 - baseline.1)
    });
    assert_eq!(by_references.map(|work| work.0), [3, 6, 12], "{by_references:?}");
    assert_eq!(by_references.map(|work| work.1), [0, 0, 0], "{by_references:?}");
}

#[test]
fn occurrence_counter_detects_an_injected_full_variable_scan() {
    Context::force_nested_signature_linear_scan(true);
    let scanned = [16, 64, 256].map(|variables| occurrence_lookup_work(variables, 2).1);
    Context::force_nested_signature_linear_scan(false);
    assert!(scanned[0] > 0, "{scanned:?}");
    assert!(scanned[1] > scanned[0], "{scanned:?}");
    assert!(scanned[2] > scanned[1], "{scanned:?}");
}
