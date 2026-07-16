# Is Role Semantics a Real, Recurring Blind Spot?

Status: observation and classification only. No mechanism, prompt change, new stage, or fix is
proposed anywhere in this document. Answers one question: is "Canopy never asks what an actor
label actually means" a real, evidenced, structural property of the current pipeline, or an
artifact of one story's particular wording?

Date: 2026-07-16

Reviewed: `docs/design/product-owner-perspective-experiment.md`, `docs/design/exploration-
enumeration-gap-investigation.md`, `docs/design/pre-behavior-planning-review.md`, `docs/design/
human-insight-inventory.md`, `docs/principles/unresolved-decisions-become-explicit-decision-
points.md`, `docs/principles/structure-emerges-from-behavior.md`, and — going past the documents,
since the questions asked require it — the actual current code: `canopy-core/src/
named_described.rs` (the `Role` type), `canopy-cli/src/commands/init.rs` and `intent.rs` (the two
places a `Role` gets written), `canopy-llm/src/prompts/intent.rs` (`suggest_roles`,
`stories_from_intent_prompt`), `canopy-llm/src/prompts/spec.rs` (the business-policy checklist's
`authorization` area, `identify_architectural_questions`'s UI-proposal check), and the real
dogfooding project's own `roles.yaml`.

---

# 1. Role Inventory

Every place an actor/role label appears or is produced, traced through the actual code:

| Location | What's preserved | What's assumed | What's never captured |
|---|---|---|---|
| `suggest_roles` (`init` bootstrap, `canopy-llm/src/prompts/intent.rs:201-226`) | A bare `Vec<String>` of role names, LLM-suggested from the idea description alone | That a short noun phrase ("case manager," "field operator") is sufficient to denote a role | Any description, scope, or internal/external classification — the prompt's own return contract is `["role one", "role two"]`, strings only, nothing else |
| `Role` type (`canopy-core/src/named_described.rs:50`) | The type itself supports two shapes: `Simple(String)` and `Described { name, description }` | Nothing at the type level — this is a capability, not a claim about what's actually stored | The type has no dedicated field for actor-kind (internal/external, employee/customer), trust level, or any structured classification — `description` is one freeform string, whatever a human chooses to type |
| `init`'s bootstrap flow (`canopy-cli/src/commands/init.rs:93-113`) | For each kept suggestion, a human is explicitly asked "Description for '<name>' (leave blank to skip)" — a real, working, human-facing prompt | That a human will voluntarily supply a distinguishing description if one is needed; no question is targeted at internal/external status specifically — it's open-ended free text | Nothing is enforced; leaving it blank (the path of least resistance) silently produces `Role::Simple` with the exact same downstream treatment as a role that was actually clarified |
| `intent`'s automatic role update (`canopy-cli/src/commands/intent.rs:90-98`) | The bare `as_a` string from an accepted story, deduplicated case-insensitively against existing role names | That the string alone is a complete, sufficient role record | A description is never requested, never asked of the model, never asked of the human — `Role::Simple(role)` is hardcoded; **there is no human gate at this specific step at all** (confirmed directly in code, matching `pre-behavior-planning-review.md`'s own row 5) |
| `stories_from_intent_prompt`'s role-reuse context (`canopy-llm/src/prompts/intent.rs:9,17-27`) | If a role happens to have a description, it *is* rendered into the prompt as `"name — description"` | That the model will use this context correctly to decide whether to reuse an existing role name | The description, even when present, is used only to help the model pick a role *name* — nothing instructs the model to use it to resolve or flag an identity ambiguity |
| Business-policy checklist, `authorization` area (`canopy-llm/src/prompts/spec.rs:554-555`) | The question "does creating or modifying this entity require a specific role or permission *beyond the actor already being authenticated*?" | **That the actor's own identity is already a settled fact** — this is the load-bearing assumption; the question only asks about *additional* constraints on top of it | Whether the actor is authenticated *as what* — this area was never designed to ask that; it presupposes an answer already exists |
| `identify_architectural_questions`'s UI check (`canopy-llm/src/prompts/spec.rs:197`) | "if the story has a human actor performing an action, there must be a frontend" | Only that *some* human actor exists, to justify proposing a UI | What kind of actor — this check is existence-only, not identity-specific |
| Scenario generation (`canopy-llm/src/prompts/spec.rs`, various) | The actor is referenced narratively ("the actor submits the mandatory fields," "The manufacturer representative is authenticated") in every scenario | That the actor reference is stable and meaningful throughout | Nothing about the actor is elaborated or questioned at this stage — it is pure narrative scaffolding derived from `as_a` |
| Generated OpenAPI (`stories/manufacturer-001/openapi.yaml`, confirmed directly) | Nothing — `POST /manufacturers` has no `security` scheme at all | — | The unresolved identity/authorization question propagates all the way to the generated API contract with zero resolution at any point |

