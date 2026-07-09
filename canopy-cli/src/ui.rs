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
/// True when stdout is an interactive terminal — gates every spinner/progress-bar effect
/// below. Piped or redirected output (exactly how this project's own driving sessions have
/// been captured all along) gets plain, timestamped-by-elapsed lines instead: indicatif's
/// escape codes render as garbage once they land in a file or a `tail -f`.
fn interactive() -> bool {
    console::Term::stdout().is_term()
}

pub(crate) fn red(s: &str) -> String {
    console::style(s).red().to_string()
}

pub(crate) fn green(s: &str) -> String {
    console::style(s).green().to_string()
}

pub(crate) fn dim(s: &str) -> String {
    console::style(s).dim().to_string()
}

/// Dim + strikethrough for a finished checklist entry — it should visibly recede, not just get
/// a line through it. console 0.15 has no built-in strikethrough style, so this applies SGR 2
/// (dim) and 9 (strikethrough) directly, in ONE escape sequence with a single trailing reset —
/// composing two independently-wrapped helpers (each with its own reset) would cut the first
/// attribute short at the inner wrapper's reset code. Gated on the same `colors_enabled()` check
/// console's own styles use internally, so it degrades on a non-color terminal or NO_COLOR.
pub(crate) fn dim_strike(s: &str) -> String {
    if console::colors_enabled() {
        format!("\x1b[2;9m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub(crate) fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        format!("{}m{:02}s", (secs / 60.0) as u64, (secs % 60.0) as u64)
    }
}

/// Shows a spinner while `f` runs, clears it on return. Used for one-off LLM calls that
/// happen outside a step checklist (e.g. plan/dependency generation, before there's a
/// list of steps to anchor a `Progress` bar to) — see `Progress::timed` for the
/// step-nested equivalent used during `implement`.
pub(crate) fn with_spinner<F, T>(label: impl Into<String>, f: F) -> T
where
    F: FnOnce() -> T,
{
    let label = label.into();
    let start = std::time::Instant::now();
    if !interactive() {
        println!("  … {label}");
        let result = f();
        println!("  {} {label} ({})", dim("done"), format_elapsed(start.elapsed()));
        return result;
    }
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Duration;
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg} ({elapsed_precise})")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(label.clone());
    let result = f();
    pb.finish_and_clear();
    println!("  {} {label} ({})", dim("done"), format_elapsed(start.elapsed()));
    result
}

/// A persistent checklist of every step in the current `implement` run, rendered as one
/// line per step via `indicatif::MultiProgress` when interactive. Pending steps sit dim
/// and unmarked; the active step gets a live spinner and phase text; finished steps freeze
/// with a checkmark and strikethrough; failed steps freeze red, un-struck (a failure stays
/// visually prominent — strikethrough would read as "handled").
///
/// Falls back to plain `println!` lines (no bars, no strikethrough) when stdout isn't a
/// terminal, matching `with_spinner`'s fallback for the same reason.
pub(crate) struct Progress {
    multi: Option<indicatif::MultiProgress>,
    bars: Vec<indicatif::ProgressBar>,
}

fn step_label(idx: usize, total: usize, file: &str) -> String {
    format!("[{}/{}] {}", idx + 1, total, file)
}

impl Progress {
    pub(crate) fn new(total: usize) -> Self {
        if !interactive() {
            return Self { multi: None, bars: Vec::new() };
        }
        use indicatif::{ProgressBar, ProgressStyle};
        let multi = indicatif::MultiProgress::new();
        let style = ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        let bars: Vec<ProgressBar> = (0..total)
            .map(|_| {
                let pb = multi.add(ProgressBar::new_spinner());
                pb.set_style(style.clone());
                pb
            })
            .collect();
        Self { multi: Some(multi), bars }
    }

    /// A no-op checklist for phases with no step list to anchor to (e.g. the final
    /// cross-service validation pass, which runs after every step bar is already frozen).
    /// `timed`/`println` fall back to plain output unconditionally — safe because nothing
    /// is ever drawn through it, so there is no live region for later output to clobber.
    pub(crate) fn none() -> Self {
        Self { multi: None, bars: Vec::new() }
    }

