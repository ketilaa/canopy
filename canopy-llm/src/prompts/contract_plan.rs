//! Stage 4 of docs/design/contract-driven-implementation-experiment.md: replace `canopy
//! implement`'s LLM-driven file discovery with contract-driven enumeration, for a story that
//! already has a generated `contracts.yaml`. Entirely mechanical — no LLM call anywhere in this
//! file, matching this project's "compute facts mechanically" house rule.
//!
//! Deliberately narrower than the full LLM-driven planner (`plan.rs`'s `generate_story_plan`):
//! this function either produces a complete, correct plan for every unit-scope contract, or
//! returns a `String` explaining exactly why it can't — the caller (canopy-cli) is expected to
//! fall back to the LLM-driven planner on `Err`, never to silently ship an incomplete plan.
//! Known, disclosed gaps that trigger a fallback rather than a guess:
//! - More than one non-frontend service: nothing on `Contract` yet records which service owns
//!   which entity, so with more than one backend candidate there's no mechanical way to place a
//!   contract without guessing.
//! - An `HttpRequest`/`HttpResponse` contract: its abstract layer ("route") is ambiguous between
//!   a backend controller and a frontend api-client — nothing on `Contract` yet disambiguates,
//!   so this pilot does not attempt to place one.
//! - Integration-scope contracts: these describe a cross-cutting workflow, not one file: not yet
//!   handled by this mechanical enumerator at all.

use canopy_core::{BehaviorKind, BehaviorScope, Contract, ContractSet, ImplementationStep, ServiceEntry, ServicesRegistry, StepStatus, StoryPlan};
use crate::skills::{abstract_layer_for_kind, resolve_implementation_target};
use super::plan::{frontend_tier, layer_weight};
use std::collections::{BTreeMap, HashMap};

struct Placed<'a> {
    contract: &'a Contract,
    service_name: String,
    target: String,
}

fn verb_for_kind(kind: &BehaviorKind) -> &'static str {
    match kind {
        BehaviorKind::Validation => "validates",
        BehaviorKind::Construction => "constructs",
        BehaviorKind::Persistence => "persists",
        BehaviorKind::Orchestration => "orchestrates",
        BehaviorKind::EventShape => "defines the shape of",
        BehaviorKind::Publication => "publishes",
        BehaviorKind::HttpRequest | BehaviorKind::HttpResponse => "handles",
        BehaviorKind::ErrorTranslation => "translates errors for",
    }
}

/// One mechanical sentence per file, built from the distinct kinds its owning contracts cover —
/// the same "layer verb" convention this project's own house style already names (CLAUDE.md's
/// DDD aggregate lifecycle section): "Defines" (model), "Constructs" (factory), "Persists"
/// (repository), "Orchestrates" (service). No LLM involved; the verb is a fixed lookup on `kind`.
fn describe_group(group: &[&Placed]) -> String {
    let entity = group.iter().find_map(|p| p.contract.entity.as_deref()).unwrap_or("the entity");
    let mut verbs: Vec<&str> = group.iter().filter_map(|p| p.contract.kind.as_ref()).map(verb_for_kind).collect();
    verbs.sort_unstable();
    verbs.dedup();
    let mut sentence = verbs.join(" and ");
    if let Some(first) = sentence.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    format!("{sentence} {entity}.")
}

/// Maps each contract in `group`'s own `dependencies` (other contract ids) to that dependency's
/// resolved file target — never the dependency's file *content*, only which file it lives in.
/// Excludes the group's own target (a step never depends on itself) and de-duplicates.
fn dependency_targets(group: &[&Placed], all_placed: &[Placed]) -> Vec<String> {
    let own_target = &group[0].target;
    let mut deps: Vec<String> = Vec::new();
    for p in group {
        for dep_id in &p.contract.dependencies {
            if let Some(dep) = all_placed.iter().find(|q| &q.contract.id == dep_id) {
                if &dep.target != own_target && !deps.contains(&dep.target) {
                    deps.push(dep.target.clone());
                }
            }
        }
    }
    deps
}

