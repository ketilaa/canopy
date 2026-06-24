mod roots;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

use canopy_core::*;
use canopy_explore::{
    extract_domain_from_stories, generate_adrs, generate_architecture_principles,
    generate_component_architecture, generate_delivery_intents,
    generate_files, generate_implementation_plan, generate_intent_spec,
    generate_scaffold_from_services, generate_stories_from_intent, generate_story_spec,
    generate_vision, identify_architectural_questions, services_need_jvm, suggest_domain_entities,
    suggest_roles, validate_spec, LlmClient,
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
    /// Regenerate vision.yaml from saved idea
    Vision,
    /// Regenerate delivery_intents.yaml from saved idea and vision
    DeliveryIntents,
    /// Regenerate architecture_principles.yaml from saved vision and delivery intents
    ArchitecturePrinciples,
    /// Show the accumulated domain vocabulary (entities and events)
    Domain,
    /// Generate spec.yaml and plan.yaml for a delivery intent
    Plan {
        /// Intent to plan: index (0, 1, …) or title fragment. Prompts interactively if omitted.
        #[arg(long)]
        intent: Option<String>,
    },
    /// Mark a plan as confirmed so an implementation agent can consume it
    PlanConfirm {
        /// Plan slug (directory name under .canopy/plans/)
        slug: String,
    },
    /// List all plans and their current status
    PlanList,
    /// Derive scaffold commands from component_architecture.yaml and run them
    Scaffold {
        /// Directory to run scaffold commands in (defaults to current directory)
        #[arg(long, default_value = ".")]
        dir: String,
        /// Discard existing scaffold.yaml and regenerate from the LLM
        #[arg(long)]
        regenerate: bool,
    },
    /// Implement all pending tasks in a confirmed plan
    Implement {
        /// Plan slug (directory name under .canopy/plans/)
        slug: String,
    },
    /// Validate generated files against spec scenarios
    Validate {
        /// Plan slug (directory name under .canopy/plans/)
        slug: String,
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
        Commands::Init                   => cmd_init(debug),
        Commands::Vision                 => cmd_vision(debug),
        Commands::DeliveryIntents        => cmd_delivery_intents(debug),
        Commands::ArchitecturePrinciples => cmd_architecture_principles(debug),
        Commands::Domain                 => cmd_domain_show(),
        Commands::Plan { intent }        => cmd_plan(intent, debug),
        Commands::PlanConfirm { slug }   => cmd_plan_confirm(&slug),
        Commands::PlanList               => cmd_plan_list(),
        Commands::Scaffold { dir, regenerate } => cmd_scaffold(&dir, regenerate, debug),
        Commands::Implement { slug }     => cmd_implement(&slug, debug),
        Commands::Validate { slug }      => cmd_validate(&slug, debug),
        Commands::Stories               => cmd_stories(),
        Commands::Intent { statement }   => cmd_intent(statement, debug),
        Commands::Spec { story_id }      => cmd_spec(&story_id, debug),
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
    let client = build_client("explorer", debug)?;
    print!("Suggesting domain entities... ");
    let _ = std::io::stdout().flush();
    match suggest_domain_entities(&client, &idea) {
        Ok(suggestions) if !suggestions.is_empty() => {
            println!();
            let entities = bootstrap_select(&theme, "Domain entities (deselect to remove, add missing below)", &suggestions)?;
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
            let roles = bootstrap_select(&theme, "Roles (deselect to remove, add missing below)", &suggestions)?;
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

fn cmd_vision(debug: bool) -> Result<()> {
    let client = build_client("explorer", debug)?;
    let idea = load_idea()
        .context("No idea.yaml found — run `canopy explore` first")?;
    println!("Generating vision...");
    let vision = generate_vision(&client, &idea, &[])
        .context("failed to generate vision")?;
    save_vision(&vision).context("failed to save vision.yaml")?;
    println!("Saved .canopy/vision.yaml");
    Ok(())
}

fn cmd_delivery_intents(debug: bool) -> Result<()> {
    let client = build_client("explorer", debug)?;
    let idea = load_idea()
        .context("No idea.yaml found — run `canopy explore` first")?;
    let vision = load_vision()
        .context("No vision.yaml found — run `canopy vision` first")?;
    println!("Generating delivery intents...");
    let intents = generate_delivery_intents(&client, &idea, &vision, &[])
        .context("failed to generate delivery intents")?;
    save_delivery_intents(&intents).context("failed to save delivery_intents.yaml")?;
    println!("Saved .canopy/delivery_intents.yaml");
    Ok(())
}

fn cmd_architecture_principles(debug: bool) -> Result<()> {
    let client = build_client("explorer", debug)?;
    let vision = load_vision()
        .context("No vision.yaml found — run `canopy vision` first")?;
    let intents = load_delivery_intents()
        .context("No delivery_intents.yaml found — run `canopy delivery-intents` first")?;
    println!("Generating architecture principles...");
    let principles = generate_architecture_principles(&client, &vision, &intents, &[])
        .context("failed to generate architecture principles")?;
    save_architecture_principles(&principles).context("failed to save architecture_principles.yaml")?;
    println!("Saved .canopy/architecture_principles.yaml");
    Ok(())
}


fn cmd_plan(intent: Option<String>, debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let vision = load_vision()
        .context("No vision.yaml found — run `canopy explore` first")?;
    let intents = load_delivery_intents()
        .context("No delivery_intents.yaml found — run `canopy explore` first")?;
    let principles = load_architecture_principles()
        .context("No architecture_principles.yaml found — run `canopy explore` first")?;
    // Entity vocabulary: Roots is authoritative when available (repository mode).
    // Falls back to the accumulated domain_registry.yaml in greenfield mode.
    let registry = match roots::entity_vocabulary() {
        Some(names) => {
            println!("  Using Roots index for entity vocabulary ({} symbols)", names.len());
            DomainRegistry { entities: names, events: vec![] }
        }
        None => load_domain_registry().context("failed to load domain_registry.yaml")?,
    };

    // Ensure component architecture exists; generate and confirm if not.
    let comp_arch = match load_component_architecture() {
        Ok(a) => a,
        Err(StorageError::NotFound(_)) => {
            let client = build_client("architect", debug)?;
            println!("Component architecture not found. Generating from principles...");
            let arch = generate_component_architecture(&client, &vision, &intents, &principles, &registry)
                .context("failed to generate component architecture")?;

            let arch_yaml = serde_yaml::to_string(&arch)
                .context("failed to serialize component architecture")?;
            println!("\nProposed component architecture:\n{arch_yaml}");

            let accepted = Confirm::with_theme(&theme)
                .with_prompt("Accept this component architecture?")
                .interact()
                .context("failed to read confirmation")?;

            if !accepted {
                println!("Cancelled. Adjust .canopy/architecture_principles.yaml and retry.");
                return Ok(());
            }

            save_component_architecture(&arch)
                .context("failed to save component_architecture.yaml")?;
            println!("Saved .canopy/component_architecture.yaml");

            println!("Generating ADRs...");
            let adrs = generate_adrs(&client, &arch, &principles)
                .context("failed to generate ADRs")?;
            for (i, adr) in adrs.iter().enumerate() {
                let slug = intent_slug(&adr.title);
                save_adr(i + 1, &slug, adr)
                    .context("failed to save ADR")?;
                println!("  Saved .canopy/decisions/adr-{:03}-{}.yaml", i + 1, slug);
            }

            arch
        }
        Err(e) => return Err(e.into()),
    };

    // Select delivery intent.
    let selected_idx = match &intent {
        Some(s) => {
            if let Ok(idx) = s.parse::<usize>() {
                if idx < intents.intents.len() {
                    idx
                } else {
                    anyhow::bail!(
                        "intent index {} out of range (valid: 0..{})",
                        idx,
                        intents.intents.len().saturating_sub(1)
                    );
                }
            } else {
                let s_lower = s.to_lowercase();
                intents
                    .intents
                    .iter()
                    .position(|i| i.title.to_lowercase().contains(&s_lower))
                    .ok_or_else(|| anyhow::anyhow!("no intent matching '{}'", s))?
            }
        }
        None => {
            let titles: Vec<&str> = intents.intents.iter().map(|i| i.title.as_str()).collect();
            Select::with_theme(&theme)
                .with_prompt("Select a delivery intent to plan")
                .items(&titles)
                .default(0)
                .interact()
                .context("failed to read intent selection")?
        }
    };

    let selected_intent = &intents.intents[selected_idx];
    let slug = intent_slug(&selected_intent.title);

    println!("\nPlanning: {}", selected_intent.title);

    let client = build_client("planner", debug)?;

    println!("Generating intent specification...");
    let spec = generate_intent_spec(&client, selected_intent, &registry, &comp_arch)
        .context("failed to generate intent specification")?;

    // Collect answers to open questions.
    let mut answered: Vec<AnsweredQuestion> = Vec::new();
    if !spec.open_questions.is_empty() {
        println!("\nOpen questions (press Enter to skip):\n");
        for (i, q) in spec.open_questions.iter().enumerate() {
            let answer: String = Input::with_theme(&theme)
                .with_prompt(format!("[{}/{}] {}", i + 1, spec.open_questions.len(), q))
                .allow_empty(true)
                .interact_text()
                .context("failed to read answer")?;
            if !answer.trim().is_empty() {
                answered.push(AnsweredQuestion { question: q.clone(), answer });
            }
        }
    }

    println!("\nGenerating implementation plan...");
    let mut plan = generate_implementation_plan(
        &client,
        selected_intent,
        selected_idx,
        &spec,
        &registry,
        &comp_arch,
        &intents,
        &answered,
    )
    .context("failed to generate implementation plan")?;

    plan.intent_index = selected_idx;
    plan.status = "draft".to_string();
    plan.generated_at = unix_timestamp();

    save_intent_spec(&slug, &spec)
        .context("failed to save spec.yaml")?;
    println!("  Saved .canopy/plans/{}/spec.yaml", slug);

    save_implementation_plan(&slug, &plan)
        .context("failed to save plan.yaml")?;
    println!("  Saved .canopy/plans/{}/plan.yaml", slug);

    // Merge this intent's domain scope into the registry so future plans have the vocabulary.
    let mut updated_registry = registry;
    updated_registry.merge(&plan.domain_scope);
    save_domain_registry(&updated_registry)
        .context("failed to save domain_registry.yaml")?;
    println!("  Updated .canopy/domain_registry.yaml ({} entities, {} events)",
        updated_registry.entities.len(), updated_registry.events.len());

    println!("\nPlan '{}' created (status: draft)", slug);
    if plan.open_questions.iter().any(|q| q.blocking && q.answer.is_none()) {
        println!("  WARNING: plan has unresolved blocking questions — review plan.yaml before confirming");
    }
    println!("Run `canopy plan-confirm {}` when ready.", slug);
    Ok(())
}

fn cmd_plan_confirm(slug: &str) -> Result<()> {
    let mut plan = load_implementation_plan(slug)
        .with_context(|| format!("no plan '{slug}' found — run `canopy plan` first"))?;

    if plan.status == "confirmed" {
        println!("Plan '{slug}' is already confirmed.");
        return Ok(());
    }

    plan.status = "confirmed".to_string();
    save_implementation_plan(slug, &plan)
        .with_context(|| format!("failed to save confirmed plan '{slug}'"))?;

    println!("Plan '{slug}' confirmed — ready for implementation.");
    Ok(())
}

fn cmd_plan_list() -> Result<()> {
    let slugs = list_plans().context("failed to list plans")?;

    if slugs.is_empty() {
        println!("No plans found. Run `canopy plan` to create one.");
        return Ok(());
    }

    println!("Plans:");
    for slug in &slugs {
        let status = match load_implementation_plan(slug) {
            Ok(p) => p.status,
            Err(_) => "unknown".to_string(),
        };
        println!("  {:<40}  [{}]", slug, status);
    }
    Ok(())
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

fn cmd_implement(slug: &str, debug: bool) -> Result<()> {
    let plan = load_implementation_plan(slug)
        .with_context(|| format!("no plan '{slug}' — run `canopy plan` first"))?;

    if plan.status != "confirmed" {
        anyhow::bail!(
            "plan '{slug}' is '{}' — run `canopy plan-confirm {slug}` first",
            plan.status
        );
    }

    let pending: Vec<_> = plan.tasks.iter().filter(|t| !t.completed).collect();
    if pending.is_empty() {
        println!("All tasks in '{slug}' are already complete.");
        return Ok(());
    }

    let spec = load_intent_spec(slug)
        .with_context(|| format!("no spec for '{slug}'"))?;
    let comp_arch = load_component_architecture()
        .context("no component_architecture.yaml — run `canopy plan` first")?;
    let intents = load_delivery_intents()
        .context("no delivery_intents.yaml — run `canopy explore` first")?;
    let intent = intents.intents.get(plan.intent_index)
        .ok_or_else(|| anyhow::anyhow!("intent index {} out of range", plan.intent_index))?;

    // Roots context for repository mode; None in greenfield.
    let roots_ctx = roots::get_feature_context(&intent.title)
        .map(|p| serde_json::to_string(&p).unwrap_or_default());
    if roots_ctx.is_some() {
        println!("Using Roots context for '{}'", intent.title);
    }

    let client = build_client("developer", debug)?;

    println!("\nImplementing {} pending task(s) for '{slug}'...", pending.len());
    let output = generate_files(&client, intent, &spec, &plan, &comp_arch, roots_ctx.as_deref())
        .context("developer LLM call failed")?;

    // Write files to disk.
    for file in &output.files {
        let dest = std::path::Path::new(&file.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory for {}", file.path))?;
        }
        std::fs::write(dest, &file.content)
            .with_context(|| format!("failed to write {}", file.path))?;
        println!("  wrote  {}", file.path);
    }

    // Mark tasks whose declared outputs were generated as completed.
    let generated_paths: std::collections::HashSet<&str> =
        output.files.iter().map(|f| f.path.as_str()).collect();

    let mut updated_plan = plan;
    for task in updated_plan.tasks.iter_mut() {
        if !task.completed && task.outputs.iter().any(|o| generated_paths.contains(o.as_str())) {
            task.completed = true;
        }
    }
    updated_plan.status = if updated_plan.tasks.iter().all(|t| t.completed) {
        "complete".to_string()
    } else {
        "in_progress".to_string()
    };

    save_implementation_plan(slug, &updated_plan)
        .context("failed to save updated plan")?;

    println!(
        "\n{} file(s) written. Plan '{}' status: {}",
        output.files.len(),
        slug,
        updated_plan.status
    );

    // Keep the Roots index current so the next LLM context query reflects the new files.
    if std::path::Path::new(".roots/index.db").exists() {
        use std::io::Write;
        print!("Updating Roots index... ");
        let _ = std::io::stdout().flush();
        roots::reindex();
        println!("done");
    }

    println!("Run `canopy validate {slug}` to verify against spec scenarios.");
    Ok(())
}

fn cmd_validate(slug: &str, debug: bool) -> Result<()> {
    let plan = load_implementation_plan(slug)
        .with_context(|| format!("no plan '{slug}' — run `canopy plan` first"))?;

    if matches!(plan.status.as_str(), "draft" | "confirmed") {
        anyhow::bail!(
            "plan '{slug}' has not been implemented yet (status: {})",
            plan.status
        );
    }

    let spec = load_intent_spec(slug)
        .with_context(|| format!("no spec for '{slug}'"))?;
    let intents = load_delivery_intents()
        .context("no delivery_intents.yaml — run `canopy explore` first")?;
    let intent = intents.intents.get(plan.intent_index)
        .ok_or_else(|| anyhow::anyhow!("intent index {} out of range", plan.intent_index))?;

    // Collect all output files from tasks.
    let mut generated_files: Vec<(String, String)> = Vec::new();
    for task in &plan.tasks {
        for output_path in &task.outputs {
            let p = std::path::Path::new(output_path);
            if p.exists() {
                match std::fs::read_to_string(p) {
                    Ok(content) => generated_files.push((output_path.clone(), content)),
                    Err(e) => eprintln!("  warning: could not read {output_path}: {e}"),
                }
            }
        }
    }

    if generated_files.is_empty() {
        anyhow::bail!(
            "no output files found for '{slug}' — run `canopy implement {slug}` first"
        );
    }

    let client = build_client("validator", debug)?;

    println!("Validating {slug} ({} files, {} scenarios)...",
        generated_files.len(), spec.scenarios.len());

    let report = validate_spec(&client, intent, &spec, &generated_files)
        .context("validator LLM call failed")?;

    save_validation_report(slug, &report)
        .context("failed to save validation report")?;

    println!("\nValidation results for '{slug}':");
    println!("  {}/{} scenarios passed\n", report.passed, report.total);
    for result in &report.results {
        let icon = if result.passed { "✓" } else { "✗" };
        println!("  {icon} [{}] {}", result.scenario_id, result.scenario_name);
        if !result.passed {
            println!("    reason: {}", result.reasoning);
            for issue in &result.issues {
                println!("    issue:  {issue}");
            }
        }
    }

    println!("\nReport saved to .canopy/plans/{slug}/validation.yaml");

    if report.passed < report.total {
        anyhow::bail!(
            "{} scenario(s) failed — fix the implementation and re-run `canopy validate {slug}`",
            report.total - report.passed
        );
    }
    Ok(())
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

    let client = build_client("explorer", debug)?;
    println!("\nDeriving stories from intent...");
    let new_stories = generate_stories_from_intent(
        &client, &statement, &context, &existing, &roles,
    ).context("failed to generate stories from intent")?;

    // Merge new stories (skip any whose ID already exists)
    let existing_ids: std::collections::HashSet<String> =
        existing.stories.iter().map(|s| s.id.clone()).collect();
    let mut added = 0;
    for story in new_stories.stories {
        if !existing_ids.contains(&story.id) {
            existing.stories.push(story);
            added += 1;
        }
    }
    save_user_stories(&existing).context("failed to save stories.yaml")?;

    // Update roles registry with new as_a values from accepted+draft stories
    let mut roles = load_roles_registry().context("failed to load roles")?;
    for story in &existing.stories {
        let role = story.as_a.trim().to_string();
        if !roles.roles.iter().any(|r| r.eq_ignore_ascii_case(&role)) {
            roles.roles.push(role);
        }
    }
    save_roles_registry(&roles).context("failed to save roles.yaml")?;

    // Extract domain vocabulary from the new stories
    let new_story_slice: Vec<_> = existing.stories.iter()
        .filter(|s| !existing_ids.contains(&s.id))
        .cloned()
        .collect();
    if !new_story_slice.is_empty() {
        print!("Extracting domain vocabulary...");
        match extract_domain_from_stories(&client, &new_story_slice) {
            Ok(extracted) => {
                let mut domain = load_domain_registry().context("failed to load domain registry")?;
                let mut added_entities = 0usize;
                let mut added_events = 0usize;
                for e in &extracted.entities {
                    if !domain.entities.iter().any(|x| x.eq_ignore_ascii_case(e)) {
                        domain.entities.push(e.clone());
                        added_entities += 1;
                    }
                }
                for e in &extracted.events {
                    if !domain.events.iter().any(|x| x.eq_ignore_ascii_case(e)) {
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

    println!("Added {added} new stories. Run `canopy stories` to review.");
    println!("Edit .canopy/stories.yaml to set status: accepted | rejected.");
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
        println!("  {}", e);
    }

    println!("\nEvents ({}):", domain.events.len());
    for e in &domain.events {
        println!("  {}", e);
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

    let Some(ref name) = proposal.service else { return };
    if name.is_empty() {
        return;
    }

    let filtered_responsibilities: Vec<String> = proposal
        .service_responsibilities
        .iter()
        .filter(|r| r.as_str() != "<none>")
        .cloned()
        .collect();

    if let Some(entry) = services.services.iter_mut().find(|s| s.name == *name) {
        for r in &filtered_responsibilities {
            if !entry.responsibilities.contains(r) {
                entry.responsibilities.push(r.clone());
            }
        }
        if entry.technology.is_none() {
            entry.technology = proposal.technology.clone();
        }
        if entry.component_type.is_none() {
            entry.component_type = proposal.component_type.clone();
        }
    } else {
        services.services.push(ServiceEntry {
            name: name.clone(),
            responsibilities: filtered_responsibilities,
            technology: proposal.technology.clone(),
            component_type: proposal.component_type.clone(),
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
    let proposed = identify_architectural_questions(&client, story, &existing_adrs, &services)
        .context("failed to identify architectural questions")?;

    if proposed.proposals.is_empty() {
        println!("No architectural questions identified — proceeding to spec generation.");
    } else {
        println!("\n{} architectural question(s) to address:\n", proposed.proposals.len());

        for (i, proposal) in proposed.proposals.iter().enumerate() {
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
                    update_services_from_proposal(&mut services, proposal);
                }
                1 => {
                    // Modify
                    let modified: String = dialoguer::Input::with_theme(&theme)
                        .with_prompt("Enter revised decision text")
                        .with_initial_text(&proposal.decision)
                        .interact_text()
                        .context("failed to read modified decision")?;
                    let adr = Adr {
                        title: proposal.title.clone(),
                        decision: modified,
                        reason: proposal.reason.clone(),
                        alternatives: proposal.alternatives.clone(),
                    };
                    let index = existing_adrs.len() + 1;
                    let slug = canopy_storage::intent_slug(&proposal.title);
                    save_adr(index, &slug, &adr).context("failed to save ADR")?;
                    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
                    existing_adrs.push(adr);
                    update_services_from_proposal(&mut services, proposal);
                }
                _ => {
                    println!("  Rejected — skipping.");
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
