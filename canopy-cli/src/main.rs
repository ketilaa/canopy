mod roots;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

use canopy_core::*;
use canopy_llm::{
    execute_implementation_step, extract_domain_from_stories, fix_file,
    generate_scaffold_from_services, generate_stories_from_intent, generate_story_contract,
    generate_story_plan, generate_story_spec, identify_architectural_questions,
    services_need_jvm, suggest_domain_entities, suggest_roles, LlmClient,
};
use canopy_storage::*;

#[derive(Parser)]
#[command(
    name = "canopy",
    about = "AI-powered idea exploration — structure before tokens",
    version
)]
struct Cli {
    /// Print each LLM prompt, response, model, and token counts to stderr
    #[arg(long, global = true)]
    llm_debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialise a new project — describe what you are building
    Init,
    /// Show the accumulated domain vocabulary (entities and events)
    Domain,
    /// Derive scaffold commands from services.yaml and run them
    Scaffold {
        /// Directory to run scaffold commands in (defaults to current directory)
        #[arg(long, default_value = ".")]
        dir: String,
        /// Discard existing scaffold.yaml and regenerate from the LLM
        #[arg(long)]
        regenerate: bool,
    },
    /// Implement a story using its spec and OAS contract
    Implement {
        /// Story ID to implement (must have status: accepted and a generated spec)
        story_id: String,
    },
    /// List all user stories and their current status
    Stories,
    /// Derive user stories from a behavioral intent statement
    Intent {
        /// The behavioral statement (e.g. "Products must be promoted to be available in the store").
        /// Prompts interactively if omitted.
        statement: Option<String>,
    },
    /// Generate BDD spec for an accepted story, with interactive ADR gating
    Spec {
        /// Story ID to specify (must have status: accepted)
        story_id: String,
    },
}

fn build_client(agent: &str, debug: bool) -> Result<LlmClient> {
    match canopy_storage::load_config()
        .context("failed to read .canopy/config.yaml")?
    {
        Some(cfg) => {
            let agent_cfg = cfg.for_agent(agent).ok_or_else(|| {
                anyhow::anyhow!(
                    "no LLM config for agent '{}' and no default in .canopy/config.yaml",
                    agent
                )
            })?;
            Ok(LlmClient::from_agent_config(&agent_cfg, debug))
        }
        None => Ok(LlmClient::default_local(debug)),
    }
}

fn unix_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn dispatch(cmd: Commands, debug: bool) -> Result<()> {
    match cmd {
        Commands::Init                         => cmd_init(debug),
        Commands::Domain                       => cmd_domain_show(),
        Commands::Scaffold { dir, regenerate } => cmd_scaffold(&dir, regenerate, debug),
        Commands::Implement { story_id }       => cmd_implement(&story_id, debug),
        Commands::Stories                      => cmd_stories(),
        Commands::Intent { statement }         => cmd_intent(statement, debug),
        Commands::Spec { story_id }            => cmd_spec(&story_id, debug),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let debug = cli.llm_debug;
    match cli.command {
        Some(cmd) => dispatch(cmd, debug),
        None => run_repl(debug),
    }
}

fn run_repl(debug: bool) -> Result<()> {
    use std::io::{BufRead, Write};

    println!("canopy  —  type a command or 'exit'");

    // Roots: initialise and index the repository once for the whole session.
    print!("Checking Roots index... ");
    let _ = std::io::stdout().flush();
    roots::ensure_indexed();
    println!("ready");

    let stdin = std::io::stdin();
    let mut line = String::new();

    loop {
        print!("\ncanopy> ");
        let _ = std::io::stdout().flush();

        line.clear();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if matches!(trimmed, "exit" | "quit") { break; }

        // Re-use clap to parse the typed command, prepending the binary name.
        let mut args = vec!["canopy"];
        if debug { args.push("--llm-debug"); }
        args.extend(trimmed.split_whitespace());

        match Cli::try_parse_from(args) {
            Ok(cli) => {
                if let Some(cmd) = cli.command {
                    if let Err(e) = dispatch(cmd, debug) {
                        eprintln!("  error: {e:#}");
                    }
                }
            }
            Err(e) => eprintln!("{e}"),
        }
    }

    println!("bye");
    Ok(())
}

fn project_name() -> String {
    // Try git remote name first
    if let Ok(output) = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
    {
        let url = String::from_utf8_lossy(&output.stdout);
        let name = url.trim().trim_end_matches(".git");
        if let Some(part) = name.rsplit('/').next() {
            if !part.is_empty() {
                return part.to_string();
            }
        }
    }
    // Fall back to current directory name
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_string())
}

fn bootstrap_select(theme: &ColorfulTheme, prompt: &str, suggestions: &[String]) -> Result<Vec<String>> {
    let defaults = vec![true; suggestions.len()];
    let selected_indices = MultiSelect::with_theme(theme)
        .with_prompt(prompt)
        .items(suggestions)
        .defaults(&defaults)
        .interact()
        .context("failed to read selection")?;

    let mut selected: Vec<String> = selected_indices
        .iter()
        .map(|&i| suggestions[i].clone())
        .collect();

    loop {
        let extra: String = Input::with_theme(theme)
            .with_prompt("Add missing (leave blank to finish)")
            .allow_empty(true)
            .interact_text()
            .context("failed to read additional entry")?;
        let extra = extra.trim().to_string();
        if extra.is_empty() {
            break;
        }
        selected.push(extra);
    }

    Ok(selected)
}

