//! Analysis, not Planner code: which literals a feature states (quoted strings, numbers, dates) never reach any
//! step instruction of a plan?
//!
//! usage: plan_audit <feature.feature> <plan.yaml>...
use std::collections::BTreeSet;

use canopy_implementer::plan::Plan;

fn literals(feature: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in feature.lines().filter(|l| !l.trim_start().starts_with("Feature:")) {
        let mut quoted = line.split('"');
        quoted.next();
        for (i, part) in quoted.enumerate() {
            if i % 2 == 0 {
                found.insert(format!("\"{part}\""));
            }
        }
        let outside: String = line.split('"').step_by(2).collect::<Vec<_>>().join(" ");
        for token in outside.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')) {
            let token = token.trim_matches(|c| c == '.' || c == '-');
            if token.starts_with(|c: char| c.is_ascii_digit()) {
                found.insert(token.to_string());
            }
        }
    }
    found
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: plan_audit <feature.feature> <plan.yaml>...");
        std::process::exit(2);
    }
    let wanted = literals(&std::fs::read_to_string(&args[0]).expect("feature"));
    println!("{} literals in {}", wanted.len(), args[0]);
    for path in &args[1..] {
        let plan = Plan::from_yaml(&std::fs::read_to_string(path).expect("plan")).expect("plan parses");
        let text: String = plan.steps.iter().map(|s| s.instruction.as_str()).collect::<Vec<_>>().join("\n");
        let missing: Vec<&String> = wanted.iter().filter(|l| !text.contains(l.trim_matches('"'))).collect();
        println!("{path}: missing {} of {}: {}", missing.len(), wanted.len(), missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "));
    }
}
