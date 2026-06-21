mod commands;
mod config;
mod db;
mod discover;
mod output;

use clap::{Parser, Subcommand};

use commands::workspace::WorkspaceCmd;

#[derive(Parser)]
#[command(name = "roots", about = "Repository intelligence engine", version)]
struct Cli {
    /// Override the active workspace for this command
    #[arg(long, global = true)]
    workspace: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a .roots/ workspace in the current directory
    Init,
    /// Index source files under a path
    Index {
        #[arg(default_value = ".")]
        path: String,
    },
    /// Show indexed project, file, symbol and relationship counts
    Status,
    /// Exact symbol name lookup
    Symbol { name: String },
    /// Substring search across symbol names
    Query { term: String },
    /// Dump all indexed symbols as JSON
    Dump,
    /// All direct relationships for a symbol (incoming and outgoing)
    Graph { symbol: String },
    /// Symbols that CALL this symbol
    Callers { symbol: String },
    /// Symbols this symbol CALLs
    Callees { symbol: String },
    /// All outgoing relationships (IMPORTS, EXTENDS, IMPLEMENTS, CALLS)
    Deps { symbol: String },
    /// Transitive reverse impact: what breaks if this symbol changes
    Impact { symbol: String },
    /// Generate a structured context packet for AI reasoning
    Context {
        /// Symbol FQN (positional)
        symbol: Option<String>,
        /// Feature goal — keyword-based graph traversal
        #[arg(long, conflicts_with_all = ["symbol", "project", "file"])]
        feature: Option<String>,
        /// Project name
        #[arg(long, conflicts_with_all = ["symbol", "feature", "file"])]
        project: Option<String>,
        /// File path (relative to repo root)
        #[arg(long, conflicts_with_all = ["symbol", "feature", "project"])]
        file: Option<String>,
    },
    /// List graph facts for a symbol as human-readable sentences
    Facts { symbol: String },
    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        subcommand: WorkspaceSubcommand,
    },
}

#[derive(Subcommand)]
enum WorkspaceSubcommand {
    /// List all workspaces
    List,
    /// Create a new workspace
    Add { name: String },
    /// Set the active workspace
    Use { name: String },
}

fn main() {
    let cli = Cli::parse();
    let ws = cli.workspace.as_deref();

    let result = match cli.command {
        Command::Init               => commands::init::run(),
        Command::Index { path }     => commands::index::run(ws, &path),
        Command::Status             => commands::status::run(ws),
        Command::Symbol { name }    => commands::symbol::run(ws, &name),
        Command::Query { term }     => commands::query::run(ws, &term),
        Command::Dump               => commands::dump::run(ws),
        Command::Graph   { symbol } => commands::graph::run(ws, &symbol),
        Command::Callers { symbol } => commands::callers::run(ws, &symbol),
        Command::Callees { symbol } => commands::callees::run(ws, &symbol),
        Command::Deps    { symbol } => commands::deps::run(ws, &symbol),
        Command::Impact  { symbol } => commands::impact::run(ws, &symbol),
        Command::Context { symbol, feature, project, file } =>
            commands::context::run(ws, symbol.as_deref(), feature.as_deref(), project.as_deref(), file.as_deref()),
        Command::Facts { symbol } => commands::facts::run(ws, &symbol),
        Command::Workspace { subcommand } => match subcommand {
            WorkspaceSubcommand::List          => commands::workspace::run(WorkspaceCmd::List),
            WorkspaceSubcommand::Add { name }  => commands::workspace::run(WorkspaceCmd::Add { name }),
            WorkspaceSubcommand::Use { name }  => commands::workspace::run(WorkspaceCmd::Use { name }),
        },
    };

    if let Err(e) = result {
        output::error(&e.to_string());
        std::process::exit(1);
    }
}
