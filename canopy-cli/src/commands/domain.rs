use anyhow::{Context, Result};
use canopy_storage::load_domain_registry;

pub(crate) fn cmd_domain_show() -> Result<()> {
    let domain = load_domain_registry().context("failed to load domain registry")?;

    if domain.entities.is_empty() && domain.events.is_empty() {
        println!("No domain vocabulary yet.");
        println!("Run `canopy intent` to start building stories — entities and events are extracted automatically.");
        return Ok(());
    }

    println!("Entities ({}):", domain.entities.len());
    for e in &domain.entities {
        match e.description() {
            Some(d) => println!("  {} — {}", e.name(), d),
            None    => println!("  {}", e.name()),
        }
    }

    println!("\nEvents ({}):", domain.events.len());
    for e in &domain.events {
        match e.description() {
            Some(d) => println!("  {} — {}", e.name(), d),
            None    => println!("  {}", e.name()),
        }
    }

    println!("\nEdit .canopy/domain_registry.yaml to add, rename, or remove entries.");
    Ok(())
}
