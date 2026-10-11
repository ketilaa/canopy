//! Replays the redundancy audit over the stored plans and, where a plan was run by the Implementer, against how
//! its steps fared. Pre-registered in docs/design/planner-first-increment.md:
//!   - it would reject at most 40% of the valid plans;
//!   - of the executed steps refused with "must change in this step", at least 70% are flagged;
//!   - of the flagged steps that were executed, at least 50% hit that refusal or failed.
//!
//! usage: plan_replay <planner runs dir> [<planner runs dir>...]
use std::collections::BTreeSet;
use std::path::Path;

use canopy_implementer::plan::Plan;
use canopy_implementer::report::RunReport;
use canopy_planner::verify::{redundant_steps, Basis};

/// A step whose `modify` lists an existing test that is not a test of any production class the step lists: the
/// step is forced to change a test it has no reason to touch (found in the first replay, after the pre-registered
/// audit failed; fitted to these plans, so it needs fresh plans to confirm).
fn unrelated_test(step: &canopy_implementer::plan::PlanStep) -> bool {
    let stem = |p: &String| p.rsplit('/').next().unwrap_or(p).trim_end_matches(".java").to_string();
    let production: Vec<String> = step
        .scope
        .create
        .iter()
        .chain(&step.scope.modify)
        .filter(|p| p.starts_with("src/main/java/"))
        .map(stem)
        .collect();
    step.scope
        .modify
        .iter()
        .filter(|p| p.starts_with("src/test/java/"))
        .any(|t| !production.contains(&stem(t).trim_end_matches("Test").to_string()))
}

#[derive(Default)]
struct Tally {
    plans: usize,
    flagged_plans: usize,
    executed_steps: usize,
    must_change: usize,
    must_change_flagged: usize,
    flagged_executed: usize,
    flagged_executed_bad: usize,
}

#[derive(Default)]
struct Unrelated {
    plans: usize,
    plans_with: usize,
    executed: usize,
    executed_with: usize,
    refused: usize,
    refused_with: usize,
}

fn main() {
    let implementer_runs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../canopy-implementer/runs");
    let (mut created, mut touched) = (Tally::default(), Tally::default());
    let mut unrelated = Unrelated::default();
    let mut examples: Vec<String> = Vec::new();
    for dir in std::env::args().skip(1) {
        let mut batches: Vec<_> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        batches.sort();
        for batch in batches {
            let mut runs: Vec<_> = std::fs::read_dir(&batch)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            runs.sort();
            for run in runs {
                let Ok(text) = std::fs::read_to_string(run.join("plan.yaml")) else {
                    continue;
                };
                let Ok(plan) = Plan::from_yaml(&text) else { continue };
                // How each execution of this plan went, step by step.
                let executions: Vec<RunReport> = std::fs::read_to_string(run.join("implementer-runs.jsonl"))
                    .into_iter()
                    .flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>())
                    .filter_map(|l| serde_json::from_str::<serde_json::Value>(&l).ok())
                    .filter_map(|j| j["implementer_batch"].as_str().map(str::to_string))
                    .filter_map(|b| std::fs::read_to_string(implementer_runs.join(b).join("run-01/events.jsonl")).ok())
                    .map(|t| RunReport::from_jsonl(&t))
                    .collect();
                unrelated.plans += 1;
                if plan.steps.iter().any(unrelated_test) {
                    unrelated.plans_with += 1;
                }
                for report in &executions {
                    for step in &report.steps {
                        let Some(planned) = plan.steps.iter().find(|p| p.id == step.step_id) else {
                            continue;
                        };
                        let has = unrelated_test(planned);
                        let refused = step.refusals.keys().any(|k| k.contains("must change in this step"));
                        unrelated.executed += 1;
                        unrelated.executed_with += usize::from(has);
                        unrelated.refused += usize::from(refused);
                        unrelated.refused_with += usize::from(refused && has);
                    }
                }
                for (tally, basis) in [(&mut created, Basis::Created), (&mut touched, Basis::Touched)] {
                    let flagged: BTreeSet<String> = redundant_steps(&plan, basis)
                        .into_iter()
                        .filter_map(|i| i.step)
                        .collect();
                    tally.plans += 1;
                    if !flagged.is_empty() {
                        tally.flagged_plans += 1;
                    }
                    for report in &executions {
                        for step in &report.steps {
                            tally.executed_steps += 1;
                            let refused = step.refusals.keys().any(|k| k.contains("must change in this step"));
                            let is_flagged = flagged.contains(&step.step_id);
                            if refused {
                                tally.must_change += 1;
                                if is_flagged {
                                    tally.must_change_flagged += 1;
                                }
                            }
                            if is_flagged {
                                tally.flagged_executed += 1;
                                if refused || !step.passed {
                                    tally.flagged_executed_bad += 1;
                                }
                            }
                            if basis == Basis::Touched && refused && !is_flagged && examples.len() < 6 {
                                examples.push(format!(
                                    "{} step {} (refused, not flagged)",
                                    run.display(),
                                    step.step_id
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    let pct = |n: usize, of: usize| if of == 0 { 0.0 } else { 100.0 * n as f64 / of as f64 };
    for (name, t) in [
        ("created basis (as pre-registered)", &created),
        ("touched basis (broader)", &touched),
    ] {
        println!("== {name}");
        println!(
            "  plans: {}; the audit would reject {} ({:.0}%)",
            t.plans,
            t.flagged_plans,
            pct(t.flagged_plans, t.plans)
        );
        println!(
            "  executed steps: {}; refused with \"must change in this step\": {}",
            t.executed_steps, t.must_change
        );
        println!(
            "  of those, flagged: {} ({:.0}%)",
            t.must_change_flagged,
            pct(t.must_change_flagged, t.must_change)
        );
        println!(
            "  flagged steps that were executed: {}; of those, refused or failed: {} ({:.0}%)",
            t.flagged_executed,
            t.flagged_executed_bad,
            pct(t.flagged_executed_bad, t.flagged_executed)
        );
    }
    println!("== the pattern the misses showed: a step that must modify an existing test unrelated to its classes (post hoc)");
    println!(
        "  plans with such a step: {} of {} ({:.0}%)",
        unrelated.plans_with,
        unrelated.plans,
        pct(unrelated.plans_with, unrelated.plans)
    );
    println!(
        "  executed steps with it: {} of {} ({:.0}%); of the {} refused with \"must change in this step\", {} have it ({:.0}%)",
        unrelated.executed_with,
        unrelated.executed,
        pct(unrelated.executed_with, unrelated.executed),
        unrelated.refused,
        unrelated.refused_with,
        pct(unrelated.refused_with, unrelated.refused)
    );
    for e in &examples {
        println!("  {e}");
    }
}
