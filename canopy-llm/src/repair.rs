//! Post-processing helpers for raw LLM text output: YAML quirks the model
//! commonly introduces (bad indentation, duplicate keys, unquoted colons in
//! scalars, quoted arrays) and generic markdown code-fence stripping.

use crate::client::LlmError;
use canopy_core::ImplementationStep;

pub(crate) fn fix_yaml_list_indentation(yaml: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_item = false;
    let mut item_indent = 0usize;

    for line in yaml.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if trimmed.starts_with("- ") || trimmed == "-" {
            in_item = true;
            item_indent = indent;
            out.push(line.to_string());
        } else if in_item && !trimmed.is_empty() && indent == item_indent && !trimmed.starts_with('#') {
            // At the same column as the `- ` marker but without one — belongs to the item
            out.push(format!("{}  {}", " ".repeat(item_indent), trimmed));
        } else {
            // Leaving the list item if we hit a non-empty, non-indented line that isn't a new item
            if !trimmed.is_empty() && indent <= item_indent && !trimmed.starts_with('-') {
                in_item = false;
            }
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

pub(crate) fn dedup_yaml_keys(yaml: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    // Track (indent_len, key) pairs seen since the last list-item marker.
    let mut seen: Vec<(usize, &str)> = Vec::new();

    for line in yaml.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        // A new list item resets the seen set for this indent level and deeper.
        if trimmed.starts_with("- ") || trimmed == "-" {
            seen.retain(|(d, _)| *d < indent);
        }

        // Detect a plain mapping key: `key: ...` (no leading `-`).
        if !trimmed.starts_with('-') {
            if let Some(colon) = trimmed.find(": ").or_else(|| trimmed.ends_with(':').then_some(trimmed.len() - 1)) {
                let key = &trimmed[..colon];
                if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    // If already seen at this indent, skip this line (keep the last occurrence
                    // by retroactively removing the earlier one).
                    if let Some(pos) = seen.iter().position(|(d, k)| *d == indent && *k == key) {
                        seen.remove(pos);
                        // Remove the earlier line from out.
                        if let Some(prev) = out.iter().rposition(|l: &&str| {
                            let t = l.trim_start();
                            let id = l.len() - t.len();
                            id == indent && (t.starts_with(&format!("{key}: ")) || t == &format!("{key}:"))
                        }) {
                            out.remove(prev);
                        }
                    }
                    seen.push((indent, key));
                }
            }
        }
        out.push(line);
    }
    out.join("\n")
}

pub(crate) fn fix_yaml_colon_in_scalars(yaml: &str) -> String {
    yaml.lines().map(|line| {
        // Match lines of the form: <indent><key>: <value> where value is unquoted and contains ':'
        if let Some(colon_pos) = line.find(": ") {
            let (key_part, rest) = line.split_at(colon_pos + 2);
            let value = rest.trim_end();
            // type: [string] — LLM uses bracket notation for array types but YAML parses
            // it as an inline sequence. Quote any unquoted bracket-enclosed type annotation.
            // Quote bracket-wrapped type annotations like `type: [string]` but NOT
            // real YAML inline sequences like `depends_on: ["path/to/file.ts"]` or a genuinely
            // empty sequence like `constraints: []` — a real type annotation always names a
            // type, so empty inner content can only be a real empty sequence, never one of these.
            let inner = if value.len() >= 2 { &value[1..value.len()-1] } else { "" };
            if value.starts_with('[') && value.ends_with(']') && value.len() >= 2
                && !value.starts_with("[\n")
                && !inner.is_empty()
                && !inner.contains('"')
                && !inner.contains('\'')
            {
                return format!("{}\"{}\"", key_part, value);
            }
            // Only fix plain (unquoted) scalars that contain a colon
            if !value.is_empty()
                && !value.starts_with('"')
                && !value.starts_with('\'')
                && !value.starts_with('{')
                && !value.starts_with('[')
                && !value.starts_with('|')
                && !value.starts_with('>')
                && value.contains(':')
            {
                let escaped = value.replace('"', "\\\"");
                return format!("{}\"{}\"", key_part, escaped);
            }
        }
        line.to_string()
    }).collect::<Vec<_>>().join("\n")
}

pub(crate) fn fix_broken_quoted_continuations(yaml: &str) -> String {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        // Peek ahead: if next line looks like an orphaned continuation, absorb it.
        if i + 1 < lines.len() && line.ends_with('"') && line.contains(": \"") {
            let next = lines[i + 1].trim();
            let is_continuation = !next.is_empty()
                && !next.starts_with('-')
                && !next.contains(": ")
                && !next.ends_with(':');
            if is_continuation {
                // Insert continuation text before the closing quote.
                let merged = format!("{} {}", &line[..line.len() - 1], next);
                result.push(merged);
                i += 2;
                continue;
            }
        }
        result.push(line.to_string());
        i += 1;
    }
    result.join("\n")
}

// The model sometimes wraps a JSON-style array in double-quotes:
//   depends_on: "["a", "b"]"  →  depends_on: ["a", "b"]
// Strip the outer quotes so serde_yaml sees a valid inline sequence.
pub(crate) fn fix_quoted_depends_on(raw: &str) -> String {
    raw.lines().map(|line| {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("depends_on:") { return line.to_string(); }
        let value = trimmed["depends_on:".len()..].trim();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            let inner = &value[1..value.len() - 1];
            if inner.starts_with('[') {
                let indent = &line[..line.len() - trimmed.len()];
                return format!("{}depends_on: {}", indent, inner);
            }
        }
        line.to_string()
    }).collect::<Vec<_>>().join("\n")
}

