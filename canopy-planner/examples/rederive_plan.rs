//! Re-derives a stored plan from the raw draft the model wrote, under the harness as it is now: the model's
//! output is unchanged, so only what the harness derives (create or modify, read-only files, context) can differ.
//! Writes `plan.rederived.yaml` beside the plan and says which steps' read-only files changed.
//!
//! usage: rederive_plan <planner run dir>...      (a run dir holds plan.yaml, attempt-N.json and repo/)
use std::path::Path;

use canopy_implementer::plan::Plan;
use canopy_planner::draft::into_plan;
use canopy_planner::facts::RepoFacts;

fn main() {
    for dir in std::env::args().skip(1) {
        let dir = Path::new(&dir);
        match rederive(dir) {
            Ok(note) => println!("{}: {note}", dir.display()),
            Err(e) => println!("{}: not re-derived: {e}", dir.display()),
        }
    }
}

fn rederive(dir: &Path) -> Result<String, String> {
    let original = Plan::from_yaml(&std::fs::read_to_string(dir.join("plan.yaml")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    // The attempt that became the plan: the last one with no problems.
    let raw = (1..=2)
        .rev()
        .find_map(|n| {
            let problems = std::fs::read_to_string(dir.join(format!("attempt-{n}.problems.json"))).ok()?;
            let none = serde_json::from_str::<Vec<serde_json::Value>>(&problems)
                .ok()?
                .is_empty();
            none.then(|| std::fs::read_to_string(dir.join(format!("attempt-{n}.json"))).ok())
                .flatten()
        })
        .ok_or("no valid attempt found")?;
    let root = dir.join("repo");
    let facts = RepoFacts::collect(&root).map_err(|e| e.to_string())?;
    let route = |classes: &[String]| {
        let layers = facts.layers_of_classes(&root, classes);
        facts.render_for_step(&layers)
    };
    let plan = into_plan(
        &raw,
        &original.id,
        original.story.as_deref().unwrap_or_default(),
        original.feature.as_deref(),
        &route,
        &facts.root_package,
        &facts.files,
    )?;
    std::fs::write(dir.join("plan.rederived.yaml"), plan.to_yaml()).map_err(|e| e.to_string())?;
    let changed: Vec<String> = plan
        .steps
        .iter()
        .zip(&original.steps)
        .filter(|(new, old)| new.scope.read_only != old.scope.read_only)
        .map(|(new, old)| {
            format!(
                "{} ({} -> {} read-only files)",
                new.id,
                old.scope.read_only.len(),
                new.scope.read_only.len()
            )
        })
        .collect();
    Ok(format!(
        "{} steps; read-only changed in {}: {}",
        plan.steps.len(),
        changed.len(),
        changed.join(", ")
    ))
}
