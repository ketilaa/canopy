//! Phase 2 arms A2 and A3 (`docs/design/structured-plan-schema-introduction-plan.md`): renders a
//! structured plan that is already fully written down (no model call, no drafting) to `plan.yaml` and
//! `plan.md`, so it can be run through `run_step` the same way `run_structured`'s A1 output is.
//!
//! usage: render_fixed --in <plan.structured.yaml> --out <dir> [--strip-decisions]
//!
//! `--strip-decisions` is arm A3: the same fixture with its `decisions` removed, to test whether the
//! human-answered gaps the fixture states actually matter.

use std::path::PathBuf;

use canopy_planner::gherkin::Feature;
use canopy_planner::render::render_markdown;
use canopy_planner::structured::{to_legacy_plan, StructuredPlan};

fn main() {
    if let Err(message) = real_main() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut feature_path: Option<PathBuf> = None;
    let mut strip_decisions = false;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--in" => input = Some(value("--in")?.into()),
            "--out" => out = Some(value("--out")?.into()),
            "--feature" => feature_path = Some(value("--feature")?.into()),
            "--strip-decisions" => strip_decisions = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    let input = input.ok_or("--in is required")?;
    let out = out.ok_or("--out is required")?;
    let text = std::fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut plan = StructuredPlan::from_yaml(&text).map_err(|e| e.to_string())?;
    if strip_decisions {
        plan.decisions.clear();
    }
    let legacy = to_legacy_plan(&plan)?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::write(out.join("plan.yaml"), legacy.to_yaml()).map_err(|e| e.to_string())?;
    if let Some(feature_path) = feature_path {
        let feature_text = std::fs::read_to_string(&feature_path).map_err(|e| format!("{}: {e}", feature_path.display()))?;
        let feature = Feature::parse(&feature_text).map_err(|e| e.to_string())?;
        std::fs::write(out.join("plan.md"), render_markdown(&legacy, &feature)).map_err(|e| e.to_string())?;
    }
    println!("{} step(s) -> {}", legacy.steps.len(), out.join("plan.yaml").display());
    Ok(())
}