fn cmd_init(debug: bool) -> Result<()> {
    use std::io::Write;
    let theme = ColorfulTheme::default();

    let description: String = Input::with_theme(&theme)
        .with_prompt("What are you building?")
        .interact_text()
        .context("failed to read idea description from terminal")?;

    ensure_storage_dir().context("failed to create .canopy/ directory")?;
    let idea = Idea { description };
    save_idea(&idea).context("failed to save idea.yaml")?;

    // Architecture style — pre-authored ADR written at adr-000
    let arch_styles = ["Event-driven microservices (DDD)"];
    let arch_idx = Select::with_theme(&theme)
        .with_prompt("Architecture style")
        .items(&arch_styles)
        .default(0)
        .interact()
        .context("failed to read architecture style selection")?;
    let arch_adr = architecture_style_adr(arch_idx);
    save_adr(0, "architecture-style", &arch_adr)
        .context("failed to save adr-000-architecture-style.yaml")?;
    println!("  Saved .canopy/decisions/adr-000-architecture-style.yaml");

    // Deployment style — pre-authored ADR written at adr-001
    let deploy_styles = ["Docker Compose (local development)"];
    let deploy_idx = Select::with_theme(&theme)
        .with_prompt("Deployment style")
        .items(&deploy_styles)
        .default(0)
        .interact()
        .context("failed to read deployment style selection")?;
    let deploy_adr = deployment_style_adr(deploy_idx);
    save_adr(1, "deployment-style", &deploy_adr)
        .context("failed to save adr-001-deployment-style.yaml")?;
    println!("  Saved .canopy/decisions/adr-001-deployment-style.yaml");

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
                let desc: String = Input::with_theme(&theme)
                    .with_prompt(format!("Description for '{}' (leave blank to skip)", name))
                    .allow_empty(true)
                    .interact_text()
                    .context("failed to read entity description")?;
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
                let desc: String = Input::with_theme(&theme)
                    .with_prompt(format!("Description for '{}' (leave blank to skip)", name))
                    .allow_empty(true)
                    .interact_text()
                    .context("failed to read role description")?;
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

fn architecture_style_adr(idx: usize) -> Adr {
    match idx {
        _ => Adr {
            title: "Architecture Style".to_string(),
            decision: "Event-driven microservices using Domain-Driven Design".to_string(),
            reason: "Services are bounded by domain context and communicate through domain events. \
                     This enables independent deployability, clear ownership boundaries, \
                     and natural alignment with the domain model."
                .to_string(),
            alternatives: vec![
                "Modular monolith".to_string(),
                "Layered monolith".to_string(),
            ],
        },
    }
}

fn deployment_style_adr(idx: usize) -> Adr {
    match idx {
        _ => Adr {
            title: "Deployment Style".to_string(),
            decision: "Docker Compose for local development".to_string(),
            reason: "All services, databases, and event infrastructure run locally in Docker Compose. \
                     This provides a consistent, portable local development environment without \
                     requiring a Kubernetes cluster. Production deployment strategy is decided separately."
                .to_string(),
            alternatives: vec![
                "Kubernetes with local cluster (minikube or kind)".to_string(),
                "Native processes per service".to_string(),
            ],
        },
    }
}

fn cmd_scaffold(dir: &str, regenerate: bool, _debug: bool) -> Result<()> {
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
                Input::with_theme(&theme)
                    .with_prompt("Java groupId / base package")
                    .default(format!("com.example.{slug}"))
                    .interact_text()
                    .context("failed to read groupId")?
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

    let proceed = Confirm::with_theme(&theme)
        .with_prompt("Execute these scaffold commands?")
        .interact()
        .context("failed to read confirmation")?;

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
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd.command)
            .current_dir(&wd)
            .status()
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

fn scan_project_files(services: &ServicesRegistry) -> Vec<String> {
    let mut files = Vec::new();
    for service in &services.services {
        if service.component_type.as_deref() == Some("infrastructure") { continue; }
        let base = match service.component_type.as_deref().unwrap_or("service") {
            "frontend" => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };
        collect_files(std::path::Path::new(&base), &mut files);
    }
    files
}

/// Walk a JVM service's src/main/java tree to find *Application.java and read its package declaration.
/// Returns the fully qualified base package as declared in the file (e.g. "com.example.canopyecommerce.product_service").
fn detect_service_package(service_name: &str) -> Option<String> {
    let root = std::path::Path::new("services")
        .join(service_name)
        .join("src/main/java");
    find_application_package(&root)
}

fn find_application_package(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if let Some(pkg) = find_application_package(&path) {
                return Some(pkg);
            }
        } else if name.ends_with("Application.java") {
            let content = std::fs::read_to_string(&path).ok()?;
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("package ") {
                    return Some(rest.trim_end_matches(';').trim().to_string());
                }
            }
        }
    }
    None
}

