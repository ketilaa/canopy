use crate::adr_wizard::{
    architecture_style_adr, deployment_style_adr, event_broker_adr, topic_naming_convention_adr,
};
use crate::ui::{bootstrap_select, input_text_optional, input_text_required, select_required};
use crate::util::{build_client, project_name};
use anyhow::{Context, Result};
use canopy_core::{DomainEntity, DomainRegistry, Idea, Role, RolesRegistry};
use canopy_llm::{suggest_domain_entities, suggest_roles};
use canopy_storage::{
    ensure_storage_dir, save_adr, save_domain_registry, save_idea, save_roles_registry,
};
use dialoguer::theme::ColorfulTheme;

pub(crate) fn cmd_init(debug: bool) -> Result<()> {
    use std::io::Write;
    let theme = ColorfulTheme::default();

    let description = input_text_required(&theme, "What are you building?", "failed to read idea description from terminal")?;

    ensure_storage_dir().context("failed to create .canopy/ directory")?;
    let idea = Idea { description };
    save_idea(&idea).context("failed to save idea.yaml")?;

    // Architecture style — pre-authored ADR written at adr-000
    let arch_styles = ["Event-driven microservices (DDD)"];
    let arch_idx = select_required(&theme, "Architecture style", &arch_styles, 0, "failed to read architecture style selection")?;
    let arch_adr = architecture_style_adr(arch_idx);
    save_adr(0, "architecture-style", &arch_adr)
        .context("failed to save adr-000-architecture-style.yaml")?;
    println!("  Saved .canopy/decisions/adr-000-architecture-style.yaml");

    // Deployment style — pre-authored ADR written at adr-001
    let deploy_styles = ["Docker Compose (local development)"];
    let deploy_idx = select_required(&theme, "Deployment style", &deploy_styles, 0, "failed to read deployment style selection")?;
    let deploy_adr = deployment_style_adr(deploy_idx);
    save_adr(1, "deployment-style", &deploy_adr)
        .context("failed to save adr-001-deployment-style.yaml")?;
    println!("  Saved .canopy/decisions/adr-001-deployment-style.yaml");

    // Event broker — mandatory for event-driven architecture; saved at adr-002
    let arch_decision = arch_adr.decision.to_lowercase();
    if arch_decision.contains("event-driven") || arch_decision.contains("event driven") {
        let broker_options = [
            "Redpanda  (Kafka-compatible, first-class Docker support — recommended for local dev)",
            "Apache Kafka",
            "RabbitMQ  (AMQP message broker)",
            "NATS      (lightweight, cloud-native messaging)",
        ];
        let broker_idx = select_required(&theme, "Event broker", &broker_options, 0, "failed to read event broker selection")?;
        let broker_adr = event_broker_adr(broker_idx);
        save_adr(2, "event-broker", &broker_adr)
            .context("failed to save adr-002-event-broker.yaml")?;
        println!("  Saved .canopy/decisions/adr-002-event-broker.yaml");

        let convention_options = [
            "<aggregate>-events  (e.g. product-events, order-events — one topic per aggregate)",
            "<service>-events    (e.g. product-service-events — one topic per service)",
            "<domain>.<aggregate>.events  (reverse-DNS style, e.g. commerce.product.events)",
        ];
        let convention_idx = select_required(&theme, "Topic naming convention", &convention_options, 0, "failed to read topic naming convention selection")?;
        let convention_adr = topic_naming_convention_adr(convention_idx);
        save_adr(3, "topic-naming-convention", &convention_adr)
            .context("failed to save adr-003-topic-naming-convention.yaml")?;
        println!("  Saved .canopy/decisions/adr-003-topic-naming-convention.yaml");
    }

    // Bootstrap domain entities
    let client = build_client("intent", debug)?;
    print!("Suggesting domain entities... ");
    let _ = std::io::stdout().flush();
    match suggest_domain_entities(&client, &idea) {
        Ok(suggestions) if !suggestions.is_empty() => {
            println!();
            let names = bootstrap_select(&theme, "Domain entities (deselect to remove, add missing below)", &suggestions)?;
            let mut entities = Vec::new();
            for name in names {
                let desc = input_text_optional(&theme, &format!("Description for '{}' (leave blank to skip)", name), "failed to read entity description")?;
                let desc = desc.trim().to_string();
                entities.push(if desc.is_empty() {
                    DomainEntity::Simple(name)
                } else {
                    DomainEntity::Described { name, description: desc }
                });
            }
            let registry = DomainRegistry { entities, events: vec![] };
            save_domain_registry(&registry).context("failed to save domain_registry.yaml")?;
            println!("  Saved .canopy/domain_registry.yaml ({} entities)", registry.entities.len());
        }
        Ok(_) => println!("none suggested"),
        Err(e) => println!("skipped ({e})"),
    }

    // Bootstrap roles
    print!("Suggesting roles... ");
    let _ = std::io::stdout().flush();
    match suggest_roles(&client, &idea) {
        Ok(suggestions) if !suggestions.is_empty() => {
            println!();
            let names = bootstrap_select(&theme, "Roles (deselect to remove, add missing below)", &suggestions)?;
            let mut roles = Vec::new();
            for name in names {
                let desc = input_text_optional(&theme, &format!("Description for '{}' (leave blank to skip)", name), "failed to read role description")?;
                let desc = desc.trim().to_string();
                roles.push(if desc.is_empty() {
                    Role::Simple(name)
                } else {
                    Role::Described { name, description: desc }
                });
            }
            let registry = RolesRegistry { roles };
            save_roles_registry(&registry).context("failed to save roles.yaml")?;
            println!("  Saved .canopy/roles.yaml ({} roles)", registry.roles.len());
        }
        Ok(_) => println!("none suggested"),
        Err(e) => println!("skipped ({e})"),
    }

    println!("Project: {}", project_name());
    println!("Next: run `canopy intent` to add your first behavioral requirement.");
    Ok(())
}
