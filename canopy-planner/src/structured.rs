//! Phase 0 of `docs/design/structured-plan-schema-introduction-plan.md`: the structured plan's own
//! types and its deterministic audits, with no model call and no dependency on the rendered
//! `canopy_implementer::plan::Plan` shape.
//!
//! **Scope note.** This is the minimal subset of `docs/design/structured-plan-schema.md` that the six
//! audits below need: `units`, each with `owns_rules`, `methods` (a `preserves` list and `throws`), and
//! `examples`. The full schema's `facts`, `decisions`, `non_goals` and `api` (component/type detail) are
//! not modelled here — they carry no audit of their own yet, and Phase 1 (the renderer) is where they
//! first matter. Two fields were added beyond the design doc while writing the audits, because a free-text
//! `setup`/`call` is not enough to check them mechanically without a real Java parser:
//! - `Example::setup_values`: a literal, author-supplied anchor (component name -> value) for the
//!   preserved-field check, kept separate from the free-text `setup` so the latter stays natural prose.
//! - `Example::defines`: the identifiers `setup` introduces, for the undefined-identifier check.
//! Both default to empty and are additive; neither changes how `setup`/`call`/`expect` read.
//!
//! Every rule is an audit: it accepts or rejects a plan already written, and never changes a field's
//! value (`docs/principles/deterministic-audits-vs-compensation.md`). Each rule below is built directly
//! from one real run in `canopy-implementer/docs/prompt-and-plan-changelog.md`'s S6a/S6c entries, not a
//! hypothetical case; the test module cites the run each fixture is drawn from.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use canopy_implementer::implementer::{Scope, StepKind};
use canopy_implementer::plan::{Plan, PlanStep};
use serde::Deserialize;

use crate::layered;

/// One exception a method throws, checked in `order`.
#[derive(Debug, Clone, Deserialize)]
pub struct Throw {
    pub order: u32,
    pub exception_type: String,
    pub message: String,
    pub condition: String,
}

/// One method of a unit's API.
#[derive(Debug, Clone, Deserialize)]
pub struct Method {
    pub signature: String,
    /// Fields of the receiver that are unchanged by this call (e.g. `id`, `issuedAt` on `validate`).
    #[serde(default)]
    pub preserves: Vec<String>,
    #[serde(default)]
    pub throws: Vec<Throw>,
}

/// One executable example: a setup, a call, and the expected outcome, as literal strings (never a
/// prose sentence) so two examples can be compared for contradiction without parsing Java.
#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub id: String,
    pub setup: String,
    /// Literal values the setup establishes, by component name. Optional; used only by the
    /// preserved-field check. Example: `{issuedAt: "2026-03-01T10:00"}`.
    #[serde(default)]
    pub setup_values: BTreeMap<String, String>,
    /// Identifiers `setup` binds (e.g. a `setup` of `Ticket ticket = new Ticket(...)` defines `ticket`).
    /// Optional; used only by the undefined-identifier check.
    #[serde(default)]
    pub defines: Vec<String>,
    pub call: String,
    /// The expected outcome, as a flat map of named facts (e.g. `status: VALIDATED`, `empty: "true"`,
    /// `throws: IllegalStateException`, `message: "Ticket has expired"`). Deliberately not a sentence.
    #[serde(default)]
    pub expect: BTreeMap<String, String>,
}

/// One unit (a class, record or interface) of the plan.
#[derive(Debug, Clone, Deserialize)]
pub struct Unit {
    pub id: String,
    /// Rule ids this unit is the sole authority for. Empty means the unit asserts no domain rule of its
    /// own (e.g. a plain repository) — checked by `exceptions_without_ownership` and
    /// `rule_ownership_collisions`.
    #[serde(default)]
    pub owns_rules: Vec<String>,
    #[serde(default)]
    pub methods: Vec<Method>,
    #[serde(default)]
    pub examples: Vec<Example>,
    // --- Phase 1 additions (`to_legacy_plan`); not used by Phase 0's audits. ---
    /// Ids of other units this one's step may read (their main files become its `read_only`), and
    /// which also fixes this unit's position in the rendered plan's step order (a topological sort,
    /// never model-authored): a unit is only rendered once everything it depends on already is.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Scenario ids this unit's step helps make pass, copied verbatim into the rendered step's `covers`.
    #[serde(default)]
    pub covers: Vec<String>,
    /// Free-text Rules prose for this unit (today hand- or model-authored, like the existing layered
    /// Planner's `Draft.rules`; composing this automatically from `owns_rules`/`methods` is not
    /// attempted here — writing the rule in English is the part that stays a model or human job).
    #[serde(default)]
    pub rules: String,
}

