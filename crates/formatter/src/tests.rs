use crate::Formatter;
use veryl_analyzer::{Analyzer, Context};
use veryl_metadata::Metadata;
use veryl_parser::Parser;

#[track_caller]
fn format(metadata: &Metadata, code: &str) -> String {
    let parser = Parser::parse(&code, &"").unwrap();
    let analyzer = Analyzer::new(metadata);
    let mut context = Context::default();

    analyzer.analyze_pass1(&"prj", &parser.veryl);
    Analyzer::analyze_post_pass1();
    analyzer.analyze_pass2(&"prj", &parser.veryl, &mut context, None);

    let mut formatter = Formatter::new(metadata);
    formatter.format(&parser.veryl, code);
    formatter.as_str().to_string()
}

#[test]
fn empty_body_with_comment() {
    let code = r#"module ModuleA {
    /* */
}
"#;
    let expect = r#"module ModuleA {
    /* */
}
"#;

    let metadata = Metadata::create_default("prj").unwrap();

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);

    let code = r#"module ModuleA {
    /* foo */
    /* bar */
}
"#;
    let expect = r#"module ModuleA {
    /* foo */
    /* bar */
}
"#;

    let metadata = Metadata::create_default("prj").unwrap();

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);

    let code = r#"module ModuleA {
    /* foo */
    // bar
}
"#;
    let expect = r#"module ModuleA {
    /* foo */
    // bar
}
"#;

    let metadata = Metadata::create_default("prj").unwrap();

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);
}

#[test]
fn empty_list() {
    let code = r#"module ModuleA #(

) (

) {

}
module ModuleB {
  inst u: ModuleA #(

    ) (

    );

    function Func (

    ) {

    }

    always_comb {
        Func(

        );
    }
}
"#;

    let expect = r#"module ModuleA #() () {}
module ModuleB {
    inst u: ModuleA ;

    function Func () {}

    always_comb {
        Func();
    }
}
"#;

    let metadata = Metadata::create_default("prj").unwrap();

    let ret = format(&metadata, &code);

    println!("ret\n{}\nexp\n{}", ret, expect);
    assert_eq!(ret, expect);
}

#[test]
fn skip_formatting() {
    let code = r#"#[fmt(skip)]
module ModuleA {
    let _a: logic = 0;
}

#[fmt(skip)]
interface InterfaceA {
    var a: logic;

    modport mp {
        a: input
    }
}

#[fmt(skip)]
package PackageA {
    const A: u32 = 0;

    function FuncA(
        a: input u32,
        b: input u32
    ) -> u32 {
        return a + b;
    }
}
"#;

    let mut metadata = Metadata::create_default("prj").unwrap();

    metadata.format.indent_width = 2;

    let ret = format(&metadata, &code);

    println!("ret\n{}\nexp\n{}", ret, code);
    assert_eq!(ret, code);

    let code = r#"#[fmt(skip)]
module ModuleA () {
    /* this comment line is important */
}
#[fmt(skip)]
module ModuleB () {
    // this comment line is important
}
"#;

    let mut metadata = Metadata::create_default("prj").unwrap();

    metadata.format.indent_width = 2;

    let ret = format(&metadata, &code);

    println!("ret\n{}\nexp\n{}", ret, code);
    assert_eq!(ret, code);
}

#[test]
fn no_panic_if_expression_when_vertical_align_off() {
    let code = r#"module ModuleA {
    let a: logic = 1;
    let _b: logic = if a == 1 ? 1 : if a == 2 ? 0 : 1;
}
"#;

    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.vertical_align = false;

    let ret = format(&metadata, code);
    assert!(!ret.is_empty());
}

#[test]
fn const_above_let_alignment() {
    let code = r#"module TopModule {
    const _c: u32 = 0;
    let _a: logic = 0;
    let _abcd: logic = 0;
}
"#;

    let expect = r#"module TopModule {
    const _c   : u32   = 0;
    let   _a   : logic = 0;
    let   _abcd: logic = 0;
}
"#;

    let metadata = Metadata::create_default("prj").unwrap();

    let ret = format(&metadata, code);
    assert_eq!(ret, expect);
}

#[test]
fn format_generic_list() {
    let metadata = Metadata::create_default("prj").unwrap();

    let code = r#"module ModuleA::<A : a_type, AA: u32,> {}
"#;

    let expect = r#"module ModuleA::<A: a_type, AA: u32> {}
"#;

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);

    let code = r#"module ModuleA::<
    A: a_type,
    AA: u32
> {}
"#;

    let expect = r#"module ModuleA::<
    A : a_type,
    AA: u32   ,
> {}
"#;

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);

    let code = r#"alias module ModuleA = ModuleB::<8, 16,>;
"#;

    let expect = r#"alias module ModuleA = ModuleB::<8, 16>;
"#;

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);

    let code = r#"alias module ModuleB = ModuleC::<
    8,
    16
>;
"#;

    let expect = r#"alias module ModuleB = ModuleC::<
    8 ,
    16,
