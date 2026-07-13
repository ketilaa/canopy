//! Stage 4 of the behavior-first planning pipeline (docs/design/behavior-first-planning.md):
//! Contract Generation. One contract per cluster, mechanically — never "design a component,"
//! always "given this cluster, generate a contract that owns these behaviors." Unit contracts
//! are entirely mechanical (name, owned behaviors, dependencies all derivable from the cluster
//! and its sibling clusters). Integration contracts get one bounded LLM step: a mechanical
//! dependency baseline (substring-matched against unit contract subjects) is deliberately crude,
//! so a review pass adds/removes from it — it never touches owned behaviors or invents a
//! contract from scratch.

use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
use canopy_core::*;

fn pascal_case(kebab: &str) -> String {
    kebab.split('-').map(|word| {
        let mut chars = word.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    }).collect()
}

/// One unit contract per unit cluster. Name and owned behaviors are pure derivations of the
/// cluster; dependencies use the one sound mechanical rule available at unit scope: a
/// non-construction contract depends on the construction contract for the same `subject`, when
/// one exists (you need a constructed instance before persisting or publishing it). Doesn't fire
/// for `product-001` (no unit-scope persistence/event contract shares a subject with a
/// construction cluster there), but holds for any story where it will.
fn mechanical_unit_contracts(
    clustering: &ClusteringResult,
    behaviors: &BehaviorList,
    next_id: &mut impl FnMut() -> String,
) -> Vec<Contract> {
    let by_id: std::collections::HashMap<&str, &Behavior> =
        behaviors.behaviors.iter().map(|b| (b.id.as_str(), b)).collect();

    clustering.unit_clusters.iter().map(|c| {
        let dependencies = if c.kind == BehaviorKind::Construction {
            Vec::new()
        } else {
            clustering.unit_clusters.iter()
                .filter(|other| other.subject == c.subject && other.kind == BehaviorKind::Construction)
                .map(|other| other.id.clone())
                .collect()
        };
        let required_tests = c.behavior_ids.iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .map(|b| b.statement.clone())
            .collect();
        Contract {
            id: next_id(),
            name: format!("{}{}", c.subject, pascal_case(c.kind.label())),
            scope: BehaviorScope::Unit,
            source_cluster: c.id.clone(),
            owned_behaviors: c.behavior_ids.clone(),
            required_tests,
            dependencies,
            derivation: ContractDerivation::Mechanical,
        }
    }).collect()
}

/// One integration contract per integration grouping, with a mechanical dependency BASELINE —
/// deliberately crude (case-insensitive substring match of each unit contract's `subject` against
/// the grouping's own behavior statements), corrected by `review_dependencies` below. A baseline,
/// not a final answer: e.g. "a ProductCreated event is published" matches the `ProductCreated`
/// event-shape contract by name, but never mentions "EventPublisher" literally, so the real
/// publication dependency is invisible to substring matching alone — the review step exists
/// specifically to catch cases like this, not as optional polish.
fn mechanical_integration_contract_baseline(
    clustering: &ClusteringResult,
    unit_contracts: &[Contract],
    behaviors: &BehaviorList,
    next_id: &mut impl FnMut() -> String,
) -> Vec<Contract> {
    let by_id: std::collections::HashMap<&str, &Behavior> =
        behaviors.behaviors.iter().map(|b| (b.id.as_str(), b)).collect();

    clustering.integration_groupings.iter().map(|g| {
        let statements: Vec<&str> = g.behavior_ids.iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .map(|b| b.statement.as_str())
            .collect();
        let joined = statements.join(" ").to_lowercase();
        // Match against each unit cluster's own `subject` (not the contract's derived `name`,
        // which also carries a kind suffix like "Validation" that would never appear literally
        // in a behavior statement) — then map the matched cluster to its generated contract id.
        let dependencies = clustering.unit_clusters.iter()
            .filter(|c| joined.contains(&c.subject.to_lowercase()))
            .filter_map(|c| unit_contracts.iter().find(|uc| uc.source_cluster == c.id))
            .map(|uc| uc.id.clone())
            .collect();
        let required_tests = g.behavior_ids.iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .map(|b| b.statement.clone())
            .collect();
        Contract {
            id: next_id(),
            name: format!("{}Workflow", g.subject),
            scope: BehaviorScope::Integration,
            source_cluster: g.id.clone(),
            owned_behaviors: g.behavior_ids.clone(),
            required_tests,
            dependencies,
            derivation: ContractDerivation::Mechanical,
        }
    }).collect()
}

