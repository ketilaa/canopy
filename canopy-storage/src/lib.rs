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
    std::fs::create_dir_all(storage_dir().join("plans"))?;
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

pub fn save_idea(idea: &Idea) -> Result<(), StorageError>              { save("idea.yaml", idea) }
pub fn load_idea() -> Result<Idea, StorageError>                       { load("idea.yaml") }

pub fn save_vision(v: &Vision) -> Result<(), StorageError>             { save("vision.yaml", v) }
pub fn load_vision() -> Result<Vision, StorageError>                   { load("vision.yaml") }

pub fn save_requirements(r: &Requirements) -> Result<(), StorageError> { save("requirements.yaml", r) }
pub fn load_requirements() -> Result<Requirements, StorageError>       { load("requirements.yaml") }

pub fn save_domain(d: &DomainModel) -> Result<(), StorageError>        { save("domain.yaml", d) }
pub fn load_domain() -> Result<DomainModel, StorageError>              { load("domain.yaml") }

pub fn save_delivery_intents(di: &DeliveryIntents) -> Result<(), StorageError>         { save("delivery_intents.yaml", di) }
pub fn load_delivery_intents() -> Result<DeliveryIntents, StorageError>               { load("delivery_intents.yaml") }

pub fn save_architecture_principles(ap: &ArchitecturePrinciples) -> Result<(), StorageError> { save("architecture_principles.yaml", ap) }
pub fn load_architecture_principles() -> Result<ArchitecturePrinciples, StorageError>        { load("architecture_principles.yaml") }

pub fn save_component_architecture(a: &ComponentArchitecture) -> Result<(), StorageError> { save("component_architecture.yaml", a) }
pub fn load_component_architecture() -> Result<ComponentArchitecture, StorageError>       { load("component_architecture.yaml") }

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
/// doesn't exist yet — it is always optional and built incrementally via `canopy plan`.
/// When Roots is integrated, callers should prefer Roots over this file.
pub fn load_domain_registry() -> Result<DomainRegistry, StorageError> {
    match load::<DomainRegistry>("domain_registry.yaml") {
        Ok(r) => Ok(r),
        Err(StorageError::NotFound(_)) => Ok(DomainRegistry::default()),
        Err(e) => Err(e),
    }
}

pub fn save_domain_registry(r: &DomainRegistry) -> Result<(), StorageError> {
    save("domain_registry.yaml", r)
}

pub fn save_intent_spec(slug: &str, spec: &IntentSpec) -> Result<(), StorageError> {
    save(&format!("plans/{}/spec.yaml", slug), spec)
}

pub fn load_intent_spec(slug: &str) -> Result<IntentSpec, StorageError> {
    load(&format!("plans/{}/spec.yaml", slug))
}

pub fn save_implementation_plan(slug: &str, plan: &ImplementationPlan) -> Result<(), StorageError> {
    save(&format!("plans/{}/plan.yaml", slug), plan)
}

pub fn load_implementation_plan(slug: &str) -> Result<ImplementationPlan, StorageError> {
    load(&format!("plans/{}/plan.yaml", slug))
}

pub fn list_plans() -> Result<Vec<String>, StorageError> {
    let dir = storage_dir().join("plans");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut slugs: Vec<String> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    slugs.sort();
    Ok(slugs)
}

pub fn save_scaffold_plan(plan: &ScaffoldPlan) -> Result<(), StorageError> {
    save("scaffold.yaml", plan)
}

pub fn load_scaffold_plan() -> Result<ScaffoldPlan, StorageError> {
    load("scaffold.yaml")
}

pub fn save_user_stories(s: &UserStories) -> Result<(), StorageError> { save("stories.yaml", s) }
pub fn load_user_stories() -> Result<UserStories, StorageError> {
    match load::<UserStories>("stories.yaml") {
        Ok(s) => Ok(s),
        Err(StorageError::NotFound(_)) => Ok(UserStories::default()),
        Err(e) => Err(e),
    }
}

