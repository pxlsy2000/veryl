use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::is_executable_file;

const MAX_HELP_DESCRIPTION_CHARS: usize = 160;
const MAX_HELP_SIDECAR_BYTES: u64 = 4096;

pub struct ExternalHelpSubcommand {
    pub name: String,
    pub binary_name: String,
    pub executable_path: PathBuf,
    pub description: Option<String>,
}

pub fn discover_help_subcommands(builtins: &[&str]) -> Vec<ExternalHelpSubcommand> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };

    discover_from_path_entries_excluding(std::env::split_paths(&path), builtins.iter().copied())
}

fn discover_from_path_entries_excluding<P, B>(
    path_entries: impl IntoIterator<Item = P>,
    builtins: impl IntoIterator<Item = B>,
) -> Vec<ExternalHelpSubcommand>
where
    P: AsRef<Path>,
    B: AsRef<str>,
{
    let mut excluded = builtins
        .into_iter()
        .map(|builtin| builtin.as_ref().to_owned())
        .collect::<BTreeSet<_>>();
    excluded.insert("help".to_owned());

    let mut discovered = BTreeMap::new();
    for dir in path_entries {
        let Ok(entries) = fs::read_dir(dir.as_ref()) else {
            continue;
        };

        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if !is_executable_file(&path) {
                continue;
            }

            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let Some(suffix) = file_name.strip_prefix("veryl-") else {
                continue;
            };
            if !suffix.is_empty()
                && !suffix.contains('/')
                && !suffix.contains('\\')
                && !suffix.contains(std::path::MAIN_SEPARATOR)
                && !excluded.contains(suffix)
            {
                discovered
                    .entry(suffix.to_owned())
                    .or_insert_with(|| ExternalHelpSubcommand {
                        name: suffix.to_owned(),
                        binary_name: file_name.to_owned(),
                        executable_path: path.clone(),
                        description: read_help_description(file_name, &path),
                    });
            }
        }
    }

    discovered.into_values().collect()
}

fn read_help_description(binary_name: &str, executable_path: &Path) -> Option<String> {
    let sidecar_path = executable_path.with_file_name(format!("{binary_name}.help.toml"));
    let metadata = fs::metadata(&sidecar_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_HELP_SIDECAR_BYTES {
        return None;
    }

    let content = fs::read_to_string(sidecar_path).ok()?;
    let document = content.parse::<toml::Table>().ok()?;
    let description = document.get("description")?.as_str()?;
    parse_help_description(description)
}

fn parse_help_description(description: &str) -> Option<String> {
    if description.chars().any(char::is_control) {
        return None;
    }

    let description = description.trim();
    if description.is_empty() || description.chars().count() > MAX_HELP_DESCRIPTION_CHARS {
        return None;
    }

    Some(description.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    fn write_file(dir: &Path, name: &OsStr, mode: u32) {
        let path = dir.join(Path::new(name));
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(mode);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(unix)]
    fn write_executable(dir: &Path, name: &OsStr) {
        write_file(dir, name, 0o755);
    }

    #[cfg(unix)]
    fn write_non_executable(dir: &Path, name: &OsStr) {
        write_file(dir, name, 0o644);
    }

    #[test]
    #[cfg(unix)]
    fn discover_subcommands_keeps_valid_utf8_executable_suffixes_sorted() {
        // Given: duplicate valid external subcommands exist across PATH entries.
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        write_executable(first.path(), OsStr::new("veryl-import"));
        write_executable(second.path(), OsStr::new("veryl-import"));
        write_executable(second.path(), OsStr::new("veryl-flist"));
        fs::write(
            first.path().join("veryl-import.help.toml"),
            "description = \"First import description\"\n",
        )
        .unwrap();
        fs::write(
            second.path().join("veryl-import.help.toml"),
            "description = \"Second import description\"\n",
        )
        .unwrap();

        // When: discovery scans those PATH entries.
        let discovered = discover_from_path_entries_excluding(
            [first.path(), second.path()],
            std::iter::empty::<&str>(),
        );

        // Then: names are deduped and sorted by Rust String ordering.
        assert_eq!(names(&discovered), ["flist", "import"]);
        assert_eq!(
            discovered[1].description.as_deref(),
            Some("First import description")
        );
    }

    #[test]
    #[cfg(unix)]
    fn discover_subcommands_ignores_builtin_collisions_and_help() {
        // Given: PATH contains valid external binaries and names reserved by Veryl.
        let dir = tempfile::tempdir().unwrap();
        write_executable(dir.path(), OsStr::new("veryl-build"));
        write_executable(dir.path(), OsStr::new("veryl-check"));
        write_executable(dir.path(), OsStr::new("veryl-help"));
        write_executable(dir.path(), OsStr::new("veryl-import"));

        // When: discovery scans with the built-in command exclusion list.
        let discovered = discover_from_path_entries_excluding([dir.path()], ["build", "check"]);

        // Then: reserved names are omitted and external-only names remain.
        assert_eq!(names(&discovered), ["import"]);
    }

    #[test]
    #[cfg(unix)]
    fn discover_subcommands_ignores_non_executable_non_directory_and_malformed_entries() {
        // Given: PATH includes malformed candidates, a plain file entry, and one valid command.
        let dir = tempfile::tempdir().unwrap();
        let plain_file = tempfile::NamedTempFile::new().unwrap();
        write_executable(dir.path(), OsStr::new("veryl-flist"));
        write_executable(dir.path(), OsStr::new("veryl-"));
        write_executable(dir.path(), OsStr::new("veryl-bad\\name"));
        write_executable(dir.path(), OsStr::from_bytes(b"veryl-im\xffort"));
        write_non_executable(dir.path(), OsStr::new("veryl-import"));

        // When: discovery scans all entries.
        let discovered =
            discover_from_path_entries_excluding([dir.path(), plain_file.path()], ["flist"]);

        // Then: only valid executable, non-colliding suffixes survive.
        assert!(discovered.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn discover_subcommands_ignores_unreadable_path_entries() {
        // Given: PATH includes an unreadable directory before a readable external command directory.
        let unreadable = tempfile::tempdir().unwrap();
        let readable = tempfile::tempdir().unwrap();
        let mut permissions = fs::metadata(unreadable.path()).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(unreadable.path(), permissions).unwrap();
        write_executable(readable.path(), OsStr::new("veryl-import"));

        // When: discovery scans all entries.
        let discovered = discover_from_path_entries_excluding(
            [unreadable.path(), readable.path()],
            std::iter::empty::<&str>(),
        );

        let mut permissions = fs::metadata(unreadable.path()).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(unreadable.path(), permissions).unwrap();

        // Then: unreadable PATH entries do not fail discovery.
        assert_eq!(names(&discovered), ["import"]);
    }

    fn names(discovered: &[ExternalHelpSubcommand]) -> Vec<&str> {
        discovered
            .iter()
            .map(|subcommand| subcommand.name.as_str())
            .collect()
    }
}
