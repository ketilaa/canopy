use crate::commands::dependencies::cmd_dependencies;
use crate::commands::domain::cmd_domain_show;
use crate::commands::implement::cmd_implement;
use crate::commands::init::cmd_init;
use crate::commands::intent::cmd_intent;
use crate::commands::scaffold::cmd_scaffold;
use crate::commands::spec::cmd_spec;
use crate::commands::stories::cmd_stories;
use crate::roots;
use crate::util::{iso_now, uuid_v4};
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "canopy",
    about = "AI-powered idea exploration — structure before tokens",
    version
)]
pub(crate) struct Cli {
    /// Print each LLM prompt, response, model, and token counts to stderr
    #[arg(long, global = true)]
    pub(crate) llm_debug: bool,
}

/// Used only for parsing commands typed inside the REPL.
#[derive(Parser)]
#[command(name = "canopy")]
struct ReplCli {
    #[arg(long, global = true)]
    #[allow(dead_code)]
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
    /// Implement a story using its spec and OpenAPI spec
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

pub(crate) fn run_repl(debug: bool) -> Result<()> {
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
        // Persist immediately — a Ctrl-C during the command below (e.g. a long
        // `implement` run) kills the process outright and never reaches the
        // save_history() call after the loop, so batching to exit loses history.
        let _ = rl.append_history(&history_path);

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