fn collect_files(dir: &std::path::Path, out: &mut Vec<String>) {
    let skip = ["target", "node_modules", ".git", ".roots"];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if skip.contains(&name) { continue; }
            if path.is_dir() {
                collect_files(&path, out);
            } else {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
}

fn format_roots_context(packet: &roots_context::FeatureContextPacket) -> String {
    let mut parts = Vec::new();
    if !packet.symbols.is_empty() {
        let syms: Vec<String> = packet.symbols.iter()
            .map(|s| format!("  {} {} ({}:{})", s.kind, s.fqn, s.file, s.line))
            .collect();
        parts.push(format!("Symbols:\n{}", syms.join("\n")));
    }
    if !packet.facts.is_empty() {
        let facts = packet.facts.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n");
        parts.push(format!("Facts:\n{facts}"));
    }
    parts.join("\n")
}

fn cmd_implement(story_id: &str, debug: bool) -> Result<()> {
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

    let contract_path = canopy_storage::storage_dir()
        .join(format!("stories/{}/contract.yaml", story_id));
    let contract_yaml = std::fs::read_to_string(&contract_path)
        .with_context(|| format!("no contract for '{}' — run `canopy spec {story_id}` first", story_id))?;

    let services = load_services_registry()
        .context("no services.yaml — run `canopy spec` first")?;

    let adrs = load_all_adrs().unwrap_or_default();

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

    // Load or generate implementation plan
    let mut plan = match load_story_plan(story_id) {
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
            let existing_files = scan_project_files(&services);
            println!("Generating implementation plan for '{story_id}'...");
            let client = build_client("planner", debug)?;
            let plan = generate_story_plan(
                &client, story, &spec, &contract_yaml, &services, &adrs, &existing_files, &service_packages,
            )
            .context("failed to generate implementation plan")?;

            println!("\nImplementation plan ({} steps):\n", plan.steps.len());
            for step in &plan.steps {
                let op = if step.operation == "modify" { "✎" } else { "+" };
                println!("  [{}] {} {} — {}", step.id, op, step.file, step.description);
            }
            println!();

            let confirmed = Confirm::with_theme(&theme)
                .with_prompt("Execute this plan?")
                .default(true)
                .interact()
                .context("failed to read confirmation")?;

            save_story_plan(story_id, &plan)
                .context("failed to save implementation plan")?;

            if !confirmed {
                println!("Plan saved. Edit .canopy/stories/{story_id}/plan.yaml and re-run `canopy implement {story_id}` to execute.");
                return Ok(());
            }
            plan
        }
    };

    let client = build_client("developer", debug)?;
    let total = plan.steps.len();
    let mut written = 0usize;

    roots::ensure_indexed();

    for i in 0..total {
        if plan.steps[i].status != StepStatus::Pending { continue; }

        let step = &plan.steps[i];
        let op_label = if step.operation == "modify" { "modify" } else { "create" };
        println!("\n[{}/{}] {} {}", step.id, total, op_label, step.file);
        println!("  {}", step.description);

        let current_content = if step.operation == "modify" {
            std::fs::read_to_string(&step.file).ok()
        } else {
            None
        };

        let roots_context = roots::get_feature_context(&step.description)
            .map(|p| format_roots_context(&p))
            .filter(|s| !s.is_empty());

        let content = execute_implementation_step(
            &client, story, &spec, &contract_yaml,
            step, current_content.as_deref(), roots_context.as_deref(),
            &service_packages, &services,
        )
        .with_context(|| format!("LLM call failed for step {}", step.id))?;

        let dest = std::path::Path::new(&step.file);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory for {}", step.file))?;
        }
        std::fs::write(dest, &content)
            .with_context(|| format!("failed to write {}", step.file))?;
        println!("  wrote {}", step.file);
        written += 1;

        plan.steps[i].status = StepStatus::Done;
        save_story_plan(story_id, &plan)
            .context("failed to save plan progress")?;

        roots::reindex();
    }

    println!("\n{written} file(s) written.");

    // Test/fix loop — run build+tests per service and fix compiler errors with LLM
    let implementable: Vec<_> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .filter(|s| s.technology.is_some())
        .collect();

    const MAX_FIX_ITERATIONS: usize = 5;

    for service in &implementable {
        let service_dir = match service.component_type.as_deref().unwrap_or("service") {
            "frontend" => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };

        if !std::path::Path::new(&service_dir).exists() {
            continue;
        }

        let test_cmd = test_command_for_service(service, &service_dir);
        let service_source_files = scan_service_source_files(&service_dir);

        // Ensure frontend dependencies are installed before building
        if !std::path::Path::new(&format!("{service_dir}/node_modules")).exists()
            && std::path::Path::new(&format!("{service_dir}/package.json")).exists()
        {
            println!("  running npm install in {service_dir}...");
            let _ = std::process::Command::new("npm")
                .arg("install")
                .current_dir(&service_dir)
                .status();
        }
        println!("\nRunning: {} (in {})", test_cmd, service_dir);

        for iteration in 0..MAX_FIX_ITERATIONS {
            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(&test_cmd)
                .current_dir(&service_dir)
                .output();

            let output = match output {
                Ok(o) => o,
                Err(e) => { eprintln!("  failed to run test command: {e}"); break; }
            };

            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            if output.status.success() {
                println!("  ✓ {} passes", service.name);
                break;
            }

            let missing_pkgs = extract_missing_packages(&combined);
            let mut fixed_any = false;
            let broken_files = extract_error_files(&combined, &service_dir);

            // javax.* → jakarta.* is a mechanical rename, no LLM needed.
            // Spring Boot 3+ dropped the javax.* namespace entirely.
            if missing_pkgs.iter().any(|p| p.starts_with("javax.")) {
                let n = migrate_javax_to_jakarta(&service_dir);
                if n > 0 {
                    println!("  migrated javax.* → jakarta.* in {n} file(s)");
                    fixed_any = true;
                }
            }

            // Remaining missing packages are genuine pom.xml gaps — let LLM add them.
            let non_javax: Vec<_> = missing_pkgs.iter()
                .filter(|p| !p.starts_with("javax."))
                .collect();
            if !non_javax.is_empty() {
                let build_file = format!("{service_dir}/pom.xml");
                if std::path::Path::new(&build_file).exists() {
                    let content = match std::fs::read_to_string(&build_file) {
                        Ok(c) => c,
                        Err(e) => { eprintln!("  cannot read {build_file}: {e}"); String::new() }
                    };
                    if !content.is_empty() {
                        let errors = format!(
                            "The following packages are missing from the Maven dependencies:\n{}",
                            non_javax.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n")
                        );
                        println!("  fixing pom.xml ({} missing package(s))", non_javax.len());
                        match fix_file(&client, &build_file, &content, &errors, &service_source_files) {
                            Ok(fixed) => {
                                if let Err(e) = std::fs::write(&build_file, &fixed) {
                                    eprintln!("    failed to write {build_file}: {e}");
                                } else {
                                    fixed_any = true;
                                }
                            }
                            Err(e) => eprintln!("    LLM fix failed for {build_file}: {e}"),
                        }
                    }
                }
            }

            if broken_files.is_empty() && !fixed_any {
                eprintln!("  Tests failed but no fixable errors found — manual fix needed.");
                eprintln!("{combined}");
                break;
            }

            if !broken_files.is_empty() {
                println!(
                    "  iteration {}/{}: {} source file(s) with errors",
                    iteration + 1, MAX_FIX_ITERATIONS, broken_files.len()
                );
            }

            for file_path in &broken_files {
                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => { eprintln!("  cannot read {file_path}: {e}"); continue; }
                };
                let errors = errors_for_file(&combined, file_path);
                if errors.trim().is_empty() {
                    eprintln!("  skipping {} — in error list but no matching error lines found", file_path);
                    continue;
                }
                println!("    fixing {} ({} error line(s))", file_path, errors.lines().count());

                match fix_file(&client, file_path, &content, &errors, &service_source_files) {
                    Ok(fixed) => {
                        if let Err(e) = std::fs::write(file_path, &fixed) {
                            eprintln!("    failed to write {file_path}: {e}");
                        }
                    }
                    Err(e) => eprintln!("    LLM fix failed for {file_path}: {e}"),
                }
            }
        }
    }

    Ok(())
}

