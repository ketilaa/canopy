use anyhow::{Context, Result};
use canopy_llm::LlmClient;
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

pub(crate) fn dim(s: &str) -> String {
    console::style(s).dim().to_string()
}

/// Light grey for a step that hasn't started yet — deliberately a DIFFERENT ANSI attribute
/// (bright black, SGR 90) from `dim`/`dim_strike`'s faint (SGR 2), not just the same grey minus
/// strikethrough: a pending step and a completed one should read as two different states at a
/// glance, not the same shade with the only difference being a line through it.
pub(crate) fn pending_grey(s: &str) -> String {
    console::style(s).black().bright().to_string()
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

/// Consistent "1h 2m 3s" elapsed format, used everywhere a duration is shown — including the
/// live-ticking bars via the custom `{myelapsed}` template key registered by `with_myelapsed`,
/// so a step's active header and its finished summary line never disagree on format.
pub(crate) fn format_elapsed(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Registers a custom `{myelapsed}` template key rendering via `format_elapsed`, replacing
/// indicatif's own `{elapsed_precise}` (which is a fixed `HH:MM:SS`/`Hh:MMm:SSs`-ish format we
/// don't control) so every live-ticking bar matches the same "1h 2m 3s" format used everywhere
/// else in this file.
fn with_myelapsed(style: indicatif::ProgressStyle) -> indicatif::ProgressStyle {
    style.with_key("myelapsed", |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
        let _ = write!(w, "{}", format_elapsed(state.elapsed()));
    })
}

/// Compact token-count formatting for checklist lines — "847", "8.4k" — matching the precision
/// a human scans a running total at, not the exact figure a cost report would need.
fn format_tokens(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
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
    pb.set_style(with_myelapsed(
        ProgressStyle::with_template("  {spinner:.cyan} {msg} ({myelapsed})")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    ));
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(label.clone());
    let result = f();
    pb.finish_and_clear();
    println!("  {} {label} ({})", dim("done"), format_elapsed(start.elapsed()));
    result
}

/// One entry in the list `Progress::new` builds a checklist from. `group` is the owning
/// service/frontend's name — consecutive `StepMeta`s sharing a `group` are rendered together
/// under one header naming that group and whether it's a backend service or a frontend app.
pub(crate) struct StepMeta {
    pub file: String,
    pub group: String,
    pub is_frontend: bool,
}

/// One line in the live checklist: either a permanent section header for a group of steps,
/// or an actual step (indexing back into `Progress::files`/`group_index`/`group_size`).
enum Row {
    Header(String),
    Step(usize),
}

/// A checklist of every step in the current `implement` run, rendered as one line per step
/// via `indicatif::MultiProgress` when interactive, GROUPED under a header per owning
/// service/frontend (consecutive steps sharing a `StepMeta::group` nest under one header —
/// see `Row`). The header names the group and marks it as a backend service or frontend
/// application, so at a glance it's clear which is which; headers stay visible for the whole
/// run once revealed (there are only ever a handful of groups, so this is cheap).
///
/// Pending steps sit dim and unmarked; the active step expands with a live spinner, phase
/// text, and a nested child spinner for whatever LLM call is currently running; a finished
/// step collapses to a single dim, struck-through line and is dropped for good — see the
/// `bars` field doc for why that drop matters. Failed steps freeze red, un-struck (a failure
/// stays visually prominent — strikethrough would read as "handled").
///
/// Rows are revealed LAZILY, not all created upfront: only the active step plus a preview
/// window of upcoming steps (and whatever header precedes them) are ever live at once, sized
/// to the terminal's actual height (see `new`) so a huge plan can't push the active step off
/// the bottom of the viewport. Anything beyond the window is summarized in one trailing
/// "N more step(s)" line that always stays last.
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
    /// One slot per ROW (headers included), not per step — see `Row`. `Mutex<Option<_>>`, not
    /// a plain `ProgressBar`, so a finished step's bar can actually be dropped (via `.take()`)
    /// instead of held for the run's lifetime. Indicatif only folds a finished bar permanently
    /// into scrollback (stops redrawing it — "collapses" it) when its LAST strong handle is
    /// dropped; holding every bar in a plain `Vec` for the whole run is exactly why finished
    /// steps kept reappearing after every subsequent redraw. A step's slot is `None` both
    /// before its bar is revealed and after it's finished — the two states are never queried
    /// in a way that would confuse them, since a step's phase/timed/collapse calls only ever
    /// happen while it's the active one. A header's slot, once revealed, is simply never
    /// taken — it stays for the whole run.
    bars: Vec<std::sync::Mutex<Option<indicatif::ProgressBar>>>,
    /// Finished (but not yet dropped) `timed` sub-bars for each STEP index (not row index —
    /// only steps have sub-operations, headers never do). Kept alive — unlike `bars`, not
    /// taken/dropped as soon as they finish — so they stay visible as a nested "done" history
    /// until `collapse` drains and drops the whole group at once.
    children: Vec<std::sync::Mutex<Vec<indicatif::ProgressBar>>>,
    /// A per-step "… N earlier attempt(s) folded" line, created lazily the first time that
    /// step's `children` grows past `MAX_VISIBLE_CHILDREN` — older child bars are dropped
    /// (finish_and_clear'd) rather than left to accumulate forever. A step needing several
    /// fix attempts (5 is the existing max) would otherwise keep growing the live region's
    /// height for the rest of the run; once a live region has ever grown taller than the
    /// terminal, indicatif's cursor-based redraw can't fully reach its own earlier rows,
    /// which is how a supposedly-permanent line (e.g. a group header) has been seen to render
    /// twice — a stray copy where the cursor undershot, plus the real one printed later.
    fold_bar: Vec<std::sync::Mutex<Option<indicatif::ProgressBar>>>,
    folded_count: Vec<std::sync::Mutex<usize>>,
    /// Per-step wall-clock start (set once, on the step's first `phase()` call) and cumulative
    /// (input, output) LLM token totals (accumulated by every `timed()` call for that step) —
    /// surfaced together on the step's collapsed line in `done`/`skipped`/`failed`. `None`
    /// start means the step never actually ran any LLM work in this invocation (e.g. "already
    /// done" on resume), so no time/token summary is shown for it.
    step_start: Vec<std::sync::Mutex<Option<std::time::Instant>>>,
    step_tokens: Vec<std::sync::Mutex<(u64, u64)>>,
    /// Current phase text for the active step (e.g. "TDD Red — generating stub") — kept so
    /// `timed()` can refresh the parent bar's message (phase + latest token total) without
    /// clobbering the phase text it doesn't otherwise have access to.
    step_phase_text: Vec<std::sync::Mutex<String>>,
    files: Vec<String>,
    /// Per-step 1-based position within its own group, and that group's total step count —
    /// e.g. step 3 of 9 in "product", not step 3 of 14 overall. Parallel to `files`.
    group_index: Vec<usize>,
    group_size: Vec<usize>,
    /// Maps a step index to its row index in `rows` (accounting for header rows interleaved
    /// ahead of it).
    step_to_row: Vec<usize>,
    rows: Vec<Row>,
    /// How many upcoming steps preview below the active one (headers don't count against it).
    window: usize,
    /// How many ROWS have been created so far (0..revealed are live; revealed..rows.len() are
    /// still `None`, not yet inserted into the MultiProgress at all).
    revealed: std::sync::Mutex<usize>,
    /// Always the last bar in visual order — every newly-revealed row is inserted just before
    /// it, so it never has to move. Empty message once nothing is left to summarize.
    tail: Option<indicatif::ProgressBar>,
}

