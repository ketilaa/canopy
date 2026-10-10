//! A small Gherkin reader: the subset a plan needs to know what has to pass.
//!
//! Reads Feature, Background, Scenario, Scenario Outline (with Examples tables) and their steps. Tags,
//! comments, free-text descriptions and doc strings are skipped. Anything else is an error that names the
//! line, so a feature the Planner cannot account for is refused, never half-read.

#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    pub title: String,
    pub background: Vec<Step>,
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    /// A slug of the title: stable, and what a plan step names in `covers`. An outline is one scenario
    /// with one id; its rows are the cases the plan has to account for.
    pub id: String,
    pub title: String,
    pub steps: Vec<Step>,
    /// Present for an outline. Several `Examples` tables are merged, in order.
    pub examples: Option<Examples>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub keyword: Keyword,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Keyword {
    Given,
    When,
    Then,
    And,
    But,
}

/// The cells are kept verbatim: they are the bounded set the model must account for.
#[derive(Debug, Clone, PartialEq)]
pub struct Examples {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("feature line {line}: {message}")]
pub struct GherkinError {
    pub line: usize,
    pub message: String,
}

fn err(line: usize, message: impl Into<String>) -> GherkinError {
    GherkinError {
        line,
        message: message.into(),
    }
}

impl Feature {
    pub fn parse(text: &str) -> Result<Self, GherkinError> {
        Parser::default().run(text)
    }

    pub fn scenario_ids(&self) -> Vec<String> {
        self.scenarios.iter().map(|s| s.id.clone()).collect()
    }
}

/// Lowercase, every run of non-alphanumerics becomes one `-`, none at either end.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn keyword_step(line: &str) -> Option<Step> {
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let keyword = match word {
        "Given" => Keyword::Given,
        "When" => Keyword::When,
        "Then" => Keyword::Then,
        "And" | "*" => Keyword::And,
        "But" => Keyword::But,
        _ => return None,
    };
    Some(Step {
        keyword,
        text: rest.trim().to_string(),
    })
}

fn after<'a>(line: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| line.strip_prefix(p)).map(str::trim)
}

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

enum Section {
    Description,
    Background,
    Scenario,
    Examples { line: usize, header_seen: bool },
}

struct Open {
    line: usize,
    outline: bool,
    scenario: Scenario,
}

struct Parser {
    title: Option<String>,
    background: Vec<Step>,
    scenarios: Vec<Scenario>,
    open: Option<Open>,
    section: Section,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            title: None,
            background: vec![],
            scenarios: vec![],
            open: None,
            section: Section::Description,
        }
    }
}

impl Parser {
    fn run(mut self, text: &str) -> Result<Feature, GherkinError> {
        let mut in_doc_string = false;
        for (index, raw) in text.lines().enumerate() {
            let number = index + 1;
            let line = raw.trim();
            if line.starts_with("\"\"\"") || line.starts_with("```") {
                in_doc_string = !in_doc_string;
                continue;
            }
            if in_doc_string || line.is_empty() || line.starts_with('#') || line.starts_with('@') {
                continue;
            }
            self.line(number, line)?;
        }
        self.close_scenario()?;
        let title = self
            .title
            .ok_or_else(|| err(1, "the feature must start with a Feature: line"))?;
        if self.scenarios.is_empty() {
            return Err(err(1, "the feature has no scenario"));
        }
        Ok(Feature {
            title,
            background: self.background,
            scenarios: self.scenarios,
        })
    }

    fn line(&mut self, number: usize, line: &str) -> Result<(), GherkinError> {
        if self.title.is_none() {
            return match after(line, &["Feature:"]) {
                Some(title) => {
                    self.title = Some(title.to_string());
                    Ok(())
                }
                None => Err(err(number, "the feature must start with a Feature: line")),
            };
        }
        if after(line, &["Feature:"]).is_some() {
            return Err(err(number, "a second Feature: line; one feature per file"));
        }
        if after(line, &["Background:"]).is_some() {
            self.close_scenario()?;
            self.section = Section::Background;
            return Ok(());
        }
        if let Some(title) = after(line, &["Scenario Outline:", "Scenario Template:"]) {
            return self.open_scenario(number, title, true);
        }
        if let Some(title) = after(line, &["Scenario:", "Example:"]) {
            return self.open_scenario(number, title, false);
        }
        if after(line, &["Examples:", "Scenarios:"]).is_some() {
            return self.open_examples(number);
        }
        if line.starts_with('|') {
            return self.row(number, line);
        }
        if let Some(step) = keyword_step(line) {
            return self.step(number, step);
        }
        match self.section {
            Section::Description => Ok(()),
            _ => Err(err(number, format!("not understood: {line:?}"))),
        }
    }

