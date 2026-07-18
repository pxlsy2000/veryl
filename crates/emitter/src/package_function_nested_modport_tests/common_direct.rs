use crate::Emitter;
use crate::sv_structure::{DeclarationExpectation, SvStructure};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use veryl_analyzer::{Analyzer, Context, attribute_table, symbol_table};
use veryl_metadata::Metadata;
use veryl_parser::Parser;
use veryl_sourcemap::{SourceMap, SourceMapError};

static NEXT_SOURCE_MAP_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct CleanTempDir {
    path: PathBuf,
}

impl CleanTempDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_SOURCE_MAP_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "veryl-emitter-source-map-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("dedicated source-map fixture directory must be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CleanTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn emit_artifacts(metadata: &Metadata, code: &str) -> (String, Vec<u8>) {
    symbol_table::clear();
    attribute_table::clear();
    let source = PathBuf::from("package_function.veryl");
    let parser = Parser::parse(code, &source).unwrap();
    let analyzer = Analyzer::new(metadata);
    let mut context = Context::default();
    analyzer.analyze_pass1("prj", &parser.veryl);
    Analyzer::analyze_post_pass1();
    let errors = analyzer.analyze_pass2(&parser.veryl, &mut context, None);
    assert!(errors.is_empty(), "analysis errors: {errors:?}");
    let analysis = context.finish_nested_modport_analysis().unwrap();
    let mut emitter = Emitter::new(
        metadata,
        &source,
        &PathBuf::from("package_function.sv"),
        &PathBuf::from("package_function.sv.map"),
    );
    emitter.emit(&parser.veryl, code, &analysis).unwrap();
    let text = emitter.as_str().to_owned();
    let map = emitter.source_map().to_bytes().unwrap();
    (text, map)
}

fn emit(code: &str) -> String {
    let metadata = Metadata::create_default("prj").unwrap();
    emit_artifacts(&metadata, code).0
}

fn generated_token_position(emitted: &str, line_marker: &str, token_text: &str) -> (u32, u32) {
    let (line, generated_line) = emitted
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(line_marker))
        .unwrap_or_else(|| panic!("missing generated line marker `{line_marker}`:\n{emitted}"));
    let column = generated_line
        .find(token_text)
        .unwrap_or_else(|| panic!("missing generated token `{token_text}` in `{generated_line}`"));
    (
        u32::try_from(line + 1).expect("generated line must fit u32"),
        u32::try_from(column + 1).expect("generated column must fit u32"),
    )
}

struct MappingFixture<'a> {
    map: &'a SourceMap,
    canonical_source: &'a Path,
    source: &'a str,
    emitted: &'a str,
}

struct MappingExpectation<'a> {
    line_marker: &'a str,
    emitted_token: &'a str,
    control_previous_lines: u32,
    source_position: (u32, u32),
    source_line: &'a str,
    source_token: &'a str,
}

impl MappingFixture<'_> {
    fn assert_exact(&self, expected: MappingExpectation<'_>) {
        let destination =
            generated_token_position(self.emitted, expected.line_marker, expected.emitted_token);
        let mapped = (
            self.canonical_source.to_path_buf(),
            expected.source_position.0,
            expected.source_position.1,
        );
        assert_eq!(
            self.map.lookup(destination.0, destination.1),
            Some(mapped.clone())
        );
        let control_line = destination
            .0
            .checked_sub(expected.control_previous_lines)
            .expect("control line must exist");
        assert_ne!(
            self.map.lookup(control_line, 1),
            Some(mapped),
            "non-token control for `{}` must not inherit its source tuple",
            expected.emitted_token
        );
        let source_line = self
            .source
            .lines()
            .nth(usize::try_from(expected.source_position.0 - 1).expect("line must fit usize"))
            .expect("mapped source line must exist");
        assert_eq!(source_line, expected.source_line);
        let source_column =
            usize::try_from(expected.source_position.1 - 1).expect("column must fit usize");
        assert!(
            source_line[source_column..].starts_with(expected.source_token),
            "source column must start at `{}` in `{source_line}`",
            expected.source_token
        );
    }
}

#[test]
fn direct_package_function_control_is_stable() {
    let code = r#"interface DirectIf {
    var payload: logic;
    modport sink { payload: input, }
}

