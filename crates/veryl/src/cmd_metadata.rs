use crate::{Format, OptMetadata};
use miette::{IntoDiagnostic, Result, bail};
use veryl_metadata::{Metadata, MetadataOutputV1};

pub struct CmdMetadata {
    opt: OptMetadata,
}

impl CmdMetadata {
    pub fn new(opt: OptMetadata) -> Self {
        Self { opt }
    }

    pub fn exec(&self, metadata: &Metadata) -> Result<bool> {
        let text = self.format_metadata(metadata)?;

        println!("{text}");

        Ok(true)
    }

    fn format_metadata(&self, metadata: &Metadata) -> Result<String> {
        match (self.opt.format, self.opt.format_version) {
            (Format::Json, None) => serde_json::to_string(metadata).into_diagnostic(),
            (Format::Pretty, None) => Ok(format!("{metadata:#?}")),
            (Format::Json, Some(1)) => {
                let output = MetadataOutputV1::from_metadata(metadata);
                serde_json::to_string(&output).into_diagnostic()
            }
            (Format::Pretty, Some(_)) => {
                bail!("--format-version is only supported with --format json")
            }
            (Format::Json, Some(version)) => {
                bail!("unsupported --format-version {version}; supported version: 1")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const TEST_TOML: &str = r#"
[project]
name = "test"
version = "0.1.0"

[metadata.vloom]
files = ["src/**/*.v"]
"#;

    fn load_metadata() -> (Metadata, tempfile::TempDir) {
        let tempdir = tempfile::tempdir().unwrap();
        let project_dir = tempdir.path().join("test");
        fs::create_dir(&project_dir).unwrap();
        let toml_path = project_dir.join("Veryl.toml");
        fs::write(&toml_path, TEST_TOML).unwrap();
        let metadata = Metadata::load(&toml_path).unwrap();
        (metadata, tempdir)
    }

    fn command(format: Format, format_version: Option<u32>) -> CmdMetadata {
        CmdMetadata::new(OptMetadata {
            format,
            format_version,
        })
    }

    #[test]
    fn versioned_json_contains_format_version() {
        let (metadata, _tempdir) = load_metadata();
        let text = command(Format::Json, Some(1))
            .format_metadata(&metadata)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["format_version"], 1);
        assert_eq!(value["root"]["name"], "test");
        assert_eq!(value["metadata"]["vloom"]["files"][0], "src/**/*.v");
    }

    #[test]
    fn unversioned_json_preserves_internal_metadata_shape() {
        let (metadata, _tempdir) = load_metadata();
        let text = command(Format::Json, None)
            .format_metadata(&metadata)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert!(value.get("format_version").is_none());
        assert!(value.get("root").is_none());
        assert!(value.get("project").is_some());
    }

    #[test]
    fn pretty_format_version_is_rejected() {
        let (metadata, _tempdir) = load_metadata();
        let error = command(Format::Pretty, Some(1))
            .format_metadata(&metadata)
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("--format-version is only supported with --format json")
        );
    }

    #[test]
    fn unsupported_format_version_is_rejected() {
        let (metadata, _tempdir) = load_metadata();
        let error = command(Format::Json, Some(2))
            .format_metadata(&metadata)
            .unwrap_err();

        assert!(error.to_string().contains("unsupported --format-version 2"));
        assert!(error.to_string().contains("supported version: 1"));
    }
}
