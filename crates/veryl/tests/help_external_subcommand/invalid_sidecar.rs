use std::fs;

use super::{root_help, subcommand_line_count, tempdir, write_executable};

fn sidecar_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("veryl-import.help.toml")
}

fn write_sidecar(dir: &std::path::Path, content: impl AsRef<[u8]>) {
    fs::write(sidecar_path(dir), content).unwrap();
}

fn assert_import_falls_back(help: &str) {
    assert_eq!(
        subcommand_line_count(help, "import"),
        1,
        "expected invalid sidecar to keep external command listed once; help output:\n{help}"
    );

    let line = help
        .lines()
        .find(|line| line.split_whitespace().next() == Some("import"))
        .unwrap_or("");
    assert!(
        line.contains("External Veryl subcommand from PATH (veryl-import)"),
        "expected invalid sidecar to use generic External fallback; observed line:\n{line}\nfull help output:\n{help}"
    );
}

fn assert_invalid_sidecar_falls_back(content: impl AsRef<[u8]>) {
    let temp = tempdir();
    write_executable(temp.path(), "veryl-import");
    write_sidecar(temp.path(), content);

    let help = root_help(temp.path(), &[temp.path()], "-h");

    assert_import_falls_back(&help);
}

#[test]
fn root_help_falls_back_for_malformed_toml_sidecar() {
    assert_invalid_sidecar_falls_back(b"description = [\n");
}

#[test]
fn root_help_falls_back_for_missing_description_sidecar() {
    assert_invalid_sidecar_falls_back(b"title = \"Veryl import\"\n");
}

#[test]
fn root_help_falls_back_for_non_string_description_sidecar() {
    assert_invalid_sidecar_falls_back(b"description = 42\n");
}

#[test]
fn root_help_falls_back_for_empty_description_sidecar() {
    assert_invalid_sidecar_falls_back(b"description = \"   \"\n");
}

#[test]
fn root_help_falls_back_for_multiline_description_sidecar() {
    assert_invalid_sidecar_falls_back(b"description = \"\"\"first line\nsecond line\"\"\"\n");
}

#[test]
fn root_help_falls_back_for_control_character_description_sidecar() {
    assert_invalid_sidecar_falls_back(b"description = \"bad\\u0007description\"\n");
}

#[test]
fn root_help_falls_back_for_oversized_description_sidecar() {
    let description = "x".repeat(161);
    assert_invalid_sidecar_falls_back(format!("description = \"{description}\"\n"));
}

#[test]
fn root_help_falls_back_for_oversized_sidecar_file() {
    let mut sidecar = String::from("description = \"Valid\"\n");
    sidecar.push_str(&"#".repeat(4096));

    assert_invalid_sidecar_falls_back(sidecar);
}

#[test]
fn root_help_falls_back_for_non_utf8_sidecar_bytes() {
    assert_invalid_sidecar_falls_back(b"description = \"bad\xffdescription\"\n");
}

#[test]
fn root_help_falls_back_for_unreadable_sidecar() {
    let temp = tempdir();
    write_executable(temp.path(), "veryl-import");
    fs::create_dir(sidecar_path(temp.path())).unwrap();

    let help = root_help(temp.path(), &[temp.path()], "-h");

    assert_import_falls_back(&help);
}
