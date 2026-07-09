use crate::repair::strip_code_fences;

/// Result of any LLM call that produces file content: the content itself, plus whatever
/// the model reported about its own work.
pub struct StepResult {
    pub content: String,
    pub summary: Option<String>,
    /// Anything the model flagged as NOT fully following an instruction in the prompt
    /// (a skill rule, a contract shape, a test requirement) — `None` when the model
    /// reported full compliance. Surfacing this is cheap: it turns a silent, guessed
    /// deviation into a printed line the developer can act on immediately, instead of
    /// discovering it three steps later as a compile error or a wrong test assertion.
    pub deviations: Option<String>,
}

const SUMMARY_SEPARATOR: &str = "##CANOPY_SUMMARY##";
const DEVIATIONS_SEPARATOR: &str = "##CANOPY_DEVIATIONS##";

/// Closing instructions shared by every implementation-facing prompt (Red test, Red stub,
/// Green, direct, and fix). Keeping this in one place means the summary/deviations contract
/// can't drift between call sites the way the separator text once did.
pub(crate) fn canopy_summary_contract() -> &'static str {
    "Then append exactly this separator on its own line:\n\
     ##CANOPY_SUMMARY##\n\
     Then one line: what you did and the key decision you made — up to 60 words, be concrete\n\
     and specific (name the exact fields, types, or patterns involved) rather than terse.\n\
     Then one line: why — the reason behind that decision, same 60-word allowance.\n\
     Then append exactly this separator on its own line:\n\
     ##CANOPY_DEVIATIONS##\n\
     Then one line: any rule or instruction above that you did NOT fully follow, and why — \
     or exactly \"None\" if you followed all of them.\n\
     No code fences, no markdown, no extra text outside the file content, summary, and deviations."
}

pub(crate) fn split_step_response(raw: &str) -> StepResult {
    let stripped = strip_code_fences(raw);
    let Some(pos) = stripped.find(SUMMARY_SEPARATOR) else {
        return StepResult { content: stripped, summary: None, deviations: None };
    };
    let content = stripped[..pos].trim_end().to_string();
    let rest = stripped[pos + SUMMARY_SEPARATOR.len()..].trim_start();
    let (summary_part, deviations_part) = match rest.find(DEVIATIONS_SEPARATOR) {
        Some(dpos) => (rest[..dpos].trim().to_string(), rest[dpos + DEVIATIONS_SEPARATOR.len()..].trim().to_string()),
        None => (rest.trim().to_string(), String::new()),
    };
    let summary = if summary_part.is_empty() { None } else { Some(summary_part) };
    let deviations = if deviations_part.is_empty() || deviations_part.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(deviations_part)
    };
    StepResult { content, summary, deviations }
}