/// A question only a human could resolve, with the answer and who resolved it. Folded into every
/// rendered step's instruction (a per-unit relevance filter is not attempted here; on a five-unit story
/// this has not yet been a problem, and splitting it is a straightforward later change).
#[derive(Debug, Clone, Deserialize)]
pub struct Decision {
    pub id: String,
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub resolved_by: String,
}

/// The structured plan. `api`, when present, is the existing layered Planner's own API text
/// (`Package a.b.c.` then one unit per line, `layered::parse_api`'s format) — reused rather than
/// reinvented, and required only by `to_legacy_plan`; Phase 0's audits never read it. Its lines must
/// align 1:1, in order, with `units`.
#[derive(Debug, Clone, Deserialize)]
pub struct StructuredPlan {
    pub id: String,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    pub units: Vec<Unit>,
}

impl StructuredPlan {
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }
}

/// Which rule an issue broke, mirroring the style of `crate::verify::Rule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    /// Two examples share the same `setup` and `call` but disagree on `expect` — the plan asks for two
    /// outcomes from one action, and no retry can satisfy both. S6a run 3 and run 5.
    DuplicateSetupConflictingExpect,
    /// An example's `setup`, `call` or `expect` reads the wall clock (`now()` or `LocalDateTime.now`),
    /// so two "fixed" examples can silently disagree with each other. S6a run 4.
    WallClockCall,
    /// A unit that owns no rule (`owns_rules` empty) still has a method that throws — an exception that
    /// belongs to another unit has leaked into this one. S6c's loans `Loan` unit.
    ExceptionWithoutOwnership,
    /// An example's `expect` states a value for a field the called method declares `preserves`, and
    /// that value differs from the field's value in `setup_values`. S6a run 8 (`issuedAt` overwritten).
    PreservedFieldContradicted,
    /// A bare identifier appears in an example's `call` or `expect` that `setup` never defines (via
    /// `defines`) and that is not a quoted literal, a dotted enum constant, or the unit's own receiver
    /// name. S6a run 5 (`ticket`, `updatedTicket`).
    UndefinedIdentifier,
    /// The same rule id appears in more than one unit's `owns_rules` — two units claim the same rule,
    /// which is exactly how the tickets service plan duplicated `Ticket`'s own validation logic.
    RuleOwnershipCollision,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub rule: Rule,
    pub unit: Option<String>,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.unit {
            Some(unit) => write!(f, "unit {unit}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Names a handful of receiver identifiers every fixture is allowed to use without declaring them via
/// `defines` — the names the schema doc's own examples use for "the thing under test" or a dependency
/// passed into a constructor, not a value the example itself introduces.
const KNOWN_RECEIVERS: &[&str] = &["repository", "repo", "service"];

/// Java keywords that tokenize as a bare lowercase identifier but are never a value an example would
/// need `setup` to define.
const KEYWORDS: &[&str] = &["new", "true", "false", "null", "var"];

/// True for an all-caps or dotted-enum-style token (`ISSUED`, `TicketStatus.ISSUED`) and for a bare
/// number — these are literals, never a variable a setup would need to define.
fn is_constant_literal(token: &str) -> bool {
    let core = token.rsplit('.').next().unwrap_or(token);
    (!core.is_empty() && core.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        || token.chars().all(|c| c.is_ascii_digit())
}

/// Extracts bare lowercase-leading identifier tokens from an expression-like string: a crude tokenizer,
/// not a Java parser, deliberately scoped to the simple `unit.method(args)` shape examples use.
fn identifiers_in(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    for c in text.chars() {
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if c.is_alphanumeric() || c == '_' || c == '.' {
            current.push(c);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
        .into_iter()
        .filter(|t| {
            let first = t.chars().next().unwrap_or(' ');
            first.is_ascii_lowercase() && !is_constant_literal(t)
        })
        // A call's own method name (`findById` in `repository.findById(...)`) is not a value; only the
        // last dotted segment's *leading receiver* matters, so drop tokens that are clearly a method name
        // by position is out of scope for this tokenizer — instead we only flag a token that is not
        // itself followed by `(` in the original text is handled by the caller via a simpler heuristic:
        // see `identifiers_as_values` below, which this function feeds.
        .collect()
}

/// A narrower pass over `identifiers_in`: drops any identifier immediately followed by `(` in the
/// original text (a method call name, e.g. `findById`), and the first segment of a dotted receiver
/// chain (`repository` in `repository.findById(...)`) is kept since it still needs to be a known or
/// defined name.
fn identifiers_as_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    for raw in identifiers_in(text) {
        let name = raw.split('.').next().unwrap_or(&raw).to_string();
        let followed_by_call = text
            .find(&raw)
            .map(|i| text[i + raw.len()..].trim_start().starts_with('('))
            .unwrap_or(false);
        if followed_by_call {
            // `foo(...)` or `a.foo(...)`: `foo` is a method name, not a value; but if it's a dotted
            // chain (`a.b(...)`), `a` itself is still a value reference and must be checked separately.
            if raw.contains('.') && !KEYWORDS.contains(&name.as_str()) {
                values.push(name);
            }
            continue;
        }
        if !KEYWORDS.contains(&name.as_str()) {
            values.push(name);
        }
    }
    values
}

fn duplicate_setup_conflicting_expect(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        let mut seen: BTreeMap<(String, String), &Example> = BTreeMap::new();
        for example in &unit.examples {
            let key = (example.setup.clone(), example.call.clone());
            if let Some(prior) = seen.get(&key) {
                if prior.expect != example.expect {
                    issues.push(Issue {
                        rule: Rule::DuplicateSetupConflictingExpect,
                        unit: Some(unit.id.clone()),
                        message: format!(
                            "examples {} and {} both do {:?} then {:?}, but expect {:?} and {:?}",
                            prior.id, example.id, example.setup, example.call, prior.expect, example.expect
                        ),
                    });
                }
            } else {
                seen.insert(key, example);
            }
        }
    }
    issues
}

fn wall_clock_call(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        for example in &unit.examples {
            let text = format!("{} {} {:?}", example.setup, example.call, example.expect);
            if text.contains("now()") || text.contains(".now") {
                issues.push(Issue {
                    rule: Rule::WallClockCall,
                    unit: Some(unit.id.clone()),
                    message: format!(
                        "example {} reads the wall clock; every time must be a literal passed in",
                        example.id
                    ),
                });
            }
        }
    }
    issues
}

fn exceptions_without_ownership(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        if unit.owns_rules.is_empty() {
            for method in &unit.methods {
                if !method.throws.is_empty() {
                    issues.push(Issue {
                        rule: Rule::ExceptionWithoutOwnership,
                        unit: Some(unit.id.clone()),
                        message: format!(
                            "{} throws {} exception(s) but this unit owns no rule",
                            method.signature,
                            method.throws.len()
                        ),
                    });
                }
            }
        }
    }
    issues
}

fn preserved_field_contradicted(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        for method in &unit.methods {
            if method.preserves.is_empty() {
                continue;
            }
            // The signature may name the method as `Type.method(...)` or `ReturnType method(...)`;
            // either way, what an example's `call` actually contains is the bare method name.
            let before_paren = method.signature.split('(').next().unwrap_or(&method.signature);
            let method_name = before_paren
                .rsplit(|c: char| c == '.' || c == ' ')
                .next()
                .unwrap_or(before_paren);
            for example in &unit.examples {
                if !example.call.contains(method_name) {
                    continue;
                }
                for field in &method.preserves {
                    if let (Some(before), Some(after)) =
                        (example.setup_values.get(field), example.expect.get(field))
                    {
                        if before != after {
                            issues.push(Issue {
                                rule: Rule::PreservedFieldContradicted,
                                unit: Some(unit.id.clone()),
                                message: format!(
                                    "example {} calls {} which preserves {field}, but expects {field} \
                                     to change from {before:?} to {after:?}",
                                    example.id, method_name
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
    issues
}

fn undefined_identifier(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        for example in &unit.examples {
            let mut known: Vec<&str> = KNOWN_RECEIVERS.to_vec();
            known.extend(example.defines.iter().map(|s| s.as_str()));
            // `setup` is scanned too: the real failure (S6a run 5) is an identifier used inside
            // `setup` itself (`repository.save(ticket)`) that nothing ever constructs.
            let mut values = identifiers_as_values(&example.setup);
            values.extend(identifiers_as_values(&example.call));
            for token in values {
                if !known.contains(&token.as_str()) {
                    issues.push(Issue {
                        rule: Rule::UndefinedIdentifier,
                        unit: Some(unit.id.clone()),
                        message: format!(
                            "example {} uses `{token}`, but setup never defines it",
                            example.id
                        ),
                    });
                }
            }
        }
    }
    issues
}

fn rule_ownership_collisions(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for unit in &plan.units {
        for rule_id in &unit.owns_rules {
            owners.entry(rule_id.as_str()).or_default().push(&unit.id);
        }
    }
    for (rule_id, units) in owners {
        if units.len() > 1 {
            issues.push(Issue {
                rule: Rule::RuleOwnershipCollision,
                unit: None,
                message: format!("rule `{rule_id}` is claimed by more than one unit: {units:?}"),
            });
        }
    }
    issues
}

/// Runs every audit, in the order declared on `Rule`. Empty means the structured plan has none of the
/// six known contradiction shapes — not that it is correct, only that it is not self-contradicting.
pub fn verify(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    issues.extend(duplicate_setup_conflicting_expect(plan));
    issues.extend(wall_clock_call(plan));
    issues.extend(exceptions_without_ownership(plan));
    issues.extend(preserved_field_contradicted(plan));
    issues.extend(undefined_identifier(plan));
    issues.extend(rule_ownership_collisions(plan));
    issues
}

// --- Phase 1: a renderer from the structured plan down to the existing `plan.yaml` shape. No model
// call; `canopy_implementer::plan::Plan` and its contract are untouched. ---

/// Unit indices in dependency order (Kahn's algorithm over `depends_on`): a unit is only placed once
/// every unit it depends on already is. Ties keep declaration order, so the order is deterministic.
/// Steps are never model-authored — they follow from `depends_on` alone.
fn topological_order(plan: &StructuredPlan) -> Result<Vec<usize>, String> {
    let index_of: BTreeMap<&str, usize> =
        plan.units.iter().enumerate().map(|(i, u)| (u.id.as_str(), i)).collect();
    let n = plan.units.len();
    let mut indegree = vec![0usize; n];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, unit) in plan.units.iter().enumerate() {
        for dep in &unit.depends_on {
            let &j = index_of
                .get(dep.as_str())
                .ok_or_else(|| format!("unit `{}` depends on unknown unit `{dep}`", unit.id))?;
            dependents[j].push(i);
            indegree[i] += 1;
        }
    }
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::new();
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &dependent in &dependents[i] {
            indegree[dependent] -= 1;
            if indegree[dependent] == 0 {
                queue.push_back(dependent);
            }
        }
    }
    if order.len() != n {
        return Err("depends_on has a cycle".to_string());
    }
    Ok(order)
}

/// `Ticket.validate(LocalDateTime at)` or `Ticket validate(LocalDateTime at)` -> `validate(LocalDateTime at)`:
/// the bare method name and its parameters, stripped of any leading type qualifier, matching the way the
/// hand-written plans and the existing layered Planner's Throws lines name a method.
fn throws_label(signature: &str) -> String {
    let paren = signature.find('(').unwrap_or(signature.len());
    let (head, rest) = signature.split_at(paren);
    let name = head.rsplit(|c: char| c == '.' || c == ' ').next().unwrap_or(head);
    format!("{name}{rest}")
}

/// `expect` is a flat map of named facts, not a sentence (`docs/design/structured-plan-schema.md`'s
/// design principle 3). Rendered mechanically here, not as polished English — Phase 1's job is to prove
/// the renderer produces a valid, auditable plan; whether model-drafted `examples` read naturally is a
/// Phase 2 question, scored against the Implementer, not decided by how this function words it.
fn render_expect(expect: &BTreeMap<String, String>) -> String {
    if let Some(exception_type) = expect.get("throws") {
        let message = expect
            .get("message")
            .map(|m| format!(" with the message \"{m}\""))
            .unwrap_or_default();
        return format!("throws {exception_type}{message}");
    }
    expect
        .iter()
        .map(|(field, value)| format!("{field} is {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_instruction(
    plan: &StructuredPlan,
    api: &layered::Api,
    index: usize,
    unit: &Unit,
) -> String {
    let api_unit = &api.units[index];
    let api_lines: Vec<&str> = api.units[..=index].iter().map(|u| u.line.as_str()).collect();
    let mut out = format!(
        "Create the following in the package {}: {}\nRules: {}\n",
        api.package,
        api_unit.line,
        unit.rules.trim()
    );
    // `order` ranks a method's own conditions against each other ("a method checks its conditions in
    // the order listed") — it is not comparable across methods. Methods render in their declared
    // order; only within one method are its throws sorted by `order`.
    let throws: Vec<(&Method, &Throw)> = unit
        .methods
        .iter()
        .flat_map(|m| {
            let mut own: Vec<&Throw> = m.throws.iter().collect();
            own.sort_by_key(|t| t.order);
            own.into_iter().map(move |t| (m, t))
        })
        .collect();
    if !throws.is_empty() {
        out.push_str("Throws (a method checks its conditions in the order listed):\n");
        for (method, throw) in throws {
            out.push_str(&format!(
                "- {}: throws {} with the message \"{}\" when {}\n",
                throws_label(&method.signature),
                throw.exception_type.trim(),
                throw.message.trim(),
                throw.condition.trim().trim_end_matches('.')
            ));
        }
    }
    if !plan.non_goals.is_empty() {
        out.push_str(&format!("Non-goals: {}.\n", plan.non_goals.join("; ")));
    }
    if !plan.decisions.is_empty() {
        let decided: Vec<String> = plan
            .decisions
            .iter()
            .map(|d| format!("{} {}", d.question.trim().trim_end_matches('?'), d.answer.trim()))
            .collect();
        out.push_str(&format!("Decided: {}\n", decided.join(" ")));
    }
    out.push_str(&format!(
        "API the tests use: {}\nAdd {}Test. {}\n",
        api_lines.join(" "),
        api_unit.primary(),
        layered::MUST
    ));
    for (i, example) in unit.examples.iter().enumerate() {
        let setup = example.setup.trim().trim_end_matches(';');
        let lead = if setup.is_empty() { String::new() } else { format!("{setup}; ") };
        out.push_str(&format!(
            "{}. {lead}{}: {}\n",
            i + 1,
            example.call.trim().trim_end_matches('.'),
            render_expect(&example.expect)
        ));
    }
    out.trim_end().to_string()
}

/// Renders the structured plan down to the shape `canopy_implementer::plan::Plan::from_yaml` reads —
/// the only format the Implementer's orchestrator, gates and hints know about. Nothing here changes
/// that contract; this function is the sole place the two shapes meet.
pub fn to_legacy_plan(plan: &StructuredPlan) -> Result<Plan, String> {
    let api_text = plan.api.as_deref().ok_or("the structured plan has no `api` text to render from")?;
    let api = layered::parse_api(api_text).map_err(|e| format!("api: {e}"))?;
    if api.units.len() != plan.units.len() {
        return Err(format!(
            "the api declares {} unit(s) but the structured plan has {} — `units` must align 1:1, in \
             order, with the api's lines",
            api.units.len(),
            plan.units.len()
        ));
    }
    let order = topological_order(plan)?;
    let mut steps = Vec::with_capacity(order.len());
    for &i in &order {
        let unit = &plan.units[i];
        let api_unit = &api.units[i];
        let mut create = api_unit.main_files(&api.package);
        create.push(api_unit.test_file(&api.package));
        let read_only: Vec<String> = unit
            .depends_on
            .iter()
            .filter_map(|dep| plan.units.iter().position(|u| &u.id == dep))
            .flat_map(|j| api.units[j].main_files(&api.package))
            .collect();
        steps.push(PlanStep {
            id: format!("{}-{}-{}", plan.id, steps.len() + 1, unit.id),
            instruction: render_instruction(plan, &api, i, unit),
            kind: StepKind::default(),
            context: None,
            scope: Scope { modify: Vec::new(), create, read_only },
            covers: unit.covers.clone(),
        });
    }
    Ok(Plan { id: plan.id.clone(), story: None, feature: None, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only_rule(issues: &[Issue]) -> Rule {
        assert!(!issues.is_empty(), "expected at least one issue");
        let rules: Vec<Rule> = issues.iter().map(|i| i.rule).collect();
        assert!(
            rules.iter().all(|r| *r == rules[0]),
            "more than one rule fired: {issues:?}"
        );
        rules[0]
    }

    // --- DuplicateSetupConflictingExpect: S6a run 3 (tickets, step 2) ---------------------------------

    const RUN3_CONTRADICTING: &str = r#"
id: tickets
units:
  - id: ticket-repository
    examples:
      - id: item-1
        setup: 'repository.save(new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED))'
        call: 'repository.findById("T-1")'
        expect: {empty: "true"}
      - id: item-2
        setup: 'repository.save(new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED))'
        call: 'repository.findById("T-1")'
        expect: {present: "true", status: ISSUED}
"#;

    #[test]
    fn flags_the_run3_contradiction() {
        let plan = StructuredPlan::from_yaml(RUN3_CONTRADICTING).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::DuplicateSetupConflictingExpect);
    }

    const RUN1_CLEAN_REPOSITORY: &str = r#"
id: tickets
units:
  - id: ticket-repository
    examples:
      - id: find-missing-is-empty
        setup: 'new InMemoryTicketRepository()'
        call: 'repository.findById("T-9")'
        expect: {empty: "true"}
      - id: save-then-find
        setup: 'repository.save(new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED))'
        call: 'repository.findById("T-1")'
        expect: {present: "true", status: ISSUED}
"#;

    #[test]
    fn does_not_flag_the_clean_run1_repository_step() {
        let plan = StructuredPlan::from_yaml(RUN1_CLEAN_REPOSITORY).unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    // --- WallClockCall: S6a run 4 (tickets, step 2, two now() calls expected to agree) ----------------

    const RUN4_NOW: &str = r#"
id: tickets
units:
  - id: ticket-repository
    examples:
      - id: two-now-calls-agree
        setup: 'repository.save(new Ticket("T-1", LocalDateTime.now(), TicketStatus.ISSUED))'
        call: 'repository.findById("T-1")'
        expect: {issuedAt: 'LocalDateTime.now()'}
"#;

    #[test]
    fn flags_the_run4_wall_clock_call() {
        let plan = StructuredPlan::from_yaml(RUN4_NOW).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::WallClockCall);
    }

    #[test]
    fn does_not_flag_a_literal_time() {
        let plan = StructuredPlan::from_yaml(RUN1_CLEAN_REPOSITORY).unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    // --- ExceptionWithoutOwnership: S6c (loans, the Loan unit has none, step 1 leaks step 3's) --------

    const S6C_LOAN_LEAK: &str = r#"
id: loans
units:
  - id: loan
    owns_rules: []
    methods:
      - signature: 'Loan.borrow(String memberId, String isbn, LocalDate borrowedOn)'
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "Loan limit reached", condition: "x"}
          - {order: 2, exception_type: IllegalStateException, message: "Book is already on loan", condition: "y"}
"#;

    #[test]
    fn flags_the_s6c_loan_leak() {
        let plan = StructuredPlan::from_yaml(S6C_LOAN_LEAK).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::ExceptionWithoutOwnership);
    }

    const LOAN_SERVICE_OWNS_ITS_OWN_THROWS: &str = r#"
id: loans
units:
  - id: loan
    owns_rules: [late-fee-calculation]
    methods:
      - signature: 'Loan.lateFee(LocalDate returnedOn)'
  - id: loan-service
    owns_rules: [borrow-limit, borrow-conflict]
    methods:
      - signature: 'LoanService.borrow(String memberId, String isbn, LocalDate borrowedOn)'
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "Loan limit reached", condition: "x"}
"#;

    #[test]
    fn does_not_flag_a_throw_on_a_unit_that_owns_a_rule() {
        let plan = StructuredPlan::from_yaml(LOAN_SERVICE_OWNS_ITS_OWN_THROWS).unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    // --- PreservedFieldContradicted: S6a run 8 (tickets, validate overwrites issuedAt) ----------------

    const RUN8_ISSUED_AT_OVERWRITTEN: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window]
    methods:
      - signature: 'Ticket.validate(LocalDateTime at)'
        preserves: [issuedAt]
    examples:
      - id: validate-changes-issued-at
        setup: 'Ticket ticket = new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED)'
        setup_values: {issuedAt: '2026-03-01T10:00'}
        defines: [ticket]
        call: 'ticket.validate(LocalDateTime.of(2026, 3, 1, 10, 30))'
        expect: {status: VALIDATED, issuedAt: '2026-03-01T10:30'}
"#;

    #[test]
    fn flags_the_run8_preserved_field_contradiction() {
        let plan = StructuredPlan::from_yaml(RUN8_ISSUED_AT_OVERWRITTEN).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::PreservedFieldContradicted);
    }

    const VALIDATE_PRESERVES_ISSUED_AT: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window]
    methods:
      - signature: 'Ticket.validate(LocalDateTime at)'
        preserves: [issuedAt]
    examples:
      - id: validate-keeps-issued-at
        setup: 'Ticket ticket = new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED)'
        setup_values: {issuedAt: '2026-03-01T10:00'}
        defines: [ticket]
        call: 'ticket.validate(LocalDateTime.of(2026, 3, 1, 10, 30))'
        expect: {status: VALIDATED, issuedAt: '2026-03-01T10:00'}
"#;

    #[test]
    fn does_not_flag_a_preserved_field_that_matches() {
        let plan = StructuredPlan::from_yaml(VALIDATE_PRESERVES_ISSUED_AT).unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    // --- UndefinedIdentifier: S6a run 5 (tickets, step 2, `ticket` never defined) ---------------------

    const RUN5_UNDEFINED_TICKET: &str = r#"
id: tickets
units:
  - id: ticket-repository
    examples:
      - id: save-undefined-ticket
        setup: 'repository.save(ticket)'
        call: 'repository.findById("T-1")'
        expect: {empty: "true"}
"#;

    #[test]
    fn flags_the_run5_undefined_identifier() {
        let plan = StructuredPlan::from_yaml(RUN5_UNDEFINED_TICKET).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::UndefinedIdentifier);
    }

    const TICKET_DEFINED_BEFORE_USE: &str = r#"
id: tickets
units:
  - id: ticket-repository
    examples:
      - id: save-a-defined-ticket
        setup: 'Ticket ticket = new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED)'
        defines: [ticket]
        call: 'repository.save(ticket)'
        expect: {}
"#;

    #[test]
    fn does_not_flag_an_identifier_setup_defines() {
        let plan = StructuredPlan::from_yaml(TICKET_DEFINED_BEFORE_USE).unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    // --- RuleOwnershipCollision: the tickets service plan duplicating Ticket's own rule ---------------

    const SERVICE_DUPLICATES_TICKET_RULE: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window, validation-precedence, cancellation-rule]
  - id: ticket-service
    owns_rules: [unknown-ticket-rejection, validation-window]
"#;

    #[test]
    fn flags_the_duplicated_validation_window_rule() {
        let plan = StructuredPlan::from_yaml(SERVICE_DUPLICATES_TICKET_RULE).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::RuleOwnershipCollision);
    }

    const SERVICE_DELEGATES_INSTEAD_OF_OWNING: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window, validation-precedence, cancellation-rule]
  - id: ticket-service
    owns_rules: [unknown-ticket-rejection]
"#;

    #[test]
    fn does_not_flag_disjoint_ownership() {
        let plan = StructuredPlan::from_yaml(SERVICE_DELEGATES_INSTEAD_OF_OWNING).unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    // --- Phase 1: the golden tickets fixture, exactly as docs/design/structured-plan-schema.md's -----
    // --- example, hand-authored from the same facts as the existing 10/10 hand plan. -----------------

    const TICKETS_STRUCTURED: &str =
        include_str!("../../canopy-implementer/tasks/tickets/plan.structured.yaml");

    #[test]
    fn the_tickets_fixture_has_no_contradictions() {
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        assert_eq!(verify(&plan), vec![], "the golden fixture must not trip its own audits");
    }

    #[test]
    fn renders_down_to_a_plan_yaml_accepted_by_from_yaml() {
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        let legacy = to_legacy_plan(&plan).unwrap();
        // The real contract boundary: canopy-implementer's own parser, not just a type that happens to
        // share a name, must accept exactly what the renderer produced.
        let reparsed = Plan::from_yaml(&legacy.to_yaml()).unwrap();
        assert_eq!(reparsed.steps.len(), 3);
        assert_eq!(reparsed.steps[0].id, "tickets-1-ticket");
        assert_eq!(reparsed.steps[1].id, "tickets-2-ticket-repository");
        assert_eq!(reparsed.steps[2].id, "tickets-3-ticket-service");
        // depends_on put the repository after the model, and the service after both, by construction
        // of the topological sort, not because the fixture happened to list them in that order.
        assert_eq!(
            reparsed.steps[1].scope.read_only,
            vec![
                "src/main/java/no/experis/tickets/TicketStatus.java",
                "src/main/java/no/experis/tickets/Ticket.java",
            ]
        );
        assert_eq!(
            reparsed.steps[2].scope.read_only,
            vec![
                "src/main/java/no/experis/tickets/TicketStatus.java",
                "src/main/java/no/experis/tickets/Ticket.java",
                "src/main/java/no/experis/tickets/TicketRepository.java",
                "src/main/java/no/experis/tickets/InMemoryTicketRepository.java",
            ]
        );
    }

    /// The S6c regression this renderer exists to make structurally impossible: a unit that throws
    /// nothing must never have a Throws block, regardless of what another unit in the same plan throws.
    #[test]
    fn the_repository_step_has_no_throws_block() {
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        let legacy = to_legacy_plan(&plan).unwrap();
        let repository_step = &legacy.steps[1];
        assert!(
            !repository_step.instruction.contains("Throws"),
            "the repository unit throws nothing, but its instruction reads:\n{}",
            repository_step.instruction
        );
    }

    #[test]
    fn the_ticket_step_has_its_four_throws_rendered_in_order_grouped_by_method() {
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        let legacy = to_legacy_plan(&plan).unwrap();
        let text = &legacy.steps[0].instruction;
        let block_start = text.find("Throws (").expect("a Throws block");
        let block_end = text.find("Non-goals:").expect("the block ends before Non-goals");
        let block = &text[block_start..block_end];
        // `order` ranks a method's own conditions, not the unit's: `validate`'s three throws (its own
        // declared order) must stay adjacent and in order, and `cancel`'s one throw must not be
        // interleaved between them just because its `order: 1` collides numerically with validate's.
        assert_eq!(
            block.trim(),
            "Throws (a method checks its conditions in the order listed):\n\
             - validate(LocalDateTime at): throws IllegalStateException with the message \"Ticket is cancelled\" when the ticket is cancelled\n\
             - validate(LocalDateTime at): throws IllegalStateException with the message \"Ticket is already validated\" when the ticket is already validated\n\
             - validate(LocalDateTime at): throws IllegalStateException with the message \"Ticket has expired\" when at is more than 90 minutes after issuedAt\n\
             - cancel(): throws IllegalStateException with the message \"A validated ticket cannot be cancelled\" when the ticket is validated",
        );
    }
}