fn test_command_for_service(service: &ServiceEntry, service_dir: &str) -> String {
    let tech = service.technology.as_deref().unwrap_or("").to_lowercase();
    if tech.contains("spring") || tech.contains("maven") || tech.contains("java") {
        return "./mvnw test -B".to_string();
    }
    if tech.contains("gradle") {
        return "./gradlew test".to_string();
    }
    // Frontend: check package.json scripts to pick the right command
    let pkg_path = format!("{service_dir}/package.json");
    if let Ok(pkg) = std::fs::read_to_string(&pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&pkg) {
            let scripts = json.get("scripts");
            if scripts.and_then(|s| s.get("test")).is_some() {
                return "npm test -- --watchAll=false".to_string();
            }
            if scripts.and_then(|s| s.get("build")).is_some() {
                return "npm run build".to_string();
            }
        }
    }
    "npx tsc --noEmit".to_string()
}

fn extract_error_files(output: &str, service_dir: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    let svc_path = std::path::Path::new(service_dir);

    let resolve = |path: &str| -> Option<String> {
        let p = std::path::Path::new(path);
        let candidates = if p.is_absolute() {
            vec![p.to_path_buf()]
        } else {
            vec![
                svc_path.join(p),                          // relative to service dir
                std::env::current_dir().ok()?.join(p),     // relative to project root
            ]
        };
        candidates.into_iter()
            .map(|c| c.to_string_lossy().to_string())
            .find(|s| std::path::Path::new(s).exists())
    };

    for line in output.lines() {
        // Maven: [ERROR] /abs/path/to/File.java:[line,col] message
        if let Some(rest) = line.strip_prefix("[ERROR] ") {
            if let Some(bracket) = rest.find(":[") {
                let path = rest[..bracket].trim();
                if path.ends_with(".java") || path.ends_with(".ts") || path.ends_with(".tsx") {
                    if let Some(resolved) = resolve(path) {
                        if !files.contains(&resolved) { files.push(resolved); }
                    }
                }
            }
        }
        // TypeScript / vite: path/to/File.ts(line,col): error TS...
        // Must contain ): error or ): warning after the (line,col) part
        if line.contains("): error ") || line.contains("): warning ") {
            if let Some(paren) = line.find('(') {
                let path = line[..paren].trim_start();
                if path.ends_with(".ts") || path.ends_with(".tsx") {
                    if let Some(resolved) = resolve(path) {
                        if !files.contains(&resolved) { files.push(resolved); }
                    }
                }
            }
        }
    }
    files
}

