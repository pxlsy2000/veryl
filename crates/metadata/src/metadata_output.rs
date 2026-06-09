use crate::lockfile::{Lock, LockSource};
use crate::metadata::Metadata;
use semver::Version;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MetadataOutputV2 {
    pub format_version: usize,
    pub root: MetadataProjectV2,
    pub dependencies: Vec<MetadataDependencyV2>,
    pub metadata: BTreeMap<String, toml::Value>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MetadataProjectV2 {
    pub name: String,
    pub version: Option<Version>,
    pub local_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MetadataDependencyV2 {
    pub id: String,
    pub name: String,
    pub project: String,
    pub source: MetadataSourceV2,
    pub local_path: PathBuf,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MetadataSourceV2 {
    Path {
        path: PathBuf,
    },
    Repository {
        url: String,
        project: String,
        version: Version,
        revision: String,
        path: PathBuf,
    },
}

impl MetadataOutputV2 {
    pub fn from_metadata(metadata: &Metadata) -> Self {
        let project_path = metadata.project_path();
        let locks = metadata.lockfile.projects();
        let dependency_ids = locks
            .iter()
            .map(|lock| (lock.source.clone(), format!("dep:{}", lock.name)))
            .collect::<HashMap<_, _>>();
        let mut dependencies = locks
            .iter()
            .map(|lock| MetadataDependencyV2::from_lock(lock, &project_path, &dependency_ids))
            .collect::<Vec<_>>();
        dependencies.sort_by(|x, y| x.id.cmp(&y.id));

        Self {
            format_version: 2,
            root: MetadataProjectV2 {
                name: metadata.project.name.clone(),
                version: metadata.project.version.clone(),
                local_path: project_path,
            },
            dependencies,
            metadata: metadata.metadata.clone().into_iter().collect(),
        }
    }
}

impl MetadataDependencyV2 {
    fn from_lock(
        lock: &Lock,
        root_path: &Path,
        dependency_ids: &HashMap<LockSource, String>,
    ) -> Self {
        let source = MetadataSourceV2::from_lock_source(&lock.source);
        let mut dependencies = lock
            .dependencies
            .iter()
            .map(|dependency| {
                dependency_ids
                    .get(&dependency.source)
                    .cloned()
                    .unwrap_or_else(|| format!("dep:{}", dependency.name))
            })
            .collect::<Vec<_>>();
        dependencies.sort();

        Self {
            // Lock names are conflict-disambiguated during lock generation, so they are stable ids.
            id: format!("dep:{}", lock.name),
            name: lock.name.clone(),
            project: lock.source.project().unwrap_or(&lock.name).to_string(),
            local_path: lock
                .source
                .local_path(root_path)
                .expect("lock source local path should be derivable"),
            source,
            dependencies,
        }
    }
}

impl MetadataSourceV2 {
    fn from_lock_source(source: &LockSource) -> Self {
        match source {
            LockSource::Path(path) => Self::Path { path: path.clone() },
            LockSource::Repository(repository) => Self::Repository {
                url: repository.url().to_string(),
                project: repository.project().to_string(),
                version: repository.version().clone(),
                revision: repository.revision().to_string(),
                path: repository.path().to_path_buf(),
            },
        }
    }
}
