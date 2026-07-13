use crate::dependency_gate::{pkg_constraints_note, run_dependency_gate};
use crate::project_scan::{read_installed_deps, scan_project_files};
use crate::tdd::{is_tdd_candidate, is_test_file};
use crate::ui::with_spinner;
use crate::util::{build_client, iso_now};
use anyhow::{Context, Result};
use canopy_core::{Adr, IntentSpec, ServicesRegistry, StepStatus, StoryPlan, UserStory};
use canopy_llm::{generate_story_plan, propose_dependencies, skill_for_technology_all_layers};
use canopy_storage::{save_story_plan, load_story_plan};
use dialoguer::{theme::ColorfulTheme, Confirm};
use std::collections::HashMap;

/// Loads the existing plan for `story_id`, or generates one (including the
/// dependency-gate pass) when none exists yet.
///
/// Returns `Ok(None)` when the plan was freshly generated but the user declined
/// to execute it — the plan is still saved to disk, but the caller should stop
/// (mirrors the original inline `return Ok(())` from within `cmd_implement`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn load_or_generate_plan(
    story_id: &str,
    debug: bool,
    story: &UserStory,
    spec: &IntentSpec,
    openapi_yaml: &str,
    services: &ServicesRegistry,
    adrs: &[Adr],
    service_packages: &HashMap<String, String>,
    theme: &ColorfulTheme,
) -> Result<Option<StoryPlan>> {
    let plan = match load_story_plan(story_id) {
        Ok(existing) => {
            let pending = existing.steps.iter().filter(|s| s.status == StepStatus::Pending).count();
            if pending == 0 {
                println!("All steps for '{}' are done — running test/fix loop.", story_id);
            } else {
                println!("Resuming plan for '{}' ({} pending step(s)).", story_id, pending);
            }
            existing
        }
        Err(_) => {
            // Collect installed packages per service so the planner knows what's available.
            let installed_deps_by_service: std::collections::HashMap<String, Vec<String>> =
                services.services.iter()
                    .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
                    .map(|s| {
                        let dir = match s.component_type.as_deref() {
                            Some("frontend") => format!("frontend/{}", s.name),
                            _ => format!("services/{}", s.name),
                        };
                        let tech = s.technology.as_deref().unwrap_or("");
                        (s.name.clone(), read_installed_deps(&dir, tech))
                    })
                    .collect();

            let existing_files = scan_project_files(services);
            let client = build_client("planner", debug)?;
            let plan = with_spinner(
                format!("generating plan for {story_id}"),
                || generate_story_plan(
                    &client, story, spec, openapi_yaml, services, adrs,
                    &existing_files, service_packages, &installed_deps_by_service,
                ),
            ).context("failed to generate implementation plan")?;

            // ── Dependency gate ──────────────────────────────────────────────────
            // Runs once per service with a known tech stack. For npm services,
            // approved packages are installed via npm install. For JVM services,
            // approved coordinates are injected as constraints into step prompts
            // so the LLM includes them in the generated pom.xml / build.gradle.
            let mut all_proposed: Vec<(String, String, Vec<canopy_core::ProposedDependency>)> = Vec::new();
            for service in services.services.iter()
                .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
            {
                let tech = service.technology.as_deref().unwrap_or("");
                if tech.is_empty() { continue; }

                let installed = installed_deps_by_service.get(&service.name)
                    .cloned()
                    .unwrap_or_default();
                let service_steps: Vec<_> = plan.steps.iter()
                    .filter(|s| s.service == service.name)
                    .cloned()
                    .collect();
                if service_steps.is_empty() { continue; }

                let global_log = canopy_storage::load_dependency_decisions().unwrap_or_default();
                let previously_rejected: Vec<String> = global_log.decisions.iter()
                    .filter(|d| d.service == service.name && d.decision == "rejected")
                    .map(|d| d.package.clone())
                    .collect();
                let dep_tech_skill = skill_for_technology_all_layers(tech, "", "", &service.name);
                let dep_result = with_spinner(
                    format!("analysing dependencies for {}", service.name),
                    || propose_dependencies(&client, &service.name, tech, story, &service_steps, &installed, &previously_rejected, adrs, &dep_tech_skill),
                );
                match dep_result {
                    Ok(proposed) if !proposed.is_empty() => {
                        all_proposed.push((service.name.clone(), tech.to_string(), proposed));
                    }
                    Ok(_) => println!("  No new dependencies proposed for '{}'.", service.name),
                    Err(e) => eprintln!("  Warning: dependency analysis failed for '{}': {e}", service.name),
                }
            }

            // Collect the gate results before showing the plan.
            let mut pkg_constraints_by_service: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            let mut dep_log = canopy_storage::load_dependency_decisions()
                .unwrap_or_default();
            for (svc_name, svc_tech, proposed) in &all_proposed {
                let installed = installed_deps_by_service.get(svc_name).cloned().unwrap_or_default();
                println!("\nDependency gate for service '{svc_name}':");
                let gate_results = run_dependency_gate(proposed, theme);

                // Append decisions to the global log.
                for (dep, accepted) in &gate_results {
                    dep_log.decisions.push(canopy_core::DependencyDecision {
                        story_id: story_id.to_string(),
                        service: svc_name.clone(),
                        package: dep.package.clone(),
                        decision: if *accepted { "accepted".to_string() } else { "rejected".to_string() },
                        justification: dep.justification.clone(),
                        alternatives: dep.alternatives.clone(),
                        dev: dep.dev,
                        decided_at: iso_now(),
                    });
                }

                let approved: Vec<String> = gate_results.iter()
                    .filter(|(_, ok)| *ok)
                    .map(|(d, _)| d.package.clone())
                    .collect();
                let approved_prod: Vec<String> = gate_results.iter()
                    .filter(|(d, ok)| *ok && !d.dev)
                    .map(|(d, _)| d.package.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                let approved_dev: Vec<String> = gate_results.iter()
                    .filter(|(d, ok)| *ok && d.dev)
                    .map(|(d, _)| d.package.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                let rejected: Vec<String> = gate_results.iter()
                    .filter(|(_, ok)| !*ok)
                    .map(|(d, _)| d.package.clone())
                    .collect();

                let svc_family = canopy_core::TechFamily::classify(svc_tech);
                let is_npm_svc = svc_family == canopy_core::TechFamily::Npm;
                let is_jvm_svc = svc_family.is_jvm();

                let svc_dir = if services.services.iter()
                    .find(|s| &s.name == svc_name)
                    .and_then(|s| s.component_type.as_deref())
                    == Some("frontend")
                {
                    format!("frontend/{svc_name}")
                } else {
                    format!("services/{svc_name}")
                };

                // npm: install approved packages immediately, respecting the dev/prod split
                // the LLM proposed (@types/* and test tooling belong in devDependencies).
                // JVM: no install step — the LLM writes them into pom.xml/build.gradle.
                if is_npm_svc && !approved.is_empty() && std::path::Path::new(&svc_dir).exists() {
                    if !approved_prod.is_empty() {
                        println!("  Installing: {}", approved_prod.join(" "));
                        let _ = crate::shell::npm_install(&svc_dir, &approved_prod, false);
                    }
                    if !approved_dev.is_empty() {
                        println!("  Installing (dev): {}", approved_dev.join(" "));
                        let _ = crate::shell::npm_install(&svc_dir, &approved_dev, true);
                    }
                } else if is_jvm_svc && !approved.is_empty() {
                    println!("  Approved JVM dependencies will be included in the generated build manifest.");
                }

                // Build tech-appropriate constraint strings for step prompts.
                let all_available: Vec<String> = {
                    let mut v = installed.clone();
                    v.extend(approved.iter().cloned());
                    v.sort(); v.dedup(); v
                };
                let note = pkg_constraints_note(svc_family, &all_available, &rejected);
                if !note.is_empty() {
                    pkg_constraints_by_service.insert(svc_name.clone(), note);
                }
            }
            // Persist the decision log after all gates are complete.
            if let Err(e) = canopy_storage::save_dependency_decisions(&dep_log) {
                eprintln!("Warning: could not save dependency decisions: {e}");
            }
            // For services with no gate interaction, still populate available packages
            // so step prompts know what is declared in the build manifest.
            for service in services.services.iter()
                .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
            {
                if pkg_constraints_by_service.contains_key(&service.name) { continue; }
                let installed = installed_deps_by_service.get(&service.name).cloned().unwrap_or_default();
                if installed.is_empty() { continue; }
                let tech = service.technology.as_deref().unwrap_or("");
                let family = canopy_core::TechFamily::classify(tech);
                pkg_constraints_by_service.insert(service.name.clone(), pkg_constraints_note(family, &installed, &[]));
            }
            // Store constraints for execution phase via a plan-level side-channel.
            // We write it to a temp file keyed by story_id so the resumed path can also use it.
            let constraints_path = canopy_storage::storage_dir()
                .join(format!("stories/{}/pkg_constraints.yaml", story_id));
            if let Ok(yaml) = serde_yaml::to_string(&pkg_constraints_by_service) {
                let _ = std::fs::write(&constraints_path, yaml);
            }

            println!("\nImplementation plan ({} steps):\n", plan.steps.len());
            for step in &plan.steps {
                let op = if step.operation == "modify" { "✎" } else { "+" };
                let tdd_tag = if is_tdd_candidate(&step.file) {
                    "  [Red→Green]"
                } else if is_test_file(&step.file) {
                    "  [test]"
                } else {
                    ""
                };
                println!("  [{}] {} {}{} — {}", step.id, op, step.file, tdd_tag, step.description);
            }
            println!();

            let confirmed = Confirm::with_theme(theme)
                .with_prompt("Execute this plan?")
                .default(true)
                .interact()
                .unwrap_or_else(|_| {
                    eprintln!("\nInput interrupted — plan saved, not executed. Re-run `canopy implement {story_id}` to continue.");
                    false
                });

            save_story_plan(story_id, &plan)
                .context("failed to save implementation plan")?;

            if !confirmed {
                println!("Plan saved. Edit .canopy/stories/{story_id}/plan.yaml and re-run `canopy implement {story_id}` to execute.");
                return Ok(None);
            }
            plan
        }
    };

    Ok(Some(plan))
}