fn scan_service_source_files(service_dir: &str) -> Vec<String> {
    let mut all = Vec::new();
    collect_files(std::path::Path::new(service_dir), &mut all);
    let base = std::path::Path::new(service_dir);
    all.into_iter()
        .filter(|f| f.ends_with(".java") || f.ends_with(".ts") || f.ends_with(".tsx"))
        .map(|f| {
            std::path::Path::new(&f)
                .strip_prefix(base)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(f)
        })
        .collect()
}

fn migrate_javax_to_jakarta(service_dir: &str) -> usize {
    let mut count = 0;
    let mut java_files = Vec::new();
    collect_files(std::path::Path::new(service_dir), &mut java_files);
    for path in java_files.iter().filter(|p| p.ends_with(".java")) {
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.contains("javax.") {
                let fixed = content
                    .replace("javax.persistence", "jakarta.persistence")
                    .replace("javax.validation", "jakarta.validation")
                    .replace("javax.servlet", "jakarta.servlet")
                    .replace("javax.annotation", "jakarta.annotation")
                    .replace("javax.transaction", "jakarta.transaction");
                if fixed != content {
                    let _ = std::fs::write(path, fixed);
                    count += 1;
                }
            }
        }
    }
    count
}

fn extract_missing_packages(output: &str) -> Vec<String> {
    let mut packages = Vec::new();
    for line in output.lines() {
        // javac: error: package javax.validation does not exist
        if line.contains("does not exist") {
            if let Some(start) = line.rfind("package ") {
                let rest = &line[start + 8..];
                if let Some(end) = rest.find(" does not exist") {
                    let pkg = rest[..end].trim().to_string();
                    if !pkg.is_empty() && !packages.contains(&pkg) {
                        packages.push(pkg);
                    }
                }
            }
        }
        // javac: error: cannot access X — class file for X not found
        if line.contains("class file for") && line.contains("not found") {
            if let Some(start) = line.rfind("class file for ") {
                let rest = &line[start + 15..];
                let cls = rest.split_whitespace().next().unwrap_or("").trim_end_matches(" not").to_string();
                if !cls.is_empty() && !packages.contains(&cls) {
                    packages.push(cls);
                }
            }
        }
    }
    packages
}

fn errors_for_file(output: &str, file_path: &str) -> String {
    // tsc emits relative paths from within the service dir; file_path may be the
    // fully-resolved absolute-or-project-relative path. Match on any suffix.
    let suffixes: Vec<&str> = {
        let mut v = vec![file_path];
        // strip leading service-dir prefix variants (frontend/admin-portal/, services/product-service/)
        for sep in &['/', '\\'] {
            if let Some(pos) = file_path.find(*sep) {
                // try each progressively shorter tail
                let mut rest = &file_path[pos + 1..];
                loop {
                    v.push(rest);
                    match rest.find(*sep) {
                        Some(p) => rest = &rest[p + 1..],
                        None => break,
                    }
                }
            }
        }
        v
    };
    output
        .lines()
        .filter(|l| suffixes.iter().any(|s| l.contains(s)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn print_stories_section(label: &str, stories: &[&canopy_core::UserStory]) {
    if stories.is_empty() { return; }
    println!("  {label} ({})", stories.len());
    for s in stories {
        println!("    [{}] As a {}, I want {}", s.id, s.as_a, s.want);
        println!("          so that {}", s.so_that);
        if !s.depends_on.is_empty() {
            println!("          depends on: {}", s.depends_on.join(", "));
        }
    }
    println!();
}

fn cmd_stories() -> Result<()> {
    let stories = load_user_stories().context("failed to load stories.yaml")?;

    let accepted: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == canopy_core::StoryStatus::Accepted).collect();
    let draft: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == canopy_core::StoryStatus::Draft).collect();
    let rejected: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == canopy_core::StoryStatus::Rejected).collect();

    println!("{} user stories:\n", stories.stories.len());
    print_stories_section("Accepted", &accepted);
    print_stories_section("Draft", &draft);
    print_stories_section("Rejected", &rejected);

    if stories.stories.is_empty() {
        println!("No stories yet. Run `canopy intent` to add your first behavioral requirement.");
    } else {
        println!("Edit .canopy/stories.yaml to curate: set status to accepted | rejected.");
        println!("Run `canopy intent` to add more stories, `canopy spec <id>` to specify an accepted story.");
    }
    Ok(())
}

