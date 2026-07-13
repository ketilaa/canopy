mod execute;
mod plan;

use crate::dependency_gate::ensure_accepted_dependencies_installed;
use crate::project_scan::detect_service_package;
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{ServicesRegistry, StoryStatus, TechFamily};
use canopy_llm::generate_story_openapi;
use canopy_storage::{
    load_all_adrs, load_services_registry, load_story_spec, load_user_stories, save_story_openapi,
};
use dialoguer::theme::ColorfulTheme;

/// Checks that every active (non-infrastructure) service with a decided technology has
/// actually been scaffolded, so `implement` fails fast with a clear message instead of
/// letting tsc/npm/jest fail with ENOENT partway through a multi-step plan (as happened
/// when frontend/admin-portal was never scaffolded but steps 15-17 still tried to write
/// TDD tests and run `npm test` inside it).
///
/// A service with no decided technology yet is a different, pre-existing problem (run
/// `canopy spec` first) and is left for the normal step-generation error path to report.
fn ensure_services_scaffolded(services: &ServicesRegistry) -> Result<()> {
    let mut not_scaffolded: Vec<String> = Vec::new();
    for service in services.services.iter().filter(|s| s.component_type.as_deref() != Some("infrastructure")) {
        let Some(tech) = service.technology.as_deref() else { continue; };
        let dir = match service.component_type.as_deref() {
            Some("frontend") => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };
        let looks_scaffolded = match TechFamily::classify(tech) {
            TechFamily::Npm => std::path::Path::new(&format!("{dir}/package.json")).exists(),
            TechFamily::JvmMaven => std::path::Path::new(&format!("{dir}/pom.xml")).exists(),
            TechFamily::JvmGradle => std::path::Path::new(&format!("{dir}/build.gradle")).exists()
                || std::path::Path::new(&format!("{dir}/build.gradle.kts")).exists(),
        };
        if !looks_scaffolded {
            not_scaffolded.push(format!("{} ({dir})", service.name));
        }
    }
    if !not_scaffolded.is_empty() {
        anyhow::bail!(
            "The following service(s) have not been scaffolded yet:\n  {}\n\n\
             Run `canopy scaffold` first, then re-run `canopy implement`.",
            not_scaffolded.join("\n  ")
        );
    }
    Ok(())
}

pub(crate) fn cmd_implement(story_id: &str, debug: bool, fix_log_dir: &std::path::Path) -> Result<()> {
    let theme = ColorfulTheme::default();

    let stories = load_user_stories()
        .context("no stories.yaml — run `canopy intent` first")?;
    let story = stories.stories.iter()
        .find(|s| s.id == story_id)
        .ok_or_else(|| anyhow::anyhow!("story '{}' not found", story_id))?;
    if story.status != StoryStatus::Accepted {
        anyhow::bail!("story '{}' is not accepted", story_id);
    }

    let spec = load_story_spec(story_id)
        .with_context(|| format!("no spec for '{}' — run `canopy spec {story_id}` first", story_id))?;

    let services = load_services_registry()
        .context("no services.yaml — run `canopy spec` first")?;
    ensure_services_scaffolded(&services)?;

    let adrs = load_all_adrs().unwrap_or_default();

    let openapi_path = canopy_storage::storage_dir()
        .join(format!("stories/{}/openapi.yaml", story_id));
    let openapi_yaml = if openapi_path.exists() {
        std::fs::read_to_string(&openapi_path)
            .context("failed to read openapi.yaml")?
    } else {
        println!("No OpenAPI spec found for '{}' — generating from spec...", story_id);
        let client = build_client("openapi", debug)?;
        match generate_story_openapi(&client, story, &spec, &services, &adrs) {
            Ok(yaml) => {
                save_story_openapi(story_id, &yaml).context("failed to save OpenAPI spec")?;
                println!("OpenAPI spec saved to .canopy/stories/{}/openapi.yaml", story_id);
                yaml
            }
            Err(e) => anyhow::bail!("OpenAPI spec generation failed: {e}"),
        }
    };

    // Detect the actual base package per JVM service from the scaffolded *Application.java.
    // This adapts to whatever naming convention the scaffold tool used (Spring Initializr
    // converts "product-service" to "product_service", not "productservice").
    let service_packages: std::collections::HashMap<String, String> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure")
                 && s.component_type.as_deref() != Some("frontend"))
        .filter_map(|s| detect_service_package(&s.name).map(|pkg| (s.name.clone(), pkg)))
        .collect();
    if service_packages.is_empty() {
        println!("Note: no scaffolded JVM services found — package detection skipped.");
    } else {
        for (name, pkg) in &service_packages {
            println!("Detected package for {name}: {pkg}");
        }
    }

    let plan = plan::load_or_generate_plan(
        story_id, debug, story, &spec, &openapi_yaml, &services, &adrs, &service_packages, &theme,
    )?;
    let Some(plan) = plan else { return Ok(()); };

    // Catches both a failed install during the gate above (same run) and dependencies
    // accepted in a prior session that were never actually installed (resumed run).
    ensure_accepted_dependencies_installed(story_id, &services);

    execute::execute_steps(
        story_id, debug, story, &spec, &openapi_yaml, &services, &adrs, &service_packages,
        fix_log_dir, plan,
    )
}
