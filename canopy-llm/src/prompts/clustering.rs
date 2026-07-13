//! Stage 3 of the behavior-first planning pipeline (docs/design/behavior-first-planning.md):
//! Mechanical Clustering. Unit behaviors are grouped by `(subject, kind)`; integration behaviors
//! by `subject` alone — both purely mechanical, no LLM involved. A single bounded LLM call then
//! reviews the precomputed baseline (never generating a grouping from scratch) for cohesion,
//! mis-tagging, merge candidates, and cross-layer dependencies. A human gate follows: accept the
//! baseline as-is, or edit `clusters.yaml` directly per the review's findings.

use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
use canopy_core::*;

/// Groups behaviors purely by their Stage 1 tags — no LLM, no judgment. Unit behaviors group by
/// `(subject, kind)`; integration behaviors group by `subject` alone, since an integration
/// behavior's `kind` names which observable effect it is (persistence, orchestration, http), not
/// a separate grouping axis — the workflow named by `subject` is the natural integration-test
/// boundary (see Stage 1's `scope=integration` note in the design doc).
pub fn mechanical_cluster(story_id: &str, behaviors: &BehaviorList) -> ClusteringResult {
    let mut unit_clusters: Vec<UnitCluster> = Vec::new();
    let mut integration_groupings: Vec<IntegrationGrouping> = Vec::new();
    let mut cluster_counter = 0usize;
    let mut group_counter = 0usize;

    for b in &behaviors.behaviors {
        match b.scope {
            BehaviorScope::Unit => {
                if let Some(c) = unit_clusters.iter_mut().find(|c| c.subject == b.subject && c.kind == b.kind) {
                    c.behavior_ids.push(b.id.clone());
                } else {
                    cluster_counter += 1;
                    unit_clusters.push(UnitCluster {
                        id: format!("{story_id}-cluster-{:03}", cluster_counter),
                        subject: b.subject.clone(),
                        kind: b.kind.clone(),
                        behavior_ids: vec![b.id.clone()],
                    });
                }
            }
            BehaviorScope::Integration => {
                if let Some(g) = integration_groupings.iter_mut().find(|g| g.subject == b.subject) {
                    g.behavior_ids.push(b.id.clone());
                } else {
                    group_counter += 1;
                    integration_groupings.push(IntegrationGrouping {
                        id: format!("{story_id}-group-{:03}", group_counter),
                        subject: b.subject.clone(),
                        behavior_ids: vec![b.id.clone()],
                    });
                }
            }
        }
    }

    ClusteringResult { unit_clusters, integration_groupings }
}

/// Stage 3's own mechanical audit, same shape as Stage 0/1/2's: does every behavior land in
/// exactly one cluster or grouping matching its own scope? Always expected to pass given
/// `mechanical_cluster` above places every behavior somewhere — computed anyway, as the safety
/// net for a future scope-handling bug, matching this pipeline's audit-after-generation pattern.
pub fn audit_clustering(behaviors: &BehaviorList, clustering: &ClusteringResult) -> ClusteringAudit {
    let mut findings = Vec::new();
    for b in &behaviors.behaviors {
        let covered = match b.scope {
            BehaviorScope::Unit => clustering.unit_clusters.iter().any(|c| c.behavior_ids.contains(&b.id)),
            BehaviorScope::Integration => clustering.integration_groupings.iter().any(|g| g.behavior_ids.contains(&b.id)),
        };
        if !covered {
            findings.push(ClusteringAuditFinding {
                description: format!("Behavior '{}' ({}) is not assigned to any cluster or grouping.", b.id, b.statement),
            });
        }
    }
    ClusteringAudit { findings }
}

fn cluster_review_prompt(behaviors: &BehaviorList, clustering: &ClusteringResult) -> String {
    let by_id: std::collections::HashMap<&str, &Behavior> =
        behaviors.behaviors.iter().map(|b| (b.id.as_str(), b)).collect();

    let render_members = |ids: &[String]| -> String {
        ids.iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .map(|b| format!("    - {}", b.statement))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut sections: Vec<String> = Vec::new();

    if !clustering.unit_clusters.is_empty() {
        let unit_list = clustering.unit_clusters.iter().enumerate()
            .map(|(i, c)| format!(
                "{}. id={}, subject={}, kind={}\n{}",
                i + 1, c.id, c.subject, c.kind.label(), render_members(&c.behavior_ids)
            ))
            .collect::<Vec<_>>().join("\n");
        sections.push(format!(
            "Unit clusters (grouped by subject+kind):\n{unit_list}\n\n\
Kind definitions — a behavior's statement must stay within its own kind's scope:\n\
- validation: accepting or rejecting a field value against a rule. Never mentions persistence, \
HTTP status/response, or event publication.\n\
- construction: assigning a field on a newly constructed instance (e.g. id, timestamps). Never \
mentions persistence, HTTP status/response, or event publication.\n\
- event-shape: an event's own payload fields. Never mentions topic/broker routing, persistence, \
or HTTP status/response.\n\
- publication: which topic/broker an event is published to. Never mentions the event's own \
payload fields, persistence, or HTTP status/response.\n\n\
For EACH unit cluster above, ONE AT A TIME: does any behavior's statement mention something \
outside its cluster's kind, per the definitions above? Flag it by name, quoting the exact phrase \
that's out of scope."
        ));
        if clustering.unit_clusters.len() > 1 {
            sections.push(
                "Then, across ALL unit clusters together: are there two clusters with the SAME \
kind whose subjects describe the same real responsibility and should merge? Name both cluster \
ids. NEVER propose merging two clusters with DIFFERENT kinds — construction and event-shape, \
for example, are always separate concerns by design, no matter how related their subjects \
look.".to_string()
            );
        }
    }

    if !clustering.integration_groupings.is_empty() {
        let integration_list = clustering.integration_groupings.iter().enumerate()
            .map(|(i, g)| format!(
                "{}. id={}, subject={}\n{}",
                i + 1, g.id, g.subject, render_members(&g.behavior_ids)
            ))
            .collect::<Vec<_>>().join("\n");
        sections.push(format!(
            "Integration groupings (grouped by subject) — reference only, do not propose \
changes to these:\n{integration_list}"
        ));
    }

    format!(
        r#"You are reviewing a MECHANICALLY pre-computed grouping of behaviors into clusters. You
are NOT inventing a new grouping — it already exists. Your only job is to flag real problems
with it.

{sections}

NEVER invent a finding just to have something to report — an empty `findings` list is a
correct, expected result when the baseline is sound.

Return ONLY valid YAML — no prose, no code fences:

findings:
  - description: "<one concrete, specific finding, naming the cluster id(s) involved>"
"#,
        sections = sections.join("\n\n"),
    )
}

/// Bounded LLM review of the mechanical baseline above — reviews, never regenerates. Skipped
/// entirely if there's nothing to cluster yet.
pub fn review_clustering(
    client: &LlmClient,
    behaviors: &BehaviorList,
    clustering: &ClusteringResult,
) -> Result<ClusterReview, LlmError> {
    if clustering.unit_clusters.is_empty() && clustering.integration_groupings.is_empty() {
        return Ok(ClusterReview::default());
    }
    let raw = client.complete_large(&cluster_review_prompt(behaviors, clustering))?;
    let stripped = strip_code_fence(&raw);
    let findings = parse_lenient_sequence::<ClusterReviewFinding>(&stripped, "findings")?;
    Ok(ClusterReview { findings })
}
