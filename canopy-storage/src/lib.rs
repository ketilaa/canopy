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
    let path = storage_dir().join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml::to_string(value)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

pub fn intent_slug(title: &str) -> String {
    title
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

fn load<T: serde::de::DeserializeOwned>(relative: &str) -> Result<T, StorageError> {
    let path = storage_dir().join(relative);
    if !path.exists() {
        return Err(StorageError::NotFound(relative.to_string()));
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_yaml::from_str(&content)?)
}

/// Load an optional artifact, falling back to its default when the file doesn't exist yet.
fn load_or_default<T: serde::de::DeserializeOwned + Default>(relative: &str) -> Result<T, StorageError> {
    match load::<T>(relative) {
        Ok(v) => Ok(v),
        Err(StorageError::NotFound(_)) => Ok(T::default()),
        Err(e) => Err(e),
    }
}

pub fn save_idea(idea: &Idea) -> Result<(), StorageError>              { save("idea.yaml", idea) }
pub fn load_idea() -> Result<Idea, StorageError>                       { load("idea.yaml") }

pub fn load_dependency_decisions() -> Result<DependencyDecisionLog, StorageError> {
    load_or_default("dependency_decisions.yaml")
}
pub fn save_dependency_decisions(log: &DependencyDecisionLog) -> Result<(), StorageError> {
    save("dependency_decisions.yaml", log)
}

pub fn save_adr(index: usize, slug: &str, adr: &Adr) -> Result<(), StorageError> {
    save(&format!("decisions/adr-{:03}-{}.yaml", index, slug), adr)
}

pub fn load_config() -> Result<Option<CanopyConfig>, StorageError> {
    match load::<CanopyConfig>("config.yaml") {
        Ok(cfg) => Ok(Some(cfg)),
        Err(StorageError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn save_config(config: &CanopyConfig) -> Result<(), StorageError> {
    save("config.yaml", config)
}

/// Load the accumulated domain vocabulary. Returns an empty registry when the file
/// doesn't exist yet — it is always optional and built incrementally via `canopy intent`.
/// When Roots is integrated, callers should prefer Roots over this file.
pub fn load_domain_registry() -> Result<DomainRegistry, StorageError> {
    load_or_default("domain_registry.yaml")
}

pub fn save_domain_registry(r: &DomainRegistry) -> Result<(), StorageError> {
    save("domain_registry.yaml", r)
}

pub fn save_scaffold_plan(plan: &ScaffoldPlan) -> Result<(), StorageError> {
    save("scaffold.yaml", plan)
}

pub fn load_scaffold_plan() -> Result<ScaffoldPlan, StorageError> {
    load("scaffold.yaml")
}

pub fn save_user_stories(s: &UserStories) -> Result<(), StorageError> { save("stories.yaml", s) }
pub fn load_user_stories() -> Result<UserStories, StorageError> {
    load_or_default("stories.yaml")
}

pub fn save_roles_registry(r: &RolesRegistry) -> Result<(), StorageError> { save("roles.yaml", r) }
pub fn load_roles_registry() -> Result<RolesRegistry, StorageError> {
    load_or_default("roles.yaml")
}

pub fn save_services_registry(r: &ServicesRegistry) -> Result<(), StorageError> { save("services.yaml", r) }
pub fn load_services_registry() -> Result<ServicesRegistry, StorageError> {
    load_or_default("services.yaml")
}

pub fn save_story_spec(story_id: &str, spec: &IntentSpec) -> Result<(), StorageError> {
    save(&format!("stories/{}/spec.yaml", story_id), spec)
}

pub fn load_story_spec(story_id: &str) -> Result<IntentSpec, StorageError> {
    load(&format!("stories/{}/spec.yaml", story_id))
}

pub fn save_story_plan(story_id: &str, plan: &StoryPlan) -> Result<(), StorageError> {
    save(&format!("stories/{}/plan.yaml", story_id), plan)
}

pub fn load_story_plan(story_id: &str) -> Result<StoryPlan, StorageError> {
    load(&format!("stories/{}/plan.yaml", story_id))
}

pub fn save_story_openapi(story_id: &str, openapi: &str) -> Result<(), StorageError> {
    let path = storage_dir().join(format!("stories/{}/openapi.yaml", story_id));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, openapi)?;
    Ok(())
}

/// Returns `Ok(None)` (not an error) when no OpenAPI spec has been generated yet for this story —
/// callers that treat it as optional context (e.g. Stage 1 behavior extraction) shouldn't have
/// to special-case a `StorageError::NotFound`.
pub fn load_story_openapi(story_id: &str) -> Result<Option<String>, StorageError> {
    let path = storage_dir().join(format!("stories/{}/openapi.yaml", story_id));
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(path)?))
}

/// Stage 0 (Specification Completeness) output — see docs/design/behavior-first-planning.md.
pub fn save_specification_completeness(story_id: &str, completeness: &SpecificationCompleteness) -> Result<(), StorageError> {
    save(&format!("stories/{}/completeness.yaml", story_id), completeness)
}

pub fn load_specification_completeness(story_id: &str) -> Result<SpecificationCompleteness, StorageError> {
    load(&format!("stories/{}/completeness.yaml", story_id))
}

/// Stage 1 (Behavior Extraction) outputs — see docs/design/behavior-first-planning.md.
/// `behavior-coverage.yaml` is a derived view (see `BehaviorList::coverage`), saved alongside
/// for a human to audit without re-deriving it.
pub fn save_behaviors(story_id: &str, behaviors: &BehaviorList) -> Result<(), StorageError> {
    save(&format!("stories/{}/behaviors.yaml", story_id), behaviors)?;
    save(&format!("stories/{}/behavior-coverage.yaml", story_id), &behaviors.coverage())
}

pub fn load_behaviors(story_id: &str) -> Result<BehaviorList, StorageError> {
    load(&format!("stories/{}/behaviors.yaml", story_id))
}

pub fn save_behavior_gaps(story_id: &str, gaps: &BehaviorGaps) -> Result<(), StorageError> {
    save(&format!("stories/{}/behavior-gaps.yaml", story_id), gaps)
}

pub fn save_behavior_audit(story_id: &str, audit: &BehaviorAudit) -> Result<(), StorageError> {
    save(&format!("stories/{}/behavior-audit.yaml", story_id), audit)
}

/// Stage 2 (Decision Extraction and Gating) outputs — see docs/design/behavior-first-planning.md.
pub fn save_decisions(story_id: &str, decisions: &DecisionLog) -> Result<(), StorageError> {
    save(&format!("stories/{}/decisions.yaml", story_id), decisions)
}

pub fn load_decisions(story_id: &str) -> Result<DecisionLog, StorageError> {
    load(&format!("stories/{}/decisions.yaml", story_id))
}

pub fn save_decision_audit(story_id: &str, audit: &DecisionAudit) -> Result<(), StorageError> {
    save(&format!("stories/{}/decision-audit.yaml", story_id), audit)
}

/// Stage 3 (Mechanical Clustering) outputs — see docs/design/behavior-first-planning.md.
pub fn save_clustering(story_id: &str, clustering: &ClusteringResult) -> Result<(), StorageError> {
    save(&format!("stories/{}/clusters.yaml", story_id), clustering)
}

pub fn load_clustering(story_id: &str) -> Result<ClusteringResult, StorageError> {
    load(&format!("stories/{}/clusters.yaml", story_id))
}

pub fn save_cluster_review(story_id: &str, review: &ClusterReview) -> Result<(), StorageError> {
    save(&format!("stories/{}/cluster-review.yaml", story_id), review)
}

pub fn save_clustering_audit(story_id: &str, audit: &ClusteringAudit) -> Result<(), StorageError> {
    save(&format!("stories/{}/clustering-audit.yaml", story_id), audit)
}

pub fn load_all_adrs() -> Result<Vec<Adr>, StorageError> {
    let paths = list_adrs()?;
    let mut adrs = Vec::new();
    for path in paths {
        let content = std::fs::read_to_string(&path)?;
        if let Ok(adr) = serde_yaml::from_str::<Adr>(&content) {
            adrs.push(adr);
        }
    }
    Ok(adrs)
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
    fn load_missing_artifact_returns_not_found() {
        with_tmpdir(|| {
            let result = load_story_spec("nonexistent-story");
            assert!(matches!(result, Err(StorageError::NotFound(_))));
        });
    }

    #[test]
    fn load_config_missing_returns_none() {
        with_tmpdir(|| {
            let result = load_config().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn save_load_config_round_trip() {
        with_tmpdir(|| {
            use canopy_core::{AgentLlmConfig, CanopyConfig, LlmProvider};
            let config = CanopyConfig {
                default: Some(AgentLlmConfig {
                    provider: LlmProvider::Ollama,
                    model: "qwen2.5:32b".into(),
                    base_url: None,
                }),
                agents: None,
            };
            save_config(&config).unwrap();
            let loaded = load_config().unwrap().unwrap();
            let default = loaded.default.unwrap();
            assert_eq!(default.provider, LlmProvider::Ollama);
            assert_eq!(default.model, "qwen2.5:32b");
        });
    }

    #[test]
    fn intent_slug_normalizes_title() {
        assert_eq!(intent_slug("User Authentication"), "user-authentication");
        assert_eq!(intent_slug("Dashboard & Reports"), "dashboard--reports");
        assert_eq!(intent_slug("  Spaces  "), "spaces");
    }

    #[test]
    fn save_load_scaffold_plan_round_trip() {
        with_tmpdir(|| {
            use canopy_core::{ScaffoldCommand, ScaffoldPlan};
            let plan = ScaffoldPlan {
                generated_at: "1750000000".into(),
                commands: vec![ScaffoldCommand {
                    label: "storefront".into(),
                    command: "npx create-next-app@latest storefront".into(),
                    working_dir: ".".into(),
                    creates: "storefront/".into(),
                }],
            };
            save_scaffold_plan(&plan).unwrap();
            let loaded = load_scaffold_plan().unwrap();
            assert_eq!(plan.commands.len(), loaded.commands.len());
            assert_eq!(plan.commands[0].creates, loaded.commands[0].creates);
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
