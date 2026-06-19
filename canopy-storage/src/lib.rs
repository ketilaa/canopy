use canopy_core::*;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Artifact not found: {0}. Run the appropriate command first.")]
    NotFound(String),
}

pub fn storage_dir() -> PathBuf {
    std::env::current_dir()
        .expect("cannot determine current working directory")
        .join(".canopy")
}

pub fn ensure_storage_dir() -> Result<(), StorageError> {
    std::fs::create_dir_all(storage_dir().join("decisions"))?;
    Ok(())
}

fn save<T: serde::Serialize>(relative: &str, value: &T) -> Result<(), StorageError> {
    ensure_storage_dir()?;
    let path = storage_dir().join(relative);
    let yaml = serde_yaml::to_string(value)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

fn load<T: serde::de::DeserializeOwned>(relative: &str) -> Result<T, StorageError> {
    let path = storage_dir().join(relative);
    if !path.exists() {
        return Err(StorageError::NotFound(relative.to_string()));
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_yaml::from_str(&content)?)
}

pub fn save_idea(idea: &Idea) -> Result<(), StorageError>              { save("idea.yaml", idea) }
pub fn load_idea() -> Result<Idea, StorageError>                       { load("idea.yaml") }

pub fn save_vision(v: &Vision) -> Result<(), StorageError>             { save("vision.yaml", v) }
pub fn load_vision() -> Result<Vision, StorageError>                   { load("vision.yaml") }

pub fn save_requirements(r: &Requirements) -> Result<(), StorageError> { save("requirements.yaml", r) }
pub fn load_requirements() -> Result<Requirements, StorageError>       { load("requirements.yaml") }

pub fn save_domain(d: &DomainModel) -> Result<(), StorageError>        { save("domain.yaml", d) }
pub fn load_domain() -> Result<DomainModel, StorageError>              { load("domain.yaml") }

pub fn save_architecture(a: &Architecture) -> Result<(), StorageError> { save("architecture.yaml", a) }
pub fn load_architecture() -> Result<Architecture, StorageError>       { load("architecture.yaml") }

pub fn save_adr(index: usize, slug: &str, adr: &Adr) -> Result<(), StorageError> {
    save(&format!("decisions/adr-{:03}-{}.yaml", index, slug), adr)
}

pub fn list_adrs() -> Result<Vec<PathBuf>, StorageError> {
    let dir = storage_dir().join("decisions");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |ext| ext == "yaml"))
        .collect();
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn with_tmpdir<F: FnOnce()>(f: F) {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let prev = env::current_dir().unwrap();
        env::set_current_dir(tmp.path()).unwrap();
        f();
        env::set_current_dir(prev).unwrap();
    }

    #[test]
    fn save_load_idea_round_trip() {
        with_tmpdir(|| {
            let idea = Idea { description: "test idea".into() };
            save_idea(&idea).unwrap();
            let loaded = load_idea().unwrap();
            assert_eq!(idea.description, loaded.description);
        });
    }

    #[test]
    fn save_load_vision_round_trip() {
        with_tmpdir(|| {
            let v = Vision {
                project: "TestApp".into(),
                problem: "Some problem".into(),
                users: vec!["Admin".into()],
                goals: vec!["Goal A".into()],
            };
            save_vision(&v).unwrap();
            let loaded = load_vision().unwrap();
            assert_eq!(v.project, loaded.project);
            assert_eq!(v.users, loaded.users);
        });
    }

    #[test]
    fn load_missing_artifact_returns_not_found() {
        with_tmpdir(|| {
            let result = load_vision();
            assert!(matches!(result, Err(StorageError::NotFound(_))));
        });
    }

    #[test]
    fn save_adr_creates_file_with_correct_name() {
        with_tmpdir(|| {
            let adr = Adr {
                title: "Use PostgreSQL".into(),
                decision: "PostgreSQL as primary DB".into(),
                reason: "Relational model fits".into(),
                alternatives: vec!["MongoDB".into()],
            };
            save_adr(1, "use-postgresql", &adr).unwrap();
            let adrs = list_adrs().unwrap();
            assert_eq!(adrs.len(), 1);
            assert!(adrs[0].file_name().unwrap().to_str().unwrap().starts_with("adr-001-use-postgresql"));
        });
    }
}