/// How many finished `timed()` sub-bars stay individually visible under an active step before
/// older ones fold into a single summary line — see the `fold_bar` field doc.
const MAX_VISIBLE_CHILDREN: usize = 3;

fn step_label(group_index: usize, group_size: usize, file: &str) -> String {
    format!("[{group_index}/{group_size}] {file}")
}

fn header_label(group: &str, is_frontend: bool) -> String {
    if is_frontend {
        console::style(format!("▢ {group} — frontend application")).magenta().bold().to_string()
    } else {
        console::style(format!("▣ {group} — backend service")).cyan().bold().to_string()
    }
}

impl Progress {
    pub(crate) fn new(steps: &[StepMeta]) -> Self {
        let total = steps.len();
        // Precompute the row layout up front: a Header the first time a group is seen,
        // immediately followed by that group's own steps in order.
        let mut rows: Vec<Row> = Vec::new();
        let mut step_to_row = vec![0usize; total];
        let mut group_index = vec![0usize; total];
        let mut group_size = vec![0usize; total];
        let mut group_totals: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for s in steps {
            *group_totals.entry(s.group.as_str()).or_insert(0) += 1;
        }
        let mut seen_groups: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut running_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, s) in steps.iter().enumerate() {
            if seen_groups.insert(s.group.as_str()) {
                rows.push(Row::Header(header_label(&s.group, s.is_frontend)));
            }
            let counter = running_index.entry(s.group.as_str()).or_insert(0);
            *counter += 1;
            group_index[i] = *counter;
            group_size[i] = group_totals[s.group.as_str()];
            rows.push(Row::Step(i));
            step_to_row[i] = rows.len() - 1;
        }
        let files: Vec<String> = steps.iter().map(|s| s.file.clone()).collect();