pub fn generate_story_plan_from_contracts(
    story_id: &str,
    contracts: &ContractSet,
    services: &ServicesRegistry,
    service_packages: &HashMap<String, String>,
    existing_files: &[String],
) -> Result<StoryPlan, String> {
    let unit_contracts: Vec<&Contract> = contracts.contracts.iter()
        .filter(|c| c.scope == BehaviorScope::Unit)
        .collect();
    if unit_contracts.is_empty() {
        return Err("no unit-scope contracts found in contracts.yaml — nothing to enumerate mechanically".to_string());
    }
    // A safety review caught this: silently covering only unit-scope contracts while an
    // integration-scope contract exists (a normal, expected output of Stage 3's clustering
    // whenever a story's behaviors include an integration grouping — not a hypothetical) would
    // contradict this function's own "complete plan or an explicit Err" contract. Integration
    // contracts describe a cross-cutting workflow, not one file, and aren't handled by this
    // enumerator at all yet — refuse rather than ship a plan that quietly omits them.
    let integration_count = contracts.contracts.iter().filter(|c| c.scope == BehaviorScope::Integration).count();
    if integration_count > 0 {
        return Err(format!(
            "{integration_count} integration-scope contract(s) exist alongside {} unit-scope \
             contract(s) — integration contracts describe a cross-cutting workflow, not one file, \
             and are not yet handled by contract-driven discovery",
            unit_contracts.len()
        ));
    }

    let active: Vec<&ServiceEntry> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .collect();
    let backend_services: Vec<&ServiceEntry> = active.iter()
        .filter(|s| s.component_type.as_deref() != Some("frontend"))
        .copied().collect();

    if backend_services.len() > 1 {
        return Err(format!(
            "{} backend services exist for this story, but no mechanical entity-to-service \
             ownership mapping exists yet on Contract — cannot place backend contracts without guessing",
            backend_services.len()
        ));
    }
    let Some(backend_service) = backend_services.first() else {
        return Err("no backend service exists to own this story's contracts".to_string());
    };
    let tech = backend_service.technology.as_deref().unwrap_or("");
    if tech.is_empty() {
        return Err(format!("service '{}' has no decided technology yet", backend_service.name));
    }
    // `resolve_implementation_target`'s `pkg_path` parameter expects a slash-separated path
    // (e.g. "com/example/manufacturer"), not the dotted Java package name a real scaffold
    // detection returns (e.g. "com.example.manufacturer") — the same conversion `plan.rs`'s own
    // LLM-driven planner already applies via `detected.replace('.', "/")`. A live safety review
    // caught this: passing the dotted form straight through (as the earlier, single-word
    // fallback-only "manufacturer_service" package always happened to make invisible, since it
    // has no dots at all) would silently produce a bogus single-directory path like
    // ".../java/com.example.manufacturer/domain/..." the first time a real JVM package is
    // detected, instead of the correct ".../java/com/example/manufacturer/domain/..." tree.
    let pkg_path = service_packages.get(&backend_service.name).cloned()
        .unwrap_or_else(|| backend_service.name.replace('-', "_"))
        .replace('.', "/");

    let mut placed: Vec<Placed> = Vec::new();
    for c in &unit_contracts {
        let Some(kind) = &c.kind else {
            return Err(format!("contract '{}' has scope=Unit but no kind — cannot resolve a layer", c.id));
        };
        if matches!(kind, BehaviorKind::HttpRequest | BehaviorKind::HttpResponse) {
            return Err(format!(
                "contract '{}' is kind={kind:?} (\"route\" layer) — ambiguous between a backend \
                 controller and a frontend api client with nothing on Contract yet to disambiguate; \
                 not yet handled by contract-driven discovery", c.id
            ));
        }
        let Some(entity) = c.entity.as_deref() else {
            return Err(format!("contract '{}' has no entity — cannot resolve a file target", c.id));
        };
        let layer = abstract_layer_for_kind(kind);
        // `Contract` carries no distinct `subject` field for the event's own name (unlike
        // `UnitCluster`/`Behavior`) — `file_targets.rs`'s own module doc still refers to a
        // "Contract.subject" that doesn't exist, a stale reference from before that field was
        // dropped. Recovered here instead of reintroducing the field: `mechanical_unit_contracts`
        // always names an event-shape contract `{subject}{"EventShape"}` (the fixed
        // `{subject}{PascalCase(kind.label())}` convention every unit contract's `name` follows),
        // so stripping that fixed suffix recovers the event's own name exactly. Without this,
        // `event_name` was always `None` below, so `resolve_implementation_target`'s "event"
        // layer could never resolve for *any* tech family, not just JVM — found and fixed
        // 2026-07-15 alongside the JVM event/infrastructure convention itself
        // (docs/design/contract-composition-assessment.md §8).
        let event_name = if matches!(kind, BehaviorKind::EventShape) {
            c.name.strip_suffix("EventShape").map(|s| s.to_string())
        } else {
            None
        };
        let Some(target) = resolve_implementation_target(tech, &pkg_path, &backend_service.name, layer, entity, event_name.as_deref()) else {
            return Err(format!(
                "no mechanical file-target convention exists yet for tech='{tech}' layer='{layer}' (contract '{}')",
                c.id
            ));
        };
        placed.push(Placed { contract: c, service_name: backend_service.name.clone(), target });
    }

    let mut by_target: BTreeMap<String, Vec<&Placed>> = BTreeMap::new();
    for p in &placed {
        by_target.entry(p.target.clone()).or_default().push(p);
    }

    let mut steps: Vec<ImplementationStep> = by_target.iter().map(|(target, group)| {
        let operation = if existing_files.iter().any(|f| f == target) { "modify" } else { "create" };
        ImplementationStep {
            id: String::new(),
            service: group[0].service_name.clone(),
            file: target.clone(),
            operation: operation.to_string(),
            description: describe_group(group),
            depends_on: dependency_targets(group, &placed),
            status: StepStatus::Pending,
        }
    }).collect();

    // Same ordering convention the LLM-driven planner uses (plan.rs) — backend-before-frontend,
    // then per-file layer weight — reused, not reimplemented, so the two planners stay consistent.
    let is_frontend_service = |name: &str| {
        services.services.iter().find(|s| s.name == name)
            .and_then(|s| s.component_type.as_deref()).map(|t| t == "frontend").unwrap_or(false)
    };
    steps.sort_by_key(|s| {
        let is_fe = is_frontend_service(&s.service);
        let service_tier = if is_fe { 1u8 } else { 0u8 };
        let file_tier = if is_fe { frontend_tier(&s.file) } else { layer_weight(&s.file) };
        (service_tier, file_tier)
    });
    for (i, step) in steps.iter_mut().enumerate() {
        step.id = (i + 1).to_string();
    }

    Ok(StoryPlan { story_id: story_id.to_string(), steps })
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_core::ContractDerivation;

    fn contract(id: &str, kind: BehaviorKind, entity: &str, member: Option<&str>, deps: Vec<&str>) -> Contract {
        Contract {
            id: id.to_string(),
            name: format!("{entity}{kind:?}"),
            scope: BehaviorScope::Unit,
            kind: Some(kind),
            entity: Some(entity.to_string()),
            member: member.map(|m| m.to_string()),
            mandatory: None,
            source_cluster: format!("{id}-cluster"),
            owned_behaviors: vec![],
            required_tests: vec![],
            dependencies: deps.into_iter().map(String::from).collect(),
            derivation: ContractDerivation::Mechanical,
        }
    }

    fn one_backend_service() -> ServicesRegistry {
        ServicesRegistry {
            services: vec![ServiceEntry {
                name: "widget-service".to_string(),
                responsibilities: vec![],
                technology: Some("Spring Boot".to_string()),
                component_type: Some("service".to_string()),
            }],
        }
    }

    #[test]
    fn places_validation_and_construction_in_one_file_when_they_share_an_entity() {
        let contracts = ContractSet {
            contracts: vec![
                contract("c1", BehaviorKind::Validation, "Widget", Some("name"), vec![]),
                contract("c2", BehaviorKind::Construction, "Widget", None, vec![]),
            ],
        };
        let plan = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &[],
        ).expect("should produce a plan");

        assert_eq!(plan.steps.len(), 1, "both contracts resolve to the same file, so they merge into one step");
        assert_eq!(plan.steps[0].file, "services/widget-service/src/main/java/widget_service/domain/Widget.java");
        assert_eq!(plan.steps[0].operation, "create");
        assert!(plan.steps[0].description.contains("Widget"));
    }

    #[test]
    fn marks_operation_modify_when_the_target_is_already_an_existing_file() {
        let contracts = ContractSet {
            contracts: vec![contract("c1", BehaviorKind::Validation, "Widget", Some("name"), vec![])],
        };
        let existing = vec!["services/widget-service/src/main/java/widget_service/domain/Widget.java".to_string()];
        let plan = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &existing,
        ).expect("should produce a plan");
        assert_eq!(plan.steps[0].operation, "modify");
    }

    #[test]
    fn maps_a_contract_dependency_to_the_dependencys_own_resolved_target() {
        let contracts = ContractSet {
            contracts: vec![
                contract("c1", BehaviorKind::Validation, "Widget", Some("name"), vec!["c2"]),
                contract("c2", BehaviorKind::Construction, "Widget", None, vec![]),
            ],
        };
        let plan = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &[],
        ).expect("should produce a plan");
        // Validation and Construction both resolve to the "model" layer -> same file -> one
        // step, so the dependency collapses (a step never depends on its own file).
        assert_eq!(plan.steps.len(), 1);
        assert!(plan.steps[0].depends_on.is_empty());
    }

    #[test]
    fn keeps_persistence_and_orchestration_as_separate_steps_with_a_real_dependency_edge() {
        let contracts = ContractSet {
            contracts: vec![
                contract("c1", BehaviorKind::Orchestration, "Widget", None, vec!["c2"]),
                contract("c2", BehaviorKind::Persistence, "Widget", None, vec![]),
            ],
        };
        let plan = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &[],
        ).expect("should produce a plan");
        assert_eq!(plan.steps.len(), 2, "service and repository layers resolve to different files");
        let service_step = plan.steps.iter().find(|s| s.file.contains("Service")).expect("service step");
        let repo_step = plan.steps.iter().find(|s| s.file.contains("Repository")).expect("repository step");
        assert!(service_step.depends_on.contains(&repo_step.file));
    }

    #[test]
    fn errors_rather_than_guesses_when_more_than_one_backend_service_exists() {
        let services = ServicesRegistry {
            services: vec![
                ServiceEntry { name: "a".to_string(), responsibilities: vec![], technology: Some("Spring Boot".to_string()), component_type: Some("service".to_string()) },
                ServiceEntry { name: "b".to_string(), responsibilities: vec![], technology: Some("Spring Boot".to_string()), component_type: Some("service".to_string()) },
            ],
        };
        let contracts = ContractSet {
            contracts: vec![contract("c1", BehaviorKind::Validation, "Widget", Some("name"), vec![])],
        };
        let err = generate_story_plan_from_contracts("widget-001", &contracts, &services, &HashMap::new(), &[])
            .expect_err("should refuse to guess");
        assert!(err.contains("backend services"));
    }

    #[test]
    fn errors_rather_than_guesses_for_an_http_contract() {
        let contracts = ContractSet {
            contracts: vec![contract("c1", BehaviorKind::HttpResponse, "Widget", None, vec![])],
        };
        let err = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &[],
        ).expect_err("should refuse to guess");
        assert!(err.contains("ambiguous"));
    }

    #[test]
    fn errors_when_only_integration_scope_contracts_exist() {
        let mut integration = contract("c1", BehaviorKind::Persistence, "Widget", None, vec![]);
        integration.scope = BehaviorScope::Integration;
        let contracts = ContractSet { contracts: vec![integration] };
        let err = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &[],
        ).expect_err("no unit contracts at all");
        assert!(err.contains("no unit-scope contracts"));
    }

    /// Regression test for a live safety-review finding: a story with BOTH unit and integration
    /// contracts (the normal case whenever Stage 3's clustering produces an integration grouping)
    /// must refuse rather than silently produce a plan covering only the unit contracts.
    #[test]
    fn errors_rather_than_silently_dropping_integration_contracts_mixed_with_unit_ones() {
        let unit = contract("c1", BehaviorKind::Validation, "Widget", Some("name"), vec![]);
        let mut integration = contract("c2", BehaviorKind::Persistence, "WidgetRegistration", None, vec![]);
        integration.scope = BehaviorScope::Integration;
        let contracts = ContractSet { contracts: vec![unit, integration] };
        let err = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &HashMap::new(), &[],
        ).expect_err("should refuse rather than silently omit the integration contract");
        assert!(err.contains("integration-scope contract"));
    }

    /// Regression test for a live safety-review finding: a real scaffold-detected package name
    /// is dotted ("com.example.widget"), not slash-separated — resolve_implementation_target
    /// needs the slash form to build a correct directory tree, not a single bogus directory
    /// literally named "com.example.widget".
    #[test]
    fn converts_a_dotted_detected_package_to_a_slash_path() {
        let contracts = ContractSet {
            contracts: vec![contract("c1", BehaviorKind::Validation, "Widget", Some("name"), vec![])],
        };
        let mut service_packages = HashMap::new();
        service_packages.insert("widget-service".to_string(), "com.example.widget".to_string());
        let plan = generate_story_plan_from_contracts(
            "widget-001", &contracts, &one_backend_service(), &service_packages, &[],
        ).expect("should produce a plan");
        assert_eq!(
            plan.steps[0].file,
            "services/widget-service/src/main/java/com/example/widget/domain/Widget.java"
        );
    }
}