fn cmd_intent(statement: Option<String>, debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let statement = match statement {
        Some(s) => s,
        None => Input::with_theme(&theme)
            .with_prompt("Behavioral intent")
            .interact_text()
            .context("failed to read intent statement")?,
    };

    let context = load_idea()
        .map(|i| i.description)
        .unwrap_or_else(|_| String::from("No context available."));

    let mut existing = load_user_stories().context("failed to load stories")?;
    let roles = load_roles_registry().context("failed to load roles")?;

    let client = build_client("intent", debug)?;
    println!("\nDeriving stories from intent...");
    let new_stories = generate_stories_from_intent(
        &client, &statement, &context, &existing, &roles,
    ).context("failed to generate stories from intent")?;

    // Gate each new story interactively before saving.
    let existing_ids: std::collections::HashSet<String> =
        existing.stories.iter().map(|s| s.id.clone()).collect();

    let fresh: Vec<_> = new_stories.stories.into_iter()
        .filter(|s| !existing_ids.contains(&s.id))
        .collect();

    if fresh.is_empty() {
        println!("No new stories generated.");
        return Ok(());
    }

    println!("\n{} new story/stories to review:\n", fresh.len());

    let mut accepted_count = 0;
    let mut rejected_count = 0;
    let mut curated: Vec<UserStory> = Vec::new();

    for (i, mut story) in fresh.into_iter().enumerate() {
        println!("--- Story {} ---", i + 1);
        println!("As a   : {}", story.as_a);
        println!("I want : {}", story.want);
        println!("So that: {}", story.so_that);

        let choice = Select::with_theme(&theme)
            .with_prompt("Accept this story?")
            .items(&["Accept", "Accept with edit", "Reject"])
            .default(0)
            .interact()
            .context("failed to read story choice")?;

        match choice {
            0 => {
                story.status = StoryStatus::Accepted;
                accepted_count += 1;
                println!("  Accepted.");
            }
            1 => {
                let want: String = Input::with_theme(&theme)
                    .with_prompt("I want")
                    .with_initial_text(&story.want)
                    .interact_text()
                    .context("failed to read edited want")?;
                let so_that: String = Input::with_theme(&theme)
                    .with_prompt("So that")
                    .with_initial_text(&story.so_that)
                    .interact_text()
                    .context("failed to read edited so_that")?;
                story.want = want;
                story.so_that = so_that;
                story.status = StoryStatus::Accepted;
                accepted_count += 1;
                println!("  Accepted with edits.");
            }
            _ => {
                story.status = StoryStatus::Rejected;
                rejected_count += 1;
                println!("  Rejected.");
            }
        }

        curated.push(story);
    }

    for story in &curated {
        existing.stories.push(story.clone());
    }
    save_user_stories(&existing).context("failed to save stories.yaml")?;

    // Update roles registry from accepted stories only.
    let mut roles = load_roles_registry().context("failed to load roles")?;
    for story in curated.iter().filter(|s| s.status == StoryStatus::Accepted) {
        let role = story.as_a.trim().to_string();
        if !roles.roles.iter().any(|r| r.name().eq_ignore_ascii_case(&role)) {
            roles.roles.push(Role::Simple(role));
        }
    }
    save_roles_registry(&roles).context("failed to save roles.yaml")?;

    // Extract domain vocabulary from accepted stories only.
    let accepted_stories: Vec<_> = curated.iter()
        .filter(|s| s.status == StoryStatus::Accepted)
        .cloned()
        .collect();
    if !accepted_stories.is_empty() {
        print!("Extracting domain vocabulary...");
        match extract_domain_from_stories(&client, &accepted_stories) {
            Ok(extracted) => {
                let mut domain = load_domain_registry().context("failed to load domain registry")?;
                let mut added_entities = 0usize;
                let mut added_events = 0usize;
                for e in &extracted.entities {
                    if !domain.entities.iter().any(|x| x.name().eq_ignore_ascii_case(e.name())) {
                        domain.entities.push(e.clone());
                        added_entities += 1;
                    }
                }
                for e in &extracted.events {
                    if !domain.events.iter().any(|x| x.name().eq_ignore_ascii_case(e.name())) {
                        domain.events.push(e.clone());
                        added_events += 1;
                    }
                }
                save_domain_registry(&domain).context("failed to save domain_registry.yaml")?;
                println!(" +{added_entities} entities, +{added_events} events → .canopy/domain_registry.yaml");
            }
            Err(e) => println!(" (skipped: {e})"),
        }
    }

    println!("\n{accepted_count} accepted, {rejected_count} rejected. Run `canopy stories` to view backlog.");
    Ok(())
}