/// `unit_contracts` is guaranteed non-empty by `review_dependencies`'s own guard below — this
/// function still renders the section conditionally anyway, matching `behaviors.rs`'s
/// `checklist_section` convention: an empty "Available unit contracts" header with nothing under
/// it, followed by an instruction to check the list for a missing entry, previously caused a
/// small model to hallucinate a finding against the placeholder rather than recognizing "nothing
/// to check" — omit the section entirely rather than render it empty.
fn dependency_review_prompt(unit_contracts: &[Contract], integration_contracts: &[Contract]) -> String {
    let integration_list = integration_contracts.iter().enumerate()
        .map(|(i, c)| {
            let behaviors = c.required_tests.iter().map(|s| format!("    - {s}")).collect::<Vec<_>>().join("\n");
            let deps = if c.dependencies.is_empty() { "none".to_string() } else { c.dependencies.join(", ") };
            format!("{}. id={}, name={}\n  owned behaviors:\n{behaviors}\n  current (mechanically-inferred) dependencies: {deps}", i + 1, c.id, c.name)
        })
        .collect::<Vec<_>>().join("\n");

    let unit_section = if unit_contracts.is_empty() {
        String::new()
    } else {
        let unit_list = unit_contracts.iter().enumerate()
            .map(|(i, c)| format!("{}. id={}, name={}", i + 1, c.id, c.name))
            .collect::<Vec<_>>().join("\n");
        format!("Available unit contracts:\n{unit_list}\n\n")
    };

    format!(
        r#"You are reviewing MECHANICALLY pre-computed dependency baselines for integration
contracts. You are NOT redesigning any contract and NOT touching owned behaviors — only checking
whether each integration contract's dependency list correctly names every unit contract its
behaviors actually exercise.

{unit_section}For EACH integration contract below, ONE AT A TIME:
1. Read its owned behaviors and its current dependency list.
2. Is any unit contract genuinely exercised by these behaviors missing from the list? Record its
   id under `add`.
3. Does the list contain a unit contract NOT actually exercised by these behaviors? Record its id
   under `remove`.

{integration_list}

NEVER invent a finding just to have something to report — empty `add`/`remove` lists are a
correct, expected result when the baseline is already right.

Return ONLY valid YAML — no prose, no code fences:

reviews:
  - contract_id: "<integration contract id>"
    add:
      - "<unit contract id missing from the baseline>"
    remove:
      - "<unit contract id that doesn't belong>"
"#,
        unit_section = unit_section,
        integration_list = integration_list,
    )
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawDependencyReview {
    contract_id: String,
    #[serde(default)]
    add: Vec<String>,
    #[serde(default)]
    remove: Vec<String>,
}

/// Bounded LLM review of the mechanical integration-dependency baseline. Applies add/remove
/// directly to each integration contract's `dependencies`, sets `derivation: Reviewed`
/// unconditionally on every integration contract that went through this call (even one with no
/// changes was still reviewed), and records what changed as findings for human visibility.
/// Skipped entirely if there are no integration contracts, or no unit contracts to reference —
/// live-verified need for the latter guard: nothing guarantees a story's clustering ever
/// produces a unit cluster (e.g. an entity schema with no mandatory-field constraints and no
/// domain-event ADRs, whose scenario-derived behaviors all land scope=integration), and reviewing
/// against an empty catalog can only ever produce a hallucinated `add` — there is no valid id to
/// find.
fn review_dependencies(
    client: &LlmClient,
    unit_contracts: &[Contract],
    integration_contracts: &mut [Contract],
) -> Result<DependencyReview, LlmError> {
    if integration_contracts.is_empty() || unit_contracts.is_empty() {
        return Ok(DependencyReview::default());
    }

    let raw = client.complete_large(&dependency_review_prompt(unit_contracts, integration_contracts))?;
    let stripped = strip_code_fence(&raw);
    let reviews = parse_lenient_sequence::<RawDependencyReview>(&stripped, "reviews")?;

    let mut findings = Vec::new();
    for review in reviews {
        let Some(contract) = integration_contracts.iter_mut().find(|c| c.id == review.contract_id) else { continue };
        for add in &review.add {
            if !contract.dependencies.contains(add) {
                contract.dependencies.push(add.clone());
                findings.push(DependencyReviewFinding {
                    description: format!("Added '{}' as a dependency of '{}' (missed by the mechanical baseline).", add, contract.name),
                });
            }
        }
        for remove in &review.remove {
            if contract.dependencies.iter().any(|d| d == remove) {
                contract.dependencies.retain(|d| d != remove);
                findings.push(DependencyReviewFinding {
                    description: format!("Removed '{}' as a dependency of '{}' (not actually exercised).", remove, contract.name),
                });
            }
        }
    }

    for contract in integration_contracts.iter_mut() {
        contract.derivation = ContractDerivation::Reviewed;
    }

    Ok(DependencyReview { findings })
}

/// Stage 4's own mechanical audit, same shape as Stage 0/1/2/3's: does every cluster/grouping
/// produce exactly one contract, does every contract own at least one behavior, and does every
/// clustered behavior appear in exactly one contract?
pub fn audit_contracts(clustering: &ClusteringResult, contracts: &ContractSet) -> ContractAudit {
    let mut findings = Vec::new();

    for c in &clustering.unit_clusters {
        let count = contracts.contracts.iter().filter(|contract| contract.source_cluster == c.id).count();
        if count != 1 {
            findings.push(ContractAuditFinding {
                description: format!("Unit cluster '{}' produced {} contract(s), expected exactly 1.", c.id, count),
            });
        }
    }
    for g in &clustering.integration_groupings {
        let count = contracts.contracts.iter().filter(|contract| contract.source_cluster == g.id).count();
        if count != 1 {
            findings.push(ContractAuditFinding {
                description: format!("Integration grouping '{}' produced {} contract(s), expected exactly 1.", g.id, count),
            });
        }
    }
    for contract in &contracts.contracts {
        if contract.owned_behaviors.is_empty() {
            findings.push(ContractAuditFinding {
                description: format!("Contract '{}' ({}) owns no behaviors.", contract.id, contract.name),
            });
        }
    }

    let mut all_clustered_behaviors: Vec<&str> = clustering.unit_clusters.iter()
        .flat_map(|c| c.behavior_ids.iter().map(String::as_str))
        .chain(clustering.integration_groupings.iter().flat_map(|g| g.behavior_ids.iter().map(String::as_str)))
        .collect();
    all_clustered_behaviors.sort_unstable();
    all_clustered_behaviors.dedup();

    for behavior_id in all_clustered_behaviors {
        let count = contracts.contracts.iter().filter(|c| c.owned_behaviors.iter().any(|id| id == behavior_id)).count();
        if count != 1 {
            findings.push(ContractAuditFinding {
                description: format!("Behavior '{behavior_id}' appears in {count} contract(s), expected exactly 1."),
            });
        }
    }

    ContractAudit { findings }
}

/// Stage 4 entry point. Generates one contract per cluster/grouping mechanically, then runs the
/// bounded dependency review over integration contracts only.
pub fn generate_contracts(
    story_id: &str,
    client: &LlmClient,
    behaviors: &BehaviorList,
    clustering: &ClusteringResult,
) -> Result<(ContractSet, DependencyReview), LlmError> {
    let story_id_owned = story_id.to_string();
    let mut counter = 0usize;
    let mut next_id = move || { counter += 1; format!("{story_id_owned}-contract-{:03}", counter) };

    let unit_contracts = mechanical_unit_contracts(clustering, behaviors, &mut next_id);
    let mut integration_contracts = mechanical_integration_contract_baseline(
        clustering, &unit_contracts, behaviors, &mut next_id,
    );

    let review = review_dependencies(client, &unit_contracts, &mut integration_contracts)?;

    let mut contracts = unit_contracts;
    contracts.extend(integration_contracts);

    Ok((ContractSet { contracts }, review))
}
