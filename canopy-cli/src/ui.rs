use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

pub(crate) fn confirm_required(theme: &ColorfulTheme, prompt: &str, ctx: &'static str) -> Result<bool> {
    Confirm::with_theme(theme).with_prompt(prompt).interact().context(ctx)
}

pub(crate) fn confirm_default(theme: &ColorfulTheme, prompt: &str, default: bool) -> bool {
    Confirm::with_theme(theme).with_prompt(prompt).default(default).interact().unwrap_or(default)
}

pub(crate) fn select_required(theme: &ColorfulTheme, prompt: &str, items: &[&str], default: usize, ctx: &'static str) -> Result<usize> {
    Select::with_theme(theme).with_prompt(prompt).items(items).default(default).interact().context(ctx)
}

pub(crate) fn select_or(theme: &ColorfulTheme, prompt: &str, items: &[&str], default: usize, fallback: usize) -> usize {
    Select::with_theme(theme).with_prompt(prompt).items(items).default(default).interact().unwrap_or(fallback)
}

pub(crate) fn input_text_required(theme: &ColorfulTheme, prompt: &str, ctx: &'static str) -> Result<String> {
    Input::with_theme(theme).with_prompt(prompt).interact_text().context(ctx)
}

pub(crate) fn input_text_optional(theme: &ColorfulTheme, prompt: &str, ctx: &'static str) -> Result<String> {
    Input::with_theme(theme).with_prompt(prompt).allow_empty(true).interact_text().context(ctx)
}

pub(crate) fn input_text_with_initial(theme: &ColorfulTheme, prompt: &str, initial: &str, ctx: &'static str) -> Result<String> {
    Input::with_theme(theme).with_prompt(prompt).with_initial_text(initial).interact_text().context(ctx)
}

pub(crate) fn input_text_default(theme: &ColorfulTheme, prompt: &str, default: String, ctx: &'static str) -> Result<String> {
    Input::with_theme(theme).with_prompt(prompt).default(default).interact_text().context(ctx)
}

pub(crate) fn bootstrap_select(theme: &ColorfulTheme, prompt: &str, suggestions: &[String]) -> Result<Vec<String>> {
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
/// Shows a spinner while `f` runs, clears it on return.
pub(crate) fn with_spinner<F, T>(label: impl Into<String>, f: F) -> T
where
    F: FnOnce() -> T,
{
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Duration;
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(label.into());
    let result = f();
    pb.finish_and_clear();
    result
}

/// Prints the model's self-reported summary and, when present, anything it flagged as
/// not fully followed — the latter is a direct signal of a skill/prompt gap, surfaced
/// immediately instead of waiting for it to show up as a compile error later.
pub(crate) fn print_step_notes(summary: &Option<String>, deviations: &Option<String>) {
    if let Some(s) = summary { println!("    {}", s); }
    if let Some(d) = deviations { println!("    ⚠ did not follow: {}", d); }
}
