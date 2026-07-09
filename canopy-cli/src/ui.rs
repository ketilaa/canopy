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

/// A checklist of every step in the current `implement` run, rendered as one line per step
/// via `indicatif::MultiProgress` when interactive. Pending steps sit dim and unmarked; the
/// active step expands with a live spinner, phase text, and a nested child spinner for
/// whatever LLM call is currently running; a finished step collapses to a single dim,
/// struck-through line and is dropped for good — see the `bars` field doc for why that drop
/// matters. Failed steps freeze red, un-struck (a failure stays visually prominent —
/// strikethrough would read as "handled").
///
/// Step bars are revealed LAZILY, not all created upfront: only the active step plus a
/// preview window of upcoming steps are ever live at once, sized to the terminal's actual
/// height (see `new`) so a huge plan can't push the active step off the bottom of the
/// viewport. Anything beyond the window is summarized in one trailing "N more step(s)" line
/// that always stays last.
///
/// Each sub-operation `timed` runs (test-gen, stub-gen, one fix attempt, ...) gets its own
/// nested line that STAYS visible — showing "✓ done" rather than clearing — once it finishes.
/// These accumulate under the active step as a running history of what's been tried so far,
/// and only disappear all at once when the step itself collapses (see `collapse`) — a failed
/// step's history is kept instead, since what was tried is useful context for a failure.
///
/// Falls back to plain `println!` lines (no bars, no strikethrough) when stdout isn't a
/// terminal, matching `with_spinner`'s fallback for the same reason.
pub(crate) struct Progress {
    multi: Option<indicatif::MultiProgress>,
    /// `Mutex<Option<_>>`, not a plain `ProgressBar`, so a finished step's bar can actually be
    /// dropped (via `.take()`) instead of held for the run's lifetime. Indicatif only folds a
    /// finished bar permanently into scrollback (stops redrawing it — "collapses" it) when its
    /// LAST strong handle is dropped; holding every bar in a plain `Vec` for the whole run is
    /// exactly why finished steps kept reappearing after every subsequent redraw. A slot is
    /// `None` both before its bar is revealed and after it's finished — the two states are
    /// never queried in a way that would confuse them, since a step's phase/timed/collapse
    /// calls only ever happen while it's the active one.
    bars: Vec<std::sync::Mutex<Option<indicatif::ProgressBar>>>,
    /// Finished (but not yet dropped) `timed` sub-bars for each step index, in order. Kept
    /// alive — unlike `bars`, not taken/dropped as soon as they finish — so they stay visible
    /// as a nested "done" history until `collapse` drains and drops the whole group at once.
    children: Vec<std::sync::Mutex<Vec<indicatif::ProgressBar>>>,
    files: Vec<String>,
    /// How many upcoming pending steps preview below the active one.
    window: usize,
    /// How many step bars have been created so far (0..revealed are live; revealed..total
    /// are still `None`, not yet inserted into the MultiProgress at all).
    revealed: std::sync::Mutex<usize>,
    /// Always the last bar in visual order — every newly-revealed step bar is inserted just
    /// before it, so it never has to move. Empty message once nothing is left to summarize.
    tail: Option<indicatif::ProgressBar>,
}

fn step_label(idx: usize, total: usize, file: &str) -> String {
    format!("[{}/{}] {}", idx + 1, total, file)
}