pub fn save_roles_registry(r: &RolesRegistry) -> Result<(), StorageError> { save("roles.yaml", r) }
pub fn load_roles_registry() -> Result<RolesRegistry, StorageError> {
    match load::<RolesRegistry>("roles.yaml") {
        Ok(r) => Ok(r),
        Err(StorageError::NotFound(_)) => Ok(RolesRegistry::default()),
        Err(e) => Err(e),
    }
}

pub fn save_services_registry(r: &ServicesRegistry) -> Result<(), StorageError> { save("services.yaml", r) }
pub fn load_services_registry() -> Result<ServicesRegistry, StorageError> {
    match load::<ServicesRegistry>("services.yaml") {
        Ok(r) => Ok(r),
        Err(StorageError::NotFound(_)) => Ok(ServicesRegistry::default()),
        Err(e) => Err(e),
    }
}

pub fn save_story_spec(story_id: &str, spec: &IntentSpec) -> Result<(), StorageError> {
    save(&format!("stories/{}/spec.yaml", story_id), spec)
}

pub fn load_story_spec(story_id: &str) -> Result<IntentSpec, StorageError> {
    load(&format!("stories/{}/spec.yaml", story_id))
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

pub fn save_validation_report(slug: &str, report: &ValidationReport) -> Result<(), StorageError> {
    save(&format!("plans/{}/validation.yaml", slug), report)
}

pub fn load_validation_report(slug: &str) -> Result<ValidationReport, StorageError> {
    load(&format!("plans/{}/validation.yaml", slug))
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
    fn list_plans_empty_when_no_plans() {
        with_tmpdir(|| {
            let plans = list_plans().unwrap();
            assert!(plans.is_empty());
        });
    }

    #[test]
    fn save_load_implementation_plan_round_trip() {
        with_tmpdir(|| {
            use canopy_core::{DomainScope, ImplementationPlan, ImplementationTask};
            let plan = ImplementationPlan {
                intent_ref: "User Authentication".into(),
                intent_index: 0,
                generated_at: "1750000000".into(),
                status: "draft".into(),
                depends_on_intents: vec![],
                domain_scope: DomainScope {
                    entities: vec!["User".into()],
                    events: vec![],
                    relationships: vec![],
                },
                tasks: vec![ImplementationTask {
                    id: "task-001".into(),
                    title: "Define User schema".into(),
                    task_type: "schema".into(),
                    inputs: vec![],
                    outputs: vec!["migrations/001.sql".into()],
                    acceptance_criteria_refs: vec!["auth-001".into()],
                    estimated_complexity: "low".into(),
                    blocking: true,
                    completed: false,
                }],
                reasoning: vec!["Schema blocks all other tasks".into()],
                open_questions: vec![],
            };
            let slug = intent_slug("User Authentication");
            save_implementation_plan(&slug, &plan).unwrap();
            let loaded = load_implementation_plan(&slug).unwrap();
            assert_eq!(plan.intent_ref, loaded.intent_ref);
            assert_eq!(plan.tasks.len(), loaded.tasks.len());
            assert_eq!(plan.tasks[0].blocking, loaded.tasks[0].blocking);

            let plans = list_plans().unwrap();
            assert_eq!(plans.len(), 1);
            assert_eq!(plans[0], slug);
        });
    }

    #[test]
    fn save_load_intent_spec_round_trip() {
        with_tmpdir(|| {
            use canopy_core::{IntentSpec, Scenario};
            let spec = IntentSpec {
                intent_ref: "User Authentication".into(),
                entity_schema: None,
                scenarios: vec![Scenario {
                    id: "auth-001".into(),
                    name: "Successful login".into(),
                    given: vec!["A registered User exists".into()],
                    when: "User submits credentials".into(),
                    then: vec!["Session token returned".into()],
                    constraints: vec![],
                }],
                out_of_scope: vec!["OAuth".into()],
                open_questions: vec![],
            };
            let slug = intent_slug("User Authentication");
            save_intent_spec(&slug, &spec).unwrap();
            let loaded = load_intent_spec(&slug).unwrap();
            assert_eq!(spec.intent_ref, loaded.intent_ref);
            assert_eq!(spec.scenarios[0].id, loaded.scenarios[0].id);
        });
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