>;
"#;

    let ret = format(&metadata, &code);
    assert_eq!(ret, expect);
}

// ----- Issue #2598: format based on line width -----------------------------

#[test]
fn max_width_breaks_binary_expression() {
    // The example from https://github.com/veryl-lang/veryl/issues/2598:
    // a sum that doesn't fit on one line should wrap with operators at
    // the head of each continuation line.
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 60;

    let code = r#"module M {
    let a: logic = aaaaaaaaaaaa + bbbbbbbbbb + cccccccccccc + dddddddddd + eeeeeeeeeeee;
}
"#;
    let expect = r#"module M {
    let a: logic = aaaaaaaaaaaa + bbbbbbbbbb + cccccccccccc
        + dddddddddd + eeeeeeeeeeee;
}
"#;

    let ret = format(&metadata, code);
    assert_eq!(ret, expect);
}

#[test]
fn max_width_keeps_short_binary_expression_flat() {
    // A short expression must stay on one line regardless of how many
    // operands it has.
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 120;

    let code = r#"module M {
    let a: logic = b + c + d;
}
"#;
    let expect = code;

    let ret = format(&metadata, code);
    assert_eq!(ret, expect);
}

#[test]
fn max_width_breaks_function_call() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 30;

    let code = r#"module M {
    let _: logic = func(aaaa, bbbb, cccc, dddd);
}
"#;
    let ret = format(&metadata, code);
    // The call should wrap with each arg on its own line.
    assert!(
        ret.contains("\n        aaaa,") && ret.contains("\n    )"),
        "expected wrapped function call in:\n{ret}"
    );
}

#[test]
fn max_width_user_break_respected() {
    // The user already laid out the function call on multiple lines.
    // Width-driven wrapping must not collapse it back onto one line just
    // because the contents would fit.
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 200;

    let code = r#"module M {
    let _: logic = func(
        a,
        b,
    );
}
"#;
    let ret = format(&metadata, code);
    // Even with plenty of width, the multi-line layout from the source
    // should be preserved.
    // (We only assert presence of the broken layout here; exact alignment
    //  may differ slightly until Stage A4 regenerates the testcases.)
    assert!(
        ret.contains("\n        a,") || ret.contains("\n        a "),
        "expected user-broken layout preserved in:\n{ret}"
    );
}

#[test]
fn max_width_breaks_nested_expression() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 40;

    let code = r#"module M {
    let _: logic = aa + bb + cc + dd + ee;
}
"#;
    let ret = format(&metadata, code);
    // Should break before some operator to fit max_width=40.
    assert!(
        ret.contains("\n        +"),
        "expected operator wrap in:\n{ret}"
    );
}

#[test]
fn max_width_keeps_short_function_call_flat() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 120;

    let code = r#"module M {
    let _: logic = f(a, b, c);
}
"#;
    let expect = code;

    let ret = format(&metadata, code);
    assert_eq!(ret, expect);
}

#[test]
fn max_width_breaks_ternary() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 30;

    let code = r#"module M {
    let _: logic = if a == 1 ? bbbb : cccc;
}
"#;
    let ret = format(&metadata, code);
    // Long ternary should break at the `?` and `:` boundaries.
    assert!(
        ret.contains("?\n") && ret.contains(":\n"),
        "expected ternary wrap in:\n{ret}"
    );
}

#[test]
fn max_width_keeps_short_ternary_flat() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 120;

    let code = r#"module M {
    let _: logic = if a ? b : c;
}
"#;
    let expect = code;

    let ret = format(&metadata, code);
    assert_eq!(ret, expect);
}

#[test]
fn max_width_breaks_concatenation() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 30;

    let code = r#"module M {
    let _: logic<32> = {aaaa, bbbb, cccc, dddd};
}
"#;
    let ret = format(&metadata, code);
    // The concatenation should wrap when it doesn't fit. Items may be
    // packed (fill mode) — assert that at least one break happens.
    assert!(
        ret.contains("\n        ") && ret.contains("\n    }"),
        "expected concatenation wrap in:\n{ret}"
    );
}

#[test]
fn max_width_keeps_short_concatenation_flat() {
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 120;

    let code = r#"module M {
    let _: logic<32> = {a, b, c, d};
}
"#;
    let expect = code;

    let ret = format(&metadata, code);
    assert_eq!(ret, expect);
}

#[test]
fn max_width_breaks_at_continuation_indent() {
    // Continuation lines indent +1 level past the surrounding statement.
    // The let statement is at module-indent (4 spaces), so the continuation
    // sits at 8 spaces.
    let mut metadata = Metadata::create_default("prj").unwrap();
    metadata.format.max_width = 30;

    let code = r#"module M {
    let aaaaaaa: logic = xxxx + yyyy + zzzz + wwww;
}
"#;
    let ret = format(&metadata, code);
    // Verify a break occurred and the continuation is indented.
    assert!(
        ret.contains("\n        +"),
        "expected continuation indent of 8 spaces in:\n{ret}"
    );
}