    /// Prints a line while the checklist may be live. A bare `println!` is unsafe here:
    /// indicatif assumes exclusive control of the terminal region it last drew and will
    /// move the cursor up to redraw it on the next update, clobbering anything printed
    /// in between — `MultiProgress::println` accounts for the extra line instead.
    pub(crate) fn println(&self, msg: impl AsRef<str>) {
        match &self.multi {
            Some(multi) => { let _ = multi.println(msg.as_ref()); }
            None => println!("{}", msg.as_ref()),
        }
    }

    /// Marks step `idx` as the active one and sets its current phase text
    /// (e.g. "TDD Red — generating stub"). Safe to call repeatedly as the phase changes.
    pub(crate) fn phase(&self, idx: usize, total: usize, file: &str, phase: &str) {
        match self.bars.get(idx) {
            Some(pb) => {
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                pb.set_message(format!("{} — {phase}", step_label(idx, total, file)));
            }
            None => self.println(format!("\n{} — {phase}", step_label(idx, total, file))),
        }
    }

    pub(crate) fn done(&self, idx: usize, total: usize, file: &str, note: &str) {
        let line = dim_strike(&format!("✓ {} ({note})", step_label(idx, total, file)));
        match self.bars.get(idx) {
            Some(pb) => pb.finish_with_message(line),
            None => self.println(line),
        }
    }

    pub(crate) fn skipped(&self, idx: usize, total: usize, file: &str, note: &str) {
        let line = dim_strike(&format!("↷ {} ({note})", step_label(idx, total, file)));
        match self.bars.get(idx) {
            Some(pb) => pb.finish_with_message(line),
            None => self.println(line),
        }
    }

    pub(crate) fn failed(&self, idx: usize, total: usize, file: &str) {
        match self.bars.get(idx) {
            Some(pb) => pb.finish_with_message(format!("{} {}", red("✗"), step_label(idx, total, file))),
            None => self.println(format!("{} {}", red("✗"), step_label(idx, total, file))),
        }
    }

    /// Runs `f`, showing a spinner nested directly under step `idx`'s line while it runs
    /// (or a plain line when non-interactive) — the step-checklist equivalent of the
    /// free-standing `with_spinner` above.
    pub(crate) fn timed<F, T>(&self, idx: usize, label: impl Into<String>, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let label = label.into();
        let start = std::time::Instant::now();
        // "      ↳ " — deliberately NOT just more leading spaces than the parent bar's "  ": a
        // column-count difference alone reads as coincidental once a finished parent bar's blank
        // spinner slot and an active child spinner glyph land in visually similar positions. The
        // ↳ glyph makes the nesting unambiguous regardless of exact column math.
        let child = match (&self.multi, self.bars.get(idx)) {
            (Some(multi), Some(anchor)) => {
                use indicatif::{ProgressBar, ProgressStyle};
                let pb = multi.insert_after(anchor, ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::with_template("      ↳ {spinner:.cyan} {msg} ({elapsed_precise})")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                pb.set_message(label.clone());
                Some(pb)
            }
            _ => {
                self.println(format!("      ↳ … {label}"));
                None
            }
        };
        let result = f();
        if let Some(pb) = child {
            pb.finish_and_clear();
        }
        self.println(format!("      ↳ {} {label} ({})", dim("done"), format_elapsed(start.elapsed())));
        result
    }
}

/// Prints the model's self-reported summary and, when present, anything it flagged as
/// not fully followed — the latter is a direct signal of a skill/prompt gap, surfaced
/// immediately instead of waiting for it to show up as a compile error later.
pub(crate) fn print_step_notes(progress: &Progress, summary: &Option<String>, deviations: &Option<String>) {
    if let Some(s) = summary { progress.println(format!("    {}", s)); }
    if let Some(d) = deviations { progress.println(format!("    ⚠ did not follow: {}", d)); }
}
