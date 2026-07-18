use std::fs;
use std::process::Command;

#[test]
fn unrelated_prior_generic_signature_does_not_own_next_source_module() {
    let project = tempfile::tempdir().expect("temporary project must be created");
    let source_dir = project.path().join("src");
    let output_dir = project.path().join("generated");
    fs::create_dir(&source_dir).expect("fixture source directory must be created");
    fs::write(
        project.path().join("Veryl.toml"),
        "[project]\nname = \"nested_emission_session\"\nversion = \"0.1.0\"\n\n[build]\nsources = [\"src\"]\ntarget = { type = \"source\" }\n",
    )
    .expect("fixture manifest must be written");
    fs::write(
        source_dir.join("56_generic_interface.veryl"),
        include_str!("../../../testcases/veryl/56_generic_interface.veryl"),
    )
    .expect("first fixture source must be written");
    fs::write(
        source_dir.join("69_proto.veryl"),
        include_str!("../../../testcases/veryl/69_proto.veryl"),
    )
    .expect("second fixture source must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_veryl"))
        .current_dir(project.path())
        .args(["build", "--out-dir"])
        .arg(&output_dir)
        .output()
        .expect("veryl build must execute");

    assert!(
        output.status.success(),
        "veryl build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_dir.join("69_proto.sv").is_file());
}

#[test]
fn concurrent_full_builds_keep_generic_nested_specializations_isolated() {
    let builds = [8, 16].map(|width| {
        std::thread::spawn(move || {
            let project = tempfile::tempdir().expect("temporary project must be created");
            let source_dir = project.path().join("src");
            let output_dir = project.path().join("generated");
            fs::create_dir(&source_dir).expect("fixture source directory must be created");
            fs::write(
                project.path().join("Veryl.toml"),
                format!(
                    "[project]\nname = \"nested_session_{width}\"\nversion = \"0.1.0\"\n\n[build]\nsources = [\"src\"]\ntarget = {{ type = \"source\" }}\n"
                ),
            )
            .expect("fixture manifest must be written");
            fs::write(
                source_dir.join("top.veryl"),
                format!(
                    "interface ChildIf::<W: u32> {{\n\
                         var payload: logic<W>;\n\
                         modport sink {{ payload: input, }}\n\
                     }}\n\
                     interface ParentIf::<W: u32> {{\n\
                         inst child: ChildIf::<W>;\n\
                         modport sink {{ child.sink: modport, }}\n\
                     }}\n\
                     module Top {{ inst parent: ParentIf::<{width}>; }}\n"
                ),
            )
            .expect("fixture source must be written");
            let output = Command::new(env!("CARGO_BIN_EXE_veryl"))
                .current_dir(project.path())
                .args(["build", "--out-dir"])
                .arg(&output_dir)
                .output()
                .expect("veryl build must execute");
            assert!(
                output.status.success(),
                "veryl build failed for width {width}:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            fs::read_to_string(output_dir.join("top.sv"))
                .expect("generated SystemVerilog must be readable")
        })
    });
    let [emitted8, emitted16] = builds.map(|build| build.join().expect("build thread must finish"));
    assert!(emitted8.contains("ParentIf__8"), "{emitted8}");
    assert!(!emitted8.contains("ParentIf__16"), "{emitted8}");
    assert!(
        emitted8.contains("logic[8-1:0] child__payload"),
        "{emitted8}"
    );
    assert!(emitted16.contains("ParentIf__16"), "{emitted16}");
    assert!(!emitted16.contains("ParentIf__8"), "{emitted16}");
    assert!(
        emitted16.contains("logic[16-1:0] child__payload"),
        "{emitted16}"
    );
}
