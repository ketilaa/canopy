use anyhow::{Context, Result};

pub(crate) fn cmd_dependencies() -> Result<()> {
    let log = canopy_storage::load_dependency_decisions()
        .context("failed to load dependency decisions")?;

    if log.decisions.is_empty() {
        println!("No dependency decisions recorded yet.");
        println!("Run `canopy implement` to gate external dependencies as they are proposed.");
        return Ok(());
    }

    let accepted: Vec<_> = log.decisions.iter().filter(|d| d.decision == "accepted").collect();
    let rejected: Vec<_> = log.decisions.iter().filter(|d| d.decision == "rejected").collect();

    println!("Dependency decision log ({} total):\n", log.decisions.len());

    if !accepted.is_empty() {
        println!("Accepted ({}):", accepted.len());
        for d in &accepted {
            let scope = if d.dev { " [dev]" } else { "" };
            println!("  + {}{}", d.package, scope);
            println!("    story: {}  service: {}  decided: {}", d.story_id, d.service, d.decided_at);
            println!("    why: {}", d.justification);
            if !d.alternatives.is_empty() {
                println!("    alternatives: {}", d.alternatives);
            }
        }
        println!();
    }

    if !rejected.is_empty() {
        println!("Rejected ({}):", rejected.len());
        for d in &rejected {
            println!("  - {}", d.package);
            println!("    story: {}  service: {}  decided: {}", d.story_id, d.service, d.decided_at);
            println!("    why proposed: {}", d.justification);
            if !d.alternatives.is_empty() {
                println!("    alternatives: {}", d.alternatives);
            }
        }
        println!();
    }

    println!("Stored in .canopy/dependency_decisions.yaml");
    Ok(())
}
