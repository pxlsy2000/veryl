use std::fs;
use std::process::Command;

#[test]
fn build_pairs_combined_package_and_function_specializations() {
    // Given
    let project = tempfile::tempdir().expect("temporary project must be created");
    let source_dir = project.path().join("src");
    let output_dir = project.path().join("generated");
    fs::create_dir(&source_dir).expect("fixture source directory must be created");
    fs::write(
        project.path().join("Veryl.toml"),
        include_str!("fixtures/package_function_combined/Veryl.toml"),
    )
    .expect("fixture manifest must be written");
    fs::write(
        source_dir.join("top.veryl"),
        include_str!("fixtures/package_function_combined/src/top.veryl"),
    )
    .expect("fixture source must be written");

    // When
    let output = Command::new(env!("CARGO_BIN_EXE_veryl"))
        .current_dir(project.path())
        .args(["build", "--out-dir"])
        .arg(&output_dir)
        .output()
        .expect("veryl build must execute");

    // Then
    assert!(
        output.status.success(),
        "veryl build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted = fs::read_to_string(output_dir.join("top.sv"))
        .expect("generated SystemVerilog must be readable");
    assert_package_owns_function(
        &emitted,
        "package_function_combined___Combo__8",
        "__consume__8",
    );
    assert_package_owns_function(
        &emitted,
        "package_function_combined___Combo__16",
        "__consume__16",
    );
}

fn assert_package_owns_function(emitted: &str, package: &str, function: &str) {
    let body = emitted
        .split_once(&format!("package {package};"))
        .and_then(|(_, tail)| tail.split_once("endpackage"))
        .map(|(body, _)| body)
        .unwrap_or_else(|| panic!("package {package} must be emitted:\n{emitted}"));
    assert!(
        body.contains(&format!("function automatic")) && body.contains(&format!("{function}(")),
        "call target {package}::{function} must be declared by the same package:\n{emitted}"
    );
    assert!(
        emitted.contains(&format!("{package}::{function}(")),
        "qualified call {package}::{function} must be emitted:\n{emitted}"
    );
}