package DirectPkg {
    function consume (
        p: modport DirectIf::sink,
    ) -> logic {
        return p.payload;
    }
}

module DirectTop {
    inst direct: DirectIf;
    assign direct.payload = 0;
    let observed: logic = DirectPkg::consume(direct);
}
"#;
    let metadata = Metadata::create_default("prj").unwrap();
    let (emitted, source_map) = emit_artifacts(&metadata, code);
    let expected = "interface prj_DirectIf;\n    logic payload;\n    modport sink (\n        input payload\n    );\nendinterface\n\npackage prj_DirectPkg;\n    function automatic logic consume(\n        input var logic __p_payload\n    ) ;\n        return __p_payload;\n    endfunction\nendpackage\n\nmodule prj_DirectTop;\n    prj_DirectIf direct         ();\n    always_comb direct.payload = 0;\n    logic observed      ; always_comb observed       = prj_DirectPkg::consume(direct.payload);\nendmodule\n//# sourceMappingURL=package_function.sv.map\n";
    let expected_map = "{\"version\":3,\"file\":\"package_function.sv.map\",\"sources\":[\"package_function.veryl\"],\"names\":[\"\",\"interface\",\"prj_DirectIf\",\";\",\"logic\",\"payload\",\"modport\",\"sink\",\"(\",\"input\",\")\",\"endinterface\",\"package\",\"prj_DirectPkg\",\"function\",\"consume\",\"__p_payload\",\"return\",\"endfunction\",\"endpackage\",\"module\",\"prj_DirectTop\",\"direct\",\"always_comb\",\".\",\"=\",\"0\",\"observed\",\"prj_DirectPkg::consume\",\"direct.payload\",\"endmodule\"],\"mappings\":\"AAAAA,AAAAC,UAAUC,YAASC;IACFC,MAATC,OAAcF;IAClBG,QAAQC,KAAKC;QAAWC,MAATJ,OAAcL;IAAEU;AACnCC;;AAEAC,QAAQC,aAAUV;IACdW,mBAEKV,MAFIW,OAAQP;QACbC,UANSL,MAMTY,WAAyBhB;IAC7BU,EAAEV,CAASA;QACPiB,OAAOD,WAASb;IACpBe;AACJC;;AAEAC,OAAOC,aAAUlB;IACbH,AAAaE,aAARoB,iBAAgBnB;IACrBoB,YAAOD,MAAME,CAACnB,QAAQoB,EAAEC,CAACvB;IACXC,MAAVuB,4BAAAA,eAAgBF,EAAEG,sBAAkBpB,CAACqB,cAAMnB,CAACP;AACpD2B\"}";

    assert_eq!(emitted, expected);
    assert_eq!(String::from_utf8(source_map).unwrap(), expected_map);
}

#[test]
fn direct_interface_resolution_failure_is_typed_and_commits_nothing() {
    let code = r#"interface DirectIf {
    var payload: logic;
    modport sink { payload: input, }
}
package DirectPkg {
    function consume (p: modport DirectIf::sink,) -> logic {
        return p.payload;
    }
}
"#;
    symbol_table::clear();
    attribute_table::clear();
    let metadata = Metadata::create_default("prj").unwrap();
    let source = PathBuf::from("direct_resolution_fault.veryl");
    let parser = Parser::parse(code, &source).unwrap();
    let analyzer = Analyzer::new(&metadata);
    let mut context = Context::default();
    analyzer.analyze_pass1("prj", &parser.veryl);
    Analyzer::analyze_post_pass1();
    assert!(
        analyzer
            .analyze_pass2(&parser.veryl, &mut context, None)
            .is_empty()
    );
    let analysis = context.finish_nested_modport_analysis().unwrap();
    let mut emitter = Emitter::new(
        &metadata,
        &source,
        &PathBuf::from("direct_resolution_fault.sv"),
        &PathBuf::from("direct_resolution_fault.sv.map"),
    );
    crate::expaneded_modport::force_direct_interface_resolution_failure(true);
    let result = emitter.emit(&parser.veryl, code, &analysis);
    crate::expaneded_modport::force_direct_interface_resolution_failure(false);

    assert!(matches!(
        result,
        Err(crate::EmitterError::MissingExpandedPortResolution { .. })
    ));
    assert!(emitter.as_str().is_empty());
    assert!(emitter.source_map().to_bytes().is_err());
}
