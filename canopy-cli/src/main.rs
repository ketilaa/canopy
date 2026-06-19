use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Input};

use canopy_core::*;
use canopy_explore::{
    generate_architecture_principles, generate_delivery_intents, generate_questions,
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
    /// Regenerate delivery_intents.yaml from saved idea and vision
    DeliveryIntents,
    /// Regenerate architecture_principles.yaml from saved vision and delivery intents
    ArchitecturePrinciples,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let debug = cli.llm_debug;
    match cli.command {
        Commands::Explore               => cmd_explore(debug),
        Commands::Vision                => cmd_vision(debug),
        Commands::DeliveryIntents       => cmd_delivery_intents(debug),
        Commands::ArchitecturePrinciples => cmd_architecture_principles(debug),
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

    println!("Generating delivery intents...");
    let intents = generate_delivery_intents(&client, &idea, &vision, &answers)
        .context("failed to generate delivery intents")?;
    save_delivery_intents(&intents).context("failed to save delivery_intents.yaml")?;
    println!("  Saved .canopy/delivery_intents.yaml");

    println!("Generating architecture principles...");
    let principles = generate_architecture_principles(&client, &vision, &intents, &answers)
        .context("failed to generate architecture principles")?;
    save_architecture_principles(&principles).context("failed to save architecture_principles.yaml")?;
    println!("  Saved .canopy/architecture_principles.yaml");

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

fn cmd_delivery_intents(debug: bool) -> Result<()> {
    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment")?;
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
    let client = LlmClient::from_env(debug)
        .context("ANTHROPIC_API_KEY must be set in environment")?;
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
