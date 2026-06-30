// The mock external binaries are POSIX shell scripts, so these CLI help tests run on Linux CI.
#![cfg(unix)]

#[path = "help_external_subcommand/invalid_sidecar.rs"]
mod invalid_sidecar;

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

#[cfg(not(target_os = "macos"))]
use std::os::unix::ffi::OsStringExt;

fn veryl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_veryl"))
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn write_executable(dir: &Path, name: &str) {
    write_executable_with_body(dir, OsStr::new(name), "#!/bin/sh\nexit 0\n");
}

fn write_executable_os(dir: &Path, name: &OsStr) {
    write_executable_with_body(dir, name, "#!/bin/sh\nexit 0\n");
}

fn write_executable_with_body(dir: &Path, name: &OsStr, body: &str) {
    let path = dir.join(Path::new(name));
    std::fs::write(&path, body).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
}

fn write_help_sidecar(dir: &Path, binary_name: &str, description: &str) {
    std::fs::write(
        dir.join(format!("{binary_name}.help.toml")),
        format!("description = \"{description}\"\n"),
    )
    .unwrap();
}

fn write_non_executable(dir: &Path, name: &str) {
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o644);
    std::fs::set_permissions(&path, permissions).unwrap();
}

fn root_help(work_dir: &Path, path_dirs: &[&Path], flag: &str) -> String {
    let path = std::env::join_paths(path_dirs.iter().map(|path| path.to_path_buf())).unwrap();

    let output = veryl()
        .current_dir(work_dir)
        .env("PATH", path)
        .arg(flag)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected `veryl {flag}` to succeed; status: {:?}\nstderr:\n{stderr}\nstdout:\n{stdout}",
        output.status.code()
    );
    assert!(
        stderr.is_empty(),
        "expected `veryl {flag}` to keep stderr empty; stderr:\n{stderr}"
    );

    stdout.into_owned()
}

fn subcommand_line_count(help: &str, name: &str) -> usize {
    help.lines()
        .filter(|line| line.split_whitespace().next() == Some(name))
        .count()
}

fn assert_subcommand_line_contains(help: &str, name: &str, expected: &str) {
    let line = help
        .lines()
        .find(|line| line.split_whitespace().next() == Some(name))
        .unwrap_or("");
    assert!(
        line.contains(expected),
        "expected root help line for `{name}` to contain `{expected}`; observed line:\n{line}\nfull help output:\n{help}"
    );
}

fn assert_help_lists_external_subcommands(flag: &str, help: &str, expected: &[&str]) {
    for name in expected {
        assert_eq!(
            subcommand_line_count(help, name),
            1,
            "expected root help `{flag}` to list external subcommand `{name}` exactly once; help output:\n{help}"
        );
    }
}

fn assert_help_omits_external_subcommands(flag: &str, help: &str, omitted: &[&str]) {
    for name in omitted {
        assert!(
            subcommand_line_count(help, name) == 0,
            "expected root help `{flag}` to omit external subcommand `{name}`; help output:\n{help}"
        );
    }
}

fn assert_required_command_usage(output: &str) {
    assert!(
        output.contains("Usage: veryl [OPTIONS] <COMMAND>"),
        "expected usage to keep required subcommand marker `<COMMAND>`; output:\n{output}"
    );
    assert!(
        !output.contains("Usage: veryl [OPTIONS] [COMMAND]"),
        "expected usage not to show optional subcommand marker `[COMMAND]`; output:\n{output}"
    );
}

#[test]
fn root_help_lists_external_subcommands_from_path() {
    // Given: executable external Veryl subcommands are available on PATH.
    let temp = tempdir();
    write_executable(temp.path(), "veryl-import");
    write_executable(temp.path(), "veryl-flist");

    // When: root help is requested through both short and long help flags.
    let short_help = root_help(temp.path(), &[temp.path()], "-h");
    let long_help = root_help(temp.path(), &[temp.path()], "--help");

    // Then: help lists the external subcommands as observable CLI commands.
    assert_help_lists_external_subcommands("-h", &short_help, &["flist", "import"]);
    assert_help_lists_external_subcommands("--help", &long_help, &["flist", "import"]);
    assert_required_command_usage(&short_help);
    assert_required_command_usage(&long_help);
}

