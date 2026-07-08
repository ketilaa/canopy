use crate::ui::{confirm_required, input_text_default};
use crate::util::{project_name, unix_timestamp};
use anyhow::{Context, Result};
use canopy_llm::{generate_scaffold_from_services, services_need_jvm};
use canopy_storage::{load_scaffold_plan, load_services_registry, save_scaffold_plan};
use dialoguer::theme::ColorfulTheme;

pub(crate) fn cmd_scaffold(dir: &str, regenerate: bool, _debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let target_dir = dir;

    let scaffold = match load_scaffold_plan() {
        Ok(existing) if !regenerate => {
            println!("Using existing .canopy/scaffold.yaml (pass --regenerate to discard and rebuild).");
            existing
        }
        _ => {
            let services = load_services_registry()
                .context("failed to load .canopy/services.yaml")?;

            let ready: Vec<_> = services.services.iter().filter(|s| s.technology.is_some()).collect();
            let pending: Vec<_> = services.services.iter().filter(|s| s.technology.is_none()).collect();

            if ready.is_empty() {
                anyhow::bail!(
                    "No services with a decided technology stack found in .canopy/services.yaml.\n\
                     Run `canopy spec <story-id>` to accept tech stack ADRs for each service first."
                );
            }

            if !pending.is_empty() {
                println!("Warning: the following services have no technology decided and will be skipped:");
                for s in &pending {
                    println!("  - {} (run `canopy spec` to resolve)", s.name);
                }
            }

            let group_id: String = if services_need_jvm(&services) {
                let slug = project_name().to_lowercase().replace([' ', '-'], "");
                input_text_default(&theme, "Java groupId / base package", format!("com.example.{slug}"), "failed to read groupId")?
            } else {
                String::new()
            };

            println!("\nGenerating scaffold plan from services registry...");
            let mut plan = generate_scaffold_from_services(&services, &group_id);
            plan.generated_at = unix_timestamp();
            save_scaffold_plan(&plan).context("failed to save scaffold.yaml")?;
            println!("Scaffold plan saved to .canopy/scaffold.yaml");
            plan
        }
    };

    println!("\nWill run the following scaffold commands in '{}':\n", target_dir);
    for (i, cmd) in scaffold.commands.iter().enumerate() {
        println!("  [{}] {}", i + 1, cmd.label);
        println!("      $ {}", cmd.command);
        if !cmd.creates.is_empty() {
            println!("      → creates: {}", cmd.creates);
        }
        println!();
    }

    let proceed = confirm_required(&theme, "Execute these scaffold commands?", "failed to read confirmation")?;

    if !proceed {
        println!("Not executed. Edit .canopy/scaffold.yaml and re-run, or run the commands manually.");
        return Ok(());
    }

    let base = std::path::Path::new(&target_dir);
    for cmd in &scaffold.commands {
        let wd = if cmd.working_dir == "." {
            base.to_path_buf()
        } else {
            base.join(&cmd.working_dir)
        };

        std::fs::create_dir_all(&wd)
            .with_context(|| format!("failed to create working directory: {}", wd.display()))?;

        println!("\n$ {}", cmd.command);
        let status = crate::shell::run_status_in_dir("sh", &cmd.command, &wd)
            .with_context(|| format!("failed to launch: {}", cmd.command))?;

        if !status.success() {
            anyhow::bail!(
                "Command failed (exit {}): {}",
                status.code().unwrap_or(-1),
                cmd.command
            );
        }
        println!("  Done → {}", cmd.creates);
    }

    println!("\nScaffolding complete.");
    Ok(())
}
