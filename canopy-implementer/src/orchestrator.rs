//! Runs a plan: one step at a time, in order, each on top of the result of the last.
//!
//! The orchestrator knows nothing about models, files or builds. It hands each step to a
//! [`StepRunner`] and stops at the first step that does not pass, reporting what was left undone.
//! The repository itself carries the state from one step to the next.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::implementer::{StepOutcome, StepRequest};
use crate::observe::{PlanFinished, PlanStarted, RunEvent, RunObserver};
use crate::plan::Plan;

/// Anything that can carry out one step against a repository. The real one is the Implementer.
pub trait StepRunner: Send + Sync {
    fn run_step(&self, root: &Path, step: &StepRequest) -> StepOutcome;
}

#[derive(Debug, Clone, PartialEq)]
pub struct StepResult {
    pub id: String,
    pub outcome: StepOutcome,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanOutcome {
    /// The steps that were run, in order. The last one is the failure, if there was one.
    pub results: Vec<StepResult>,
    /// Ids of the steps that were never started because an earlier one failed.
    pub skipped: Vec<String>,
}

impl PlanOutcome {
    pub fn passed(&self) -> bool {
        self.skipped.is_empty()
            && self
                .results
                .iter()
                .all(|r| matches!(r.outcome, StepOutcome::Passed { .. }))
    }

    pub fn steps_passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.outcome, StepOutcome::Passed { .. }))
            .count()
    }
}

pub struct Orchestrator {
    runner: Box<dyn StepRunner>,
    observer: Arc<dyn RunObserver>,
}

impl Orchestrator {
    pub fn new(runner: Box<dyn StepRunner>, observer: Arc<dyn RunObserver>) -> Self {
        Self { runner, observer }
    }

    pub fn run(&self, root: &Path, plan: &Plan, project_context: &str) -> PlanOutcome {
        let started = Instant::now();
        let requests = plan.step_requests(project_context);
        self.observer.on(&RunEvent::PlanStarted(PlanStarted {
            plan_id: plan.id.clone(),
            step_ids: requests.iter().map(|r| r.id.clone()).collect(),
        }));

        let mut results = Vec::new();
        let mut skipped = Vec::new();
        let mut failed_step = None;
        for (index, request) in requests.iter().enumerate() {
            let outcome = self.runner.run_step(root, request);
            let passed = matches!(outcome, StepOutcome::Passed { .. });
            results.push(StepResult {
                id: request.id.clone(),
                outcome,
            });
            if !passed {
                failed_step = Some(request.id.clone());
                skipped = requests[index + 1..].iter().map(|r| r.id.clone()).collect();
                break;
            }
        }

        let outcome = PlanOutcome { results, skipped };
        self.observer.on(&RunEvent::PlanFinished(PlanFinished {
            plan_id: plan.id.clone(),
            passed: outcome.passed(),
            steps_passed: outcome.steps_passed(),
            steps_total: requests.len(),
            failed_step,
            elapsed_ms: started.elapsed().as_millis() as u64,
        }));
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::RecordingObserver;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Answers each step from a script and remembers what it was asked and what was on disk at the time.
    struct ScriptedRunner {
        outcomes: Mutex<VecDeque<StepOutcome>>,
        seen: Arc<Mutex<Vec<(String, String, String)>>>,
    }

    impl StepRunner for ScriptedRunner {
        fn run_step(&self, root: &Path, step: &StepRequest) -> StepOutcome {
            let marker = std::fs::read_to_string(root.join("marker.txt")).unwrap_or_default();
            self.seen
                .lock()
                .unwrap()
                .push((step.id.clone(), step.context.clone(), marker));
            // A passing step leaves a trace on disk that later steps can see: state carries over.
            let outcome = self.outcomes.lock().unwrap().pop_front().expect("script ran out");
            if matches!(outcome, StepOutcome::Passed { .. }) {
                let so_far = std::fs::read_to_string(root.join("marker.txt")).unwrap_or_default();
                std::fs::write(root.join("marker.txt"), format!("{so_far}{};", step.id)).unwrap();
            }
            outcome
        }
    }

    fn passed() -> StepOutcome {
        StepOutcome::Passed { attempts: 1 }
    }

    fn failed() -> StepOutcome {
        StepOutcome::Failed {
            attempts: 3,
            last_failure: "boom".into(),
        }
    }

    fn plan() -> Plan {
        Plan::from_yaml("id: p\nsteps:\n  - {id: a, instruction: do a}\n  - {id: b, instruction: do b, context: own}\n  - {id: c, instruction: do c}\n")
            .unwrap()
    }

    type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

    fn orchestrator(outcomes: Vec<StepOutcome>) -> (Orchestrator, Arc<RecordingObserver>, Seen) {
        let observer = Arc::new(RecordingObserver::new());
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let runner = ScriptedRunner {
            outcomes: Mutex::new(outcomes.into()),
            seen: seen.clone(),
        };
        (Orchestrator::new(Box::new(runner), observer.clone()), observer, seen)
    }

    #[test]
    fn every_step_runs_in_order_and_each_sees_what_the_previous_ones_left_behind() {
        let dir = tempfile::tempdir().unwrap();
        let (orchestrator, _, seen) = orchestrator(vec![passed(), passed(), passed()]);
        let outcome = orchestrator.run(dir.path(), &plan(), "facts");
        assert!(outcome.passed());
        assert_eq!(outcome.steps_passed(), 3);
        let seen = seen.lock().unwrap();
        assert_eq!(seen.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(seen[0].2, "");
        assert_eq!(seen[1].2, "a;");
        assert_eq!(seen[2].2, "a;b;");
    }

    #[test]
    fn a_step_gets_the_standing_facts_unless_it_brings_its_own_and_never_sees_the_plan() {
        let dir = tempfile::tempdir().unwrap();
        let (orchestrator, _, seen) = orchestrator(vec![passed(), passed(), passed()]);
        orchestrator.run(dir.path(), &plan(), "facts");
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].1, "facts");
        assert_eq!(seen[1].1, "own");
    }

    #[test]
    fn the_first_failing_step_stops_the_plan_and_the_rest_are_reported_as_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let (orchestrator, _, seen) = orchestrator(vec![passed(), failed()]);
        let outcome = orchestrator.run(dir.path(), &plan(), "facts");
        assert!(!outcome.passed());
        assert_eq!(outcome.steps_passed(), 1);
        assert_eq!(outcome.results.len(), 2);
        assert_eq!(
            outcome.results[1],
            StepResult {
                id: "b".into(),
                outcome: failed()
            }
        );
        assert_eq!(outcome.skipped, vec!["c".to_string()]);
        assert_eq!(seen.lock().unwrap().len(), 2, "step c was never started");
    }

    #[test]
    fn the_trace_shows_the_plan_started_and_how_it_finished() {
        let dir = tempfile::tempdir().unwrap();
        let (orchestrator, observer, _) = orchestrator(vec![passed(), failed()]);
        orchestrator.run(dir.path(), &plan(), "facts");
        let events = observer.events();
        let RunEvent::PlanStarted(started) = &events[0] else {
            panic!("{:?}", events[0])
        };
        assert_eq!(
            (started.plan_id.as_str(), started.step_ids.clone()),
            ("p", vec!["a".to_string(), "b".into(), "c".into()])
        );
        let RunEvent::PlanFinished(finished) = events.last().unwrap() else {
            panic!("{:?}", events.last())
        };
        assert!(!finished.passed);
        assert_eq!((finished.steps_passed, finished.steps_total), (1, 3));
        assert_eq!(finished.failed_step.as_deref(), Some("b"));
    }
}
