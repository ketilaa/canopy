mod roots;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

use canopy_core::*;
use canopy_llm::{
    execute_implementation_step, execute_implementation_stub, execute_implementation_with_test,
    extract_domain_from_stories, fix_file, generate_scaffold_from_services,
    generate_stories_from_intent, generate_story_contract, generate_story_plan, generate_story_spec,
    generate_unit_test_stub, identify_architectural_questions, propose_dependencies,
    services_need_jvm, skill_for_build_system, skill_for_technology, skills_for_architecture,
    suggest_domain_entities, suggest_roles, testing_skill_for_file_with_adrs, LlmClient, StepResult,
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
}

/// Used only for parsing commands typed inside the REPL.
#[derive(Parser)]
#[command(name = "canopy")]
struct ReplCli {
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
    /// Show the global dependency decision log
    Dependencies,
}

fn uuid_v4() -> String {
    let mut buf = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
    }
    buf[6] = (buf[6] & 0x0f) | 0x40;
    buf[8] = (buf[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]
    )
}

fn iso_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = epoch_to_parts(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

fn epoch_to_parts(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60; let secs = secs / 60;
    let mi = secs % 60; let secs = secs / 60;
    let h = secs % 24; let days = secs / 24;
    let mut year = 1970u64;
    let mut rem = days;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let dy = if leap { 366 } else { 365 };
        if rem < dy { break; }
        rem -= dy; year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [31u64,if leap{29}else{28},31,30,31,30,31,31,30,31,30,31];
    let mut mo = 1u64;
    for &days_in_month in &months {
        if rem < days_in_month { break; }
        rem -= days_in_month; mo += 1;
    }
    (year, mo, rem + 1, h, mi, s)
}

fn build_client(agent: &str, debug: bool) -> Result<LlmClient> {
    let client = match canopy_storage::load_config()
        .context("failed to read .canopy/config.yaml")?
    {
        Some(cfg) => {
            let agent_cfg = cfg.for_agent(agent).ok_or_else(|| {
                anyhow::anyhow!(
                    "no LLM config for agent '{}' and no default in .canopy/config.yaml",
                    agent
                )
            })?;
            LlmClient::from_agent_config(&agent_cfg, debug)
        }
        None => LlmClient::default_local(debug),
    };
    if debug {
        Ok(client.with_log_path(".canopy/logs/llm-debug.log"))
    } else {
        Ok(client)
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    run_repl(cli.llm_debug)
}

fn run_repl(debug: bool) -> Result<()> {
    use std::io::Write;

    // Session created once for the lifetime of this canopy invocation.
    let session_id = uuid_v4();
    let session_log_dir = canopy_storage::storage_dir()
        .join("logs")
        .join(&session_id);
    let fix_log_dir = session_log_dir.join("fix-loops");
    let _ = std::fs::create_dir_all(&fix_log_dir);
    let _ = std::fs::write(
        session_log_dir.join("session.yaml"),
        format!("started_at: \"{}\"\n", iso_now()),
    );

    println!("canopy  —  type a command or 'exit'");
    println!("Session: {}", session_id);

    // Roots: initialise and index the repository once for the whole session.
    print!("Checking Roots index... ");
    let _ = std::io::stdout().flush();
    roots::ensure_indexed();
    println!("ready");

    let history_path = canopy_storage::storage_dir().join("history");
    let mut rl = rustyline::DefaultEditor::new()?;
    let _ = rl.load_history(&history_path);

    loop {
        let input = match rl.readline("\ncanopy> ") {
            Ok(line) => line,
            Err(rustyline::error::ReadlineError::Eof)
            | Err(rustyline::error::ReadlineError::Interrupted) => break,
            Err(e) => { eprintln!("input error: {e}"); break; }
        };

        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }
        if matches!(trimmed, "exit" | "quit") { break; }

        let _ = rl.add_history_entry(trimmed);

        // Parse the typed command using ReplCli, prepending the binary name.
        let mut args = vec!["canopy"];
        if debug { args.push("--llm-debug"); }
        args.extend(trimmed.split_whitespace());

        match ReplCli::try_parse_from(args) {
            Ok(cli) => {
                if let Some(cmd) = cli.command {
                    let result = match cmd {
                        Commands::Init                         => cmd_init(debug),
                        Commands::Domain                       => cmd_domain_show(),
                        Commands::Scaffold { dir, regenerate } => cmd_scaffold(&dir, regenerate, debug),
                        Commands::Implement { story_id }       => cmd_implement(&story_id, debug, &fix_log_dir),
                        Commands::Stories                      => cmd_stories(),
                        Commands::Intent { statement }         => cmd_intent(statement, debug),
                        Commands::Spec { story_id }            => cmd_spec(&story_id, debug),
                        Commands::Dependencies                 => cmd_dependencies(),
                    };
                    if let Err(e) = result {
                        eprintln!("  error: {e:#}");
                    }
                }
            }
            Err(e) => eprintln!("{e}"),
        }
    }

    let _ = rl.save_history(&history_path);
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

    // Event broker — mandatory for event-driven architecture; saved at adr-002
    let arch_decision = arch_adr.decision.to_lowercase();
    if arch_decision.contains("event-driven") || arch_decision.contains("event driven") {
        let broker_options = [
            "Redpanda  (Kafka-compatible, first-class Docker support — recommended for local dev)",
            "Apache Kafka",
            "RabbitMQ  (AMQP message broker)",
            "NATS      (lightweight, cloud-native messaging)",
        ];
        let broker_idx = Select::with_theme(&theme)
            .with_prompt("Event broker")
            .items(&broker_options)
            .default(0)
            .interact()
            .context("failed to read event broker selection")?;
        let broker_adr = event_broker_adr(broker_idx);
        save_adr(2, "event-broker", &broker_adr)
            .context("failed to save adr-002-event-broker.yaml")?;
        println!("  Saved .canopy/decisions/adr-002-event-broker.yaml");

        let convention_options = [
            "<aggregate>-events  (e.g. product-events, order-events — one topic per aggregate)",
            "<service>-events    (e.g. product-service-events — one topic per service)",
            "<domain>.<aggregate>.events  (reverse-DNS style, e.g. commerce.product.events)",
        ];
        let convention_idx = Select::with_theme(&theme)
            .with_prompt("Topic naming convention")
            .items(&convention_options)
            .default(0)
            .interact()
            .context("failed to read topic naming convention selection")?;
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

fn topic_naming_convention_adr(idx: usize) -> Adr {
    match idx {
        0 => Adr {
            title: "Topic Naming Convention".to_string(),
            decision: "One topic per aggregate: <aggregate>-events (e.g. product-events, order-events)".to_string(),
            reason: "Scoping topics to the aggregate gives per-entity ordering guarantees when \
                     partitioned by entity ID, clean schema evolution per aggregate type, and \
                     lets consumers subscribe only to the aggregates they care about. \
                     Finer granularity (one topic per event type) creates subscription sprawl; \
                     coarser granularity (one topic per service or one global topic) loses \
                     ordering and complicates schema management."
                .to_string(),
            alternatives: vec![
                "One topic per service: <service>-events".to_string(),
                "Reverse-DNS style: <domain>.<aggregate>.events".to_string(),
            ],
        },
        1 => Adr {
            title: "Topic Naming Convention".to_string(),
            decision: "One topic per service: <service>-events (e.g. product-service-events)".to_string(),
            reason: "Groups all events from a single deployable service under one topic. \
                     Simpler when a service owns a single aggregate, but conflates service \
                     boundaries with aggregate boundaries as the service grows."
                .to_string(),
            alternatives: vec![
                "One topic per aggregate: <aggregate>-events".to_string(),
            ],
        },
        _ => Adr {
            title: "Topic Naming Convention".to_string(),
            decision: "Reverse-DNS style: <domain>.<aggregate>.events (e.g. commerce.product.events)".to_string(),
            reason: "Namespaced topics prevent collisions in multi-domain Kafka clusters and \
                     make ownership explicit. Common in large organisations sharing a single broker."
                .to_string(),
            alternatives: vec![
                "One topic per aggregate: <aggregate>-events".to_string(),
            ],
        },
    }
}

fn event_broker_adr(idx: usize) -> Adr {
    match idx {
        0 => Adr {
            title: "Event Broker".to_string(),
            decision: "Redpanda as the event broker".to_string(),
            reason: "Redpanda is Kafka-compatible (same producer/consumer API) with no JVM dependency \
                     and first-class Docker Compose support. It starts in milliseconds and requires \
                     no ZooKeeper, making it the lowest-friction choice for local development in an \
                     event-driven microservices architecture."
                .to_string(),
            alternatives: vec![
                "Apache Kafka".to_string(),
                "RabbitMQ".to_string(),
                "NATS".to_string(),
            ],
        },
        1 => Adr {
            title: "Event Broker".to_string(),
            decision: "Apache Kafka as the event broker".to_string(),
            reason: "Kafka is the de facto standard for high-throughput, durable event streaming. \
                     Its log-based model supports event replay and consumer group fan-out, \
                     aligning naturally with event-sourcing and DDD patterns."
                .to_string(),
            alternatives: vec![
                "Redpanda (Kafka-compatible, no JVM)".to_string(),
                "RabbitMQ".to_string(),
            ],
        },
        2 => Adr {
            title: "Event Broker".to_string(),
            decision: "RabbitMQ as the event broker".to_string(),
            reason: "RabbitMQ is a mature AMQP message broker well-suited to flexible routing \
                     patterns (exchanges, queues, bindings). It is simpler to operate than Kafka \
                     when throughput requirements are modest."
                .to_string(),
            alternatives: vec![
                "Apache Kafka".to_string(),
                "Redpanda (Kafka-compatible, no JVM)".to_string(),
            ],
        },
        _ => Adr {
            title: "Event Broker".to_string(),
            decision: "NATS as the event broker".to_string(),
            reason: "NATS is a lightweight, cloud-native messaging system with a tiny footprint. \
                     NATS JetStream adds persistence and replay. Good choice when low latency \
                     and operational simplicity matter more than Kafka's log-retention guarantees."
                .to_string(),
            alternatives: vec![
                "Redpanda (Kafka-compatible, no JVM)".to_string(),
                "Apache Kafka".to_string(),
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

/// Prompts for a testing framework and saves the ADR when the accepted tech stack is React/Vite
/// or Node.js/Express and no testing strategy ADR for this service exists yet.
/// Title and slug are scoped to the service so each frontend or backend can choose independently:
///   admin-portal → "Admin Portal Testing Strategy", adr-NNN-admin-portal-testing-strategy.yaml
///   product      → "Product Testing Strategy",      adr-NNN-product-testing-strategy.yaml
/// Angular TestBed and Spring Boot JUnit 5 are implicit — no prompt needed.
fn maybe_prompt_testing_strategy(
    theme: &dialoguer::theme::ColorfulTheme,
    existing_adrs: &mut Vec<Adr>,
    technology: &str,
    service_name: &str,
) -> Result<()> {
    let t = technology.to_lowercase();
    let is_react = t.contains("react") || t.contains("vite");
    let is_node  = t.contains("node") || t.contains("express") || t.contains("nest");
    if !is_react && !is_node {
        return Ok(());
    }
    let display_name: String = service_name.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let slug = format!("{}-testing-strategy", service_name);
    let title = format!("{} Testing Strategy", display_name);
    if existing_adrs.iter().any(|a| a.title.to_lowercase() == title.to_lowercase()) {
        return Ok(());
    }
    let (prompt, options, adr_offset) = if is_react {
        (
            format!("{} testing framework", display_name),
            vec![
                "Vitest + React Testing Library  (recommended for Vite)",
                "Jest  + React Testing Library",
            ],
            0usize,
        )
    } else {
        (
            format!("{} testing framework", display_name),
            vec![
                "Jest  + Supertest  (recommended — de facto standard)",
                "Vitest + Supertest",
            ],
            2usize,
        )
    };
    let idx = Select::with_theme(theme)
        .with_prompt(&prompt)
        .items(&options)
        .default(0)
        .interact()
        .context("failed to read testing framework selection")?;
    let adr = testing_strategy_adr(adr_offset + idx, &display_name);
    let index = existing_adrs.len() + 1;
    save_adr(index, &slug, &adr)
        .context("failed to save testing-strategy ADR")?;
    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
    existing_adrs.push(adr);
    Ok(())
}

/// Pre-authored testing strategy ADRs. Title is scoped to the service name.
/// 0 = React + Vitest, 1 = React + Jest, 2 = Node + Jest, 3 = Node + Vitest.
/// Angular TestBed and Spring Boot JUnit 5 are implicit — no ADR needed.
fn testing_strategy_adr(idx: usize, service: &str) -> Adr {
    match idx {
        0 => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Vitest + React Testing Library for unit tests".to_string(),
            reason: "Vitest is the natural testing companion for Vite-based React projects. \
                     It shares the Vite config, runs natively in ES modules, and is significantly \
                     faster than Jest for component tests. React Testing Library enforces \
                     user-behaviour-oriented assertions over implementation details."
                .to_string(),
            alternatives: vec![
                "Jest + React Testing Library".to_string(),
                "Playwright (component mode)".to_string(),
            ],
        },
        1 => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Jest + React Testing Library for unit tests".to_string(),
            reason: "Jest is the most widely adopted JavaScript test runner and works well with \
                     non-Vite React setups. It has the broadest ecosystem of matchers and utilities."
                .to_string(),
            alternatives: vec![
                "Vitest + React Testing Library".to_string(),
                "Playwright (component mode)".to_string(),
            ],
        },
        2 => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Jest + Supertest for unit and route tests".to_string(),
            reason: "Jest is the de facto standard test runner for Node.js projects. \
                     Supertest provides a clean HTTP-layer assertion API that exercises \
                     the full Express middleware stack without starting a real server."
                .to_string(),
            alternatives: vec![
                "Vitest + Supertest".to_string(),
                "Mocha + Chai + Supertest".to_string(),
            ],
        },
        _ => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Vitest + Supertest for unit and route tests".to_string(),
            reason: "Vitest offers a faster, ES-module-native alternative to Jest for Node.js \
                     projects. Its API is Jest-compatible, so migration is low-risk. \
                     Supertest provides the same HTTP-layer assertions."
                .to_string(),
            alternatives: vec![
                "Jest + Supertest".to_string(),
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

/// True for Java implementation files that benefit from a unit-test TDD cycle.
/// Excludes: build files, Spring Data repositories (interfaces), Application entry point,
/// configuration classes, and anything already in the test source tree.
fn is_tdd_candidate(file: &str) -> bool {
    if !file.ends_with(".java") || !file.contains("/src/main/java/") {
        return false;
    }
    let filename = std::path::Path::new(file)
        .file_name().and_then(|n| n.to_str()).unwrap_or("");
    !filename.ends_with("Application.java")
        && !filename.ends_with("Repository.java")
        && !filename.ends_with("Configuration.java")
        && !filename.ends_with("Config.java")
}

/// Maps `src/main/java/.../Foo.java` → `src/test/java/.../FooTest.java`.
fn derive_test_file_path(impl_file: &str) -> Option<String> {
    if !impl_file.contains("/src/main/java/") {
        return None;
    }
    let test_path = impl_file.replace("/src/main/java/", "/src/test/java/");
    let p = std::path::Path::new(&test_path);
    let stem = p.file_stem()?.to_str()?;
    let parent = p.parent()?.to_str()?;
    Some(format!("{}/{}Test.java", parent, stem))
}

/// Extracts the simple class name from a test file path (the file stem).
fn test_class_name(test_file: &str) -> Option<String> {
    std::path::Path::new(test_file)
        .file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
}

/// Builds the sibling context section for an implementation step prompt.
///
/// Tries Roots symbol surfaces first (compact). Falls back to reading full file
/// content from session_written or disk when Roots is unavailable.
fn build_sibling_section(
    deps: &[String],
    service_dir: &str,
    session_written: &std::collections::HashMap<String, String>,
) -> String {
    if deps.is_empty() {
        return String::new();
    }
    let prefix = format!("{}/", service_dir);
    let rel_paths: Vec<String> = deps.iter()
        .filter_map(|d| d.strip_prefix(&prefix).map(|s| s.to_string()))
        .collect();

    if let Some(surface) = roots::get_ts_module_surface(&rel_paths, service_dir) {
        return surface;
    }

    let mut parts: Vec<String> = Vec::new();
    for dep in deps {
        let rel = dep.strip_prefix(&prefix).unwrap_or(dep.as_str());
        let content = session_written.get(dep)
            .cloned()
            .or_else(|| std::fs::read_to_string(dep).ok());
        if let Some(c) = content {
            parts.push(format!("// {}\n{}", rel, c));
        }
    }
    parts.join("\n\n")
}

/// Returns a compile-only command (no test execution) for the service's build tool.
fn compile_command_for_service(service: &ServiceEntry, _service_dir: &str) -> String {
    let tech = service.technology.as_deref().unwrap_or("").to_lowercase();
    if tech.contains("spring") || tech.contains("maven") || tech.contains("java") {
        return "./mvnw test-compile -B".to_string();
    }
    if tech.contains("gradle") {
        return "./gradlew compileTestJava".to_string();
    }
    "npx tsc --noEmit".to_string()
}

/// Returns a test command scoped to a single test class.
fn test_class_command_for_service(service: &ServiceEntry, test_class: &str) -> String {
    let tech = service.technology.as_deref().unwrap_or("").to_lowercase();
    if tech.contains("spring") || tech.contains("maven") || tech.contains("java") {
        return format!("./mvnw test -Dtest={} -B", test_class);
    }
    if tech.contains("gradle") {
        return format!("./gradlew test --tests '*.{}'", test_class);
    }
    format!("npm test -- --testPathPattern={} --watchAll=false", test_class)
}

/// Runs a build/test command and iterates an LLM fix loop until it succeeds or max_iterations is hit.
/// `skip_files` lists files that must not be modified (e.g. the unit test in the Green phase).
/// `adrs` is used to resolve the correct testing skill when fixing test files.
/// Returns true when the command exits successfully.
fn run_fix_loop_logged(
    client: &LlmClient,
    service: &ServiceEntry,
    service_dir: &str,
    build_cmd: &str,
    service_source_files: &[String],
    skip_files: &[String],
    adrs: &[Adr],
    arch_skills: &str,
    max_iterations: usize,
    fix_log_dir: Option<&std::path::Path>,
    step_label: &str,
) -> bool {
    let mut telemetry_iterations: Vec<String> = Vec::new();
    let mut total_iterations = 0usize;

    let result = run_fix_loop_inner(
        client, service, service_dir, build_cmd, service_source_files,
        skip_files, adrs, arch_skills, max_iterations,
        &mut telemetry_iterations, &mut total_iterations,
    );

    if let Some(log_dir) = fix_log_dir {
        let label = step_label.rsplit('/').next().unwrap_or(step_label)
            .replace('.', "_");
        let log_path = log_dir.join(format!("{}-{}.yaml", service.name, label));
        let tech = service.technology.as_deref().unwrap_or("unknown");
        let passed = if result { "true" } else { "false" };
        let iterations_yaml = if telemetry_iterations.is_empty() {
            "  - iteration: 1\n    errors: []\n    result: pass\n".to_string()
        } else {
            telemetry_iterations.join("")
        };
        let content = format!(
            "service: \"{}\"\ntechnology: \"{}\"\nstep: \"{}\"\ntotal_iterations: {}\npassed: {}\niterations:\n{}",
            service.name, tech, step_label, total_iterations, passed, iterations_yaml
        );
        let _ = std::fs::write(&log_path, content);
    }

    result
}

fn run_fix_loop_inner(
    client: &LlmClient,
    service: &ServiceEntry,
    service_dir: &str,
    build_cmd: &str,
    service_source_files: &[String],
    skip_files: &[String],
    adrs: &[Adr],
    arch_skills: &str,
    max_iterations: usize,
    telemetry: &mut Vec<String>,
    total_iterations: &mut usize,
) -> bool {
    for iteration in 0..max_iterations {
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(build_cmd)
            .current_dir(service_dir)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => { eprintln!("  failed to run command: {e}"); return false; }
        };

        let combined = strip_ansi(format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));

        if output.status.success() {
            println!("  ✓ {}", service.name);
            return true;
        }

        let missing_pkgs = extract_missing_packages(&combined);
        let mut fixed_any = false;
        let broken_files: Vec<String> = extract_error_files(&combined, service_dir)
            .into_iter()
            .filter(|f| !skip_files.contains(f))
            .collect();

        if missing_pkgs.iter().any(|p| p.starts_with("javax.")) {
            let n = migrate_javax_to_jakarta(service_dir);
            if n > 0 {
                println!("  migrated javax.* → jakarta.* in {n} file(s)");
                fixed_any = true;
            }
        }

        let pom_validation = extract_pom_validation_errors(&combined);
        let unresolvable = extract_unresolvable_dependencies(&combined);
        let non_javax: Vec<_> = missing_pkgs.iter().filter(|p| !p.starts_with("javax.")).collect();
        if !pom_validation.is_empty() || !unresolvable.is_empty() || !non_javax.is_empty() {
            let build_file = format!("{service_dir}/pom.xml");
            if std::path::Path::new(&build_file).exists() {
                let content = std::fs::read_to_string(&build_file).unwrap_or_default();
                if !content.is_empty() {
                    let mut error_lines = Vec::new();
                    if !pom_validation.is_empty() {
                        error_lines.push(format!(
                            "Maven POM validation errors (artifact not in parent BOM — remove or replace with correct artifactId):\n{}",
                            pom_validation.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join("\n")
                        ));
                    }
                    if !unresolvable.is_empty() {
                        error_lines.push(format!(
                            "Unresolvable dependencies — do not exist on Maven Central. Remove from pom.xml:\n{}",
                            unresolvable.iter().map(|c| format!("  - {c}")).collect::<Vec<_>>().join("\n")
                        ));
                    }
                    if !non_javax.is_empty() {
                        error_lines.push(format!(
                            "Missing packages not in pom.xml:\n{}",
                            non_javax.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n")
                        ));
                    }
                    let errors = error_lines.join("\n\n");
                    println!("  fixing pom.xml ({} pom, {} unresolvable, {} missing)",
                        pom_validation.len(), unresolvable.len(), non_javax.len());
                    let pom_skill = skill_for_build_system(&build_file);
                    match fix_file(client, &build_file, &content, &errors, service_source_files, &[], &pom_skill, arch_skills) {
                        Ok(fixed) => { let _ = std::fs::write(&build_file, &fixed); fixed_any = true; }
                        Err(e) => eprintln!("    LLM fix failed for pom.xml: {e}"),
                    }
                }
            }
        }

        // Collect telemetry for this iteration
        *total_iterations += 1;
        {
            let error_patterns: Vec<String> = combined.lines()
                .filter(|l| {
                    let lo = l.to_lowercase();
                    lo.contains("error") || lo.contains("cannot find") || lo.contains("is not assignable")
                })
                .take(5)
                .map(|l| format!("    - \"{}\"", l.trim().replace('"', "'")))
                .collect();
            let files_yaml = broken_files.iter()
                .map(|f| format!("    - \"{}\"", f))
                .collect::<Vec<_>>()
                .join("\n");
            let patterns_yaml = if error_patterns.is_empty() {
                "    - \"(no matching error lines)\"".to_string()
            } else {
                error_patterns.join("\n")
            };
            telemetry.push(format!(
                "  - iteration: {}\n    files_with_errors:\n{}\n    error_patterns:\n{}\n",
                iteration + 1,
                if files_yaml.is_empty() { "    []".to_string() } else { files_yaml },
                patterns_yaml,
            ));
        }

        if broken_files.is_empty() && !fixed_any {
            eprintln!("  No fixable errors found — manual fix needed.");
            eprintln!("{combined}");
            return false;
        }

        if !broken_files.is_empty() {
            println!("  iteration {}/{}: {} file(s) with errors",
                iteration + 1, max_iterations, broken_files.len());
        }

        for file_path in &broken_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => { eprintln!("  cannot read {file_path}: {e}"); continue; }
            };
            let errors = errors_for_file(&combined, file_path);
            if errors.trim().is_empty() {
                eprintln!("  skipping {} — no matching error lines", file_path);
                continue;
            }
            println!("    fixing {} ({} error line(s))", file_path, errors.lines().count());

            let referenced: Vec<(String, String)> =
                if file_path.ends_with(".ts") || file_path.ends_with(".tsx") {
                    let imports = parse_ts_imports(&content, file_path, service_dir);
                    if let Some(surface) = roots::get_ts_module_surface(&imports, service_dir) {
                        vec![("module-surface (roots index)".to_string(), surface)]
                    } else {
                        // Roots not available or files not yet indexed — read the imported files.
                        imports.iter()
                            .filter_map(|rel| {
                                std::fs::read_to_string(format!("{service_dir}/{rel}"))
                                    .ok()
                                    .map(|c| (rel.clone(), c))
                            })
                            .collect()
                    }
                } else if file_path.ends_with(".java") {
                    let imported: Vec<&str> = content.lines()
                        .filter(|l| l.starts_with("import ") && !l.contains('*'))
                        .filter_map(|l| l.trim_end_matches(';').rsplit('.').next())
                        .collect();
                    if let Some(surface) = roots::get_class_surface(&imported, service_dir) {
                        vec![("type-surface (roots index)".to_string(), surface)]
                    } else {
                        service_source_files.iter()
                            .filter(|f| f.ends_with(".java"))
                            .filter_map(|rel| {
                                let full = format!("{service_dir}/{rel}");
                                if full == *file_path { return None; }
                                let stem = std::path::Path::new(rel).file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                if !imported.contains(&stem) { return None; }
                                std::fs::read_to_string(&full).ok().map(|c| (rel.clone(), c))
                            })
                            .collect()
                    }
                } else {
                    vec![]
                };

            let tech = service.technology.as_deref().unwrap_or("");
            let base_skill = skill_for_technology(tech, "", "", &service.name);
            let test_skill = testing_skill_for_file_with_adrs(file_path, tech, adrs);
            let fix_skill = if test_skill.is_empty() {
                base_skill
            } else {
                format!("{base_skill}\n\n{test_skill}")
            };
            match fix_file(client, file_path, &content, &errors, service_source_files, &referenced, &fix_skill, arch_skills) {
                Ok(fixed) => { let _ = std::fs::write(file_path, &fixed); }
                Err(e) => eprintln!("    LLM fix failed for {file_path}: {e}"),
            }
        }
    }
    false
}

fn cmd_implement(story_id: &str, debug: bool, fix_log_dir: &std::path::Path) -> Result<()> {
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

    let adrs = load_all_adrs().unwrap_or_default();

    let contract_path = canopy_storage::storage_dir()
        .join(format!("stories/{}/contract.yaml", story_id));
    let contract_yaml = if contract_path.exists() {
        std::fs::read_to_string(&contract_path)
            .context("failed to read contract.yaml")?
    } else {
        println!("No contract found for '{}' — generating from spec...", story_id);
        let client = build_client("contract", debug)?;
        match generate_story_contract(&client, story, &spec, &services, &adrs) {
            Ok(yaml) => {
                save_story_contract(story_id, &yaml).context("failed to save contract")?;
                println!("Contract saved to .canopy/stories/{}/contract.yaml", story_id);
                yaml
            }
            Err(e) => anyhow::bail!("contract generation failed: {e}"),
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

            let existing_files = scan_project_files(&services);
            println!("Generating implementation plan for '{story_id}'...");
            let client = build_client("planner", debug)?;
            let plan = generate_story_plan(
                &client, story, &spec, &contract_yaml, &services, &adrs,
                &existing_files, &service_packages, &installed_deps_by_service,
            )
            .context("failed to generate implementation plan")?;

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

                println!("Analysing dependencies for '{}'...", service.name);
                let global_log = canopy_storage::load_dependency_decisions().unwrap_or_default();
                let previously_rejected: Vec<String> = global_log.decisions.iter()
                    .filter(|d| d.service == service.name && d.decision == "rejected")
                    .map(|d| d.package.clone())
                    .collect();
                let dep_tech_skill = skill_for_technology(tech, "", "", &service.name);
                match propose_dependencies(&client, &service.name, tech, story, &service_steps, &installed, &previously_rejected, &adrs, &dep_tech_skill) {
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
                let gate_results = run_dependency_gate(proposed, &theme);

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
                let rejected: Vec<String> = gate_results.iter()
                    .filter(|(_, ok)| !*ok)
                    .map(|(d, _)| d.package.clone())
                    .collect();

                let t = svc_tech.to_lowercase();
                let is_npm_svc = t.contains("node") || t.contains("express") || t.contains("react")
                    || t.contains("angular") || t.contains("vite") || t.contains("nest");
                let is_gradle_svc = t.contains("gradle");
                let is_jvm_svc = !is_npm_svc && (t.contains("spring") || t.contains("java")
                    || t.contains("kotlin") || t.contains("quarkus") || t.contains("micronaut")
                    || is_gradle_svc);

                let svc_dir = if services.services.iter()
                    .find(|s| &s.name == svc_name)
                    .and_then(|s| s.component_type.as_deref())
                    == Some("frontend")
                {
                    format!("frontend/{svc_name}")
                } else {
                    format!("services/{svc_name}")
                };

                // npm: install approved packages immediately.
                // JVM: no install step — the LLM writes them into pom.xml/build.gradle.
                if is_npm_svc && !approved.is_empty() && std::path::Path::new(&svc_dir).exists() {
                    let approved_str = approved.join(" ");
                    println!("  Installing: {approved_str}");
                    let _ = std::process::Command::new("npm")
                        .args(["install", &approved_str])
                        .current_dir(&svc_dir)
                        .status();
                } else if is_jvm_svc && !approved.is_empty() {
                    println!("  Approved JVM dependencies will be included in the generated build manifest.");
                }

                // Build tech-appropriate constraint strings for step prompts.
                let all_available: Vec<String> = {
                    let mut v = installed.clone();
                    v.extend(approved.iter().cloned());
                    v.sort(); v.dedup(); v
                };
                let (manifest_label, available_note, reject_note) = if is_gradle_svc {
                    ("build.gradle",
                     "Declare only these coordinates in build.gradle — do not introduce others:",
                     "Do NOT add: {} — rejected by the human reviewer; use built-in alternatives.")
                } else if is_jvm_svc {
                    ("pom.xml",
                     "Declare only these coordinates in pom.xml — do not introduce others:",
                     "Do NOT add: {} — rejected by the human reviewer; use built-in alternatives.")
                } else {
                    ("package.json",
                     "Packages in package.json — do NOT import any other package (runtime crash):",
                     "Do NOT use: {} — rejected by the human reviewer; use built-in alternatives.")
                };
                let mut lines: Vec<String> = Vec::new();
                if !all_available.is_empty() {
                    lines.push(format!(
                        "## Available dependencies ({manifest_label})\n\
                         {available_note}\n{}",
                        all_available.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
                    ));
                }
                if !rejected.is_empty() {
                    lines.push(format!(
                        "## Rejected dependencies\n{}",
                        reject_note.replace("{}", &rejected.join(", "))
                    ));
                }
                if !lines.is_empty() {
                    pkg_constraints_by_service.insert(svc_name.clone(), lines.join("\n\n"));
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
                let t = tech.to_lowercase();
                let (label, note) = if t.contains("gradle") {
                    ("build.gradle", "Declare only these coordinates in build.gradle — do not introduce others:")
                } else if t.contains("spring") || t.contains("java") || t.contains("kotlin")
                    || t.contains("quarkus") || t.contains("micronaut")
                {
                    ("pom.xml", "Declare only these coordinates in pom.xml — do not introduce others:")
                } else {
                    ("package.json", "Packages in package.json — do NOT import any other package (runtime crash):")
                };
                pkg_constraints_by_service.insert(service.name.clone(), format!(
                    "## Available dependencies ({label})\n{note}\n{}",
                    installed.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
                ));
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
                println!("  [{}] {} {} — {}", step.id, op, step.file, step.description);
            }
            println!();

            let confirmed = Confirm::with_theme(&theme)
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
                return Ok(());
            }
            plan
        }
    };

    // Load package constraints (written during plan/gate phase, also available on resume).
    let constraints_path = canopy_storage::storage_dir()
        .join(format!("stories/{}/pkg_constraints.yaml", story_id));
    let pkg_constraints_by_service: std::collections::HashMap<String, String> =
        std::fs::read_to_string(&constraints_path)
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or_default();

    let client = build_client("developer", debug)?;
    let total = plan.steps.len();
    let mut written = 0usize;
    let mut session_written: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    const MAX_FIX_ITERATIONS: usize = 5;

    roots::ensure_indexed();

    for i in 0..total {
        if plan.steps[i].status != StepStatus::Pending { continue; }

        let step = &plan.steps[i];
        let op_label = if step.operation == "modify" { "modify" } else { "create" };
        println!("\n[{}/{}] {} {}", step.id, total, op_label, step.file);
        println!("  {}", step.description);

        // Resolve the service entry and directory for this step.
        let step_service_name = step.service.rsplit('/').next().unwrap_or(&step.service).to_string();
        let step_service = services.services.iter()
            .find(|s| s.name == step_service_name || s.name == step.service);
        let step_tech = step_service.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
        let arch_skills = skills_for_architecture(&adrs, step_tech);
        let step_service_dir = match step_service.and_then(|s| s.component_type.as_deref()) {
            Some("frontend") => format!("frontend/{}", step_service_name),
            _ => format!("services/{}", step_service_name),
        };

        if is_tdd_candidate(&step.file) {
            let test_file = match derive_test_file_path(&step.file) {
                Some(p) => p,
                None => {
                    eprintln!("  cannot derive test path for {} — skipping TDD", step.file);
                    continue;
                }
            };
            let test_class = test_class_name(&test_file).unwrap_or_else(|| "Test".to_string());

            // ── RED PHASE ────────────────────────────────────────────────────────
            println!("  [red] generating unit test: {}", test_file);
            let test_content = generate_unit_test_stub(
                &client, story, &spec, step, &test_file,
                &service_packages, &services, &adrs,
            ).with_context(|| format!("LLM call failed generating test for step {}", step.id))?;

            let test_dest = std::path::Path::new(&test_file);
            if let Some(parent) = test_dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(test_dest, &test_content)
                .with_context(|| format!("failed to write {}", test_file))?;
            println!("  wrote {}", test_file);

            println!("  [red] generating stub: {}", step.file);
            let stub_siblings = build_sibling_section(&step.depends_on, &step_service_dir, &session_written);
            let pkg_constraints = pkg_constraints_by_service.get(&step_service_name).map(|s| s.as_str());
            let stub_content = execute_implementation_stub(
                &client, story, &spec, &contract_yaml,
                step, None, None,
                &service_packages, &services, &stub_siblings, &arch_skills,
                &test_file, &test_content, pkg_constraints,
            ).with_context(|| format!("LLM call failed generating stub for step {}", step.id))?;

            let dest = std::path::Path::new(&step.file);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, &stub_content)
                .with_context(|| format!("failed to write {}", step.file))?;
            println!("  wrote {}", step.file);
            written += 1;
            session_written.insert(step.file.clone(), stub_content);

            if let Some(svc) = step_service {
                if std::path::Path::new(&step_service_dir).exists() {
                    let compile_cmd = compile_command_for_service(svc, &step_service_dir);
                    println!("  [red] compiling: {} (in {})", compile_cmd, step_service_dir);
                    let src_files = scan_service_source_files(&step_service_dir);
                    // Both test and impl may be fixed during Red — no skip_files here.
                    run_fix_loop_logged(&client, svc, &step_service_dir, &compile_cmd,
                        &src_files, &[], &adrs, &arch_skills, MAX_FIX_ITERATIONS,
                        Some(&fix_log_dir), &format!("red-{}", step.file));
                }
            }
            roots::reindex();

            // ── GREEN PHASE ──────────────────────────────────────────────────────
            println!("  [green] implementing: {}", step.file);
            let roots_context = roots::get_feature_context(&step.description)
                .map(|p| format_roots_context(&p))
                .filter(|s| !s.is_empty());

            // Re-read test in case the Red fix loop modified it.
            let test_content = std::fs::read_to_string(&test_file)
                .unwrap_or(test_content);

            let green_siblings = build_sibling_section(&step.depends_on, &step_service_dir, &session_written);
            let StepResult { content: impl_content, summary: impl_summary } = execute_implementation_with_test(
                &client, story, &spec, &contract_yaml,
                step, None, roots_context.as_deref(),
                &service_packages, &services, &green_siblings, &arch_skills,
                &test_file, &test_content, pkg_constraints,
            ).with_context(|| format!("LLM call failed for Green phase step {}", step.id))?;

            std::fs::write(dest, &impl_content)
                .with_context(|| format!("failed to write {}", step.file))?;
            println!("  wrote {}", step.file);
            if let Some(s) = &impl_summary { println!("  summary: {}", s); }
            session_written.insert(step.file.clone(), impl_content);

            if let Some(svc) = step_service {
                if std::path::Path::new(&step_service_dir).exists() {
                    let test_cmd = test_class_command_for_service(svc, &test_class);
                    println!("  [green] testing: {} (in {})", test_cmd, step_service_dir);
                    let src_files = scan_service_source_files(&step_service_dir);
                    // Green phase: never modify the unit test — it is the spec.
                    let abs_test = std::fs::canonicalize(&test_file)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or(test_file.clone());
                    run_fix_loop_logged(&client, svc, &step_service_dir, &test_cmd,
                        &src_files, &[test_file.clone(), abs_test], &adrs, &arch_skills, MAX_FIX_ITERATIONS,
                        Some(&fix_log_dir), &format!("green-{}", step.file));
                }
            }
            roots::reindex();
        } else {
            // ── DIRECT IMPLEMENTATION (non-TDD candidates) ───────────────────────
            let current_content = if step.operation == "modify" {
                std::fs::read_to_string(&step.file).ok()
            } else {
                None
            };
            let roots_context = roots::get_feature_context(&step.description)
                .map(|p| format_roots_context(&p))
                .filter(|s| !s.is_empty());

            let step_siblings = build_sibling_section(&step.depends_on, &step_service_dir, &session_written);
            let pkg_constraints = pkg_constraints_by_service.get(&step_service_name).map(|s| s.as_str());
            let StepResult { content, summary } = execute_implementation_step(
                &client, story, &spec, &contract_yaml,
                step, current_content.as_deref(), roots_context.as_deref(),
                &service_packages, &services, &step_siblings, &arch_skills, pkg_constraints,
            ).with_context(|| format!("LLM call failed for step {}", step.id))?;

            let dest = std::path::Path::new(&step.file);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, &content)
                .with_context(|| format!("failed to write {}", step.file))?;
            println!("  wrote {}", step.file);
            if let Some(s) = &summary { println!("  summary: {}", s); }
            written += 1;
            session_written.insert(step.file.clone(), content);
            roots::reindex();
        }

        plan.steps[i].status = StepStatus::Done;
        save_story_plan(story_id, &plan).context("failed to save plan progress")?;
    }

    println!("\n{written} file(s) written.");

    // Final integration test pass — catches e2e tests and cross-service interaction issues.
    let implementable: Vec<_> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .filter(|s| s.technology.is_some())
        .collect();

    for service in &implementable {
        let service_dir = match service.component_type.as_deref().unwrap_or("service") {
            "frontend" => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };
        if !std::path::Path::new(&service_dir).exists() { continue; }

        if !std::path::Path::new(&format!("{service_dir}/node_modules")).exists()
            && std::path::Path::new(&format!("{service_dir}/package.json")).exists()
        {
            println!("  running npm install in {service_dir}...");
            let _ = std::process::Command::new("npm").arg("install").current_dir(&service_dir).status();
        }

        let svc_tech = service.technology.as_deref().unwrap_or("unknown");
        let arch_skills = skills_for_architecture(&adrs, svc_tech);
        let test_cmd = test_command_for_service(service, &service_dir);
        let service_source_files = scan_service_source_files(&service_dir);
        println!("\nFinal validation: {} (in {})", test_cmd, service_dir);
        run_fix_loop_logged(&client, service, &service_dir, &test_cmd,
            &service_source_files, &[], &adrs, &arch_skills, MAX_FIX_ITERATIONS,
            Some(&fix_log_dir), &format!("final-{}", service.name));
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
    let is_frontend = service.component_type.as_deref() == Some("frontend");
    let pkg_path = format!("{service_dir}/package.json");
    if let Ok(pkg) = std::fs::read_to_string(&pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&pkg) {
            let scripts = json.get("scripts");
            if scripts.and_then(|s| s.get("test")).is_some() {
                // --watchAll=false is a CRA/Vite flag; plain jest doesn't accept it
                return if is_frontend {
                    "npm test -- --watchAll=false".to_string()
                } else {
                    "npx jest --forceExit".to_string()
                };
            }
            if scripts.and_then(|s| s.get("build")).is_some() {
                return "npm run build".to_string();
            }
        }
    }
    "npx tsc --noEmit".to_string()
}

/// Remove ANSI escape sequences from a string so error pattern matching works on raw text.
fn strip_ansi(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() { break; }
            }
        } else {
            out.push(ch);
        }
    }
    out
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
        // ts-jest / tsc watch: path/to/File.ts:line:col - error TSxxxx: ...
        // Leading whitespace is common in jest output; "declared here" refs lack "- error TS".
        if line.contains(" - error TS") || line.contains(" - warning TS") {
            let trimmed = line.trim_start();
            if let Some(colon_pos) = trimmed.find(':') {
                let path = &trimmed[..colon_pos];
                if path.ends_with(".ts") || path.ends_with(".tsx") {
                    if let Some(resolved) = resolve(path) {
                        if !files.contains(&resolved) { files.push(resolved); }
                    }
                }
            }
        }
        // Jest module resolution: Cannot find module '...' from 'tests/Foo.ts'
        // The file after `from '` is the test file that contains the bad import.
        if line.contains("Cannot find module") {
            if let Some(from_pos) = line.find(" from '") {
                let rest = &line[from_pos + 7..];
                if let Some(end) = rest.find('\'') {
                    let path = rest[..end].trim();
                    if path.ends_with(".ts") || path.ends_with(".tsx") {
                        if let Some(resolved) = resolve(path) {
                            if !files.contains(&resolved) { files.push(resolved); }
                        }
                    }
                }
            }
        }
        // Jest stack trace: "    at Something (path/to/file.ts:line:col)"
        // Captures the source file where the runtime error originates.
        // Skips node_modules frames — only project source files are fixable.
        if line.trim_start().starts_with("at ") && line.contains(".ts:") && !line.contains("node_modules") {
            if let Some(open) = line.rfind('(') {
                if let Some(close) = line.rfind(')') {
                    let inner = &line[open + 1..close];
                    if let Some(colon) = inner.rfind(':') {
                        if let Some(colon2) = inner[..colon].rfind(':') {
                            let path = inner[..colon2].trim();
                            if path.ends_with(".ts") || path.ends_with(".tsx") {
                                if let Some(resolved) = resolve(path) {
                                    if !files.contains(&resolved) { files.push(resolved); }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

fn read_installed_deps(service_dir: &str, tech: &str) -> Vec<String> {
    let t = tech.to_lowercase();
    if t.contains("gradle") {
        read_gradle_deps(service_dir)
    } else if t.contains("spring") || t.contains("java") || t.contains("kotlin")
        || t.contains("quarkus") || t.contains("micronaut")
    {
        read_pom_deps(service_dir)
    } else {
        read_package_json_deps(service_dir)
    }
}

fn read_pom_deps(service_dir: &str) -> Vec<String> {
    let path = std::path::Path::new(service_dir).join("pom.xml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut deps: Vec<String> = Vec::new();
    let mut group_id = String::new();
    let mut artifact_id = String::new();
    for line in content.lines() {
        let t = line.trim();
        if let Some(v) = tag_value(t, "groupId") { group_id = v; }
        if let Some(v) = tag_value(t, "artifactId") { artifact_id = v; }
        if t == "</dependency>" {
            if !group_id.is_empty() && !artifact_id.is_empty() {
                deps.push(format!("{}:{}", group_id, artifact_id));
            }
            group_id.clear();
            artifact_id.clear();
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn tag_value(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = line.find(&open)? + open.len();
    let end = line.find(&close)?;
    if start <= end { Some(line[start..end].trim().to_string()) } else { None }
}

fn read_gradle_deps(service_dir: &str) -> Vec<String> {
    // Support both build.gradle (Groovy) and build.gradle.kts (Kotlin DSL)
    let candidates = ["build.gradle.kts", "build.gradle"];
    let content = candidates.iter()
        .find_map(|name| std::fs::read_to_string(std::path::Path::new(service_dir).join(name)).ok())
        .unwrap_or_default();

    let configs = ["implementation", "testImplementation", "api",
                   "compileOnly", "runtimeOnly", "annotationProcessor"];
    let mut deps: Vec<String> = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        let coord = configs.iter().find_map(|cfg| {
            let prefix = format!("{cfg} ");
            if !t.starts_with(&prefix) { return None; }
            // Extract the string inside quotes or parentheses+quotes
            let rest = t[prefix.len()..].trim();
            let inner = rest.trim_start_matches('(').trim_end_matches(')')
                .trim_matches('"').trim_matches('\'');
            // Strip version — keep groupId:artifactId only
            let parts: Vec<&str> = inner.split(':').collect();
            if parts.len() >= 2 { Some(format!("{}:{}", parts[0], parts[1])) } else { None }
        });
        if let Some(c) = coord { deps.push(c); }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn read_package_json_deps(service_dir: &str) -> Vec<String> {
    let path = std::path::Path::new(service_dir).join("package.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let mut pkgs: Vec<String> = Vec::new();
    for key in &["dependencies", "devDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            pkgs.extend(obj.keys().cloned());
        }
    }
    pkgs.sort();
    pkgs.dedup();
    pkgs
}

fn run_dependency_gate(
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

            let choice = Select::with_theme(theme)
                .with_prompt(format!("'{}'?", dep.package))
                .items(&["Accept", "Reject"])
                .default(0)
                .interact()
                .unwrap_or(1);

            let accepted = choice == 0;
            println!("  {}: {}", if accepted { "Accepted" } else { "Rejected" }, dep.package);
            println!();
            decisions.push((dep.clone(), accepted));
        }
    }

    loop {
        let add_more = Confirm::with_theme(theme)
            .with_prompt("Add a package the LLM didn't propose?")
            .default(false)
            .interact()
            .unwrap_or(false);
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

/// Detect Maven POM validation errors that fire before compilation.
/// Pattern: [ERROR] 'dependencies.dependency.version' for X:jar is missing. @ line N
/// These indicate an artifact that has no version and isn't managed by the parent BOM.
fn extract_pom_validation_errors(output: &str) -> Vec<String> {
    output.lines()
        .filter(|line| line.contains("[ERROR]") && line.contains("is missing."))
        .map(|line| line.trim().trim_start_matches("[ERROR]").trim().to_string())
        .collect()
}

/// Extract Maven artifact coordinates that could not be resolved.
/// Matches lines like:
///   [ERROR]     Could not find artifact com.example:foo:jar:1.0-SNAPSHOT
///   [ERROR] dependency: com.example:foo:jar:1.0-SNAPSHOT (compile)
fn extract_unresolvable_dependencies(output: &str) -> Vec<String> {
    let mut coords: Vec<String> = Vec::new();
    for line in output.lines() {
        if !line.contains("[ERROR]") {
            continue;
        }
        if line.contains("Could not find artifact") || line.contains("Could not resolve") {
            // Extract the coordinates — the groupId:artifactId:... token
            if let Some(coord) = line
                .split_whitespace()
                .find(|tok| tok.matches(':').count() >= 2)
            {
                // Strip trailing parenthetical e.g. "(compile)"
                let coord = coord.trim_end_matches(|c: char| c == ')' || c == '(');
                let coord = coord.trim();
                if !coord.is_empty() && !coords.contains(&coord.to_string()) {
                    coords.push(coord.to_string());
                }
            }
        }
    }
    coords
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

/// Normalise a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => { out.pop(); }
            std::path::Component::CurDir    => {}
            other                           => out.push(other),
        }
    }
    out
}

/// Parse `import … from '…'` lines and return the service-relative paths of any
/// local imports (i.e. those starting with `./` or `../`).
///
/// `file_path` is the project-relative path of the importing file (e.g.
/// `frontend/admin-portal/src/api/ProductApi.ts`). `service_dir` is the service
/// root (e.g. `frontend/admin-portal`). Returns paths like `src/components/ProductForm.tsx`.
fn parse_ts_imports(content: &str, file_path: &str, service_dir: &str) -> Vec<String> {
    let file_rel = std::path::Path::new(file_path)
        .strip_prefix(service_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file_path.to_string());

    let file_dir = std::path::Path::new(&file_rel)
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_path_buf();

    let mut result: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("import ") && !trimmed.starts_with("export ") { continue; }

        // Extract the module specifier from: … from 'path' or "path"
        let module = if let Some(pos) = trimmed.rfind(" from ") {
            let rest = trimmed[pos + 6..].trim().trim_end_matches(';');
            rest.trim_matches('\'').trim_matches('"').to_string()
        } else {
            continue;
        };

        if !module.starts_with('.') { continue; } // skip node_modules

        let resolved = normalize_path(&file_dir.join(&module));
        let base = resolved.to_string_lossy();

        // Try the common TypeScript module resolution order.
        for candidate in &[
            format!("{}.ts",       base),
            format!("{}.tsx",      base),
            format!("{}/index.ts", base),
            format!("{}/index.tsx",base),
        ] {
            if candidate == &file_rel { continue; } // skip self-imports
            if std::path::Path::new(&format!("{}/{}", service_dir, candidate)).exists() {
                if !result.contains(candidate) {
                    result.push(candidate.clone());
                }
                break;
            }
        }
    }

    result
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

fn cmd_dependencies() -> Result<()> {
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
                    maybe_prompt_testing_strategy(
                        &theme, &mut existing_adrs,
                        proposal.technology.as_deref().unwrap_or(""),
                        proposal.service.as_deref().unwrap_or("service"),
                    )?;
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
                    maybe_prompt_testing_strategy(
                        &theme, &mut existing_adrs,
                        modified_proposal.technology.as_deref().unwrap_or(""),
                        modified_proposal.service.as_deref().unwrap_or("service"),
                    )?;
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