    fn step(&mut self, number: usize, step: Step) -> Result<(), GherkinError> {
        match (&self.section, self.open.as_mut()) {
            (Section::Background, _) => self.background.push(step),
            (Section::Scenario, Some(open)) => open.scenario.steps.push(step),
            _ => return Err(err(number, "a step outside a Background or Scenario")),
        }
        Ok(())
    }

    fn open_scenario(&mut self, number: usize, title: &str, outline: bool) -> Result<(), GherkinError> {
        self.close_scenario()?;
        let id = slug(title);
        if id.is_empty() {
            return Err(err(number, "a scenario needs a title"));
        }
        if self.scenarios.iter().any(|s| s.id == id) {
            return Err(err(
                number,
                format!("the scenario id {id:?} is already used; scenario titles must differ"),
            ));
        }
        self.section = Section::Scenario;
        self.open = Some(Open {
            line: number,
            outline,
            scenario: Scenario {
                id,
                title: title.to_string(),
                steps: vec![],
                examples: None,
            },
        });
        Ok(())
    }

    fn open_examples(&mut self, number: usize) -> Result<(), GherkinError> {
        self.close_examples()?;
        match &self.open {
            Some(open) if open.outline => {
                self.section = Section::Examples {
                    line: number,
                    header_seen: false,
                };
                Ok(())
            }
            _ => Err(err(number, "Examples belong to a Scenario Outline")),
        }
    }

    fn row(&mut self, number: usize, line: &str) -> Result<(), GherkinError> {
        let Section::Examples { header_seen, .. } = &mut self.section else {
            return Err(err(number, "a table row outside an Examples table"));
        };
        let open = self.open.as_mut().expect("an Examples section has an open outline");
        let row = cells(line);
        if !*header_seen {
            *header_seen = true;
            match &mut open.scenario.examples {
                None => {
                    open.scenario.examples = Some(Examples {
                        columns: row,
                        rows: vec![],
                    })
                }
                Some(examples) if examples.columns == row => {}
                Some(_) => return Err(err(number, "this Examples table has different columns from the first")),
            }
            return Ok(());
        }
        let examples = open.scenario.examples.as_mut().expect("the header came first");
        if row.len() != examples.columns.len() {
            return Err(err(
                number,
                format!("{} cells, but the header has {}", row.len(), examples.columns.len()),
            ));
        }
        examples.rows.push(row);
        Ok(())
    }

    fn close_examples(&mut self) -> Result<(), GherkinError> {
        if let Section::Examples {
            line,
            header_seen: false,
        } = self.section
        {
            return Err(err(line, "an Examples table needs a header row"));
        }
        Ok(())
    }