fn cmd_domain_show() -> Result<()> {
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

fn update_services_from_proposal(services: &mut ServicesRegistry, proposal: &ProposedAdr) {
    let is_infra = proposal.component_type.as_deref() == Some("infrastructure");

    if is_infra {
        // Infrastructure proposals (DB, event broker) describe a shared component, not the owning
        // service. Derive the component name from its technology so it gets its own entry.
        if let Some(ref tech) = proposal.technology {
            let infra_name = tech
                .split_whitespace()
                .next()
                .unwrap_or(tech)
                .to_lowercase();
            if !infra_name.is_empty() && !services.services.iter().any(|s| s.name == infra_name) {
                services.services.push(ServiceEntry {
                    name: infra_name,
                    responsibilities: vec![],
                    technology: Some(tech.clone()),
                    component_type: Some("infrastructure".to_string()),
                });
            }
        }
        return;
    }

    // Frontend proposals often have service: null with the component name in decision instead.
    // Derive the name: for a naming proposal (no technology) use decision; for a tech stack
    // proposal (technology set) find an existing untyped frontend entry.
    let derived_name: Option<String>;
    let name: &str = if let Some(ref svc) = proposal.service {
        if svc.is_empty() { return; }
        svc.as_str()
    } else if proposal.component_type.as_deref() == Some("frontend") {
        if proposal.technology.is_none() {
            // Component-naming proposal: decision holds the frontend name.
            let candidate = proposal.decision.trim();
            if candidate.is_empty() { return; }
            derived_name = Some(candidate.to_string());
            derived_name.as_deref().unwrap()
        } else {
            // Tech stack proposal: apply to the most recent frontend entry without technology.
            if let Some(entry) = services.services.iter_mut().find(|s| {
                s.component_type.as_deref() == Some("frontend") && s.technology.is_none()
            }) {
                entry.technology = proposal.technology.clone();
            }
            return;
        }
    } else {
        return;
    };

    let filtered_responsibilities: Vec<String> = proposal
        .service_responsibilities
        .iter()
        .filter(|r| r.as_str() != "<none>")
        .cloned()
        .collect();

    if let Some(entry) = services.services.iter_mut().find(|s| s.name == *name) {
        for r in &filtered_responsibilities {
            let normalized = r.trim().trim_end_matches('.').to_lowercase();
            let already_present = entry.responsibilities.iter().any(|existing| {
                existing.trim().trim_end_matches('.').to_lowercase() == normalized
            });
            if !already_present {
                entry.responsibilities.push(r.clone());
            }
        }
        // A proposal with an explicit component_type is a tech stack ADR and is authoritative
        // for technology — overrides any accidental earlier setting (e.g. a database ADR that
        // leaked its technology onto the service entry because component_type was not set).
        if entry.technology.is_none() || proposal.component_type.is_some() {
            entry.technology = proposal.technology.clone();
        }
        if entry.component_type.is_none() || proposal.component_type.is_some() {
            entry.component_type = Some(
                proposal.component_type.clone().unwrap_or_else(|| "service".to_string())
            );
        }
    } else {
        services.services.push(ServiceEntry {
            name: name.to_string(),
            responsibilities: filtered_responsibilities,
            technology: proposal.technology.clone(),
            component_type: Some(
                proposal.component_type.clone().unwrap_or_else(|| "service".to_string())
            ),
        });
    }
}

fn cmd_spec(story_id: &str, debug: bool) -> Result<()> {
    use dialoguer::{Select, theme::ColorfulTheme};

    let theme = ColorfulTheme::default();

    let stories = load_user_stories().context("failed to load stories.yaml")?;
    let story = stories
        .stories
        .iter()
        .find(|s| s.id == story_id)
        .ok_or_else(|| anyhow::anyhow!("story '{}' not found", story_id))?;

    if story.status != StoryStatus::Accepted {
        anyhow::bail!(
            "story '{}' has status '{:?}' — only accepted stories can be specified",
            story_id,
            story.status
        );
    }

    println!("\nStory: As a {}, I want {}, so that {}", story.as_a, story.want, story.so_that);

    let mut existing_adrs = load_all_adrs().context("failed to load ADRs")?;
    let mut services = load_services_registry().context("failed to load services registry")?;
    let domain = load_domain_registry().context("failed to load domain registry")?;

    let client = build_client("architect", debug)?;

    println!("\nIdentifying architectural questions...");
    let mut proposed = identify_architectural_questions(&client, story, &existing_adrs, &services)
        .context("failed to identify architectural questions")?;

    if proposed.proposals.is_empty() {
        println!("No architectural questions identified — proceeding to spec generation.");
    } else {
        println!("\n{} architectural question(s) to address:\n", proposed.proposals.len());

        for i in 0..proposed.proposals.len() {
            let proposal = proposed.proposals[i].clone();
            println!("--- Question {} of {} ---", i + 1, proposed.proposals.len());
            println!("Question : {}", proposal.question);
            println!("Proposed : {}", proposal.title);
            println!("Decision : {}", proposal.decision);
            println!("Reason   : {}", proposal.reason);
            if !proposal.alternatives.is_empty() {
                println!("Alternatives: {}", proposal.alternatives.join(", "));
            }
            if let Some(ref svc) = proposal.service {
                if !svc.is_empty() {
                    println!("Service  : {}", svc);
                    if let Some(ref tech) = proposal.technology {
                        if !tech.is_empty() {
                            let ct = proposal.component_type.as_deref().unwrap_or("service");
                            println!("  Technology: {} ({})", tech, ct);
                        }
                    }
                    if !proposal.service_responsibilities.is_empty() {
                        println!("  Responsibilities: {}", proposal.service_responsibilities.join(", "));
                    }
                }
            }

            let choice = Select::with_theme(&theme)
                .with_prompt("Accept this ADR?")
                .items(&["Accept", "Modify decision text", "Reject"])
                .default(0)
                .interact()
                .context("failed to read ADR choice")?;

            match choice {
                0 => {
                    // Accept
                    let adr = Adr {
                        title: proposal.title.clone(),
                        decision: proposal.decision.clone(),
                        reason: proposal.reason.clone(),
                        alternatives: proposal.alternatives.clone(),
                    };
                    let index = existing_adrs.len() + 1;
                    let slug = canopy_storage::intent_slug(&proposal.title);
                    save_adr(index, &slug, &adr).context("failed to save ADR")?;
                    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                    existing_adrs.push(adr);
                    update_services_from_proposal(&mut services, &proposal);
                }
                1 => {
                    // Modify
                    let modified_decision: String = dialoguer::Input::with_theme(&theme)
                        .with_prompt("Enter revised decision text")
                        .with_initial_text(&proposal.decision)
                        .interact_text()
                        .context("failed to read modified decision")?;

                    let mut modified_proposal = proposal.clone();
                    modified_proposal.decision = modified_decision;

                    // If this proposal names a service, let the user rename it so subsequent
                    // proposals (e.g. the database ADR) reference the correct name.
                    if let Some(ref old_name) = proposal.service {
                        if !old_name.is_empty() {
                            let new_name: String = dialoguer::Input::with_theme(&theme)
                                .with_prompt("Service name (leave unchanged to keep current)")
                                .with_initial_text(old_name)
                                .interact_text()
                                .context("failed to read modified service name")?;
                            let new_name = new_name.trim().to_string();
                            if !new_name.is_empty() && &new_name != old_name {
                                // Propagate the rename to all remaining proposals in this batch.
                                for later in proposed.proposals[i + 1..].iter_mut() {
                                    if later.service.as_deref() == Some(old_name) {
                                        later.service = Some(new_name.clone());
                                    }
                                }
                                modified_proposal.service = Some(new_name);
                            }
                        }
                    }

                    let adr = Adr {
                        title: modified_proposal.title.clone(),
                        decision: modified_proposal.decision.clone(),
                        reason: modified_proposal.reason.clone(),
                        alternatives: modified_proposal.alternatives.clone(),
                    };
                    let index = existing_adrs.len() + 1;
                    let slug = canopy_storage::intent_slug(&modified_proposal.title);
                    save_adr(index, &slug, &adr).context("failed to save ADR")?;
                    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                    existing_adrs.push(adr);
                    update_services_from_proposal(&mut services, &modified_proposal);
                }
                _ => {
                    println!("  Rejected — skipping.");
                }
            }
        }

        // Catch any service or frontend that ended up without a decided technology —
        // can happen when the LLM omits a tech stack proposal or the user renames a component.
        let missing_tech: Vec<String> = services.services.iter()
            .filter(|s| {
                let ct = s.component_type.as_deref().unwrap_or("service");
                ct != "infrastructure" && s.technology.is_none()
            })
            .map(|s| s.name.clone())
            .collect();

        for name in missing_tech {
            println!("\n  '{}' has no decided technology.", name);
            let tech: String = Input::with_theme(&theme)
                .with_prompt(format!("Technology for '{}'", name))
                .interact_text()
                .context("failed to read technology")?;
            let tech = tech.trim().to_string();
            if !tech.is_empty() {
                let ct = services.services.iter()
                    .find(|s| s.name == name)
                    .and_then(|s| s.component_type.clone())
                    .unwrap_or_else(|| "service".to_string());
                if let Some(entry) = services.services.iter_mut().find(|s| s.name == name) {
                    entry.technology = Some(tech.clone());
                }
                let adr = Adr {
                    title: format!("Tech stack for {}", name),
                    decision: tech.clone(),
                    reason: format!("Technology for {} decided during spec — no proposal was generated.", name),
                    alternatives: vec![],
                };
                let index = existing_adrs.len() + 1;
                let slug = canopy_storage::intent_slug(&adr.title);
                save_adr(index, &slug, &adr).context("failed to save tech stack ADR")?;
                println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                existing_adrs.push(adr);
                // Ensure component_type is set correctly for scaffold
                if let Some(entry) = services.services.iter_mut().find(|s| s.name == name) {
                    if entry.component_type.is_none() {
                        entry.component_type = Some(ct);
                    }
                }
            }
        }

        save_services_registry(&services).context("failed to save services registry")?;
    }

    println!("\nGenerating BDD spec for story '{}'...", story_id);
    let spec =
        generate_story_spec(&client, story, &existing_adrs, &services, &domain)
            .context("failed to generate story spec")?;

    save_story_spec(story_id, &spec).context("failed to save story spec")?;
    println!("\nSpec saved to .canopy/stories/{}/spec.yaml", story_id);

    println!("\nGenerating OAS 3.1.0 contract...");
    match generate_story_contract(&client, story, &spec, &services, &existing_adrs) {
        Ok(contract_yaml) => {
            save_story_contract(story_id, &contract_yaml).context("failed to save contract")?;
            println!("Contract saved to .canopy/stories/{}/contract.yaml", story_id);
        }
        Err(e) => {
            eprintln!("Warning: contract generation failed: {e}");
        }
    }


    if let Some(ref schema) = spec.entity_schema {
        println!("\nEntity Schema: {}", schema.entity);
        if !schema.system_generated.is_empty() {
            println!("  System-generated:");
            for f in &schema.system_generated {
                println!("    {} ({})  {}", f.name, f.field_type, f.description);
            }
        }
        if !schema.mandatory.is_empty() {
            println!("  Mandatory:");
            for f in &schema.mandatory {
                println!("    {} ({})  {}", f.name, f.field_type, f.description);
            }
        }
        if !schema.optional.is_empty() {
            println!("  Optional:");
            for f in &schema.optional {
                println!("    {} ({})  {}", f.name, f.field_type, f.description);
            }
        }
    }

    println!("\nScenarios:");
    for s in &spec.scenarios {
        println!("  [{}] {}", s.id, s.name);
        for g in &s.given {
            println!("    Given {}", g);
        }
        println!("    When  {}", s.when);
        for t in &s.then {
            println!("    Then  {}", t);
        }
        if !s.constraints.is_empty() {
            println!("    Constraints: {}", s.constraints.join("; "));
        }
    }
    if !spec.out_of_scope.is_empty() {
        println!("\nOut of scope: {}", spec.out_of_scope.join(", "));
    }
    if !spec.open_questions.is_empty() {
        println!("Open questions: {}", spec.open_questions.join("; "));
    }

    Ok(())
}
