//! Shared YAML-parsing helper for prompts that ask the model for a batch of independent items
//! (behaviors, blocked candidates, decision links, classifications). Live-verified need: a
//! single malformed item (the model put a `kind` value into a `scope` field) crashed an
//! otherwise-correct 17-item batch outright, because `serde_yaml` deserializes a `Vec<T>`
//! atomically — one bad element fails the whole sequence. Parsing item-by-item instead means
//! one bad item is skipped (and reported), not fatal to everything else the model got right.

use crate::client::LlmError;

/// Strips a leading/trailing code fence regardless of language tag (```yaml, ```json, bare ```)
/// — a fixed prefix list (previously just "```yaml") silently failed to strip other tags: a
/// leading "```json" would only lose its backticks, leaving the literal word "json" glued to the
/// content and breaking the parse. Matching up to the first newline after the opening fence
/// handles any tag, or none, uniformly.
pub(crate) fn strip_code_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    let after_open = match trimmed.strip_prefix("```") {
        Some(rest) => match rest.find('\n') {
            Some(idx) => &rest[idx + 1..],
            None => rest,
        },
        None => trimmed,
    };
    after_open.trim_end_matches("```").trim().to_string()
}

/// Parses `stripped` as a YAML mapping, then leniently parses the sequence at `key` into
/// `Vec<T>` — skipping (and printing a warning for) any element that doesn't match `T`'s shape,
/// rather than failing the whole batch. Returns an error only if `stripped` isn't valid YAML at
/// all, or `key` isn't present as a sequence.
pub(crate) fn parse_lenient_sequence<T: serde::de::DeserializeOwned>(
    stripped: &str,
    key: &str,
) -> Result<Vec<T>, LlmError> {
    let doc: serde_yaml::Value = serde_yaml::from_str(stripped)
        .map_err(|source| LlmError::YamlParse { source, raw: stripped.to_string() })?;
    let Some(serde_yaml::Value::Sequence(items)) = doc.get(key) else {
        return Ok(Vec::new());
    };
    Ok(items.iter().filter_map(|item| {
        match serde_yaml::from_value::<T>(item.clone()) {
            Ok(parsed) => Some(parsed),
            Err(e) => {
                eprintln!("  warning: skipping malformed '{key}' entry: {e}");
                None
            }
        }
    }).collect())
}
