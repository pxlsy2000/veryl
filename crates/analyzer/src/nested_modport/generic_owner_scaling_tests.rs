use super::super::collection_work::{collection_work, reset_collection_work};
use super::super::generic_owner_work::inject_owner_membership_scan_mutation;
use super::*;
use crate::ir::Ir;
use crate::symbol::{GenericMap, SymbolKind};
use crate::{Analyzer, Context, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::resource_table::{PathId, TokenId, insert_str};
use veryl_parser::{Parser, doc_comment_table};

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingSnapshot {
    id: EmissionBindingId,
    declaration: TokenId,
    kind: EmissionOwnerKind,
    owner_handle: FrozenOwnerHandle,
    enclosing_owner_handle: Option<FrozenOwnerHandle>,
    specialization: NestedModportLoweringKey,
    generic_map: SemanticGenericMap,
    enclosing_generic_map: Option<SemanticGenericMap>,
    package_scope: Option<EmissionScopeIdentity>,
    lowering: LoweringAvailability,
    owner_arguments: Vec<(String, String)>,
    emission_arguments: Vec<(String, String)>,
    required_rewrites: Vec<OccurrenceRewriteKey>,
    required_expanded_ports: Vec<ExpandedPortKey>,
}

fn generic_source(scale: usize) -> String {
    let parameters = (0..scale)
        .map(|index| format!("P{index}: u32"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = format!(
        "function same_name::<{parameters}>() -> u32 {{ return P0; }}\nmodule Uses {{\n"
    );
    for map in 0..scale {
        let arguments = (0..scale)
            .map(|index| match (map, index) {
                (0, 0) => "8".to_string(),
                (1, 0) => "16".to_string(),
                _ => (map * scale + index + 32).to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        source.push_str(&format!(
            "    #[allow(unused_variable)] let value_{map}: u32 = same_name::<{arguments}>();\n"
        ));
    }
    source.push_str("}\n");
    source
}

fn analyze(source_text: &str, project: &str) -> (PathId, Vec<crate::symbol::Symbol>) {
    symbol_table::clear();
    attribute_table::clear();
    doc_comment_table::clear();
    let path = format!("{project}.veryl");
    let metadata = Metadata::create_default(project).expect("generic-owner metadata");
    let parser = Parser::parse(source_text, &path).expect("generic source parses");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();
    let mut errors = analyzer.analyze_pass1(project, &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    errors.append(&mut Analyzer::analyze_post_pass2(&ir));
    assert!(errors.is_empty(), "generic-owner fixture errors: {errors:?}");
    let symbols = symbol_table::get_all();
    let source = symbols
        .iter()
        .find_map(|symbol| symbol.token.source.get_path())
        .expect("fixture source path");
    (source, symbols)
}

fn argument_snapshot(map: &GenericMap) -> Vec<(String, String)> {
    let mut values: Vec<_> = map
        .map
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    values.sort();
    values
}

fn binding_snapshots(analysis: &NestedModportAnalysis, source: PathId) -> Vec<BindingSnapshot> {
    analysis
        .emission_index
        .source(source)
        .map(|table| {
            table
                .bindings
                .iter()
                .map(|binding| BindingSnapshot {
                    id: binding.id,
                    declaration: binding.declaration,
                    kind: binding.kind,
                    owner_handle: binding.owner_handle,
                    enclosing_owner_handle: binding.enclosing_owner_handle,
                    specialization: binding.specialization.as_ref().clone(),
                    generic_map: SemanticGenericMap::from(&binding.emission_context.generic_map),
                    enclosing_generic_map: binding
                        .emission_context
                        .enclosing_generic_map
                        .as_ref()
                        .map(SemanticGenericMap::from),
                    package_scope: binding.package_scope.clone(),
                    lowering: binding.lowering.clone(),
                    owner_arguments: binding
                        .specialization
                        .specialization
                        .owner
                        .generic_parameters
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.to_string()))
                        .collect(),
                    emission_arguments: argument_snapshot(&binding.emission_context.generic_map),
                    required_rewrites: binding.required_rewrites.to_vec(),
                    required_expanded_ports: binding.required_expanded_ports.to_vec(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn finalize(
    source: PathId,
    owners: &[crate::symbol::Symbol],
    session: AnalysisSessionId,
    mutation: bool,
) -> (usize, Vec<BindingSnapshot>) {
    let mut pending = PendingNestedModportAnalysis::default();
    for owner in owners {
        pending.record_generic_emission_owner(PendingGenericEmissionOwner {
            session,
            source,
            declaration: owner.token.id,
            kind: EmissionOwnerKind::Function,
            symbol: owner.id,
        });
    }
    reset_collection_work();
    let guard = mutation.then(inject_owner_membership_scan_mutation);
    let analysis = pending.finalize(session).expect("generic owner finalizes");
    drop(guard);
    (collection_work(), binding_snapshots(&analysis, source))
}

fn measure_pair(
    scale: usize,
    session: AnalysisSessionId,
) -> ((usize, Vec<BindingSnapshot>), (usize, Vec<BindingSnapshot>)) {
    let (source, symbols) = analyze(&generic_source(scale), "generic_owner_scale");
    let function = symbols
        .into_iter()
        .find(|symbol| {
            symbol.token.text == insert_str("same_name")
                && matches!(symbol.kind, SymbolKind::Function(_))
        })
        .expect("same_name function symbol");
    assert_eq!(function.generic_parameters().len(), scale);
    assert_eq!(function.generic_maps().len(), scale);
    let indexed = finalize(source, std::slice::from_ref(&function), session, false);
    let scanned = finalize(source, &[function], session, true);
    (indexed, scanned)
}

#[test]
fn generic_owner_index_preserves_exact_bindings_at_quadratic_output_scale() {
    let session = AnalysisSessionId::new();
    let measurements = [4, 8, 16].map(|scale| measure_pair(scale, session));
    let indexed = measurements.each_ref().map(|(indexed, _)| indexed);
    let scanned = measurements.each_ref().map(|(_, scanned)| scanned);
    for index in 0..indexed.len() {
        assert_eq!(indexed[index].1, scanned[index].1);
        assert_eq!(indexed[index].1.len(), [4, 8, 16][index]);
    }
    assert_eq!(indexed[0].1[0].owner_arguments[0].1, "8");
    assert_eq!(indexed[0].1[1].owner_arguments[0].1, "16");
    assert_eq!(indexed[0].1[0].emission_arguments[0].1, "8");
    assert_eq!(indexed[0].1[1].emission_arguments[0].1, "16");
    let indexed_work = indexed.map(|(work, _)| *work);
    let scanned_work = scanned.map(|(work, _)| *work);
    assert!(indexed_work[1] <= indexed_work[0] * 5, "{indexed_work:?}");
    assert!(indexed_work[2] <= indexed_work[1] * 5, "{indexed_work:?}");
    let scan_overhead = std::array::from_fn::<_, 3, _>(|index| {
        scanned_work[index].saturating_sub(indexed_work[index])
    });
    println!(
        "generic-owner indexed={indexed_work:?} mutation={scanned_work:?} overhead={scan_overhead:?}"
    );
    assert!(scan_overhead[1] > scan_overhead[0] * 3, "{scan_overhead:?}");
    assert!(scan_overhead[2] > scan_overhead[1] * 3, "{scan_overhead:?}");
}

const PACKAGE_SOURCE: &str = r#"
package First::<W: u32> {
    function same_name() -> u32 { return W; }
}
package Second::<W: u32> {
    function same_name() -> u32 { return W; }
}
module Uses {
    #[allow(unused_variable)]
    let first8 : u32 = First::<8>::same_name();
    #[allow(unused_variable)]
    let first16: u32 = First::<16>::same_name();
    #[allow(unused_variable)]
    let second8 : u32 = Second::<8>::same_name();
    #[allow(unused_variable)]
    let second16: u32 = Second::<16>::same_name();
}
"#;

#[test]
fn package_owned_same_name_functions_preserve_generic8_and_generic16_bindings() {
    let session = AnalysisSessionId::new();
    let (source, symbols) = analyze(PACKAGE_SOURCE, "generic_package_owner");
    let owners: Vec<_> = symbols
        .into_iter()
        .filter(|symbol| {
            symbol.token.text == insert_str("same_name")
                && matches!(symbol.kind, SymbolKind::Function(_))
        })
        .collect();
    assert_eq!(owners.len(), 2);
    let indexed = finalize(source, &owners, session, false);
    let scanned = finalize(source, &owners, session, true);
    assert_eq!(indexed.1, scanned.1);
    assert_eq!(indexed.1.len(), 4);
    assert!(indexed.1.iter().all(|binding| binding.package_scope.is_some()));
    let arguments: Vec<_> = indexed
        .1
        .iter()
        .map(|binding| binding.owner_arguments[0].1.as_str())
        .collect();
    assert_eq!(arguments, ["8", "16", "8", "16"]);
}