---

# 2. Role Meaning Analysis — Tracing `manufacturer representative`

Stage by stage, what Canopy knows / assumes / infers / never resolves:

1. **`intent` — story generation.** The model reads "Manufacturers must be registered in the
   system before products can reference them" and produces `as_a: manufacturer representative`.
   *Knows*: nothing yet — this is the moment the label is invented. *Assumes*: that some named role
   performs this action; the prompt's only relevant instruction is "reuse a known role if it fits,"
   which doesn't apply here since no role existed yet. *Infers*: a plausible-sounding noun phrase
   from the word "manufacturer" in the intent text — not a considered choice between "the
   manufacturer's own staff" and "our internal catalog team." *Never resolved*: whether this actor
   is external (an employee of the manufacturer itself, self-registering their employer) or internal
   (a catalog/admin team member acting on the business's behalf) — this is not a case where the
   answer exists somewhere and simply wasn't asked; the phrasing itself is compatible with both
   readings, and nothing forced a choice between them.
2. **`roles.yaml` registration.** `intent.rs` appends `Role::Simple("manufacturer representative")`,
   automatically, no human gate. *Knows*: the string is now "on file." *Assumes*: that the string is
   a complete record. *Infers*: nothing — this step performs a case-insensitive name match against
   existing roles and nothing else. *Never resolved*: identical to step 1 — nothing here revisits or
   even surfaces the question.
3. **`spec` — business-policy checklist, `authorization` area.** The prompt asks whether creating a
   `Manufacturer` requires *additional* permission "beyond the actor already being authenticated."
   *Knows*: nothing about who the actor is. *Assumes*: the actor's identity and authentication status
   are already settled facts, inherited unexamined from the story. *Infers*: whatever the model
   infers here is about *additional* role/permission requirements, not the actor's own definition.
   *Never resolved*: the checklist's own design makes this structurally impossible to reach at this
   stage — it was never built to ask "who is this."
4. **`spec` — scenario generation.** Every one of the 12 real scenarios states "The manufacturer
   representative is authenticated" in its `given` clause. *Knows*: nothing new. *Assumes*: the
   phrase denotes a stable, already-understood actor. *Infers*: nothing. *Never resolved*: same
   question, now repeated verbatim 12 times with no more content each time.
5. **`spec` — `identify_architectural_questions`'s UI proposal.** Checks only "does a human actor
   exist" to justify proposing `manufacturer-registration-portal`. *Knows*/*assumes*/*infers*:
   existence only, not identity. *Never resolved*: unchanged.
6. **`spec` — OpenAPI generation.** The generated `POST /manufacturers` operation has no `security`
   scheme. *Knows*: nothing to base one on — no authorization model was ever resolved upstream.
   *Never resolved*: the ambiguity that began at step 1 propagates, unexamined, all the way to the
   final generated API contract.

**The pattern across all six stages**: the actor's own definition is invented once, at the moment
of first mention, and then carried forward by reference — quoted, restated, and built upon — but
never independently checked, clarified, or even flagged as assumed, at any single point between
story generation and the generated OpenAPI spec.

---

# 3. Counter-Evidence

Actively searching for evidence this is already handled, per the explicit instruction not to
merely support the hypothesis — and this search found something real, not nothing:

- **Role description capture is a genuine, working mechanism — it is not vaporware.**
  `canopy-core/src/named_described.rs`'s `Role::Described { name, description }` variant is real,
  tested (`role_described_roundtrip`), and reachable through real, human-facing code:
  `init.rs`'s bootstrap flow explicitly prompts "Description for '<name>' (leave blank to skip)"
  for every LLM-suggested role a human keeps. This is direct, concrete counter-evidence against a
  strong version of the hypothesis ("Canopy has no mechanism anywhere for capturing what a role
  means") — it does, and the mechanism is exercised in real code, not just declared in a type.
- **Role reuse context does render a description when one exists.** `stories_from_intent_prompt`
  formats a role as `"name — description"` when `description()` returns `Some`, not just the bare
  name — confirming the description, once captured, is not simply dead data; it does reach at least
  one later prompt.
- **However, this counter-evidence does not resolve the concern for the path that actually produced
  the real ambiguity.** `manufacturer representative` was created exclusively through `intent.rs`'s
  automatic role-registration path — confirmed directly: `roles.yaml`'s real content is the bare
  string `manufacturer representative` with no map/description structure, which is exactly what
  `Role::Simple` serializes as (the `named_described!` macro's `#[serde(untagged)]` shape means a
  `Described` entry would appear as a `{name, description}` map instead). This path — the *only*
  path that has ever populated a role in this project's real history — never prompts for a
  description, never asks the model for one, and has no human gate at all. The mechanism that could
  capture role meaning exists; the mechanism that actually produced this project's one real role
  bypasses it entirely.
- **A further, more precise finding, not previously stated**: this is a genuine internal
  inconsistency between two code paths that both write to the same `RolesRegistry`/`roles.yaml`
  artifact. `init`'s bootstrap path treats a role as worth a human-correctable, describable entry.
  `intent`'s automatic path treats a role as a disposable string. Nothing enforces that a role
  entered through the second path ever gets the same treatment as one entered through the first —
  and per `pre-behavior-planning-review.md`'s own Decision Classification table, this asymmetry was
  already implicitly on record ("`intent`'s roles-registry update" classified as `Implicit Decision`
  — "new role rows are added automatically... with no dedicated review step") without previously
  being connected to the role-*semantics* question specifically.

---

# 4. Relationship To Authorization

**These are separate concerns, and authorization does not implicitly cover role meaning — grounded
directly in the checklist's own wording, not inferred.** The `authorization` area's exact question
is "does creating or modifying this entity require a specific role or permission *beyond the actor
already being authenticated*?" This phrasing has two load-bearing presuppositions: (1) the actor is
authenticated, and (2) a specific, already-understood actor/role is what got authenticated. The
question only asks whether something *additional* is required on top of that base. It structurally
cannot surface "who is this actor, actually?" — that question sits one layer beneath what
authorization was ever designed to ask. Even a perfectly-functioning authorization check (per
`unresolved-decisions-become-explicit-decision-points`'s own citation-enforcement mechanism, which
would correctly classify this area `unresolved` given no supporting evidence in `manufacturer-001`'s
real inputs) would only ever produce "no basis to require additional permission" — it has no
vocabulary in which to say "the actor's own identity itself was never established." Role semantics
is logically prior to authorization, not a subset or restatement of it.

---

# 5. Relationship To Existing Principles

**`unresolved-decisions-become-explicit-decision-points` — fits the *shape* closely, but was never
scoped to include it.** This principle's own "Problem That Revealed It" language — a model "will
not stop and ask what an unresolved question should mean, it will pick an interpretation, and that
becomes a hidden business decision with no record it was ever made" — describes exactly what
happened at step 1 of the trace above. The mechanism this principle produced (a fixed enumeration
of checklist areas, each requiring cited evidence to resolve) is a strong, evidenced fit for this
*kind* of problem in general. But the mechanism only operates on its six named areas, and role
semantics has never been one of them — the same distinction the Exploration Enumeration Gap
Investigation already drew for uniqueness/authorization (already enumerated, artifact predates the
mechanism) does not apply here: there is no version of this mechanism, past or present, that has
ever included role semantics as a checked area. The principle's *shape* fits; its *actual scope*
does not include this concern, and never has.

**`structure-emerges-from-behavior` — compatible with the core claim, but doesn't resolve a
narrower, later-stage gap the principle doesn't itself address.** This principle's own validated
evidence explicitly names "let user roles emerge from intent, not from explore" as one of its four
supporting cases — i.e., that `manufacturer representative` emerged from a concrete story rather
than an upfront, abstract role-elicitation step is exactly the behavior this principle already
argues is *correct*. This means the concern here is **not** "Canopy should ask about role meaning
before any story exists" — the principle's own evidence (and its counter-evidence, the reintroduced
bootstrap step) would argue against that. The concern is narrower and sits *after* emergence: once a
role has emerged from real behavior, is its meaning ever clarified downstream — the same
optional-but-real treatment domain entities and (at `init` time only) bootstrap-suggested roles
already get via the `Described` variant? The principle's own reintroduced-bootstrap counter-evidence
shows this project has already recognized, once, that "ask nothing, ever" has a real cost and
built a correctable-starting-point mechanism in response — but that mechanism was never extended to
the automatic, per-story role-registration path that actually produces most of this project's real
roles. This is a genuine, evidenced gap the principle's own logic doesn't cover either way, not a
contradiction of it.

**Not clearly a fit for any other existing principle** — `compute-facts-mechanically` and
`exhaustive-enumeration-over-holistic-review` are both about *how* a check should be performed once
it exists, not about *whether* this particular check exists at all; neither argues for or against
adding a role-semantics check.

---

# 6. Scope Assessment

**The mechanism's reach is broad; the confirmed instance is a sample of one.** `intent.rs`'s
automatic role-registration path (`Role::Simple`, no human gate, no model-supplied description) is
not special-cased by role name or story content in any way — it fires identically for every
accepted story's `as_a` value, project-wide. Structurally, this means *any* future role this
mechanism ever produces — `customer`, `administrator`, `product manager`, `supplier`, `reviewer` —
would go through the exact same code path and land in `roles.yaml` with the exact same absence of
definition. The **mechanism** is universal, confirmed directly in code, not inferred from one
example.

Whether the **semantic ambiguity itself** (as opposed to the absence of a definition-capture step)
recurs as often across differently-worded roles is genuinely unverified — this project has exactly
one real role to check against. There is a real, honest reason to suspect it may not be uniform:
"manufacturer representative" is a phrase that straddles an internal/external boundary by its own
construction (a "representative of X" is ambiguous between "someone affiliated with X, acting on
X's behalf toward us" and "someone on our side who handles X's records") in a way that a role like
"administrator" or "customer" may not. This is a plausible distinction, not an evidenced one — no
second real role exists in this project to test it against.

---

# 7. Conclusion

**Is role semantics a real, evidenced concern?** Yes, grounded directly in current code across
every relevant stage, not just in the one story's output: no stage between `intent` and the
generated OpenAPI spec ever asks what an actor label denotes; the one channel that could capture
this (`Role::Described`) is real but is only reachable through a path (`init`'s bootstrap) that has
never fired for this project's one real role, while the path that did produce it (`intent`'s
automatic registration) has no such channel at all.

**Is `manufacturer-001` an isolated example?** The observed *ambiguity* is a sample of one — this
project has never had a second role to check recurrence against, and "manufacturer representative"
may be an unusually dual-meaning phrase compared to other realistic role names. But the *mechanism*
that produced the gap is not isolated: it is the only code path that has ever written a role into
this project's real `roles.yaml`, it applies uniformly regardless of role name, and it is
structurally guaranteed to reproduce the same absence-of-definition for the next role it processes,
whatever that role turns out to be called.

**Is the evidence strong enough to preserve this as an open question?** Yes — already filed as
`docs/open-questions/role-semantics-explicitness.md`. This deeper pass strengthens rather than
changes that filing: it found genuine, working counter-evidence (`Role::Described` is real, not
aspirational) and showed precisely why that counter-evidence doesn't reach the path that actually
matters — a sharper, more specific finding than "nothing addresses this at all" would have been.

**What specifically remains unknown?** Two things, honestly separated: (1) whether the *semantic*
ambiguity — not just the missing definition-capture step — actually recurs across differently-
worded roles, which cannot be answered without a second real role to observe; and (2) whether
extending description-capture to the automatic per-story path would sit comfortably alongside
`structure-emerges-from-behavior` (as a *post-emergence* clarification, which its own logic doesn't
prohibit) or would risk drifting back toward the *upfront* elicitation that principle's own evidence
argues against — a real, currently-undecided tension this document deliberately does not resolve,
since resolving it is a design question, not an observation.
