---
title: "Policy Discovery vs. Policy Invention"
date: 2026-07-14
status: draft

learning_type:
  - failure-analysis
  - reproducibility-study

topics:
  - ai-assisted-specification
  - business-rules
  - hallucination
  - human-in-the-loop

key_principles:
  - "A classification scheme with a 'resolved' option is not enough to stop a model from resolving questions it has no basis to answer — the option itself needs an evidence requirement."
  - "When a model must choose between confidently answering and admitting uncertainty, it will default to confidence unless the prompt actively penalizes ungrounded confidence."

source_artifacts:
  - "commit 0fd89d7 — Force per-item policy classification instead of a freeform resolved/open split"
  - "commit c77f322 — Require textual evidence before Policy Discovery may classify a policy resolved"
  - "reproducibility sweep, manufacturer-001, before/after evidence-grounding fix"

story_ids: []

evidence_strength: high

commits:
  - 0fd89d7
  - c77f322

initial_assumption: >
  Giving a model three explicit classification buckets for a business question — resolved, not
  applicable, unresolved — and instructing it to use "unresolved" whenever the answer isn't
  supported, would be sufficient to stop it from inventing plausible-sounding business rules.

final_understanding: >
  The classification scheme alone doesn't change the model's underlying incentive to sound
  confident. Only requiring an explicit, checkable citation for every "resolved" answer — and
  treating "resolved with no citation" as a hard failure rather than accepting it — measurably
  shifted the model's actual behavior toward defaulting to "unresolved," not just toward correctly
  labeling its guesses.
---

# Summary

We asked a model six business questions about a data entity — does this field need to be unique,
does creating it require a special role, how long should records be kept — and gave it three
honest ways to answer: resolved, not applicable, or unresolved-please-ask-a-human. Looking at a
real generated result, five of the six came back "resolved." One of them invented a specific role
name. Another invented a specific retention policy. None of it was in the request. None of it was
in any prior decision. The model wasn't confused about which bucket to use — it used "resolved"
correctly, confidently, for answers it had no basis to give.

# Original Assumption

We thought this was already solved by the output shape. Six named questions, three buckets with
clear definitions — resolved (state the rule), not applicable (say why), unresolved (ask the
question, never guess). An explicit, low-friction "I don't know" option felt like enough structure
to stop invention before it started.

# What Happened

We looked at a real specification generated for a "register a manufacturer" request. Five of six
policy questions were marked "resolved." One said manufacturer creation requires a specific named
authorization role. Another said an optional field defaults to an empty string. Another said
records persist indefinitely. Another said duplicate submissions get rejected. Another said the
entity has no dependency on anything else.

Every one of those was a real, sensible thing for a specification to state. None of them were
anywhere in the model's actual inputs. The request didn't mention roles. Nothing in the project's
prior decisions mentioned retention. Nothing established a default for that field. The model wasn't
wrong that these were real questions worth answering. It was answering them itself, with total
confidence, and labeling the guess exactly the same way it would have labeled a genuinely-supported
answer.

This is a different kind of failure than a model skipping something it should have covered. This is
a model quietly making a business decision and reporting it as already made. We'd actually written
down, earlier and separately, a version of this exact worry, before we had a concrete case: a model
"asked to extract [requirements] will not stop and ask what an unresolved question should mean, it
will pick an interpretation, and that becomes a hidden business decision with no record it was ever
made." Seeing it happen for real, with a fabricated role name sitting in a generated spec looking
exactly as authoritative as a real one, was still not what we expected — the escape hatch was
right there in the instructions, unused.

Our hypothesis: the bucket wasn't the problem — the cost of using it was. Confidently stating a
plausible-sounding rule and honestly admitting "unresolved" looked equally acceptable to the model,
so nothing pushed it toward the second one. The prediction that followed: if we made a confident
answer require something the model couldn't fake as easily as fluent prose — a specific, checkable
citation — the model should default to "unresolved" far more often, without us telling it to.

We changed what "resolved" requires. It's no longer enough to state a rule. Every "resolved" (and,
after we found the same hole from a different angle, every "not applicable" too) now has to name
its exact source — the specific sentence in the request, the specific prior decision, the specific
established vocabulary entry that actually says this. Code checking the model's response enforces
that this citation is present: a "resolved" classification with a rule but no named source now
fails the whole operation and asks for a re-run, rather than being accepted as a slightly
lower-confidence guess. We had to widen this to "not applicable" too, once review turned up that a
model could dodge the "resolved" check by relabeling a fabricated exemption as "not applicable"
instead — same fabrication, different door.

