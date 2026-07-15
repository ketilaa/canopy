//! Mechanical instrumentation for the review gates in `intent`, `spec`, and `behaviors` — see
//! `ReviewLogEntry`'s own doc comment in `canopy-core` for why this exists. Best-effort: a
//! logging failure never blocks the review flow itself, since the actual artifact (story/ADR/
//! decision) has already been decided and saved by the time this runs.

use crate::util::iso_now;
use canopy_core::ReviewLogEntry;

pub(crate) fn record_review(command: &str, story_id: Option<&str>, category: &str, subject: &str, outcome: &str) {
    let entry = ReviewLogEntry {
        timestamp: iso_now(),
        command: command.to_string(),
        story_id: story_id.map(str::to_string),
        category: category.to_string(),
        subject: subject.to_string(),
        outcome: outcome.to_string(),
    };
    if let Err(e) = canopy_storage::append_review_log_entry(entry) {
        eprintln!("Warning: failed to record review outcome: {e}");
    }
}
