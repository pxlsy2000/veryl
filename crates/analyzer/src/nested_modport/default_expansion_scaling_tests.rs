use super::super::default_expansion_work::{
    default_expansion_work, inject_member_rescan_mutation, reset_default_expansion_work,
};
use crate::conv::utils::get_component;
use crate::ir::{Component, Ir, Signature};
use crate::symbol::{Direction, SymbolKind};
use crate::{Analyzer, Context, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::resource_table::insert_str;
use veryl_parser::{Parser, doc_comment_table};

type MemberSnapshot = Vec<(String, Direction)>;

#[derive(Debug, Eq, PartialEq)]
struct ExpansionSnapshot {
    same: MemberSnapshot,
    converse: MemberSnapshot,
    all_derived: Vec<MemberSnapshot>,
}

fn scaling_source(scale: usize, nested: bool) -> String {
    let mut source = if nested {
        "interface ChildIf {\n\
             var z: logic;\n\
             var a: logic;\n\
             var b: logic;\n\
             modport first  { z: input,  a: output, }\n\
             modport second { z: output, b: inout,  }\n\
         }\n\
         interface ScaleIf {\n\
             inst child: ChildIf;\n\
             var z: logic;\n\
             function observe() -> logic { return child.z; }\n"
            .to_string()
    } else {
        "interface ScaleIf {\n\
             var z: logic;\n\
             var a: logic;\n\
             var b: logic;\n\
             function observe() -> logic { return z; }\n"
            .to_string()
    };
    for index in 0..scale {
        source.push_str(&format!("    var noise_{index}: logic;\n"));
        source.push_str(&format!(
            "    function noise_fn_{index}() -> logic {{ return 0; }}\n"
        ));
    }
    if nested {
        source.push_str(
            "    modport first  { z: input,  child.first: modport,  observe: import, }\n\
             modport second { z: output, child.second: modport, observe: import, }\n",
        );
    } else {
        source.push_str(
            "    modport first  { z: input,  a: output, observe: import, }\n\
             modport second { z: output, b: inout,  observe: import, }\n",
        );
    }
    let explicit = "z";
    for index in 0..scale {
        let default = if index % 2 == 0 { "same" } else { "converse" };
        source.push_str(&format!(
            "    modport derived_{index} {{ {explicit}: inout, ..{default}(first, second) }}\n"
        ));
    }
    source.push_str("}\n");
    source
}

fn snapshot(context: &mut Context, scale: usize) -> ExpansionSnapshot {
    let symbol = symbol_table::get_all()
        .into_iter()
        .find(|symbol| {
            symbol.token.text == insert_str("ScaleIf")
                && matches!(symbol.kind, SymbolKind::Interface(_))
        })
        .expect("ScaleIf symbol");
    let component = get_component(context, &Signature::new(symbol.id), symbol.token.into())
        .expect("ScaleIf production IR");
    let Component::Interface(interface) = component.as_ref() else {
        panic!("ScaleIf must lower to interface IR");
    };
    let member_snapshot = |name: &str| {
        interface
            .get_modport(&insert_str(name))
            .expect("generated modport exists")
            .iter_paths_directions()
            .map(|(path, direction)| (path.to_string(), *direction))
            .collect::<Vec<_>>()
    };
    ExpansionSnapshot {
        same: member_snapshot("derived_0"),
        converse: member_snapshot("derived_1"),
        all_derived: (0..scale)
            .map(|index| member_snapshot(&format!("derived_{index}")))
            .collect(),
    }
}

fn measure(scale: usize, nested: bool, mutation: bool) -> (usize, Vec<String>, ExpansionSnapshot) {
    symbol_table::clear();
    attribute_table::clear();
    doc_comment_table::clear();
    let project = if nested { "nested_scale" } else { "direct_scale" };
    let metadata = Metadata::create_default(project).expect("scaling metadata");
    let parser = Parser::parse(
        &scaling_source(scale, nested),
        &format!("{project}.veryl"),
    )
    .expect("generated default source parses");
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    let mut ir = Ir::default();
    let mut errors = analyzer.analyze_pass1(project, &parser.veryl);
    errors.append(&mut Analyzer::analyze_post_pass1());
    reset_default_expansion_work();
    let guard = mutation.then(inject_member_rescan_mutation);
    errors.append(&mut analyzer.analyze_pass2(&parser.veryl, &mut context, Some(&mut ir)));
    let semantic_snapshot = snapshot(&mut context, scale);
    if let Err(error) = context.finish_nested_modport_analysis() {
        errors.push(error);
    }
    drop(guard);
    let diagnostics = errors.iter().map(ToString::to_string).collect();
    (default_expansion_work(), diagnostics, semantic_snapshot)
}

fn assert_expected(snapshot: &ExpansionSnapshot, nested: bool) {
    let same = if nested {
        vec![
            ("z".to_string(), Direction::Inout),
            ("child.z".to_string(), Direction::Output),
            ("child.a".to_string(), Direction::Output),
            ("child.b".to_string(), Direction::Inout),
            ("observe".to_string(), Direction::Import),
        ]
    } else {
        vec![
            ("z".to_string(), Direction::Inout),
            ("a".to_string(), Direction::Output),
            ("b".to_string(), Direction::Inout),
            ("observe".to_string(), Direction::Import),
        ]
    };
    let converse = if nested {
        vec![
            ("z".to_string(), Direction::Inout),
            ("child.z".to_string(), Direction::Input),
            ("child.a".to_string(), Direction::Input),
            ("child.b".to_string(), Direction::Inout),
        ]
    } else {
        vec![
            ("z".to_string(), Direction::Inout),
            ("a".to_string(), Direction::Input),
            ("b".to_string(), Direction::Inout),
        ]
    };
    assert_eq!(snapshot.same, same);
    assert_eq!(snapshot.converse, converse);
}

fn assert_joint_axis(nested: bool) {
    let indexed = [8, 16, 32].map(|scale| measure(scale, nested, false));
    let rescanned = [8, 16, 32].map(|scale| measure(scale, nested, true));
    assert!(indexed.iter().all(|(_, errors, _)| errors.is_empty()), "{indexed:?}");
    for index in 0..indexed.len() {
        assert_eq!(indexed[index].1, rescanned[index].1);
        assert_eq!(indexed[index].2, rescanned[index].2);
    }
    assert_expected(&indexed[0].2, nested);
    let indexed_work = indexed.map(|(work, _, _)| work);
    let rescanned_work = rescanned.map(|(work, _, _)| work);
    println!("default nested={nested} indexed={indexed_work:?} mutation={rescanned_work:?}");
    assert!(indexed_work[1] <= indexed_work[0] * 3, "{indexed_work:?}");
    assert!(indexed_work[2] <= indexed_work[1] * 3, "{indexed_work:?}");
    assert!(rescanned_work[1] > rescanned_work[0] * 3, "{rescanned_work:?}");
    assert!(rescanned_work[2] > rescanned_work[1] * 3, "{rescanned_work:?}");
}

#[test]
fn direct_defaults_preserve_exact_semantics_across_joint_axes() {
    assert_joint_axis(false);
}

#[test]
fn nested_defaults_preserve_exact_semantics_across_joint_axes() {
    assert_joint_axis(true);
}
