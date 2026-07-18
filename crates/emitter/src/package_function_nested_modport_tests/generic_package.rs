#[test]
fn generic_package_function_keeps_8_and_16_specializations_distinct() {
    let code = r#"interface ChildIf::<W: u32> {
    var payload: logic<W>;
    modport sink { payload: input, }
}
interface ParentIf::<W: u32> {
    inst child: ChildIf::<W>;
    modport sink { child.sink: modport, }
}
package Pkg {
    function consume::<W: u32> (
        p: modport ParentIf::<W>::sink,
    ) -> logic<W> {
        return p.child.payload;
    }
}
module Top {
    inst parent8 : ParentIf::<8>;
    inst parent16: ParentIf::<16>;
    assign parent8.child.payload = '0;
    assign parent16.child.payload = '0;
    let observed8 : logic<8>  = Pkg::consume::<8>(parent8);
    let observed16: logic<16> = Pkg::consume::<16>(p: parent16);
}
"#;
    let emitted = emit(code);

    assert!(
        emitted.contains("logic[8-1:0] __p_child__payload"),
        "{emitted}"
    );
    assert!(
        emitted.contains("logic[16-1:0] __p_child__payload"),
        "{emitted}"
    );
    assert!(emitted.contains("prj_Pkg::__consume__8("), "{emitted}");
    assert!(emitted.contains("parent8.child__payload"), "{emitted}");
    assert!(emitted.contains("prj_Pkg::__consume__16("), "{emitted}");
    assert!(emitted.contains("parent16.child__payload"), "{emitted}");
    assert!(!emitted.contains("p.child.payload"), "{emitted}");
}

#[test]
fn package_function_nested_modport_uses_analyzer_owned_expansion() {
    let code = r#"interface ChildIf::<W: u32> {
    var payload: logic<W>;
    modport sink { payload: input, }
}
interface ParentIf::<W: u32> {
    inst child: ChildIf::<W>;
    modport sink { child.sink: modport, }
}
package Pkg {
    function consume (
        p: modport ParentIf::<8>::sink,
    ) -> logic<8> {
        return p.child.payload;
    }
}
module Top {
    inst parent: ParentIf::<8>;
    assign parent.child.payload = '0;
    let observed: logic<8> = Pkg::consume(parent);
}
"#;
    let emitted = emit(code);

    assert!(
        emitted.contains("logic[8-1:0] __p_child__payload"),
        "{emitted}"
    );
    assert!(
        emitted.contains("prj_Pkg::consume(parent.child__payload)"),
        "{emitted}"
    );
    assert!(emitted.contains("return __p_child__payload;"), "{emitted}");
    assert!(!emitted.contains("p.child.payload"), "{emitted}");
}
