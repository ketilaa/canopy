use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod adr_merge;

mod named_described;
pub use named_described::{Described, DomainEntity, DomainEvent, Role};

mod tech_family;
pub use tech_family::TechFamily;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adr {
    pub title: String,
    pub decision: String,
    pub reason: String,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub given: Vec<String>,
    pub when: String,
    pub then: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldValidation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<FieldValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub entity: String,
    #[serde(default)]
    pub system_generated: Vec<FieldDef>,
    #[serde(default)]
    pub mandatory: Vec<FieldDef>,
    #[serde(default)]
    pub optional: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSpec {
    pub intent_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_schema: Option<EntitySchema>,
    pub scenarios: Vec<Scenario>,
    #[serde(default)]
    pub out_of_scope: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

/// Stage 0 of the behavior-first planning pipeline (see docs/design/behavior-first-planning.md):
/// validates that a story's specification (entity schema, scenarios, open questions) is complete
/// enough to safely decompose into behaviors, before any behavior extraction begins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// An entity-schema field's validation constraint (max_length, min_items, etc.) has no
    /// scenario testing what happens when it's violated.
    MissingScenario,
    /// A scenario's `then` clause doesn't state an observable, checkable outcome.
    AmbiguousOutcome,
    /// An entry in `open_questions` has no accepted ADR or scenario resolving it.
    UnresolvedQuestion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GapSeverity {
    /// A false negative here causes missing requirements, incorrect contracts, or
    /// implementation defects downstream — blocking by default.
    Gap,
    /// A false positive here costs a human a moment's review, not a downstream defect —
    /// non-blocking by default, surfaced for a glance rather than gated on.
    Review,
}

impl GapKind {
    /// Severity is a deterministic property of the KIND of gap, not something to ask the model
    /// to judge per instance — live-verified 2026-07-13: a missing constraint-coverage scenario
    /// (MissingScenario) is unconditionally higher-stakes than a debatable "is this outcome
    /// observable enough" call (AmbiguousOutcome), so the distinction is computed here rather
    /// than requested as LLM output — one less degree of freedom the model can get wrong.
    pub fn severity(&self) -> GapSeverity {
        match self {
            GapKind::MissingScenario | GapKind::UnresolvedQuestion => GapSeverity::Gap,
            GapKind::AmbiguousOutcome => GapSeverity::Review,
        }
    }
}

/// Stage 1 (Behavior Extraction) — see docs/design/behavior-first-planning.md. A behavior is
/// atomic, independently-testable, and carries no file/layer/component name — only what must be
/// true, observably. `scope`/`subject`/`kind` are assigned here, while each behavior's origin is
/// still known, so Stage 3 (clustering) can group mechanically instead of inferring structure
/// from an untagged list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorScope {
    /// Verifiable by testing one component in isolation.
    Unit,
    /// A property of the assembled system spanning multiple components — cannot be observed
    /// from inside any single unit's own test.
    Integration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum BehaviorKind {
    Validation,
    Construction,
    Persistence,
    EventShape,
    Publication,
    Orchestration,
    HttpRequest,
    HttpResponse,
    ErrorTranslation,
}

/// Which specification artifact a behavior was derived from — not free text, so Stage 3+ can
/// trace a behavior back to its origin without re-reading the original specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorSource {
    EntitySchema,
    Scenario,
    Openapi,
    Adr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Behavior {
    pub id: String,
    pub source: BehaviorSource,
    /// Precise reference within `source` — e.g. "Product.name.max_length" or "product-001-01".
    /// Not prose: this is what lets Stage 3 cluster without re-reading the original spec.
    pub source_ref: String,
    pub scope: BehaviorScope,
    pub subject: String,
    pub kind: BehaviorKind,
    pub statement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BehaviorList {
    #[serde(default)]
    pub behaviors: Vec<Behavior>,
}

/// Derived view, not separately authored data — every source_ref that produced at least one
/// behavior, mapped to the behavior ids it produced. This is Stage 1's own completeness audit
/// (mirroring Stage 0's `SpecificationCompleteness`): a human can answer "did every source
/// artifact produce something" by reading this file, without re-opening the entity schema,
/// scenarios, or ADRs it was derived from.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BehaviorCoverage {
    #[serde(default)]
    pub coverage: std::collections::BTreeMap<String, Vec<String>>,
}

impl BehaviorList {
    pub fn coverage(&self) -> BehaviorCoverage {
        let mut coverage: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        for b in &self.behaviors {
            coverage.entry(b.source_ref.clone()).or_default().push(b.id.clone());
        }
        BehaviorCoverage { coverage }
    }
}

/// A candidate behavior Stage 1 could not generate unambiguously — most commonly because its
/// exact shape depends on an unresolved open question (a Decision Point candidate — see Stage 2
/// in the design doc, not yet its own tracked artifact). Recorded instead of silently picking an
/// interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedBehaviorCandidate {
    pub source: BehaviorSource,
    pub source_ref: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BehaviorGaps {
    #[serde(default)]
    pub blocked: Vec<BlockedBehaviorCandidate>,
}

/// Stage 2 (Decision Extraction and Gating) — see docs/design/behavior-first-planning.md.
/// Distinguishes what a human needs to actively decide (Business) from what's usually already
/// an ADR concern (Technical) from softer, non-blocking wording/ordering calls
/// (BehavioralAmbiguity) — so a reviewer can triage a decision list at a glance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionCategory {
    Business,
    Technical,
    BehavioralAmbiguity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Pending,
    Resolved,
    /// A stated option accepted as a temporary assumption rather than a considered decision —
    /// tracked the same as a real resolution, not silently assumed (see the design doc's Stage 2
    /// section: resolution isn't limited to "answer it").
    AcceptedAssumption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPoint {
    pub id: String,
    pub question: String,
    pub category: DecisionCategory,
    #[serde(default)]
    pub options: Vec<String>,
    pub status: DecisionStatus,
    /// The chosen (or accepted-as-assumption) option text — set once `status` leaves `Pending`.
    #[serde(default)]
    pub resolution: Option<String>,
    /// Behavior/blocked-candidate source_refs that depend on this decision — computed by
    /// reverse-indexing the linking step, not authored directly.
    #[serde(default)]
    pub affects_behaviors: Vec<String>,
    /// Non-authoritative hint at which future contracts (Stage 4) are likely affected — Stage 3
    /// clustering doesn't exist yet, so this can only ever be a guess, not a real reference.
    #[serde(default)]
    pub affects_future_contracts: Vec<String>,
}

impl DecisionPoint {
    pub fn is_blocking(&self) -> bool {
        self.status == DecisionStatus::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionLog {
    #[serde(default)]
    pub decisions: Vec<DecisionPoint>,
}

impl DecisionLog {
    pub fn has_pending_decisions(&self) -> bool {
        self.decisions.iter().any(|d| d.is_blocking())
    }
}

/// Stage 2's own completeness audit, same shape as Stage 0/1's — see docs/design/
/// behavior-first-planning.md's Audits A/B/C. Computed mechanically from `DecisionLog` +
/// `BehaviorGaps`, not authored or asked of an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAuditFinding {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionAudit {
    #[serde(default)]
    pub findings: Vec<DecisionAuditFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletenessGap {
    pub kind: GapKind,
    /// Specific and concrete — names the field, scenario, or question this gap concerns.
    pub description: String,
}

impl CompletenessGap {
    pub fn severity(&self) -> GapSeverity {
        self.kind.severity()
    }

    pub fn is_blocking(&self) -> bool {
        self.severity() == GapSeverity::Gap
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpecificationCompleteness {
    #[serde(default)]
    pub gaps: Vec<CompletenessGap>,
}

impl SpecificationCompleteness {
    pub fn has_blocking_gaps(&self) -> bool {
        self.gaps.iter().any(|g| g.is_blocking())
    }
}

/// Accumulated entity and event vocabulary across all planned delivery intents.
/// Built incrementally by `canopy intent` — no upfront global modeling required.
/// In repository mode, Roots is the authoritative source and supersedes this file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainRegistry {
    #[serde(default)]
    pub entities: Vec<DomainEntity>,
    #[serde(default)]
    pub events: Vec<DomainEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldCommand {
    pub label: String,
    pub command: String,
    pub working_dir: String,
    pub creates: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldPlan {
    #[serde(default)]
    pub generated_at: String,
    pub commands: Vec<ScaffoldCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StoryStatus {
    #[default]
    Draft,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStory {
    pub id: String,
    pub as_a: String,
    pub want: String,
    pub so_that: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub status: StoryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserStories {
    pub stories: Vec<UserStory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RolesRegistry {
    #[serde(default)]
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceEntry {
    pub name: String,
    #[serde(default)]
    pub responsibilities: Vec<String>,
    /// Technology stack decided via ADR (e.g. "Spring Boot 4.1.0", "Angular", "React + Vite")
    #[serde(default)]
    pub technology: Option<String>,
    /// "frontend" | "service" — drives scaffold working directory
    #[serde(default)]
    pub component_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServicesRegistry {
    #[serde(default)]
    pub services: Vec<ServiceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedAdr {
    pub question: String,
    pub title: String,
    pub decision: String,
    pub reason: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub service_responsibilities: Vec<String>,
    /// For tech-stack ADRs: the canonical technology identifier used for scaffold dispatch
    #[serde(default)]
    pub technology: Option<String>,
    /// For tech-stack ADRs: "frontend" | "service"
    #[serde(default)]
    pub component_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProposedAdrs {
    #[serde(default)]
    pub proposals: Vec<ProposedAdr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanopyConfig {
    pub default: Option<AgentLlmConfig>,
    pub agents: Option<HashMap<String, AgentLlmConfig>>,
}

impl CanopyConfig {
    pub fn for_agent(&self, agent: &str) -> Option<AgentLlmConfig> {
        self.agents
            .as_ref()
            .and_then(|m| m.get(agent))
            .or_else(|| self.default.as_ref())
            .cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    #[default]
    Pending,
    Done,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationStep {
    pub id: String,
    pub service: String,
    pub file: String,
    pub operation: String,
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_string_or_seq")]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub status: StepStatus,
}

fn deserialize_string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{SeqAccess, Visitor};
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a sequence or empty-list string")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            let t = v.trim();
            if t == "[]" || t.is_empty() { Ok(vec![]) } else { Ok(vec![t.to_string()]) }
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element()? { out.push(s); }
            Ok(out)
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Vec<String>, E> { Ok(vec![]) }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Vec<String>, E> { Ok(vec![]) }
    }
    deserializer.deserialize_any(V)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryPlan {
    pub story_id: String,
    #[serde(default)]
    pub steps: Vec<ImplementationStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedDependency {
    pub package: String,
    pub justification: String,
    pub alternatives: String,
    #[serde(default)]
    pub dev: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDecision {
    pub story_id: String,
    pub service: String,
    pub package: String,
    /// "accepted" or "rejected"
    pub decision: String,
    pub justification: String,
    pub alternatives: String,
    pub dev: bool,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependencyDecisionLog {
    #[serde(default)]
    pub decisions: Vec<DependencyDecision>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adr_yaml_round_trip() {
        let adr = Adr {
            title: "Use PostgreSQL".into(),
            decision: "PostgreSQL as primary database".into(),
            reason: "Relational model fits domain".into(),
            alternatives: vec!["MongoDB".into()],
        };
        let yaml = serde_yaml::to_string(&adr).unwrap();
        let adr2: Adr = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(adr.title, adr2.title);
        assert_eq!(adr.alternatives, adr2.alternatives);
    }

    #[test]
    fn canopy_config_yaml_round_trip() {
        let yaml = r#"
default:
  provider: ollama
  model: qwen2.5:32b
agents:
  intent:
    provider: anthropic
    model: claude-sonnet-4-6
"#;
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.default.as_ref().unwrap().provider, LlmProvider::Ollama);
        assert_eq!(cfg.default.as_ref().unwrap().model, "qwen2.5:32b");
        let explorer = cfg.agents.as_ref().unwrap().get("intent").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Anthropic);
    }

    #[test]
    fn canopy_config_for_agent_falls_back_to_default() {
        let cfg: CanopyConfig = serde_yaml::from_str(
            "default:\n  provider: ollama\n  model: qwen2.5:32b\n"
        ).unwrap();
        let resolved = cfg.for_agent("intent").unwrap();
        assert_eq!(resolved.provider, LlmProvider::Ollama);
        assert_eq!(resolved.model, "qwen2.5:32b");
    }

    #[test]
    fn canopy_config_for_agent_prefers_specific_over_default() {
        let yaml = r#"
default:
  provider: ollama
  model: qwen2.5:32b
agents:
  intent:
    provider: anthropic
    model: claude-haiku-4-5-20251001
"#;
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let resolved = cfg.for_agent("intent").unwrap();
        assert_eq!(resolved.provider, LlmProvider::Anthropic);
        assert_eq!(resolved.model, "claude-haiku-4-5-20251001");
    }

    #[test]
    fn canopy_config_for_agent_returns_none_when_no_match() {
        let cfg = CanopyConfig { default: None, agents: None };
        assert!(cfg.for_agent("intent").is_none());
    }

    #[test]
    fn canopy_config_full_with_base_url_parses() {
        let yaml = "default:\n  provider: ollama\n  model: \"qwen2.5:32b\"\n\nagents:\n  intent:\n    provider: ollama\n    model: \"qwen2.5:32b\"\n    base_url: \"http://localhost:11434\"\n";
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let explorer = cfg.for_agent("intent").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Ollama);
        assert_eq!(explorer.model, "qwen2.5:32b");
        assert_eq!(explorer.base_url.unwrap(), "http://localhost:11434");
    }

    #[test]
    fn intent_spec_yaml_round_trip() {
        let spec = IntentSpec {
            intent_ref: "User Authentication".into(),
            entity_schema: None,
            scenarios: vec![Scenario {
                id: "auth-001".into(),
                name: "Successful login".into(),
                given: vec!["A registered User exists".into()],
                when: "The user submits valid credentials".into(),
                then: vec!["A Session token is returned".into()],
                constraints: vec!["Response under 300ms at p99".into()],
            }],
            out_of_scope: vec!["OAuth/SSO".into()],
            open_questions: vec!["Is email case-sensitive?".into()],
        };
        let yaml = serde_yaml::to_string(&spec).unwrap();
        let spec2: IntentSpec = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(spec.intent_ref, spec2.intent_ref);
        assert_eq!(spec.scenarios.len(), spec2.scenarios.len());
        assert_eq!(spec.scenarios[0].constraints, spec2.scenarios[0].constraints);
        assert_eq!(spec.out_of_scope, spec2.out_of_scope);
    }

    #[test]
    fn scaffold_plan_yaml_round_trip() {
        let plan = ScaffoldPlan {
            generated_at: "1750000000".into(),
            commands: vec![ScaffoldCommand {
                label: "storefront (Next.js)".into(),
                command: "npx create-next-app@latest storefront --typescript --tailwind --app".into(),
                working_dir: ".".into(),
                creates: "storefront/".into(),
            }],
        };
        let yaml = serde_yaml::to_string(&plan).unwrap();
        let plan2: ScaffoldPlan = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(plan.commands.len(), plan2.commands.len());
        assert_eq!(plan.commands[0].label, plan2.commands[0].label);
        assert_eq!(plan.commands[0].creates, plan2.commands[0].creates);
    }

    #[test]
    fn canopy_config_full_unquoted_parses() {
        let yaml = "default:\n  provider: ollama\n  model: qwen2.5:32b\n\nagents:\n  intent:\n    provider: ollama\n    model: qwen2.5:32b\n    base_url: http://localhost:11434\n";
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let explorer = cfg.for_agent("intent").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Ollama);
        assert_eq!(explorer.model, "qwen2.5:32b");
    }
}