        if !interactive() {
            return Self {
                multi: None,
                bars: (0..rows.len()).map(|_| std::sync::Mutex::new(None)).collect(),
                children: (0..total).map(|_| std::sync::Mutex::new(Vec::new())).collect(),
                fold_bar: (0..total).map(|_| std::sync::Mutex::new(None)).collect(),
                folded_count: (0..total).map(|_| std::sync::Mutex::new(0)).collect(),
                step_start: (0..total).map(|_| std::sync::Mutex::new(None)).collect(),
                step_tokens: (0..total).map(|_| std::sync::Mutex::new((0, 0))).collect(),
                step_phase_text: (0..total).map(|_| std::sync::Mutex::new(String::new())).collect(),
                files, group_index, group_size, step_to_row, rows,
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
        let (term_rows, _cols) = console::Term::stdout().size();
        let window = (term_rows as usize).saturating_sub(6).clamp(1, 8);
        let tail = multi.add(ProgressBar::new_spinner());
        tail.set_style(
            ProgressStyle::with_template("  {msg}").unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        let bars = (0..rows.len()).map(|_| std::sync::Mutex::new(None)).collect();
        let children = (0..total).map(|_| std::sync::Mutex::new(Vec::new())).collect();
        let fold_bar = (0..total).map(|_| std::sync::Mutex::new(None)).collect();
        let folded_count = (0..total).map(|_| std::sync::Mutex::new(0)).collect();
        let step_start = (0..total).map(|_| std::sync::Mutex::new(None)).collect();
        let step_tokens = (0..total).map(|_| std::sync::Mutex::new((0, 0))).collect();
        let step_phase_text = (0..total).map(|_| std::sync::Mutex::new(String::new())).collect();
        let progress = Self {
            multi: Some(multi),
            bars,
            children,
            fold_bar,
            folded_count,
            step_start,
            step_tokens,
            step_phase_text,
            files, group_index, group_size, step_to_row, rows,
            window,
            revealed: std::sync::Mutex::new(0),
            tail: Some(tail),
        };
        if total > 0 {
            progress.reveal_through(0);
        }
        progress
    }

    fn file(&self, idx: usize) -> &str {
        self.files.get(idx).map(String::as_str).unwrap_or("")
    }

    fn bar_slot(&self, idx: usize) -> Option<&std::sync::Mutex<Option<indicatif::ProgressBar>>> {
        let row = *self.step_to_row.get(idx)?;
        self.bars.get(row)
    }

    /// Ensures every row from 0 up through step `idx`'s preview window exists (headers plus
    /// their steps), and updates the trailing "N more step(s)" summary. A newly-created step
    /// row beyond `idx` itself gets a dim "pending" message; `phase`/`collapse` handle
    /// upgrading `idx`'s own bar afterward. Headers get their final style immediately —
    /// they're never "activated" or collapsed the way a step is.
    fn reveal_through(&self, idx: usize) {
        let (Some(multi), Some(tail)) = (&self.multi, &self.tail) else { return };
        let Some(&target_row) = self.step_to_row.get(idx) else { return };
        // Walk forward from the target step's own row, counting STEP rows only (a header
        // doesn't count against the preview budget), until `window` more steps are included.
        let mut steps_included = 0usize;
        let mut end_row = target_row;
        while end_row + 1 < self.rows.len() && steps_included < self.window {
            end_row += 1;
            if matches!(self.rows[end_row], Row::Step(_)) {
                steps_included += 1;
            }
        }
        let target = end_row + 1;
        let mut revealed = self.revealed.lock().unwrap();
        if *revealed >= target {
            return;
        }
        use indicatif::{ProgressBar, ProgressStyle};
        let step_style = ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        let header_style = ProgressStyle::with_template("{msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        while *revealed < target {
            let pb = multi.insert_before(tail, ProgressBar::new_spinner());
            match &self.rows[*revealed] {
                Row::Header(label) => {
                    pb.set_style(header_style.clone());
                    pb.set_message(label.clone());
                }
                Row::Step(step_idx) => {
                    pb.set_style(step_style.clone());
                    pb.set_message(pending_grey(&step_label(self.group_index[*step_idx], self.group_size[*step_idx], self.file(*step_idx))));
                }
            }
            *self.bars[*revealed].lock().unwrap() = Some(pb);
            *revealed += 1;
        }
        let remaining_steps = self.rows[*revealed..].iter().filter(|r| matches!(r, Row::Step(_))).count();
        tail.set_message(if remaining_steps > 0 {
            pending_grey(&format!("… {remaining_steps} more step(s)"))
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

    /// " · 3.4k in / 890 out" once the step has spent any tokens, else empty — so a
    /// just-activated step's header doesn't show a premature "0 in / 0 out".
    fn tokens_suffix(&self, idx: usize) -> String {
        let (input, output) = self.step_tokens.get(idx).map(|m| *m.lock().unwrap()).unwrap_or((0, 0));
        if input == 0 && output == 0 {
            String::new()
        } else {
            format!(" · tokens: {} in / {} out", format_tokens(input), format_tokens(output))
        }
    }

    /// Marks step `idx` as the active one and sets its current phase text
    /// (e.g. "TDD Red — generating stub"). Safe to call repeatedly as the phase changes.
    pub(crate) fn phase(&self, idx: usize, phase: &str) {
        self.reveal_through(idx);
        let mut first_activation = false;
        if let Some(m) = self.step_start.get(idx) {
            let mut start = m.lock().unwrap();
            if start.is_none() {
                *start = Some(std::time::Instant::now());
                first_activation = true;
            }
        }
        if let Some(m) = self.step_phase_text.get(idx) {
            *m.lock().unwrap() = phase.to_string();
        }
        let label = step_label(self.group_index[idx], self.group_size[idx], self.file(idx));
        match self.bar_slot(idx).and_then(|m| m.lock().unwrap().clone()) {
            Some(pb) => {
                if first_activation {
                    // Only an activated step's bar ticks a live elapsed time — switching style
                    // (rather than baking it into the shared "pending" style from `reveal_through`)
                    // keeps upcoming preview steps from showing a growing elapsed since the moment
                    // they merely scrolled into the preview window, which isn't when they started.
                    pb.reset_elapsed();
                    use indicatif::ProgressStyle;
                    pb.set_style(with_myelapsed(
                        ProgressStyle::with_template("  {spinner:.cyan} {msg} ({myelapsed})")
                            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                    ));
                }
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                pb.set_message(format!("{label} — {phase}{}", self.tokens_suffix(idx)));
            }
            None => self.println(format!("\n{label} — {phase}{}", self.tokens_suffix(idx))),
        }
    }

    /// Re-renders step `idx`'s own header line with its current phase text and latest
    /// aggregated token total — called after `timed()` updates the token count, so the
    /// running total on the step header itself advances as sub-operations finish, not just
    /// once the whole step collapses.
    fn refresh_step_header(&self, idx: usize) {
        let label = step_label(self.group_index[idx], self.group_size[idx], self.file(idx));
        let phase = self.step_phase_text.get(idx).map(|m| m.lock().unwrap().clone()).unwrap_or_default();
        if let Some(pb) = self.bar_slot(idx).and_then(|m| m.lock().unwrap().clone()) {
            pb.set_message(format!("{label} — {phase}{}", self.tokens_suffix(idx)));
        }
    }

    /// Creates (once) or updates the "… N earlier attempt(s) folded" line for step `idx`,
    /// positioned right after the step's own parent bar so it always sits above whatever
    /// children remain visible.
    fn ensure_fold_bar(&self, idx: usize, n: usize) {
        let Some(multi) = &self.multi else { return };
        let msg = pending_grey(&format!("… {n} earlier attempt(s) folded"));
        let Some(slot_lock) = self.fold_bar.get(idx) else { return };
        let mut slot = slot_lock.lock().unwrap();
        match slot.as_ref() {
            Some(pb) => pb.set_message(msg),
            None => {
                if let Some(parent) = self.bar_slot(idx).and_then(|m| m.lock().unwrap().clone()) {
                    use indicatif::{ProgressBar, ProgressStyle};
                    let pb = multi.insert_after(&parent, ProgressBar::new_spinner());
                    pb.set_style(
                        ProgressStyle::with_template("      {msg}").unwrap_or_else(|_| ProgressStyle::default_spinner()),
                    );
                    pb.set_message(msg);
                    *slot = Some(pb);
                }
            }
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
        if let Some(m) = self.fold_bar.get(idx) {
            if let Some(pb) = m.lock().unwrap().take() {
                if clear_children { pb.finish_and_clear(); } else { pb.finish(); }
            }
        }
        if let Some(m) = self.folded_count.get(idx) {
            *m.lock().unwrap() = 0;
        }
        match self.bar_slot(idx).and_then(|m| m.lock().unwrap().take()) {
            Some(pb) => {
                // An activated step's bar was switched (in `phase`) to a template ending in
                // `({elapsed_precise})` so it ticks live. `line` here already embeds its own
                // elapsed text via `step_summary` — finishing without resetting the style would
                // render the bar's OWN elapsed suffix a second time after it, e.g.
                // "✗ [4/9] foo.ts (2m39s · 11.7k in / 668 out) (00:02:39)". Reset to a bare
                // `{msg}` template first so `line` is the only thing shown.
                use indicatif::ProgressStyle;
                pb.set_style(
                    ProgressStyle::with_template("  {msg}").unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.finish_with_message(line);
            }
            None => self.println(line),
        }
    }

    /// Elapsed time since the step's first `phase()` call, plus tokens accumulated across every
    /// `timed()` call for it — `None` when the step never ran any LLM work this invocation.
    fn step_summary(&self, idx: usize) -> Option<String> {
        let start = (*self.step_start.get(idx)?.lock().unwrap())?;
        let (input, output) = self.step_tokens.get(idx).map(|m| *m.lock().unwrap()).unwrap_or((0, 0));
        Some(format!(
            "{} · tokens: {} in / {} out",
            format_elapsed(start.elapsed()),
            format_tokens(input),
            format_tokens(output),
        ))
    }

    pub(crate) fn done(&self, idx: usize, note: &str) {
        let label = step_label(self.group_index[idx], self.group_size[idx], self.file(idx));
        let line = match self.step_summary(idx) {
            Some(s) => format!("✓ {label} ({note} · {s})"),
            None => format!("✓ {label} ({note})"),
        };
        self.collapse(idx, dim_strike(&line), true);
    }

    pub(crate) fn skipped(&self, idx: usize, note: &str) {
        let label = step_label(self.group_index[idx], self.group_size[idx], self.file(idx));
        let line = match self.step_summary(idx) {
            Some(s) => format!("↷ {label} ({note} · {s})"),
            None => format!("↷ {label} ({note})"),
        };
        self.collapse(idx, dim_strike(&line), true);
    }

    pub(crate) fn failed(&self, idx: usize) {
        let label = step_label(self.group_index[idx], self.group_size[idx], self.file(idx));
        let line = match self.step_summary(idx) {
            Some(s) => format!("{} {label} ({s})", red("✗")),
            None => format!("{} {label}", red("✗")),
        };
        self.collapse(idx, line, false);
    }

    /// Runs `f`, showing a spinner nested directly under step `idx`'s line while it runs
    /// (or a plain line when non-interactive) — the step-checklist equivalent of the
    /// free-standing `with_spinner` above.
    pub(crate) fn timed<F, T>(&self, idx: usize, label: impl Into<String>, client: &LlmClient, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let label = label.into();
        let start = std::time::Instant::now();
        let before_tokens = client.token_totals();
        // A bare "done Red — generating test..." line has no visible indication of which step
        // it belongs to once printed on its own — `n` makes every fallback (non-interactive)
        // line self-identifying regardless of what's still visible above it.
        let n = format!("[{}/{}]", self.group_index[idx], self.group_size[idx]);
        // Anchor the new spinner after whichever bar is currently LAST for this step: the most
        // recently accumulated sub-bar if there is one, else the parent itself. Always anchoring
        // on the parent would insert each new attempt BETWEEN it and the previous ones instead
        // of appending after them.
        let children_lock = self.children.get(idx);
        let anchor = match children_lock.and_then(|m| m.lock().unwrap().last().cloned()) {
            Some(last_child) => Some(last_child),
            None => self.bar_slot(idx).and_then(|m| m.lock().unwrap().clone()),
        };
        // "      ↳ " — deliberately NOT just more leading spaces than the parent bar's "  ": a
        // column-count difference alone reads as coincidental once a finished parent bar's blank
        // spinner slot and an active child spinner glyph land in visually similar positions. The
        // ↳ glyph makes the nesting unambiguous regardless of exact column math.
        let child = match (&self.multi, anchor) {
            (Some(multi), Some(anchor)) => {
                use indicatif::{ProgressBar, ProgressStyle};
                let pb = multi.insert_after(&anchor, ProgressBar::new_spinner());
                pb.set_style(with_myelapsed(
                    ProgressStyle::with_template("      ↳ {spinner:.cyan} {msg} ({myelapsed})")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                ));
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
        let after_tokens = client.token_totals();
        if let Some(m) = self.step_tokens.get(idx) {
            let mut t = m.lock().unwrap();
            t.0 += after_tokens.0.saturating_sub(before_tokens.0);
            t.1 += after_tokens.1.saturating_sub(before_tokens.1);
        }
        self.refresh_step_header(idx);
        if let Some(pb) = child {
            // Freeze with the label struck through (matching the grey/strikethrough a finished
            // STEP collapses to via `dim_strike`) and KEEP it (finish_with_message, not clear) —
            // it stays visible as part of this step's running history until the whole step
            // collapses via `collapse`, which drains and drops every bar accumulated here.
            // No elapsed time embedded here — the bar's own template already appends
            // `({elapsed_precise})` after {msg} on every render, active or finished; adding
            // another one here just duplicated it ("(1m29s) (00:01:29)").
            pb.finish_with_message(dim_strike(&label));
            if let Some(m) = children_lock {
                let mut children = m.lock().unwrap();
                children.push(pb);
                // Fold the oldest attempts away once there are more than MAX_VISIBLE_CHILDREN —
                // see the `fold_bar` field doc for why unbounded growth here is worth avoiding.
                let mut newly_folded = 0usize;
                while children.len() > MAX_VISIBLE_CHILDREN {
                    children.remove(0).finish_and_clear();
                    newly_folded += 1;
                }
                drop(children);
                if newly_folded > 0 {
                    if let Some(fc) = self.folded_count.get(idx) {
                        let mut count = fc.lock().unwrap();
                        *count += newly_folded;
                        let n = *count;
                        drop(count);
                        self.ensure_fold_bar(idx, n);
                    }
                }
            }
        } else {
            self.println(format!("      ↳ {n} {} ({})", dim_strike(&label), format_elapsed(start.elapsed())));
        }
        result
    }

    /// Freezes every bar that's still live — pending previews, group headers, the tail
    /// summary — with whatever it's currently showing. Call this before returning early (a
    /// broken build stopping the run): indicatif's default finish behavior for a bar dropped
    /// without ever being explicitly finished is `AndClear`, which makes it vanish with no
    /// trace. Without this, every step that hadn't been reached yet (and even the section
    /// headers) would silently disappear the moment this checklist itself is torn down,
    /// instead of staying visible as "here's what was still left."
    pub(crate) fn freeze(&self) {
        for m in &self.bars {
            if let Some(pb) = m.lock().unwrap().as_ref() {
                pb.finish();
            }
        }
        if let Some(tail) = &self.tail {
            tail.finish();
        }
    }

    /// Appends `note` to the most recently finished `timed` sub-bar for step `idx` — used for
    /// the model's self-reported summary/deviations, which used to go through `println` and
    /// land as a bare top-level line ABOVE THE ENTIRE MultiProgress (that's what indicatif's
    /// own `println` does: prints above every bar, not near any specific one), completely
    /// disconnected from the "done fixing ..." line it was actually describing. Attaching it
    /// to the bar itself means it travels with that bar: visible while the step is active,
    /// and cleared/kept together with the rest of the step's history on collapse.
    pub(crate) fn annotate_last_child(&self, idx: usize, note: &str) {
        if let Some(m) = self.children.get(idx) {
            if let Some(pb) = m.lock().unwrap().last() {
                let current = pb.message();
                pb.set_message(format!("{current}\n        {note}"));
                return;
            }
        }
        self.println(format!("      {note}"));
    }
}

/// Attaches the model's self-reported summary, and anything it flagged as not fully
/// followed, to step `idx`'s most recent `timed` sub-bar — the latter is a direct signal of a
/// skill/prompt gap, surfaced immediately instead of waiting for it to show up as a compile
/// error later.
pub(crate) fn print_step_notes(progress: &Progress, idx: usize, summary: &Option<String>, deviations: &Option<String>) {
    if let Some(s) = summary { progress.annotate_last_child(idx, s); }
    if let Some(d) = deviations { progress.annotate_last_child(idx, &format!("⚠ did not follow: {d}")); }
}
