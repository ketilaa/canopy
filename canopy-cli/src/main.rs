use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Input};

use canopy_core::*;
use canopy_explore::{
    generate_architecture, generate_domain, generate_questions, generate_requirements,
    generate_vision, LlmClient,
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
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactively explore a new idea and generate all artifacts
    Explore,
    /// Regenerate vision.yaml from saved idea
    Vision,
    /// Regenerate requirements.yaml from saved idea and vision
    Requirements,
    /// Regenerate domain.yaml from saved vision and requirements
    Domain,
    /// Regenerate architecture.yaml from saved vision, requirements, and domain
    Architecture,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let debug = cli.llm_debug;
    match cli.command {
        Commands::Explore      => cmd_explore(debug),
        Commands::Vision       => cmd_vision(debug),
        Commands::Requirements => cmd_requirements(debug),
        Commands::Domain       => cmd_domain(debug),
        Commands::Architecture => cmd_architecture(debug),
    }
}

fn cmd_explore(debug: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    let description: String = Input::with_theme(&theme)
        .with_prompt("What are you building?")
        .interact_text()
        .context("failed to read idea description from terminal")?;

    let idea = Idea { description };
    save_idea(&idea).context("failed to save idea.yaml")?;
    println!("Saved .canopy/idea.yaml");

    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment before running canopy")?;

    println!("\nGenerating clarifying questions...");
    let questions = generate_questions(&client, &idea)
        .context("failed to generate questions from LLM")?;

    println!("\nPlease answer these questions. Press Enter to skip any question.\n");
    let mut answers: Vec<AnsweredQuestion> = Vec::new();
    for (i, question) in questions.questions.iter().enumerate() {
        let answer: String = Input::with_theme(&theme)
            .with_prompt(format!("[{}/{}] {}", i + 1, questions.questions.len(), question))
            .allow_empty(true)
            .interact_text()
            .context("failed to read answer from terminal")?;
        if !answer.trim().is_empty() {
            answers.push(AnsweredQuestion {
                question: question.clone(),
                answer,
            });
        }
    }

    println!("\nGenerating vision...");
    let vision = generate_vision(&client, &idea, &answers)
        .context("failed to generate vision")?;
    save_vision(&vision).context("failed to save vision.yaml")?;
    println!("  Saved .canopy/vision.yaml");

    println!("Generating requirements...");
    let requirements = generate_requirements(&client, &idea, &vision, &answers)
        .context("failed to generate requirements")?;
    save_requirements(&requirements).context("failed to save requirements.yaml")?;
    println!("  Saved .canopy/requirements.yaml");

    println!("Generating domain model...");
    let domain = generate_domain(&client, &vision, &requirements)
        .context("failed to generate domain model")?;
    save_domain(&domain).context("failed to save domain.yaml")?;
    println!("  Saved .canopy/domain.yaml");

    println!("Generating architecture...");
    let architecture = generate_architecture(&client, &vision, &requirements, &domain)
        .context("failed to generate architecture")?;
    save_architecture(&architecture).context("failed to save architecture.yaml")?;
    println!("  Saved .canopy/architecture.yaml");

    println!("\nExploration complete. All artifacts saved to .canopy/");
    Ok(())
}

fn cmd_vision(debug: bool) -> Result<()> {
    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment")?;
    let idea = load_idea()
        .context("No idea.yaml found — run `canopy explore` first")?;
    println!("Generating vision...");
    let vision = generate_vision(&client, &idea, &[])
        .context("failed to generate vision")?;
    save_vision(&vision).context("failed to save vision.yaml")?;
    println!("Saved .canopy/vision.yaml");
    Ok(())
}

fn cmd_requirements(debug: bool) -> Result<()> {
    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment")?;
    let idea = load_idea()
        .context("No idea.yaml found — run `canopy explore` first")?;
    let vision = load_vision()
        .context("No vision.yaml found — run `canopy vision` first")?;
    println!("Generating requirements...");
    let requirements = generate_requirements(&client, &idea, &vision, &[])
        .context("failed to generate requirements")?;
    save_requirements(&requirements).context("failed to save requirements.yaml")?;
    println!("Saved .canopy/requirements.yaml");
    Ok(())
}

fn cmd_domain(debug: bool) -> Result<()> {
    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment")?;
    let vision = load_vision()
        .context("No vision.yaml found — run `canopy vision` first")?;
    let requirements = load_requirements()
        .context("No requirements.yaml found — run `canopy requirements` first")?;
    println!("Generating domain model...");
    let domain = generate_domain(&client, &vision, &requirements)
        .context("failed to generate domain model")?;
    save_domain(&domain).context("failed to save domain.yaml")?;
    println!("Saved .canopy/domain.yaml");
    Ok(())
}

fn cmd_architecture(debug: bool) -> Result<()> {
    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment")?;
    let vision = load_vision()
        .context("No vision.yaml found — run `canopy vision` first")?;
    let requirements = load_requirements()
        .context("No requirements.yaml found — run `canopy requirements` first")?;
    let domain = load_domain()
        .context("No domain.yaml found — run `canopy domain` first")?;
    println!("Generating architecture...");
    let architecture = generate_architecture(&client, &vision, &requirements, &domain)
        .context("failed to generate architecture")?;
    save_architecture(&architecture).context("failed to save architecture.yaml")?;
    println!("Saved .canopy/architecture.yaml");
    Ok(())
}