impl Progress {
    pub(crate) fn new(files: &[String]) -> Self {
        let total = files.len();
        if !interactive() {
            return Self {
                multi: None,
                bars: (0..total).map(|_| std::sync::Mutex::new(None)).collect(),
                children: (0..total).map(|_| std::sync::Mutex::new(Vec::new())).collect(),
                files: files.to_vec(),
                window: 0,
                revealed: std::sync::Mutex::new(0),
                tail: None,
            };
        }
        use indicatif::{ProgressBar, ProgressStyle};
        let multi = indicatif::MultiProgress::new();
        // Reserve a handful of rows for scrollback context, the active step's own parent +
        // child line, and the tail summary itself; clamp so a tiny terminal still previews at
        // least one upcoming step and a huge terminal doesn't preview an unreasonable number.
        let (rows, _cols) = console::Term::stdout().size();
        let window = (rows as usize).saturating_sub(6).clamp(1, 8);
        let tail = multi.add(ProgressBar::new_spinner());
        tail.set_style(
            ProgressStyle::with_template("  {msg}").unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        let bars = (0..total).map(|_| std::sync::Mutex::new(None)).collect();
        let children = (0..total).map(|_| std::sync::Mutex::new(Vec::new())).collect();
        let progress = Self {
            multi: Some(multi),
            bars,
            children,
            files: files.to_vec(),
            window,
            revealed: std::sync::Mutex::new(0),
            tail: Some(tail),
        };
        progress.reveal_through(0);
        progress
    }

    /// A no-op checklist for phases with no step list to anchor to (e.g. the final
    /// cross-service validation pass, which runs after every step bar is already frozen).
    /// `timed`/`println` fall back to plain output unconditionally — safe because nothing
    /// is ever drawn through it, so there is no live region for later output to clobber.
    pub(crate) fn none() -> Self {
        Self { multi: None, bars: Vec::new(), children: Vec::new(), files: Vec::new(), window: 0, revealed: std::sync::Mutex::new(0), tail: None }
    }

    fn total(&self) -> usize {
        self.files.len()
    }

    fn file(&self, idx: usize) -> &str {
        self.files.get(idx).map(String::as_str).unwrap_or("")
    }

    /// Ensures every step bar from 0 up through `idx`'s preview window exists, and updates the
    /// trailing "N more step(s)" summary. A newly-created bar beyond `idx` itself gets a dim
    /// "pending" message; `phase`/`collapse` handle upgrading `idx`'s own bar afterward.
    fn reveal_through(&self, idx: usize) {
        let (Some(multi), Some(tail)) = (&self.multi, &self.tail) else { return };
        let target = (idx + self.window + 1).min(self.total());
        let mut revealed = self.revealed.lock().unwrap();
        if *revealed >= target {
            return;
        }
        use indicatif::{ProgressBar, ProgressStyle};
        let style = ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        while *revealed < target {
            let pb = multi.insert_before(tail, ProgressBar::new_spinner());
            pb.set_style(style.clone());
            pb.set_message(dim(&step_label(*revealed, self.total(), self.file(*revealed))));
            *self.bars[*revealed].lock().unwrap() = Some(pb);
            *revealed += 1;
        }
        let remaining = self.total() - *revealed;
        tail.set_message(if remaining > 0 {
            dim(&format!("… {remaining} more step(s)"))
        } else {
            String::new()
        });
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
    pub(crate) fn phase(&self, idx: usize, phase: &str) {
        self.reveal_through(idx);
        let label = step_label(idx, self.total(), self.file(idx));
        match self.bars.get(idx).and_then(|m| m.lock().unwrap().clone()) {
            Some(pb) => {
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                pb.set_message(format!("{label} — {phase}"));
            }
            None => self.println(format!("\n{label} — {phase}")),
        }
    }

    /// Finishes step `idx`'s bar with `line`, then drops it — the drop (not `finish_with_message`
    /// alone) is what makes indicatif fold the line permanently into scrollback and stop
    /// redrawing it. See the `bars` field doc for why. Also drops every accumulated `timed`
    /// sub-bar for this step — when `clear_children` is true, each one is cleared first, so
    /// the whole nested "done" history vanishes along with the parent, leaving just `line`;
    /// when false, they're dropped as-is, keeping their content (a failure's attempt history
    /// stays visible as diagnostic context).
    fn collapse(&self, idx: usize, line: String, clear_children: bool) {
        if let Some(m) = self.children.get(idx) {
            let mut children = m.lock().unwrap();
            if clear_children {
                for pb in children.iter() {
                    pb.finish_and_clear();
                }
            }
            children.clear();
        }
        match self.bars.get(idx).and_then(|m| m.lock().unwrap().take()) {
            Some(pb) => pb.finish_with_message(line),
            None => self.println(line),
        }
    }

    pub(crate) fn done(&self, idx: usize, note: &str) {
        let label = step_label(idx, self.total(), self.file(idx));
        self.collapse(idx, dim_strike(&format!("✓ {label} ({note})")), true);
    }

    pub(crate) fn skipped(&self, idx: usize, note: &str) {
        let label = step_label(idx, self.total(), self.file(idx));
        self.collapse(idx, dim_strike(&format!("↷ {label} ({note})")), true);
    }

    pub(crate) fn failed(&self, idx: usize) {
        let label = step_label(idx, self.total(), self.file(idx));
        self.collapse(idx, format!("{} {label}", red("✗")), false);
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
        // A bare "done Red — generating test..." line has no visible indication of which step
        // it belongs to once printed on its own — `n` makes every fallback (non-interactive)
        // line self-identifying regardless of what's still visible above it.
        let n = format!("[{}/{}]", idx + 1, self.total());
        // Anchor the new spinner after whichever bar is currently LAST for this step: the most
        // recently accumulated sub-bar if there is one, else the parent itself. Always anchoring
        // on the parent would insert each new attempt BETWEEN it and the previous ones instead
        // of appending after them.
        let children_lock = self.children.get(idx);
        let anchor = match children_lock.and_then(|m| m.lock().unwrap().last().cloned()) {
            Some(last_child) => Some(last_child),
            None => self.bars.get(idx).and_then(|m| m.lock().unwrap().clone()),
        };
        // "      ↳ " — deliberately NOT just more leading spaces than the parent bar's "  ": a
        // column-count difference alone reads as coincidental once a finished parent bar's blank
        // spinner slot and an active child spinner glyph land in visually similar positions. The
        // ↳ glyph makes the nesting unambiguous regardless of exact column math.
        let child = match (&self.multi, anchor) {
            (Some(multi), Some(anchor)) => {
                use indicatif::{ProgressBar, ProgressStyle};
                let pb = multi.insert_after(&anchor, ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::with_template("      ↳ {spinner:.cyan} {msg} ({elapsed_precise})")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                pb.set_message(label.clone());
                Some(pb)
            }
            _ => {
                self.println(format!("      ↳ {n} … {label}"));
                None
            }
        };
        let result = f();
        if let Some(pb) = child {
            // Freeze with a "done" message and KEEP it (finish_with_message, not clear) — it
            // stays visible as part of this step's running history until the whole step
            // collapses via `collapse`, which drains and drops every bar accumulated here.
            pb.finish_with_message(format!("{} {label} ({})", dim("done"), format_elapsed(start.elapsed())));
            if let Some(m) = children_lock {
                m.lock().unwrap().push(pb);
            }
        } else {
            self.println(format!("      ↳ {n} {} {label} ({})", dim("done"), format_elapsed(start.elapsed())));
        }
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