pub(crate) fn parse_plan_steps(raw: &str) -> Result<Vec<ImplementationStep>, LlmError> {
    // Strip opening fence and any prose before the YAML block.
    let after_open = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim();
    // Truncate at the first closing fence line (``` or ```yaml) that appears
    // after YAML content has started. The model sometimes appends a closing
    // fence + prose explanation after valid YAML.
    let stripped = {
        let mut yaml_started = false;
        let mut end = after_open.len();
        for (i, line) in after_open.lines().enumerate() {
            let _ = i;
            let t = line.trim();
            if !yaml_started {
                if t.starts_with("steps:") || t.starts_with("- ") || t.starts_with("- id:") {
                    yaml_started = true;
                }
            } else if t.starts_with("```") {
                // Closing fence after YAML — truncate here.
                end = after_open.find(line).unwrap_or(after_open.len());
                break;
            }
        }
        after_open[..end].trim()
    };
    // If the model omitted the `steps:` root key and emitted a bare sequence, wrap it.
    let wrapped: std::borrow::Cow<str> = if stripped.starts_with("- ") || stripped.starts_with("- id:") {
        std::borrow::Cow::Owned(format!("steps:\n{}", stripped))
    } else {
        std::borrow::Cow::Borrowed(stripped)
    };
    let fixed = dedup_yaml_keys(&fix_yaml_colon_in_scalars(&fix_yaml_list_indentation(&fix_broken_quoted_continuations(&fix_quoted_depends_on(&wrapped)))));
    #[derive(serde::Deserialize)]
    struct PlanResponse { steps: Vec<ImplementationStep> }
    let parsed: PlanResponse = serde_yaml::from_str(&fixed)
        .map_err(|source| LlmError::YamlParse { source, raw: fixed })?;
    Ok(parsed.steps)
}

pub(crate) fn repair_list_item_indentation(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + 64);
    let mut in_item = false;
    for line in yaml.lines() {
        if line.trim_start().starts_with("- ") {
            in_item = true;
            out.push_str(line);
        } else if in_item
            && !line.trim().is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
        {
            out.push_str("  ");
            out.push_str(line);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

pub(crate) fn strip_code_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    let content = if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip optional language label (everything up to and including the first newline)
        if let Some(newline) = rest.find('\n') {
            &rest[newline + 1..]
        } else {
            rest.trim_start()
        }
    } else {
        trimmed
    };
    if let Some(pos) = content.rfind("\n```") {
        content[..pos].to_string()
    } else {
        content.trim_end_matches("```").trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_yaml_list_indentation_repairs_missing_indent() {
        let input = "steps:\n- id: \"1\"\nservice: product-service\nfile: foo.java\noperation: create\ndescription: Do something.\n- id: \"2\"\nservice: admin-portal\nfile: bar.tsx\noperation: create\ndescription: Do another thing.";
        let result = fix_yaml_list_indentation(input);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result).expect("should parse");
        let steps = parsed["steps"].as_sequence().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["service"].as_str().unwrap(), "product-service");
        assert_eq!(steps[1]["service"].as_str().unwrap(), "admin-portal");
    }

    #[test]
    fn dedup_yaml_keys_removes_duplicate_operation() {
        let input = "steps:\n- id: \"8\"\n  service: product-service\n  file: foo.java\n  operation: create\n  operation: modify\n  description: Do something.";
        let result = dedup_yaml_keys(input);
        // Only one `operation:` line should survive, and it should be the last one
        let count = result.lines().filter(|l| l.trim_start().starts_with("operation:")).count();
        assert_eq!(count, 1);
        assert!(result.contains("operation: modify"));
    }

    #[test]
    fn dedup_yaml_keys_leaves_unique_keys_intact() {
        let input = "steps:\n- id: \"1\"\n  service: svc\n  file: a.java\n  operation: create\n  description: Create something.";
        let result = dedup_yaml_keys(input);
        assert_eq!(result, input);
    }

    #[test]
    fn fix_yaml_colon_in_scalars_quotes_a_bracket_type_annotation() {
        let input = "    type: [string]";
        let result = fix_yaml_colon_in_scalars(input);
        assert_eq!(result, "    type: \"[string]\"");
    }

    #[test]
    fn fix_yaml_colon_in_scalars_leaves_a_real_empty_sequence_untouched() {
        // Regression: this used to get quoted into `constraints: "[]"`, a string, which then
        // fails to deserialize into a Vec<String> field one level up.
        let input = "    constraints: []";
        let result = fix_yaml_colon_in_scalars(input);
        assert_eq!(result, input);
        let parsed: serde_yaml::Value = serde_yaml::from_str(result.trim()).expect("should parse");
        assert!(parsed["constraints"].as_sequence().unwrap().is_empty());
    }

    #[test]
    fn fix_yaml_colon_in_scalars_leaves_a_real_quoted_sequence_untouched() {
        let input = "    depends_on: [\"path/to/file.ts\"]";
        let result = fix_yaml_colon_in_scalars(input);
        assert_eq!(result, input);
    }

    #[test]
    fn fix_yaml_colon_in_scalars_quotes_an_unquoted_scalar_containing_a_colon() {
        let input = "    decision: ProductCreated on topic: product-events";
        let result = fix_yaml_colon_in_scalars(input);
        assert_eq!(result, "    decision: \"ProductCreated on topic: product-events\"");
    }
}