#[test]
fn root_help_uses_sidecar_description_when_valid() {
    // Given: external Veryl subcommands have valid adjacent help sidecars.
    let temp = tempdir();
    write_executable(temp.path(), "veryl-flist");
    write_executable(temp.path(), "veryl-import");
    write_help_sidecar(temp.path(), "veryl-flist", "Veryl flist command");
    write_help_sidecar(temp.path(), "veryl-import", "Veryl import dry-run command");

    // When: root help is requested.
    let help = root_help(temp.path(), &[temp.path()], "-h");

    // Then: help renders the sidecar descriptions on the observable command rows.
    assert_subcommand_line_contains(&help, "flist", "External: Veryl flist command");
    assert_subcommand_line_contains(&help, "import", "External: Veryl import dry-run command");
}

#[test]
fn root_help_keeps_external_prefix_for_sidecar_and_fallback() {
    // Given: one external command has a sidecar and another relies on fallback text.
    let temp = tempdir();
    write_executable(temp.path(), "veryl-flist");
    write_executable(temp.path(), "veryl-import");
    write_help_sidecar(temp.path(), "veryl-flist", "Veryl flist command");

    // When: root help is requested.
    let help = root_help(temp.path(), &[temp.path()], "-h");

    // Then: both rows keep the literal External marker, whether custom or fallback.
    assert_subcommand_line_contains(&help, "flist", "External: Veryl flist command");
    assert_subcommand_line_contains(
        &help,
        "import",
        "External Veryl subcommand from PATH (veryl-import)",
    );
}

#[test]
fn root_help_does_not_execute_external_binary_for_description() {
    // Given: an external binary would create a marker file if root help executed it.
    let temp = tempdir();
    let marker = temp.path().join("executed-marker");
    write_executable_with_body(
        temp.path(),
        OsStr::new("veryl-flist"),
        "#!/bin/sh\ntouch executed-marker\nexit 0\n",
    );
    write_help_sidecar(temp.path(), "veryl-flist", "Veryl flist command");

    // When: root help is requested.
    let help = root_help(temp.path(), &[temp.path()], "-h");

    // Then: the sidecar description is used without invoking the external binary.
    assert_subcommand_line_contains(&help, "flist", "External: Veryl flist command");
    assert!(!marker.exists());
}

#[test]
fn root_help_omits_external_subcommands_without_binaries() {
    // Given: PATH is controlled and contains no external Veryl subcommand binaries.
    let temp = tempdir();

    // When: root help is requested.
    let help = root_help(temp.path(), &[temp.path()], "-h");

    // Then: help does not advertise unavailable external subcommands.
    assert_help_omits_external_subcommands("-h", &help, &["flist", "import"]);
    assert_required_command_usage(&help);
}

#[test]
fn root_help_ignores_non_executable_external_files() {
    // Given: PATH contains veryl-prefixed files that are not executable binaries.
    let temp = tempdir();
    write_non_executable(temp.path(), "veryl-import");
    write_non_executable(temp.path(), "veryl-flist");

    // When: root help is requested.
    let help = root_help(temp.path(), &[temp.path()], "-h");

    // Then: help ignores those non-executable files.
    assert_help_omits_external_subcommands("-h", &help, &["flist", "import"]);
}

#[test]
fn root_help_dedupes_and_sorts_external_subcommands() {
    // Given: duplicate external subcommands exist across multiple PATH entries.
    let first = tempdir();
    let second = tempdir();
    write_executable(first.path(), "veryl-import");
    write_executable(second.path(), "veryl-import");
    write_executable(second.path(), "veryl-flist");
    write_help_sidecar(first.path(), "veryl-import", "First import description");
    write_help_sidecar(second.path(), "veryl-import", "Second import description");

    // When: root help is requested.
    let help = root_help(first.path(), &[first.path(), second.path()], "-h");

    // Then: help lists each external command once in deterministic sorted order.
    assert_help_lists_external_subcommands("-h", &help, &["flist", "import"]);
    assert_subcommand_line_contains(&help, "import", "External: First import description");
    assert!(
        help.find("\n  flist").unwrap() < help.find("\n  import").unwrap(),
        "expected external subcommands to be sorted; help output:\n{help}"
    );
}

