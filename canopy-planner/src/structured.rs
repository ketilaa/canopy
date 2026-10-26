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
use serde::{Deserialize, Serialize};

use crate::layered;

/// One exception a method throws, checked in `order`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Throw {
    pub order: u32,
    pub exception_type: String,
    pub message: String,
    pub condition: String,
}

/// One method of a unit's API.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// An example's `setup`, `call` or `expect`, or a method's own `throws[].condition`, reads the
    /// wall clock (`now()` or `LocalDateTime.now`) instead of a literal or the method's own parameter.
    /// S6a run 4 (an example); a drafted plan, 2026-10-21 (a throws condition).
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
    /// An example's `setup_values` names (as a whole word, case-insensitively) a state that method's
    /// own `throws[].condition` also names, the example calls that method, and `expect` does not state
    /// `throws` — the example asks for a normal return from exactly the state the unit's own answer
    /// says must throw. Phase 2's real A1 run (2026-10-25): 10 of 10 drafted plans, dominantly
    /// `cancel()` on an already-`VALIDATED` ticket expecting `CANCELLED` instead of the stated exception.
    ExampleContradictsThrows,
    /// A unit's own `throws` entry has the same `(exception_type, message)` as an entry already drafted
    /// for a unit it `depends_on` — the dependency's own exception, re-declared rather than left to
    /// propagate. `draft_messages` shows a dependent exactly what its dependency already throws and
    /// `draft_system()` says never to redeclare it; the A1 re-run (2026-10-26) found the model copying
    /// the dependency's exceptions verbatim anyway in 4 of 5 fresh drafts, with no audit catching it —
    /// this is that audit.
    ExceptionBorrowedFromDependency,
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
        // A throws condition is exactly as load-bearing as an example and the same mistake is
        // possible there: a drafted tickets plan (2026-10-21) wrote the expiry check as
        // `LocalDateTime.now().isAfter(issuedAt.plusMinutes(90))` instead of comparing against the
        // method's own `at` parameter — reading the wall clock makes the condition non-deterministic
        // against a fixed-time hidden test, and no example-level check caught it since the text lives
        // on the method, not on an example.
        for method in &unit.methods {
            for throw in &method.throws {
                if throw.condition.contains("now()") || throw.condition.contains(".now") {
                    issues.push(Issue {
                        rule: Rule::WallClockCall,
                        unit: Some(unit.id.clone()),
                        message: format!(
                            "{}'s condition \"{}\" reads the wall clock; every time must be the method's own parameter",
                            throws_label(&method.signature),
                            throw.condition
                        ),
                    });
                }
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

/// True when `needle` (already lowercase) occurs in `haystack` (already lowercase) with a word
/// boundary on both sides — `"validated"` must match inside `"the ticket is already validated"` but
/// `"issued"` must not match inside `"issuedAt"`. A plain scan, not a regex, matching this file's other
/// string heuristics (`wall_clock_call`, `declared_in_setup`).
fn word_in(haystack: &str, needle: &str) -> bool {
    let is_word = |c: u8| (c as char).is_alphanumeric() || c == b'_';
    let (hb, nb) = (haystack.as_bytes(), needle.as_bytes());
    if nb.is_empty() || nb.len() > hb.len() {
        return false;
    }
    (0..=(hb.len() - nb.len())).any(|start| {
        hb[start..start + nb.len()] == *nb
            && (start == 0 || !is_word(hb[start - 1]))
            && (start + nb.len() == hb.len() || !is_word(hb[start + nb.len()]))
    })
}

/// An example's `setup_values` describe the receiver's state before `call`; a method's own `throws`
/// list describes, in prose, the preconditions under which it must throw instead of returning
/// normally. When one of an example's `setup_values` names (as a whole word) a state that one of the
/// called method's own `throws[].condition` sentences also names, and that example's `expect` does not
/// itself state `throws`, the example asks for a normal return from exactly the state the same unit's
/// own `throws` answer says must throw instead — a contradiction between the model's `examples` answer
/// and its own `throws` answer for the same unit, not a hypothetical: Phase 2's real A1 run
/// (`canopy-implementer/docs/prompt-and-plan-changelog.md`, 2026-10-25) found this in 10 of 10 drafted
/// plans, dominantly an example with `setup_values: {status: VALIDATED}` calling `cancel()` and
/// expecting `status: CANCELLED`, while the same unit's own `cancel()` throws entry reads "when the
/// ticket is already validated".
fn example_contradicts_own_throws(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        for method in &unit.methods {
            if method.throws.is_empty() {
                continue;
            }
            let before_paren = method.signature.split('(').next().unwrap_or(&method.signature);
            let method_name = before_paren
                .rsplit(|c: char| c == '.' || c == ' ')
                .next()
                .unwrap_or(before_paren);
            for example in &unit.examples {
                if !example.call.contains(method_name) || example.expect.contains_key("throws") {
                    continue;
                }
                for throw in &method.throws {
                    let condition_lower = throw.condition.to_lowercase();
                    for value in example.setup_values.values() {
                        // Shorter than 4 chars risks matching a generic token (an id, a flag) rather
                        // than a state name the condition prose actually names.
                        if value.len() >= 4 && word_in(&condition_lower, &value.to_lowercase()) {
                            issues.push(Issue {
                                rule: Rule::ExampleContradictsThrows,
                                unit: Some(unit.id.clone()),
                                message: format!(
                                    "example {} calls {method_name} from a state matching its own \
                                     throws condition (\"{}\"), but expects a normal return instead of \
                                     {} \"{}\"",
                                    example.id, throw.condition, throw.exception_type, throw.message
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

/// A unit's own `throws` entry that exactly repeats a `(exception_type, message)` pair already present
/// on a unit it `depends_on` — the dependency's own exception, re-declared instead of left to propagate.
/// `draft_messages` shows a dependent exactly what its dependency already throws, worded as a fact to
/// exclude, and `draft_system()` says never to redeclare it; checked directly against a real run
/// (2026-10-26), the model copied the dependency's exceptions verbatim anyway in 4 of 5 fresh drafts of
/// `ticket-service`, correctly typed and worded (the fact injection already fixed the wrong-type mistake
/// `ExampleContradictsThrows` targeted) but still not this unit's to throw, and dropped its own
/// legitimate `Unknown ticket` exception to make room for them. An exact-pair match, not a text-contains
/// heuristic: the fact is already shown to the model as this literal pair, so a model that ignores the
/// instruction copies it byte-for-byte, and matching anything looser would risk flagging two unrelated
/// units that simply happen to use the same exception type.
fn exception_borrowed_from_dependency(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        let dependency_throws: Vec<(&str, (&str, &str))> = unit
            .depends_on
            .iter()
            .filter_map(|dep_id| plan.units.iter().find(|u| &u.id == dep_id))
            .flat_map(|dep| {
                dep.methods
                    .iter()
                    .flat_map(|m| m.throws.iter())
                    .map(move |t| (dep.id.as_str(), (t.exception_type.as_str(), t.message.as_str())))
            })
            .collect();
        if dependency_throws.is_empty() {
            continue;
        }
        for method in &unit.methods {
            for throw in &method.throws {
                let pair = (throw.exception_type.as_str(), throw.message.as_str());
                if let Some((dep_id, _)) = dependency_throws.iter().find(|(_, p)| *p == pair) {
                    issues.push(Issue {
                        rule: Rule::ExceptionBorrowedFromDependency,
                        unit: Some(unit.id.clone()),
                        message: format!(
                            "{} throws {} \"{}\", but that is already `{dep_id}`'s own exception — call \
                             `{dep_id}`'s method and let it propagate instead of redeclaring it here",
                            throws_label(&method.signature),
                            throw.exception_type,
                            throw.message
                        ),
                    });
                }
            }
        }
    }
    issues
}

/// Identifiers a Java-style local declaration in `setup` introduces (`Ticket ticket = new Ticket(...)`
/// or `Ticket ticket = new Ticket(...).cancel()`): a type-looking word (starts uppercase) immediately
/// followed by a value-looking word (starts lowercase) immediately followed by `=` (not `==`).
///
/// Added after a live drafting run (2026-10-21): the model consistently wrote this declaration in
/// `setup` but did not also restate the name in the separate `defines` field — a fact the declaration
/// already states, which the model should not have to repeat. `defines` stays as a supplementary,
/// explicit override for a `setup` that introduces a name without this declaration shape (uncommon;
/// every case in this file's own fixtures so far fits the declaration pattern).
fn declared_in_setup(setup: &str) -> Vec<String> {
    let mut words: Vec<(&str, usize)> = Vec::new(); // (word, index right after the word)
    let bytes = setup.as_bytes();
    let is_ident = |c: u8| (c as char).is_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < bytes.len() {
        if is_ident(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
            words.push((&setup[start..i], i));
        } else {
            i += 1;
        }
    }
    let mut declared = Vec::new();
    for pair in words.windows(2) {
        let (type_word, _) = pair[0];
        let (name_word, name_end) = pair[1];
        if type_word.starts_with(|c: char| c.is_ascii_uppercase())
            && name_word.starts_with(|c: char| c.is_ascii_lowercase())
        {
            let after = setup[name_end..].trim_start();
            if after.starts_with('=') && !after.starts_with("==") {
                declared.push(name_word.to_string());
            }
        }
    }
    declared
}

fn undefined_identifier(plan: &StructuredPlan) -> Vec<Issue> {
    let mut issues = Vec::new();
    for unit in &plan.units {
        for example in &unit.examples {
            let mut known: Vec<&str> = KNOWN_RECEIVERS.to_vec();
            known.extend(example.defines.iter().map(|s| s.as_str()));
            let declared = declared_in_setup(&example.setup);
            known.extend(declared.iter().map(|s| s.as_str()));
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
    issues.extend(example_contradicts_own_throws(plan));
    issues.extend(exception_borrowed_from_dependency(plan));
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
    // X9/X10 (`canopy-implementer/docs/prompt-and-plan-changelog.md`, 2026-10-25): a unit built as a
    // service over an injected repository (the `Service(Repository)` shape) is exactly what tempts the
    // model to reach for a mocking framework when writing that unit's test — stock and tickets went
    // from 3/10 and 0/10 to 10/10 each on this one sentence, with the behavioural content unchanged. A
    // unit that only reads one other unit's type to compile against (`ticket-repository` depends on
    // `ticket` to know the entity it stores, but constructs no collaborator of its own) never had this
    // failure mode in either arm — depending on two or more units is the structural signal that this
    // unit actually holds a constructed collaborator, not just a type it reads.
    if unit.depends_on.len() >= 2 {
        out.push_str(
            "No mocking framework (Mockito or any other) is on this project's classpath: write every \
             test against a real instance of each dependency, never a mock.\n",
        );
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

// --- Phase 2 arm A1 (`docs/design/structured-plan-schema-introduction-plan.md`): the model drafts each
// unit's `examples` and `methods[].throws` only. Everything else — signatures, `preserves`,
// `owns_rules`, `depends_on`, `covers`, `rules` prose, `api`, `decisions`, `non_goals` — is held fixed,
// taken from a plan whose `examples`/`throws` have been cleared (`skeleton`). This isolates "can the
// model draft contradiction-free content" from "can the model draft the facts a human must supply,"
// which stays answered by `decisions`. No audit here rewrites anything the model wrote; a hit is
// retried with the audit's own issues as feedback, then recovered by dropping only the examples an
// issue actually names — the same shape `layered::plan_layered` already uses for its cases, applied to
// Phase 0's audits instead of hand-written ones.

use canopy_implementer::llm::{Generator, Message, StructuredRequest};
use serde_json::{json, Value};

use crate::gherkin::Feature;

const DRAFT_MAX_ATTEMPTS: u32 = 2;

/// `plan` with every unit's `examples` and every method's `throws` cleared — what `draft_structured`
/// fills back in, one unit at a time.
pub fn skeleton(plan: &StructuredPlan) -> StructuredPlan {
    let mut out = plan.clone();
    for unit in &mut out.units {
        unit.examples.clear();
        for method in &mut unit.methods {
            method.throws.clear();
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct ThrowAnswer {
    order: u32,
    exception_type: String,
    message: String,
    condition: String,
}

#[derive(Debug, Default, Deserialize)]
struct MethodAnswer {
    #[serde(default)]
    throws: Vec<ThrowAnswer>,
}

#[derive(Debug, Deserialize)]
struct ExampleAnswer {
    id: String,
    #[serde(default)]
    setup: String,
    #[serde(default)]
    setup_values: BTreeMap<String, String>,
    #[serde(default)]
    defines: Vec<String>,
    call: String,
    #[serde(default)]
    expect: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct UnitAnswer {
    methods: Vec<MethodAnswer>,
    examples: Vec<ExampleAnswer>,
}

fn draft_schema(method_count: usize) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["methods", "examples"],
        "properties": {
            "methods": {
                "type": "array",
                "minItems": method_count,
                "maxItems": method_count,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["throws"],
                    "properties": {
                        "throws": {
                            "type": "array",
                            "maxItems": 6,
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["order", "exception_type", "message", "condition"],
                                "properties": {
                                    "order": {"type": "integer", "minimum": 1, "maximum": 10},
                                    "exception_type": {"type": "string", "pattern": "^[A-Za-z]+Exception$"},
                                    "message": {"type": "string", "minLength": 1, "maxLength": 120},
                                    "condition": {"type": "string", "minLength": 1, "maxLength": 200}
                                }
                            }
                        }
                    }
                }
            },
            "examples": {
                "type": "array",
                "minItems": 1,
                "maxItems": 8,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "setup", "call", "expect"],
                    "properties": {
                        "id": {"type": "string", "minLength": 1, "maxLength": 60},
                        "setup": {"type": "string", "maxLength": 240},
                        "setup_values": {"type": "object", "additionalProperties": {"type": "string", "maxLength": 60}},
                        "defines": {"type": "array", "maxItems": 4, "items": {"type": "string", "maxLength": 40}},
                        "call": {"type": "string", "minLength": 1, "maxLength": 200},
                        "expect": {
                            "type": "object",
                            "minProperties": 1,
                            "additionalProperties": {"type": "string", "maxLength": 80}
                        }
                    }
                }
            }
        }
    })
}

fn draft_system() -> String {
    "\
You write the examples and exceptions of one unit of a Java plan, from its fixed API line, its fixed rules, and \
facts a human has already decided.
ALWAYS write every example as setup, call and expect, with literal values only: never read the system clock (no \
now()), and never use an identifier the setup does not introduce (when setup builds one, list it in `defines`).
ALWAYS give every example its own distinct `id` that names the example itself (\"valid-at-boundary\", not the \
domain value it happens to use, such as a ticket id repeated across several examples).
NEVER write two examples whose setup and call are the same but whose expect differs — that is a contradiction \
between two of your own answers, not two tests.
ALWAYS state, for a method the rules say preserves a field, the same value for that field in `setup_values` and \
in `expect` on every example that calls it.
ALWAYS list every exception of a method in `throws`, one entry per condition, in the order the method checks \
them, with the exact message the rules or the decisions give; a method the rules do not say throws anything \
gets an empty list, never a borrowed exception from elsewhere in the plan.
ALWAYS write `condition` as a plain clause that reads naturally after \"when\" (\"the ticket is cancelled\", \
\"at is more than 90 minutes after issuedAt\"), comparing only the method's own parameters and the receiver's \
fields — never as Java code (`status == CANCELLED`), never wrapped in `if (...)`, and never by reading the \
system clock.
NEVER write an example whose `setup_values` puts the receiver in a state one of this same method's own \
`throws` conditions names, unless that example's `expect` also states `throws` — a normal `expect` from a \
state your own `throws` answer says must throw is a contradiction between your two answers, not two tests.
NEVER give a method a `throws` entry for a condition already listed under \"Already thrown by a dependency\", \
even with a different exception type or message — that condition belongs to the dependency; call its own \
method and let its exception propagate unchanged, instead of catching it or re-checking the same condition \
yourself.
ALWAYS still answer `throws` with every exception this method's own API line or rules give it — the \
\"Already thrown by a dependency\" list only says which entries to leave out; it is not the complete list for \
this method, and removing an entry that matches it must never become removing every entry. A method whose \
API line states it throws on some condition unrelated to the dependency (for instance, an id the dependency \
cannot find) keeps that entry even when the dependency throws other things entirely.
NEVER invent a rule, a value or an exception that the rules, the decisions or the scenarios do not state."
        .to_string()
}

fn draft_messages(
    plan: &StructuredPlan,
    api: &layered::Api,
    feature: &Feature,
    index: usize,
    previous: Option<(&str, &[String])>,
) -> Vec<Message> {
    let unit = &plan.units[index];
    let api_unit = &api.units[index];
    let mut user = format!("## Unit\n{}\nRules: {}\n", api_unit.line, unit.rules.trim());
    if !unit.methods.is_empty() {
        user.push_str("\nMethods (answer `throws` for each, in this order; [] when it throws nothing):\n");
        for method in &unit.methods {
            let preserves = if method.preserves.is_empty() {
                String::new()
            } else {
                format!(" (preserves {})", method.preserves.join(", "))
            };
            user.push_str(&format!("- {}{}\n", method.signature, preserves));
        }
    }
    // The dependency's own throws, already drafted earlier in this same pass (`draft_structured`
    // visits units in `depends_on` order, writing each one back into `plan` before the next is
    // drafted): the gap this closes is not a missing instruction (`draft_system()` already says never
    // to borrow an exception from elsewhere, and this unit's own `rules` text, fixed from the golden
    // fixture, already says the dependency owns these rules) — it is that the model was never actually
    // shown what the dependency throws, only told in prose not to duplicate it. A1's real run
    // (2026-10-26, `canopy-implementer/docs/prompt-and-plan-changelog.md`) found the model routinely
    // re-declaring `ticket`'s own validation/cancellation exceptions on `ticket-service`, almost always
    // with the wrong exception type, because nothing in its prompt ever stated what those were.
    let dep_throws: String = unit
        .depends_on
        .iter()
        .filter_map(|dep_id| plan.units.iter().find(|u| &u.id == dep_id))
        .flat_map(|dep| dep.methods.iter())
        .flat_map(|m| m.throws.iter().map(move |t| (m, t)))
        .map(|(method, throw)| {
            format!(
                "- {}: throws {} with the message \"{}\" when {}\n",
                throws_label(&method.signature),
                throw.exception_type.trim(),
                throw.message.trim(),
                throw.condition.trim().trim_end_matches('.')
            )
        })
        .collect();
    if !dep_throws.is_empty() {
        user.push_str(
            "\n## Already thrown by a dependency (your own `throws` must have no entry for any of \
             these conditions — call the dependency's own method and let its exception propagate; this \
             list only says what to leave out, it is not the complete list for your own method, so keep \
             every exception your own API line or rules still give it)\n",
        );
        user.push_str(&dep_throws);
    }
    if !plan.decisions.is_empty() {
        user.push_str("\n## Decided\n");
        for decision in &plan.decisions {
            user.push_str(&format!("- {} {}\n", decision.question.trim(), decision.answer.trim()));
        }
    }
    if !plan.non_goals.is_empty() {
        user.push_str(&format!("\n## Non-goals\n{}\n", plan.non_goals.join("\n")));
    }
    if !unit.covers.is_empty() {
        user.push_str("\n## Scenarios this unit covers\n");
        for id in &unit.covers {
            if let Some(text) = feature.render_scenario(id) {
                user.push_str(&format!("### {id}\n{text}\n"));
            }
        }
    }
    if let Some((raw, problems)) = previous {
        user.push_str(&format!("\n## Your previous answer\n```json\n{raw}\n```\n\n## Problems\n"));
        for problem in problems {
            user.push_str(&format!("- {problem}\n"));
        }
        user.push_str("\nReturn the complete corrected answer.\n");
    }
    vec![Message::system(&draft_system()), Message::user(&user)]
}

/// Issues `verify` finds that name this unit (cross-unit rules like `RuleOwnershipCollision` cannot be
/// newly caused here, since `owns_rules` is never model-drafted).
fn issues_for_unit(plan: &StructuredPlan, unit_id: &str) -> Vec<Issue> {
    verify(plan).into_iter().filter(|i| i.unit.as_deref() == Some(unit_id)).collect()
}

/// Which example ids a set of issues names, by matching each candidate example's own id anywhere in
/// the issue's message text — used only for recovery-by-subtraction, mirroring
/// `layered::plan_layered`'s "drop flawed cases" rather than trusting the model's own retry.
///
/// A plain substring match, not a word-boundary one: `duplicate_setup_conflicting_expect`'s message
/// names its *second* id straight after "and" (`"examples e1 and e2 both do..."`), not after the word
/// "example", so matching only `"example <id>"`/`"examples <id>"` missed it in exactly this case — the
/// bug the test below was written to catch. The remaining risk is an id that is itself a substring of
/// another id (e.g. `find-missing` inside `find-missing-is-empty`), which would over-match; acceptable
/// for Phase 2's scope, not yet a problem any real fixture has produced.
fn examples_named_in(issues: &[Issue], examples: &[Example]) -> std::collections::BTreeSet<String> {
    let mut named = std::collections::BTreeSet::new();
    for issue in issues {
        for example in examples {
            if issue.message.contains(example.id.as_str()) {
                named.insert(example.id.clone());
            }
        }
    }
    named
}

fn apply_answer(unit: &Unit, answer: UnitAnswer) -> Unit {
    let mut out = unit.clone();
    for (method, answered) in out.methods.iter_mut().zip(answer.methods) {
        method.throws = answered
            .throws
            .into_iter()
            .map(|t| Throw {
                order: t.order,
                exception_type: t.exception_type,
                message: t.message,
                condition: t.condition,
            })
            .collect();
    }
    out.examples = answer
        .examples
        .into_iter()
        .map(|e| Example {
            id: e.id,
            setup: e.setup,
            setup_values: e.setup_values,
            defines: e.defines,
            call: e.call,
            expect: e.expect,
        })
        .collect();
    out
}

/// Drafts one unit's `examples` and `throws`, retrying with the audits' own issues as feedback, then
/// recovering by dropping only the examples an issue names, when some still remain usable.
fn draft_unit(
    generator: &dyn Generator,
    plan: &StructuredPlan,
    api: &layered::Api,
    feature: &Feature,
    index: usize,
) -> Result<Unit, String> {
    let unit = &plan.units[index];
    let schema = draft_schema(unit.methods.len());
    let mut previous: Option<(String, Vec<String>)> = None;
    let mut flawed: Option<(Unit, Vec<Issue>)> = None;
    for attempt in 1..=DRAFT_MAX_ATTEMPTS {
        let request = StructuredRequest {
            purpose: format!("structured-{}-{attempt}", index + 1),
            messages: draft_messages(plan, api, feature, index, previous.as_ref().map(|(r, p)| (r.as_str(), p.as_slice()))),
            schema_name: "unit".into(),
            schema: schema.clone(),
            thinking: false,
        };
        let response = match generator.complete_structured(&request) {
            Err(e) => {
                previous = Some((String::new(), vec![format!("the model call failed: {e}")]));
                continue;
            }
            Ok(r) if r.finish_reason == "length" => {
                previous =
                    Some((r.content, vec!["The answer hit the token limit; write fewer or shorter examples.".to_string()]));
                continue;
            }
            Ok(r) => r,
        };
        let answer: UnitAnswer = match serde_json::from_str(&response.content) {
            Ok(a) => a,
            Err(e) => {
                previous = Some((response.content, vec![format!("the answer is not valid: {e}")]));
                continue;
            }
        };
        if answer.methods.len() != unit.methods.len() {
            previous = Some((
                response.content,
                vec![format!(
                    "methods must have exactly {} entries, one per listed method, in order; got {}",
                    unit.methods.len(),
                    answer.methods.len()
                )],
            ));
            continue;
        }
        // The schema declares `minItems: 1` on `examples`, but a live run against the real server
        // showed that constraint is not actually enforced (the model answered `"examples": []` on a
        // retry — likely dropping every example that the previous attempt's feedback flagged, rather
        // than fixing them — and got it back unchallenged). A schema fixes shape, not truth: this is
        // the deterministic check that was missing, found by reading a real run before trusting it.
        if answer.examples.is_empty() {
            previous = Some((
                response.content,
                vec!["examples must not be empty: give at least one setup/call/expect example".to_string()],
            ));
            continue;
        }
        // A live run (2026-10-21) gave every example of a unit the same literal `id` ("T-1", the
        // ticket's own domain id, instead of a distinct name for the example itself). Beyond being
        // confusing on its own, it broke recovery: `examples_named_in` matches by id, so one issue
        // naming that id could no longer tell "all of them" apart from "one of them" — fixed above
        // too, but the real fix is not letting this answer through in the first place.
        let mut ids: Vec<&str> = answer.examples.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        let unique = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
        if unique < ids.len() {
            previous = Some((
                response.content,
                vec!["every example needs its own distinct id (name the example, not the domain value it uses)".to_string()],
            ));
            continue;
        }
        let candidate = apply_answer(unit, answer);
        let mut candidate_plan = plan.clone();
        candidate_plan.units[index] = candidate.clone();
        let issues = issues_for_unit(&candidate_plan, &candidate.id);
        if issues.is_empty() {
            return Ok(candidate);
        }
        previous = Some((response.content, issues.iter().map(|i| i.message.clone()).collect()));
        flawed = Some((candidate, issues));
    }
    if let Some((mut candidate, issues)) = flawed {
        let named = examples_named_in(&issues, &candidate.examples);
        // `named` is deduplicated (a model that reused one `id` across several examples — a live run,
        // 2026-10-21, gave every example of a unit the literal id "T-1" — collapses to one string in
        // the set even though it names every example). Comparing `named.len()` against the raw example
        // count let that run strip every example and still pass the "some remain" guard. Comparing
        // against the actual count removed is correct regardless of duplicate ids.
        if !named.is_empty() {
            let before = candidate.examples.len();
            candidate.examples.retain(|e| !named.contains(&e.id));
            if candidate.examples.len() < before && !candidate.examples.is_empty() {
                let mut candidate_plan = plan.clone();
                candidate_plan.units[index] = candidate.clone();
                if issues_for_unit(&candidate_plan, &candidate.id).is_empty() {
                    return Ok(candidate);
                }
            }
        }
    }
    Err(format!("unit `{}` never got a usable answer", unit.id))
}

/// Drafts every unit's `examples` and `throws` in turn, starting from `skeleton(plan)`. `Err` names the
/// first unit that never got a usable answer; there is no partial plan worth returning, since Phase 1's
/// renderer needs every unit complete.
pub fn draft_structured(
    generator: &dyn Generator,
    plan: &StructuredPlan,
    feature: &Feature,
) -> Result<StructuredPlan, String> {
    let api_text = plan.api.as_deref().ok_or("the structured plan has no `api` text to draft against")?;
    let api = layered::parse_api(api_text).map_err(|e| format!("api: {e}"))?;
    if api.units.len() != plan.units.len() {
        return Err(format!(
            "the api declares {} unit(s) but the structured plan has {}",
            api.units.len(),
            plan.units.len()
        ));
    }
    let mut out = skeleton(plan);
    for index in 0..out.units.len() {
        let drafted = draft_unit(generator, &out, &api, feature, index)?;
        out.units[index] = drafted;
    }
    Ok(out)
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

    // --- WallClockCall on a throws condition: a drafted tickets plan, 2026-10-21 ----------------------

    const DRAFTED_2026_10_21_WALL_CLOCK_CONDITION: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window]
    methods:
      - signature: "Ticket.validate(LocalDateTime at)"
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "Ticket has expired", condition: "if (LocalDateTime.now().isAfter(issuedAt.plusMinutes(90)))"}
"#;

    #[test]
    fn flags_a_wall_clock_read_in_a_throws_condition() {
        let plan = StructuredPlan::from_yaml(DRAFTED_2026_10_21_WALL_CLOCK_CONDITION).unwrap();
        assert_eq!(only_rule(&verify(&plan)), Rule::WallClockCall);
    }

    #[test]
    fn does_not_flag_a_condition_that_compares_the_methods_own_parameter() {
        let plan = StructuredPlan::from_yaml(
            r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window]
    methods:
      - signature: "Ticket.validate(LocalDateTime at)"
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "Ticket has expired", condition: "at is more than 90 minutes after issuedAt"}
"#,
        )
        .unwrap();
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

    // --- ExampleContradictsThrows: Phase 2's real A1 run (2026-10-25), run-01 of 10, byte-for-byte --
    // (`canopy-planner/runs/phase2-a1a2a3-20261025T223616/a1/plans/2026-10-25T21-42-37/run-01`). The
    // same contradiction (an example with `setup_values: {status: VALIDATED}` calling `cancel()` and
    // expecting a normal `CANCELLED` return) recurred in 8 of the 10 drafted plans that batch.

    const PHASE2_A1_RUN01_CANCEL_VALIDATED: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window, validation-precedence, cancellation-rule]
    methods:
      - signature: "Ticket.cancel()"
        preserves: [id, issuedAt]
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "A validated ticket cannot be cancelled", condition: "the ticket is already validated"}
    examples:
      - id: cancel-issued
        setup: 'Ticket ticket = new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED);'
        setup_values: {id: "T-1", issuedAt: "2026-03-01T10:00", status: ISSUED}
        call: 'Ticket.cancel()'
        expect: {id: "T-1", issuedAt: "2026-03-01T10:00", status: CANCELLED}
      - id: cancel-validated
        setup: 'Ticket ticket = new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.VALIDATED);'
        setup_values: {id: "T-1", issuedAt: "2026-03-01T10:00", status: VALIDATED}
        call: 'Ticket.cancel()'
        expect: {id: "T-1", issuedAt: "2026-03-01T10:00", status: CANCELLED}
"#;

    #[test]
    fn flags_the_phase2_a1_cancel_validated_contradiction() {
        let plan = StructuredPlan::from_yaml(PHASE2_A1_RUN01_CANCEL_VALIDATED).unwrap();
        let issues = verify(&plan);
        assert_eq!(only_rule(&issues), Rule::ExampleContradictsThrows);
        // `cancel-issued` is the correct, non-contradicting example (ISSUED does not match cancel()'s
        // own throws condition) and must not be swept up by a looser match.
        assert!(issues.iter().all(|i| i.message.contains("cancel-validated")));
        assert!(!issues.iter().any(|i| i.message.contains("cancel-issued")));
    }

    #[test]
    fn does_not_flag_the_golden_fixtures_setup_by_chained_call_examples() {
        // The real golden fixture (`canopy-implementer/tasks/tickets/plan.structured.yaml`) reaches the
        // same preconditions through a chained `setup` (`new Ticket(...).validate(...)`) rather than a
        // literal `setup_values: {status: VALIDATED}`, and its `expect` always states `throws` when the
        // precondition demands it — already covered by `the_tickets_fixture_has_no_contradictions`, but
        // this isolates the one unit the new rule touches.
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        let ticket_unit = plan.units.iter().find(|u| u.id == "ticket").unwrap();
        let issues = example_contradicts_own_throws(&StructuredPlan {
            units: vec![ticket_unit.clone()],
            ..plan
        });
        assert_eq!(issues, vec![]);
    }

    // --- ExceptionBorrowedFromDependency: the A1 re-run (2026-10-26), 4 of 5 fresh drafts, byte-for---
    // --- byte (`canopy-planner/runs/phase2-draft-check2/2026-10-26T21-56-46/run-01`) -----------------

    const PHASE2_A1_RERUN_SERVICE_BORROWS_TICKETS_EXCEPTIONS: &str = r#"
id: tickets
units:
  - id: ticket
    owns_rules: [validation-window, validation-precedence, cancellation-rule]
    methods:
      - signature: "Ticket.validate(LocalDateTime at)"
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "Ticket is already validated", condition: "the ticket is already validated"}
          - {order: 2, exception_type: IllegalStateException, message: "Ticket is cancelled", condition: "the ticket is cancelled"}
          - {order: 3, exception_type: IllegalStateException, message: "Ticket has expired", condition: "at is more than 90 minutes after issuedAt"}
      - signature: "Ticket.cancel()"
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "A validated ticket cannot be cancelled", condition: "the ticket is already validated"}
  - id: ticket-service
    owns_rules: [unknown-ticket-rejection]
    depends_on: [ticket]
    methods:
      - signature: "TicketService.validate(String id, LocalDateTime at)"
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "Ticket is already validated", condition: "the ticket is already validated"}
          - {order: 2, exception_type: IllegalStateException, message: "Ticket is cancelled", condition: "the ticket is cancelled"}
          - {order: 3, exception_type: IllegalStateException, message: "Ticket has expired", condition: "at is more than 90 minutes after issuedAt"}
      - signature: "TicketService.cancel(String id)"
        throws:
          - {order: 1, exception_type: IllegalStateException, message: "A validated ticket cannot be cancelled", condition: "the ticket is already validated"}
"#;

    #[test]
    fn flags_the_services_borrowed_validation_exceptions() {
        let plan = StructuredPlan::from_yaml(PHASE2_A1_RERUN_SERVICE_BORROWS_TICKETS_EXCEPTIONS).unwrap();
        let issues = verify(&plan);
        assert_eq!(only_rule(&issues), Rule::ExceptionBorrowedFromDependency);
        // Exactly the 4 borrowed entries, one per redeclared throw — not the dependency's own, which
        // must never be flagged for simply having the exceptions it legitimately owns.
        assert_eq!(issues.len(), 4, "{issues:?}");
        assert!(issues.iter().all(|i| i.unit.as_deref() == Some("ticket-service")));
    }

    #[test]
    fn does_not_flag_a_service_that_only_throws_what_it_owns() {
        // The golden fixture's own `ticket-service`: `validate`/`cancel` throw only "Unknown ticket"
        // (IllegalArgumentException), never any of `ticket`'s own IllegalStateException entries.
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        assert_eq!(exception_borrowed_from_dependency(&plan), vec![]);
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

    /// A live drafting run, 2026-10-21: `setup` declares `Ticket ticket = new Ticket(...)` (a real
    /// declaration, the same shape `TICKET_DEFINED_BEFORE_USE` above states through `defines`), but
    /// the model's answer never populated the separate `defines` field at all. Before
    /// `declared_in_setup` existed, every example in the unit was flagged, and recovery then emptied
    /// the whole list (the bug `examples_named_in_collapses_a_reused_id_to_one_entry` pins a different
    /// angle of). The declaration in `setup` is read directly now, without needing `defines` at all.
    #[test]
    fn does_not_flag_an_identifier_a_java_declaration_introduces_even_without_defines() {
        let plan = StructuredPlan::from_yaml(
            r#"
id: tickets
units:
  - id: ticket
    examples:
      - id: valid-at-boundary
        setup: 'Ticket ticket = new Ticket("T-1", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED);'
        call: 'ticket.validate(LocalDateTime.of(2026, 3, 1, 11, 30))'
        expect: {status: VALIDATED}
"#,
        )
        .unwrap();
        assert_eq!(verify(&plan), vec![]);
    }

    #[test]
    fn declared_in_setup_finds_the_declared_name_and_nothing_else() {
        assert_eq!(
            declared_in_setup("Ticket ticket = new Ticket(\"T-1\", LocalDateTime.of(2026, 3, 1, 10, 0), TicketStatus.ISSUED)"),
            vec!["ticket".to_string()]
        );
        assert_eq!(declared_in_setup("repository.save(ticket)"), Vec::<String>::new(), "no declaration, no match");
        assert_eq!(
            declared_in_setup("Ticket ticket = other.ticket()"),
            vec!["ticket".to_string()],
            "the declared name, regardless of what the right-hand side is"
        );
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

    // --- The X9/X10 no-mocking-framework line: only on a step with a collaborator -------------------

    #[test]
    fn the_service_step_states_no_mocking_framework_is_available() {
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        let legacy = to_legacy_plan(&plan).unwrap();
        let service_step = &legacy.steps[2]; // tickets-3-ticket-service, depends_on [ticket, ticket-repository]
        assert!(
            service_step.instruction.contains("No mocking framework"),
            "the service unit has a constructed collaborator, but its instruction reads:\n{}",
            service_step.instruction
        );
    }

    #[test]
    fn a_unit_that_only_reads_a_types_shape_gets_no_mocking_line() {
        let plan = StructuredPlan::from_yaml(TICKETS_STRUCTURED).unwrap();
        let legacy = to_legacy_plan(&plan).unwrap();
        // tickets-1-ticket (no depends_on) and tickets-2-ticket-repository (depends_on: [ticket], one
        // type it reads, not a collaborator it constructs) must not carry a line answering a question
        // their own step never raises.
        for step in &legacy.steps[..2] {
            assert!(
                !step.instruction.contains("No mocking framework"),
                "step {} constructs no collaborator, but its instruction reads:\n{}",
                step.id, step.instruction
            );
        }
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

    // --- Phase 2 arm A1: drafting, against a scripted model, no network. --------------------------

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use canopy_implementer::llm::{LlmError, LlmResponse};

    struct Script {
        replies: Mutex<VecDeque<Result<LlmResponse, LlmError>>>,
        requests: Arc<Mutex<Vec<StructuredRequest>>>,
    }

    impl Generator for Script {
        fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
            self.requests.lock().unwrap().push(request.clone());
            self.replies.lock().unwrap().pop_front().expect("the script ran out of replies")
        }
    }

    fn script(replies: Vec<Result<LlmResponse, LlmError>>) -> (Script, Arc<Mutex<Vec<StructuredRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        (Script { replies: Mutex::new(replies.into()), requests: requests.clone() }, requests)
    }

    fn reply(content: &serde_json::Value) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            call_id: 1,
            content: content.to_string(),
            finish_reason: "stop".into(),
            prompt_tokens: 1,
            completion_tokens: 1,
        })
    }

    fn draft_fixture() -> StructuredPlan {
        StructuredPlan::from_yaml(
            r#"
id: things
api: "Package p.\nrecord Thing(int value). Thing bump(int by) returns a new thing and throws IllegalArgumentException."
units:
  - id: thing
    owns_rules: [bump-rule]
    rules: "bump adds `by` to value; by must be positive."
    methods:
      - signature: "Thing.bump(int by)"
"#,
        )
        .unwrap()
    }

    const BLANK_FEATURE: &str = "Feature: F\n  Scenario: First\n    When x\n";

    #[test]
    fn drafts_examples_and_throws_from_a_clean_first_answer() {
        let plan = draft_fixture();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        let answer = json!({
            "methods": [{"throws": [
                {"order": 1, "exception_type": "IllegalArgumentException", "message": "by must be positive", "condition": "by is not positive"}
            ]}],
            "examples": [
                {"id": "bump-by-two", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}}
            ]
        });
        let (generator, requests) = script(vec![reply(&answer)]);
        let drafted = draft_structured(&generator, &plan, &feature).unwrap();
        assert_eq!(requests.lock().unwrap().len(), 1, "a clean first answer needs no retry");
        assert_eq!(drafted.units[0].methods[0].throws.len(), 1);
        assert_eq!(drafted.units[0].examples.len(), 1);
        // The rendered plan must be free of every Phase 0 contradiction, same as the hand-written fixture.
        assert_eq!(verify(&drafted), vec![]);
    }

    fn draft_fixture_with_dependent_service() -> StructuredPlan {
        StructuredPlan::from_yaml(
            r#"
id: things
api: "Package p.\nrecord Thing(int value). Thing bump(int by) returns a new thing and throws IllegalArgumentException.\nclass ThingService, constructor ThingService(ThingRepository): Thing bump(String id, int by) returns a new thing and throws IllegalArgumentException."
units:
  - id: thing
    owns_rules: [bump-rule]
    rules: "bump adds `by` to value; by must be positive."
    methods:
      - signature: "Thing.bump(int by)"
  - id: thing-service
    owns_rules: [unknown-thing-rejection]
    depends_on: [thing]
    rules: "ThingService loads a thing and calls bump on it. Thing owns the bump rule; the service only loads, calls and saves."
    methods:
      - signature: "ThingService.bump(String id, int by)"
"#,
        )
        .unwrap()
    }

    #[test]
    fn the_dependents_prompt_states_what_its_dependency_already_throws() {
        let plan = draft_fixture_with_dependent_service();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        let thing_answer = json!({
            "methods": [{"throws": [
                {"order": 1, "exception_type": "IllegalArgumentException", "message": "by must be positive", "condition": "by is not positive"}
            ]}],
            "examples": [
                {"id": "bump-by-two", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}}
            ]
        });
        let service_answer = json!({
            "methods": [{"throws": [
                {"order": 1, "exception_type": "IllegalArgumentException", "message": "Unknown thing", "condition": "the thing is unknown"}
            ]}],
            "examples": [
                {"id": "unknown-thing", "setup": "ThingService s = new ThingService(repo)", "defines": ["s"], "call": "s.bump(\"T-9\", 2)", "expect": {"throws": "IllegalArgumentException", "message": "Unknown thing"}}
            ]
        });
        let (generator, requests) = script(vec![reply(&thing_answer), reply(&service_answer)]);
        draft_structured(&generator, &plan, &feature).unwrap();
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        // `thing` has no dependency of its own, so its prompt states nothing.
        assert!(!requests[0].messages[1].content.contains("Already thrown by a dependency"));
        // `thing-service` depends on `thing`, which was drafted first in this same pass — its own
        // prompt must show the exact exception `thing` already throws, not just a prose reminder.
        let service_user = &requests[1].messages[1].content;
        assert!(service_user.contains("Already thrown by a dependency"), "{service_user}");
        assert!(
            service_user.contains("by must be positive") && service_user.contains("IllegalArgumentException"),
            "{service_user}"
        );
    }

    #[test]
    fn retries_with_the_audits_own_issue_text_on_a_contradiction() {
        let plan = draft_fixture();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        let contradicting = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "e1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}},
                {"id": "e2", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "4"}}
            ]
        });
        let clean = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "e1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}}
            ]
        });
        let (generator, requests) = script(vec![reply(&contradicting), reply(&clean)]);
        let drafted = draft_structured(&generator, &plan, &feature).unwrap();
        assert_eq!(drafted.units[0].examples.len(), 1);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2, "one retry");
        let retry_user = &requests[1].messages[1].content;
        assert!(retry_user.contains("e1") && retry_user.contains("e2"), "{retry_user}");
        assert!(retry_user.contains("expect"), "the retry must carry the audit's own issue text:\n{retry_user}");
    }

    #[test]
    fn drops_only_the_examples_an_issue_names_when_others_still_pass() {
        let plan = draft_fixture();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        // e1/e2 contradict each other on every attempt (the model never reconsiders); e3 is clean
        // throughout. Recovery must keep e3 and drop only e1 and e2 — not fail the whole unit.
        let persistent = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "e1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}},
                {"id": "e2", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "4"}},
                {"id": "e3", "setup": "Thing t = new Thing(5)", "defines": ["t"], "call": "t.bump(1)", "expect": {"value": "6"}}
            ]
        });
        let (generator, _) = script(vec![reply(&persistent), reply(&persistent)]);
        let drafted = draft_structured(&generator, &plan, &feature).unwrap();
        let ids: Vec<&str> = drafted.units[0].examples.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["e3"], "only the contradicting pair should be dropped");
        assert_eq!(verify(&drafted), vec![]);
    }

    #[test]
    fn fails_the_unit_when_every_example_stays_flawed() {
        let plan = draft_fixture();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        let wall_clock = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "e1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(LocalDateTime.now().getMinute())", "expect": {"value": "3"}}
            ]
        });
        let (generator, _) = script(vec![reply(&wall_clock), reply(&wall_clock)]);
        let err = draft_structured(&generator, &plan, &feature).unwrap_err();
        assert!(err.contains("thing"), "{err}");
    }

    #[test]
    fn a_wrong_method_count_is_a_shape_problem_not_a_panic() {
        let plan = draft_fixture();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        let too_many_methods = json!({
            "methods": [{"throws": []}, {"throws": []}],
            "examples": [
                {"id": "e1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}}
            ]
        });
        let clean = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "e1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}}
            ]
        });
        let (generator, requests) = script(vec![reply(&too_many_methods), reply(&clean)]);
        let drafted = draft_structured(&generator, &plan, &feature).unwrap();
        assert_eq!(drafted.units[0].examples.len(), 1);
        assert_eq!(requests.lock().unwrap().len(), 2);
    }

    /// A live run, 2026-10-21: every example of a unit came back with the same literal `id` ("T-1",
    /// the domain value, not a name for the example). Rejected at acceptance time now, before it can
    /// reach the recovery path at all.
    #[test]
    fn a_reused_example_id_is_a_shape_problem_not_a_silent_accept() {
        let plan = draft_fixture();
        let feature = Feature::parse(BLANK_FEATURE).unwrap();
        let duplicate_ids = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "T-1", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}},
                {"id": "T-1", "setup": "Thing t = new Thing(5)", "defines": ["t"], "call": "t.bump(1)", "expect": {"value": "6"}}
            ]
        });
        let clean = json!({
            "methods": [{"throws": []}],
            "examples": [
                {"id": "bump-by-two", "setup": "Thing t = new Thing(1)", "defines": ["t"], "call": "t.bump(2)", "expect": {"value": "3"}},
                {"id": "bump-by-one", "setup": "Thing t = new Thing(5)", "defines": ["t"], "call": "t.bump(1)", "expect": {"value": "6"}}
            ]
        });
        let (generator, requests) = script(vec![reply(&duplicate_ids), reply(&clean)]);
        let drafted = draft_structured(&generator, &plan, &feature).unwrap();
        assert_eq!(drafted.units[0].examples.len(), 2);
        let retry_user = &requests.lock().unwrap()[1].messages[1].content;
        assert!(retry_user.contains("distinct id"), "{retry_user}");
    }

    /// Direct test of the mechanism that actually broke on the live run: `examples_named_in` returns a
    /// *set* of names, so three examples sharing one id collapse to a set of size 1 — which is why
    /// comparing `named.len()` against the raw example count (the original recovery guard) read "only
    /// one example is in trouble" when every example was about to be removed. The guard was changed to
    /// compare the actual count removed instead; this test pins the set's own collapsing behaviour so
    /// a future change to `examples_named_in` cannot silently reintroduce the mismatch.
    #[test]
    fn examples_named_in_collapses_a_reused_id_to_one_entry() {
        let examples = vec![
            Example {
                id: "dup".into(),
                setup: "Thing t = new Thing(1)".into(),
                setup_values: BTreeMap::new(),
                defines: vec!["t".into()],
                call: "t.bump(2)".into(),
                expect: BTreeMap::new(),
            },
            Example {
                id: "dup".into(),
                setup: "Thing t = new Thing(1)".into(),
                setup_values: BTreeMap::new(),
                defines: vec!["t".into()],
                call: "t.bump(2)".into(),
                expect: BTreeMap::new(),
            },
        ];
        let issues = vec![Issue {
            rule: Rule::WallClockCall,
            unit: Some("thing".into()),
            message: "example dup reads the wall clock".into(),
        }];
        let named = examples_named_in(&issues, &examples);
        // Three examples share the id; the set still has one entry. A caller that only compares
        // `named.len()` (1) against `examples.len()` (2) would wrongly conclude "not all of them,"
        // even though removing every example whose id is in `named` removes all of them.
        assert_eq!(named.len(), 1);
        let remaining: Vec<&Example> = examples.iter().filter(|e| !named.contains(&e.id)).collect();
        assert!(remaining.is_empty(), "every example shares the named id, so none should remain");
    }
}