    fn close_scenario(&mut self) -> Result<(), GherkinError> {
        self.close_examples()?;
        if let Some(open) = self.open.take() {
            if open.scenario.steps.is_empty() {
                return Err(err(open.line, "a scenario needs at least one step"));
            }
            if open.outline && open.scenario.examples.as_ref().is_none_or(|e| e.rows.is_empty()) {
                return Err(err(open.line, "a Scenario Outline needs an Examples table with rows"));
            }
            self.scenarios.push(open.scenario);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDGETS: &str = include_str!("../../canopy-implementer/tasks/widgets/feature.feature");

    #[test]
    fn the_widgets_feature_has_a_plain_scenario_and_an_outline_with_stable_ids() {
        let feature = Feature::parse(WIDGETS).unwrap();
        assert_eq!(feature.title, "Create a widget");
        assert_eq!(
            feature.scenario_ids(),
            [
                "a-valid-name-creates-a-widget-with-a-generated-id",
                "an-invalid-name-is-rejected"
            ]
        );
        let plain = &feature.scenarios[0];
        assert_eq!(plain.title, "A valid name creates a widget with a generated id");
        assert_eq!(plain.examples, None, "a plain scenario has no examples");
        assert_eq!(plain.steps.len(), 3);
        assert_eq!(plain.steps[0].keyword, Keyword::When);
        assert_eq!(plain.steps[0].text, "a client posts a widget named \"Alpha\"");
        assert_eq!(plain.steps[2].keyword, Keyword::And);
    }

    #[test]
    fn an_outline_keeps_its_example_cells_verbatim_one_case_per_row() {
        let feature = Feature::parse(WIDGETS).unwrap();
        let outline = &feature.scenarios[1];
        let examples = outline.examples.as_ref().expect("an outline has examples");
        assert_eq!(examples.columns, ["name"]);
        assert_eq!(
            examples.rows,
            [["a blank name"], ["no name"], ["a name of 51 characters"]]
        );
        assert_eq!(outline.steps[0].text, "a client posts a widget with <name>");
    }

    #[test]
    fn comments_tags_blank_lines_descriptions_and_doc_strings_are_ignored() {
        let text = "# a comment\n@wip\nFeature: F\n  As a user\n  I want things\n\n  @smoke\n  Scenario: S one\n    # inside\n    Given a thing\n    \"\"\"\n    When not a step\n    \"\"\"\n    Then done\n";
        let feature = Feature::parse(text).unwrap();
        assert_eq!(feature.scenarios.len(), 1);
        let keywords: Vec<_> = feature.scenarios[0]
            .steps
            .iter()
            .map(|s| (s.keyword, s.text.as_str()))
            .collect();
        assert_eq!(keywords, [(Keyword::Given, "a thing"), (Keyword::Then, "done")]);
    }

    #[test]
    fn a_background_is_kept_apart_from_the_scenarios() {
        let text = "Feature: F\n  Background:\n    Given a client\n  Scenario: S\n    When x\n";
        let feature = Feature::parse(text).unwrap();
        assert_eq!(feature.background.len(), 1);
        assert_eq!(feature.background[0].text, "a client");
        assert_eq!(feature.scenarios[0].steps.len(), 1);
    }

    #[test]
    fn scenario_template_and_scenarios_are_the_outline_synonyms() {
        let text = "Feature: F\n  Scenario Template: S\n    When <a>\n    Scenarios:\n      | a |\n      | 1 |\n";
        let feature = Feature::parse(text).unwrap();
        assert_eq!(feature.scenarios[0].examples.as_ref().unwrap().rows, [["1"]]);
    }

    #[test]
    fn several_columns_and_several_example_tables_are_kept_in_order() {
        let text = "Feature: F\n  Scenario Outline: S\n    When <a> and <b>\n    Examples:\n      | a | b |\n      | 1 | two words |\n    Examples:\n      | a | b |\n      | 3 | 4 |\n";
        let examples = Feature::parse(text).unwrap().scenarios[0].examples.clone().unwrap();
        assert_eq!(examples.columns, ["a", "b"]);
        assert_eq!(examples.rows, [["1", "two words"], ["3", "4"]]);
    }

    fn error_of(text: &str) -> GherkinError {
        Feature::parse(text).expect_err("should not parse")
    }

    #[test]
    fn a_feature_must_start_with_a_feature_line() {
        let e = error_of("Scenario: S\n  When x\n");
        assert_eq!(e.line, 1);
        assert!(e.message.contains("Feature"), "{e}");
    }

    #[test]
    fn a_feature_without_scenarios_is_an_error() {
        let e = error_of("Feature: F\n");
        assert!(e.message.contains("no scenario"), "{e}");
    }

    #[test]
    fn an_outline_without_examples_is_an_error_naming_the_outline_line() {
        let e = error_of("Feature: F\n\n  Scenario Outline: S\n    When <a>\n");
        assert_eq!(e.line, 3);
        assert!(e.message.contains("Examples"), "{e}");
    }

    #[test]
    fn examples_in_a_plain_scenario_or_without_a_header_row_are_errors() {
        let e = error_of("Feature: F\n  Scenario: S\n    When x\n    Examples:\n      | a |\n      | 1 |\n");
        assert_eq!(e.line, 4);
        let e = error_of("Feature: F\n  Scenario Outline: S\n    When <a>\n    Examples:\n");
        assert_eq!(e.line, 4);
        assert!(e.message.contains("header"), "{e}");
    }

    #[test]
    fn a_row_with_the_wrong_number_of_cells_is_an_error_naming_its_line() {
        let e =
            error_of("Feature: F\n  Scenario Outline: S\n    When <a>\n    Examples:\n      | a | b |\n      | 1 |\n");
        assert_eq!(e.line, 6);
        assert!(e.message.contains("cells"), "{e}");
    }

    #[test]
    fn an_unrecognised_line_is_an_error_naming_it() {
        let e = error_of("Feature: F\n  Scenario: S\n    When x\n    Nonsense here\n");
        assert_eq!(e.line, 4);
        assert!(e.message.contains("Nonsense here"), "{e}");
    }

    #[test]
    fn two_scenarios_with_the_same_title_are_an_error_because_their_ids_would_collide() {
        let e = error_of("Feature: F\n  Scenario: Same!\n    When x\n  Scenario: same\n    When y\n");
        assert_eq!(e.line, 4);
        assert!(e.message.contains("same"), "{e}");
    }

    #[test]
    fn a_scenario_without_steps_is_an_error() {
        let e = error_of("Feature: F\n  Scenario: S\n");
        assert_eq!(e.line, 2);
        assert!(e.message.contains("step"), "{e}");
    }

    #[test]
    fn the_hand_written_widgets_plan_covers_exactly_the_ids_the_parser_produces() {
        let plan =
            canopy_implementer::plan::Plan::from_yaml(include_str!("../../canopy-implementer/tasks/widgets/plan.yaml"))
                .unwrap();
        let ids = Feature::parse(WIDGETS).unwrap().scenario_ids();
        assert_eq!(plan.verify_coverage(&ids), Vec::<String>::new());
    }
}
