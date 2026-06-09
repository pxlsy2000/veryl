use crate::{Format, OptMetadata};
use miette::{IntoDiagnostic, Result, bail};
use veryl_metadata::{Metadata, MetadataOutputV2};

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
            (Format::Json, Some(1)) => serde_json::to_string(metadata).into_diagnostic(),
            (Format::Json, Some(2)) => {
                let output = MetadataOutputV2::from_metadata(metadata);
                serde_json::to_string(&output).into_diagnostic()
            }
            (Format::Pretty, Some(_)) => {
                bail!("--format-version is only supported with --format json")
            }
            (Format::Json, Some(version)) => {
                bail!("unsupported --format-version {version}; supported versions: 1, 2")
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
    fn json_format_version_1_preserves_internal_metadata_shape() {
        // Given: project metadata with extension-owned metadata.
        let (metadata, _tempdir) = load_metadata();

        // When: JSON metadata is formatted with legacy format version 1.
        let versioned_text = command(Format::Json, Some(1))
            .format_metadata(&metadata)
            .unwrap();
        let unversioned_text = command(Format::Json, None)
            .format_metadata(&metadata)
            .unwrap();
        let versioned_value: serde_json::Value = serde_json::from_str(&versioned_text).unwrap();
        let unversioned_value: serde_json::Value = serde_json::from_str(&unversioned_text).unwrap();

        // Then: version 1 is the same legacy/internal JSON shape as unversioned JSON.
        assert_eq!(versioned_value, unversioned_value);
        assert!(versioned_value.get("format_version").is_none());
        assert!(versioned_value.get("root").is_none());
        assert!(versioned_value.get("project").is_some());
        assert_eq!(
            versioned_value["metadata"]["vloom"]["files"][0],
            "src/**/*.v"
        );
    }

    #[test]
    fn json_format_version_2_emits_stable_graph_metadata_shape() {
        // Given: project metadata with extension-owned metadata.
        let (metadata, _tempdir) = load_metadata();

        // When: JSON metadata is formatted with stable graph format version 2.
        let text = command(Format::Json, Some(2)).format_metadata(&metadata);

        // Then: version 2 uses the stable graph metadata contract.
        assert!(
            text.as_ref().is_ok(),
            "format version 2 should be supported: {:?}",
            text.as_ref().err()
        );
        let value: serde_json::Value = serde_json::from_str(&text.unwrap()).unwrap();
        assert_eq!(value["format_version"], 2);
        assert_eq!(value["root"]["name"], "test");
        assert!(value["dependencies"].as_array().unwrap().is_empty());
        assert_eq!(value["metadata"]["vloom"]["files"][0], "src/**/*.v");
        assert!(value.get("project").is_none());
    }

    #[test]
    fn unversioned_json_preserves_internal_metadata_shape() {
        // Given: project metadata with extension-owned metadata.
        let (metadata, _tempdir) = load_metadata();

        // When: JSON metadata is formatted without an explicit version.
        let text = command(Format::Json, None)
            .format_metadata(&metadata)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        // Then: unversioned JSON preserves the existing internal metadata shape.
        assert!(value.get("format_version").is_none());
        assert!(value.get("root").is_none());
        assert!(value.get("project").is_some());
    }

    #[test]
    fn pretty_format_version_is_rejected() {
        // Given: project metadata and pretty output with explicit format versions.
        let (metadata, _tempdir) = load_metadata();

        for version in [1, 2] {
            // When: pretty metadata is formatted with an explicit version.
            let error = command(Format::Pretty, Some(version))
                .format_metadata(&metadata)
                .unwrap_err();

            // Then: pretty format rejects every explicit version.
            assert!(
                error
                    .to_string()
                    .contains("--format-version is only supported with --format json")
            );
        }
    }

    #[test]
    fn unsupported_format_version_is_rejected() {
        // Given: project metadata.
        let (metadata, _tempdir) = load_metadata();

        // When: JSON metadata is formatted with an unsupported version.
        let error = command(Format::Json, Some(3))
            .format_metadata(&metadata)
            .unwrap_err();

        // Then: the error reports all supported versions.
        assert!(error.to_string().contains("unsupported --format-version 3"));
        assert!(error.to_string().contains("supported versions: 1, 2"));
    }
}
