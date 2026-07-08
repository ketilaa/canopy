use crate::project_scan::read_installed_deps;
use crate::shell::npm_install;
use crate::ui::{confirm_default, select_or};
use canopy_core::{ServicesRegistry, TechFamily};
use dialoguer::{theme::ColorfulTheme, Input};

/// Re-verifies that every dependency previously ACCEPTED for this story is actually
/// present in its service's manifest, and installs whatever is missing.
///
/// The dependency gate only runs once, while a plan is first generated (see
/// `commands::implement::plan::load_or_generate_plan`) — once `plan.yaml` exists, later
/// `canopy implement` invocations resume straight into step execution and never revisit
/// dependency installation. If the original `npm install` failed, or `node_modules` wasn't
/// carried over between sessions, that gap otherwise has no self-healing path: the LLM keeps
/// generating code that imports a package the decision log says was accepted but that was
/// never actually written to package.json.
///
/// Only handles npm services — an accepted JVM coordinate must be hand-declared in the
/// pom.xml/build.gradle the LLM writes, there's no separate "install" step to retry.
pub(crate) fn ensure_accepted_dependencies_installed(story_id: &str, services: &ServicesRegistry) {
    let Ok(log) = canopy_storage::load_dependency_decisions() else { return; };
    let accepted: Vec<&canopy_core::DependencyDecision> = log.decisions.iter()
        .filter(|d| d.story_id == story_id && d.decision == "accepted")
        .collect();
    if accepted.is_empty() { return; }

    for service in &services.services {
        if service.component_type.as_deref() == Some("infrastructure") { continue; }
        let Some(tech) = service.technology.as_deref() else { continue; };
        if !matches!(TechFamily::classify(tech), TechFamily::Npm) { continue; }

        let service_accepted: Vec<&canopy_core::DependencyDecision> = accepted.iter()
            .filter(|d| d.service == service.name)
            .copied()
            .collect();
        if service_accepted.is_empty() { continue; }

        let dir = match service.component_type.as_deref() {
            Some("frontend") => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };
        if !std::path::Path::new(&dir).exists() { continue; }

        let installed = read_installed_deps(&dir, tech);
        let is_missing = |pkg: &str| !installed.iter().any(|i| i.eq_ignore_ascii_case(pkg));
        let missing_prod: Vec<String> = service_accepted.iter()
            .filter(|d| !d.dev && is_missing(&d.package))
            .map(|d| d.package.clone())
            .collect();
        let missing_dev: Vec<String> = service_accepted.iter()
            .filter(|d| d.dev && is_missing(&d.package))
            .map(|d| d.package.clone())
            .collect();
        if missing_prod.is_empty() && missing_dev.is_empty() { continue; }

        println!(
            "\nPreviously accepted but not installed in '{}': {}",
            service.name,
            missing_prod.iter().chain(missing_dev.iter()).cloned().collect::<Vec<_>>().join(", ")
        );
        for (missing, dev) in [(&missing_prod, false), (&missing_dev, true)] {
            if missing.is_empty() { continue; }
            println!("  Installing{}: {}", if dev { " (dev)" } else { "" }, missing.join(", "));
            match npm_install(&dir, missing, dev) {
                Ok(status) if status.success() => println!("  Done."),
                Ok(status) => eprintln!("  npm install exited with {status} — check {dir}/package.json manually."),
                Err(e) => eprintln!("  failed to run npm install: {e}"),
            }
        }
    }
}

/// Builds the "## Available dependencies" / "## Rejected dependencies" prompt
/// fragment for a service's step prompts, based on its tech family. Returns an
/// empty string when there is nothing to report.
pub(crate) fn pkg_constraints_note(family: TechFamily, available: &[String], rejected: &[String]) -> String {
    let (manifest_label, available_note, reject_note) = match family {
        TechFamily::JvmGradle => (
            "build.gradle",
            "Declare only these coordinates in build.gradle — do not introduce others:",
            "Do NOT add: {} — rejected by the human reviewer; use built-in alternatives.",
        ),
        TechFamily::JvmMaven => (
            "pom.xml",
            "Declare only these coordinates in pom.xml — do not introduce others:",
            "Do NOT add: {} — rejected by the human reviewer; use built-in alternatives.",
        ),
        TechFamily::Npm => (
            "package.json",
            "Packages in package.json — do NOT import any other package (runtime crash):",
            "Do NOT use: {} — rejected by the human reviewer; use built-in alternatives.",
        ),
    };
    let mut lines: Vec<String> = Vec::new();
    if !available.is_empty() {
        lines.push(format!(
            "## Available dependencies ({manifest_label})\n{available_note}\n{}",
            available.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
        ));
    }
    if !rejected.is_empty() {
        lines.push(format!(
            "## Rejected dependencies\n{}",
            reject_note.replace("{}", &rejected.join(", "))
        ));
    }
    lines.join("\n\n")
}

pub(crate) fn run_dependency_gate(
    proposed: &[canopy_core::ProposedDependency],
    theme: &ColorfulTheme,
) -> Vec<(canopy_core::ProposedDependency, bool)> {
    let mut decisions: Vec<(canopy_core::ProposedDependency, bool)> = Vec::new();

    if !proposed.is_empty() {
        println!("\nDependency gate — review proposed external packages:\n");
        for dep in proposed {
            println!("  Package:       {}", dep.package);
            println!("  Type:          {}", if dep.dev { "devDependency" } else { "dependency" });
            println!("  Justification: {}", dep.justification);
            println!("  Alternatives:  {}", dep.alternatives);
            println!();

            let choice = select_or(theme, &format!("'{}'?", dep.package), &["Accept", "Reject"], 0, 1);

            let accepted = choice == 0;
            println!("  {}: {}", if accepted { "Accepted" } else { "Rejected" }, dep.package);
            println!();
            decisions.push((dep.clone(), accepted));
        }
    }

    loop {
        let add_more = confirm_default(theme, "Add a package the LLM didn't propose?", false);
        if !add_more { break; }

        let pkg: String = Input::with_theme(theme)
            .with_prompt("Package name")
            .interact_text()
            .unwrap_or_default();
        let pkg = pkg.trim().to_string();
        if !pkg.is_empty() {
            println!("  Added: {pkg}");
            decisions.push((canopy_core::ProposedDependency {
                package: pkg,
                justification: "Added by developer".to_string(),
                alternatives: String::new(),
                dev: false,
            }, true));
        }
    }

    decisions
}
