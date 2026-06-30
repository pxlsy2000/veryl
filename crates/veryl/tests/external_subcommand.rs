// The mock external binaries are POSIX shell scripts, so these CLI dispatch tests run on Linux CI.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn veryl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_veryl"))
}

fn write_mock(dir: &Path, script: &str) {
    let path = dir.join("veryl-import");
    fs::write(&path, script).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn run_with_path(work_dir: &Path, path_dir: &Path, args: &[&str]) -> Output {
    let path = match std::env::var_os("PATH") {
        Some(path) => std::env::join_paths(
            std::iter::once(path_dir.to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .unwrap(),
        None => path_dir.as_os_str().to_os_string(),
    };

    veryl()
        .current_dir(work_dir)
        .env("PATH", path)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn external_command_executes_from_path() {
    let temp = tempfile::tempdir().unwrap();
    write_mock(temp.path(), "#!/bin/sh\nprintf 'mock import ran\\n'\n");

    let output = run_with_path(temp.path(), temp.path(), &["import"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "mock import ran\n");
}

#[test]
fn external_command_forwards_arguments_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let args_file = temp.path().join("args.txt");
    write_mock(
        temp.path(),
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '<%s>\\n' \"$arg\"; done > {}\n",
            args_file.display()
        ),
    );

    let output = run_with_path(
        temp.path(),
        temp.path(),
        &["import", "--target", "syn", "two words"],
    );

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(args_file).unwrap(),
        "<--target>\n<syn>\n<two words>\n"
    );
}

#[test]
fn external_command_exit_code_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    write_mock(temp.path(), "#!/bin/sh\nexit 17\n");

    let output = run_with_path(temp.path(), temp.path(), &["import"]);

    assert_eq!(output.status.code(), Some(17));
}

#[test]
fn missing_external_binary_reports_actionable_diagnostic() {
    let temp = tempfile::tempdir().unwrap();

    let output = veryl()
        .current_dir(temp.path())
        .env("PATH", temp.path())
        .args(["import", "--target", "syn"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("veryl-import"));
    assert!(stderr.contains("install"));
    assert!(stderr.contains("PATH"));
}

#[test]
fn external_command_does_not_require_metadata_or_create_build_dir() {
    let temp = tempfile::tempdir().unwrap();
    write_mock(temp.path(), "#!/bin/sh\nprintf 'no metadata\\n'\n");

    let output = run_with_path(temp.path(), temp.path(), &["import", "--target", "syn"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "no metadata\n");
    assert!(!temp.path().join(".build").exists());
    assert!(!temp.path().join("Veryl.toml").exists());
}
