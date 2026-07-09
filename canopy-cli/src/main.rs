mod adr_wizard;
mod build_output;
mod cli;
mod commands;
mod dependency_gate;
mod fix_loop;
mod project_scan;
mod roots;
mod shell;
mod tdd;
mod ui;
mod util;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli::run_repl(cli.llm_debug)
}