We tested the prediction directly with a controlled before/after comparison — three identical runs
each time, same starting state, only the model's sampling varying — to check whether this actually
changed behavior or just changed which error message we'd see. Before the fix: 5 of 6 questions
"resolved" in two of the three runs,
3 of 6 in the third, each with a specific fabricated answer. After the fix, across three more
identical runs: 1 of 6 resolved in two runs, 2 of 6 in the third — and the new citation check never
once rejected a response outright across any of those three runs, meaning the model was actually
satisfying the requirement, not just failing it repeatedly. The model didn't get better at admitting
uncertainty. It changed its default answer once confidence required a receipt.

# Evidence

- A live-generated specification with five of six business-policy questions marked "resolved,"
  each carrying a specific, invented answer — a named role, a default value, a retention
  statement, a duplicate-handling rule, a no-dependency statement — none of them present anywhere
  in the request, prior decisions, or established project vocabulary shown to the model.
- Commit `0fd89d7`: the first fix — forcing a fixed, named set of six classification entries so
  the model couldn't skip a question or invent a fourth output bucket. Closed a related bucketing
  bug. Did not, on its own, stop the fabrication.
- Commit `c77f322`: the fix that changed outcomes — "resolved" and "not applicable" both now
  require a named `evidence` field pointing at "the story, an ADR, or domain vocabulary," checked
  by code that fails the entire operation if either classification is missing its citation.
- A controlled reproducibility comparison, three runs before and three runs after: before, two of
  three runs resolved 5 of 6 questions with fabricated specifics and the third resolved 3 of 6;
  after, the three runs resolved 1, 1, and 2 of 6 respectively, with the remainder correctly routed
  to an open question — and the citation-presence check never fired an error across any of the
  post-fix runs.

# Evolution of Understanding

We believed an explicit three-way classification, correctly defined, was enough structure to stop
a model from inventing business rules — it had a low-friction, explicit way to say "I don't know,"
so there was no obvious reason it would prefer to guess.

The evidence said the option existing didn't matter if nothing made the alternative more costly.
The classification scheme constrained the *shape* of the output without touching its *epistemic
basis* at all.

We added exactly that missing constraint: every confident answer now has to point at a specific,
checkable source, and an answer with no source is invalid output, not an acceptable low-confidence
guess. Small structural change, large effect — it turns "does this look plausible" (a question a
model will usually answer yes to) into "can I point at exactly where this came from" (a question
with a genuine right answer, including an honest "no").

We now assume a model won't reliably use an "unknown" option just because it's available. It has to
be cheaper than fabricating, and that only happens once every confident answer needs a citation that
gets checked by something other than the model's own self-report.

# Engineering Principle

A model given the option to classify something as "unresolved" or "unknown" will not reliably
choose that option just because it's available — it will keep choosing confident answers unless
every confident answer is required to cite a specific, checkable source, and answers without one
are rejected outright rather than silently accepted.

# Why It Generalizes

Any AI-assisted system asking a model to resolve open questions from limited context — filling a
missing config value, inferring a business rule from partial documentation, deciding whether a
requirement is already satisfied — faces the same incentive. Language models produce fluent,
complete-sounding answers by default, and a plausible answer with zero support usually looks, on
the surface, just as good as a well-grounded one. An "unknown" bucket is necessary but not
sufficient. Making confidence cost something — a citable source, checked outside the model's own
report — is what actually changes outcomes, anywhere a system uses a model to make a call that
should really be a human decision or a lookup, not a guess dressed up as one.

# Remaining Questions

The evidence requirement measurably cut how often the model resolved a question with zero support.
Looking closer at the runs that still resolved something, the cited evidence was sometimes weak —
quoting the entire original request as the source for a claim the request doesn't actually make
explicit. The check verifies a citation exists. It doesn't yet verify the citation actually
supports the specific claim. That's a smaller problem than the original one — a named-but-unverified
source is far easier for a human to catch than an unsupported claim with no source at all — but it
isn't fully closed. A natural next test: does requiring the evidence field to quote a specific
substring, rather than name a source category, close that gap too, and does it hold up as reliably
under repeated sampling as the current fix did?
