use super::super::collection_work::{collection_work, reset_collection_work};
use super::super::direct_terminal_resolution::collect_emitted_owner_paths;
use crate::symbol::SymbolKind;
use crate::{Analyzer, HashMap, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::Parser;
use veryl_parser::resource_table::insert_str;

fn measure(direct: usize, unrelated: usize) -> usize {
    symbol_table::clear();
    attribute_table::clear();
    let mut source = String::from("interface Target {\n");
    for index in 0..direct {
        source.push_str(&format!("var target_{index}: logic;\n"));
    }
    source.push_str("}\ninterface Unrelated {\n");
    for index in 0..unrelated {
        source.push_str(&format!("var unrelated_{index}: logic;\n"));
    }
    source.push_str("}\n");
    let parser = Parser::parse(&source, &"owner_index.veryl").expect("fixture parses");
    let analyzer = Analyzer::new(&Metadata::create_default("scale").expect("metadata"));
    assert!(analyzer.analyze_pass1("scale", &parser.veryl).is_empty());
    assert!(Analyzer::analyze_post_pass1().is_empty());
    let owner = symbol_table::get_all()
        .into_iter()
        .find(|symbol| {
            symbol.token.text == insert_str("Target")
                && matches!(symbol.kind, SymbolKind::Interface(_))
        })
        .expect("Target interface");
    reset_collection_work();
    let paths = collect_emitted_owner_paths(&HashMap::default(), &HashMap::default(), Some(owner.id));
    let work = collection_work();
    assert_eq!(paths.len(), direct);
    work
}

#[test]
fn owner_emission_index_ignores_unrelated_symbols_and_scales_with_direct_symbols() {
    let unrelated = [1, 64, 256].map(|count| measure(2, count));
    assert_eq!(unrelated, [2, 2, 2]);
    let direct = [1, 2, 4].map(|count| measure(count, 256));
    assert_eq!(direct, [1, 2, 4]);
}
