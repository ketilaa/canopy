//! Pure helpers for replaying a stored retry prompt in variants (the `retry_replay` example).
//!
//! A retry prompt is a user message built by `prompt::tool_messages`: the task, the project, the files the step
//! shows, then `## Your previous answer` (the failed files), then the failure feedback under a heading, then
//! the retry text. A variant changes only what the model is shown about the failure; everything else, including
//! the system rules, stays as recorded.

/// The files of one answer, as (path, content) in submission order.
pub type Files = Vec<(String, String)>;

const PREVIOUS: &str = "\n## Your previous answer\n";
const FAILURE_HEADINGS: [&str; 2] = ["\n## The build failed to compile\n", "\n## The tests failed\n"];

/// The user message without its `## Your previous answer` section: the model is told what failed, not shown
/// the failing code. `None` when the message has no such section followed by a compile or test failure.
pub fn without_previous_answer(user: &str) -> Option<String> {
    let start = user.find(PREVIOUS)?;
    let end = FAILURE_HEADINGS.iter().filter_map(|h| user[start..].find(h)).min()? + start;
    Some(format!("{}{}", &user[..start], &user[end..]))
}

/// The user message with a `## Where it failed` block added right after the failure feedback, quoting the
/// source lines the feedback points at. Unchanged when no line can be found; `None` when the feedback is not in
/// the message.
pub fn with_failure_locations(user: &str, feedback: &str, previous: &Files) -> Option<String> {
    let at = user.find(feedback)? + feedback.len();
    let lines = locations(feedback, previous);
    if lines.is_empty() {
        return Some(user.to_string());
    }
    let block = format!("\n\n## Where it failed\n{}", lines.join("\n"));
    Some(format!("{}{}{}", &user[..at], block, &user[at..]))
}

/// The source lines a failure points at, as `<path> line <n>: <text>`, at most six, each once. Reads compile
/// errors (`path.java:line:col`) and test failures (`Class.method:line` and `->method:line` frames).
pub fn locations(feedback: &str, files: &Files) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |path: &str, line: usize| {
        let Some((_, content)) = files.iter().find(|(p, _)| p == path) else { return };
        let Some(text) = content.lines().nth(line.saturating_sub(1)) else { return };
        let entry = format!("{path} line {line}: {}", text.trim());
        if !out.contains(&entry) {
            out.push(entry);
        }
    };
    let by_class = |class: &str| files.iter().map(|(p, _)| p.as_str()).find(|p| p.ends_with(&format!("/{class}.java")));
    for token in feedback.split_whitespace() {
        let token = token.trim_matches(|c| c == ',' || c == ';');
        for frame in token.split("->") {
            let parts: Vec<&str> = frame.split(':').collect();
            // path.java:line[:col]
            if parts.len() >= 2 && parts[0].ends_with(".java") {
                if let Ok(line) = parts[1].parse::<usize>() {
                    if let Some((path, _)) = files.iter().find(|(p, _)| p == parts[0]) {
                        add(&path.clone(), line);
                    }
                }
            }
        }
        // Class.method:line, then ->method:line frames of the same class
        let mut frames = token.split("->");
        if let Some(first) = frames.next() {
            let parts: Vec<&str> = first.split(':').collect();
            if parts.len() == 2 && !parts[0].ends_with(".java") {
                if let (Some((class, _)), Ok(line)) = (parts[0].split_once('.'), parts[1].parse::<usize>()) {
                    if let Some(path) = by_class(class) {
                        let path = path.to_string();
                        add(&path, line);
                        for next in frames {
                            if let Some((_, n)) = next.split_once(':') {
                                if let Ok(line) = n.parse::<usize>() {
                                    add(&path, line);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out.truncate(6);
    out
}

/// The same files at the same paths with the same content, in any order.
pub fn same_files(a: &Files, b: &Files) -> bool {
    let key = |f: &Files| {
        let mut v = f.clone();
        v.sort();
        v
    };
    key(a) == key(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Files {
        vec![(
            "src/test/java/a/OrderTest.java".to_string(),
            "line one\nassertEquals(\"x\", getExceptionMessage());\nmore\n  return thrown.getMessage();\n".to_string(),
        )]
    }

    const FEEDBACK: &str = "OrderTest.testQuantity0:2->getExceptionMessage:4 NullPointer boom\nTests run: 9, Failures: 0, Errors: 2, Skipped: 0";

    fn prompt() -> String {
        format!(
            "## Task\nDo it.\n\n## Project\nP\n\n## Files to create\n- a\n\n## Your previous answer\n### src/test/java/a/OrderTest.java\n```\ncode\n```\n\n## The tests failed\n{FEEDBACK}\n\nYour previous answer was undone.\n"
        )
    }

    #[test]
    fn without_the_previous_answer_the_failure_stays_and_the_code_goes() {
        let user = without_previous_answer(&prompt()).unwrap();
        assert!(!user.contains("Your previous answer\n###") && !user.contains("code\n```"), "{user}");
        assert!(user.contains("## Files to create\n- a\n\n## The tests failed\n"), "{user}");
        assert!(user.contains(FEEDBACK) && user.contains("Your previous answer was undone."), "{user}");
    }

    #[test]
    fn a_prompt_without_a_previous_answer_or_a_failure_gives_none() {
        assert_eq!(without_previous_answer("## Task\nx\n"), None);
        assert_eq!(without_previous_answer("## Your previous answer\n### a\n```\nc\n```\n\n## Your answer could not be used\n"), None);
    }

    #[test]
    fn test_failure_frames_are_quoted_with_their_source_lines() {
        let found = locations(FEEDBACK, &files());
        assert_eq!(
            found,
            [
                "src/test/java/a/OrderTest.java line 2: assertEquals(\"x\", getExceptionMessage());",
                "src/test/java/a/OrderTest.java line 4: return thrown.getMessage();",
            ]
        );
    }

    #[test]
    fn compile_errors_are_quoted_by_path_line_and_column() {
        let files = vec![("src/main/java/A.java".to_string(), "class A {\n  Optional<String> o;\n}\n".to_string())];
        let found = locations("src/main/java/A.java:2:3: cannot find symbol\n  symbol: class Optional", &files);
        assert_eq!(found, ["src/main/java/A.java line 2: Optional<String> o;"]);
        assert_eq!(locations("src/main/java/Other.java:2:3: x", &files), Vec::<String>::new());
    }

    #[test]
    fn the_location_block_goes_right_after_the_feedback() {
        let user = with_failure_locations(&prompt(), FEEDBACK, &files()).unwrap();
        let at = user.find("## Where it failed").unwrap();
        assert!(user.find(FEEDBACK).unwrap() < at && at < user.find("Your previous answer was undone").unwrap(), "{user}");
        assert!(user.contains("OrderTest.java line 2:"), "{user}");
        assert_eq!(with_failure_locations(&prompt(), "no such feedback", &files()), None);
        assert_eq!(with_failure_locations(&prompt(), FEEDBACK, &vec![]), Some(prompt()), "nothing to quote, nothing added");
    }

    #[test]
    fn answers_are_the_same_when_paths_and_contents_match_in_any_order() {
        let a = vec![("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())];
        let b = vec![("b".to_string(), "2".to_string()), ("a".to_string(), "1".to_string())];
        assert!(same_files(&a, &b));
        assert!(!same_files(&a, &vec![("a".to_string(), "1".to_string())]));
        assert!(!same_files(&a, &vec![("a".to_string(), "1".to_string()), ("b".to_string(), "3".to_string())]));
    }
}