#[test]
fn root_help_ignores_builtin_collisions() {
    // Given: PATH contains external binaries that collide with built-in Veryl subcommands.
    let baseline_dir = tempdir();
    let external_dir = tempdir();
    write_executable(external_dir.path(), "veryl-build");
    write_executable(external_dir.path(), "veryl-check");
    write_executable(external_dir.path(), "veryl-import");

    // When: root help is requested with and without those colliding binaries.
    let baseline_help = root_help(baseline_dir.path(), &[baseline_dir.path()], "-h");
    let help = root_help(external_dir.path(), &[external_dir.path()], "-h");

    // Then: colliding external binaries do not duplicate built-ins, while real external commands appear.
    assert_help_lists_external_subcommands("-h", &help, &["import"]);
    assert_eq!(
        subcommand_line_count(&help, "build"),
        subcommand_line_count(&baseline_help, "build"),
        "expected external `veryl-build` to avoid duplicating built-in `build`; help output:\n{help}"
    );
    assert_eq!(
        subcommand_line_count(&help, "check"),
        subcommand_line_count(&baseline_help, "check"),
        "expected external `veryl-check` to avoid duplicating built-in `check`; help output:\n{help}"
    );
}

#[test]
#[cfg(not(target_os = "macos"))]
fn root_help_ignores_non_utf8_external_suffixes() {
    // Given: PATH contains a valid external command and a veryl-prefixed executable with a non-UTF-8 suffix.
    let temp = tempdir();
    write_executable(temp.path(), "veryl-flist");
    let non_utf8_name = std::ffi::OsString::from_vec(vec![
        b'v', b'e', b'r', b'y', b'l', b'-', b'i', b'm', 0xff, b'o', b'r', b't',
    ]);
    write_executable_os(temp.path(), non_utf8_name.as_os_str());

    // When: root help is requested.
    let help = root_help(temp.path(), &[temp.path()], "-h");

    // Then: help keeps valid UTF-8 external names and omits the malformed suffix.
    assert_help_lists_external_subcommands("-h", &help, &["flist"]);
    assert_help_omits_external_subcommands("-h", &help, &["im\u{fffd}ort"]);
}

#[test]
fn bare_cli_reports_missing_required_subcommand_usage() {
    // Given: PATH is controlled and contains no command candidates.
    let temp = tempdir();

    // When: the CLI is invoked without a subcommand.
    let output = veryl()
        .current_dir(temp.path())
        .env("PATH", temp.path())
        .output()
        .unwrap();

    // Then: it fails as a missing-subcommand error and keeps `<COMMAND>` in usage.
    assert!(
        !output.status.success(),
        "expected bare `veryl` to fail; status: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires a subcommand") || stderr.contains("subcommand"),
        "expected missing-subcommand style error; stderr:\n{stderr}"
    );
    assert_required_command_usage(&stderr);
}

#[test]
fn root_help_intercept_requires_exact_help_flag() {
    // Given: an executable external Veryl subcommand is available on PATH.
    let temp = tempdir();
    write_executable(temp.path(), "veryl-import");

    // When: `-h` is accompanied by another argument.
    let path = std::env::join_paths([temp.path()]).unwrap();
    let output = veryl()
        .current_dir(temp.path())
        .env("PATH", path)
        .args(["-h", "extra"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Then: normal clap handling answers, not the augmented exact-root-help path.
    assert!(
        output.status.success(),
        "expected clap help to succeed; status: {:?}",
        output.status.code()
    );
    assert_help_omits_external_subcommands("-h extra", &stdout, &["import"]);
    assert_required_command_usage(&stdout);
}
