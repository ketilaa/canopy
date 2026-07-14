---
title: "Every Example Noun Is a Candidate Answer"
date: 2026-07-14
status: draft

learning_type:
  - failure-analysis
  - design-evolution

topics:
  - prompt-engineering
  - few-shot-examples
  - small-language-models
  - anchoring

key_principles:
  - "An example noun in a prompt is a candidate output, not a neutral illustration."
  - "A 'safe' generic placeholder is still a concrete, copyable word — only a structural placeholder that isn't a real word removes the attractor risk entirely."

source_artifacts:
  - "commit a254b25 — Add Entity Continuity gate and fix the want-field example that anchored on it"
  - "commit 966fadf — Prefer structural placeholders over example nouns, even Widget"
  - "reproducibility sweep, manufacturer-001, entity_schema divergence to Account"

story_ids: []

evidence_strength: high

commits:
  - a254b25
  - 966fadf

initial_assumption: >
  A prompt's worked example is a teaching aid. As long as the example noun is either the real
  domain (bad — it might leak into unrelated projects) or a deliberately generic stand-in like
  "Widget" (safe — it can't be mistaken for anything real), the model will treat it as illustrative
  and substitute the actual subject matter in its own output.

final_understanding: >
  The reference model does not reliably distinguish "this is an example" from "this is the
  answer." Any concrete noun in a prompt — domain-specific or deliberately generic — is a candidate
  the model can copy forward into unrelated output. Only a placeholder that isn't a real word at
  all removes the risk rather than just relocating it.
cluster: "Prompt Crafting Idiosyncrasies of Small Models"
---

# Summary

We ran the same request three times, from an identical starting point, changing nothing but the
model's own sampling. In all three runs, a phrase from one of our prompt's worked examples —
"register an account" — showed up almost verbatim in the output, for requests that had nothing to
do with accounts at all. One run went further: the model generated an entire data schema with
username, password, and email fields, for an entity that every other piece of context in the same
prompt called "Manufacturer." We already had a standing convention elsewhere in the same codebase
meant to prevent exactly this: use a deliberately fake, generic noun ("Widget") instead of a real
one. If that convention actually worked, prompts already following it shouldn't show the same
pattern. The next day, one did.

# Original Assumption

Standard advice says: don't put your real domain vocabulary in a prompt's worked examples, use a
generic placeholder instead, so the model doesn't leak project-specific naming into unrelated
contexts. We followed that advice and expected it to be enough — a generic, obviously-fake noun
should read to the model as "this is illustrative," cleanly separable from whatever the actual
request is about.

# What Happened

A rule in our pipeline told the model how to phrase the core action of a user story, and the rule
needed a worked example. We picked something concrete-sounding but generic: "register an account."
It felt safe — "account" isn't a specific product decision, it's the kind of word you'd use in any
tutorial.

We ran the same underlying request three times to check for consistency. All three times, that
exact phrase — "register an account" — showed up in the model's output, regardless of what the
actual request was about. In one of the three runs, it went further than phrasing: the model's
generated data schema, for a story that was explicitly about registering a *manufacturer* — the
word "manufacturer" was right there in the same prompt, established both in the story and in the
project's own accumulated vocabulary — came back as an "Account" schema. Username field. Password
field. Email field. None of which had any basis in the actual request. The only thing in the prompt
that pointed toward an account of any kind was the example.

We rewrote the rule two ways. First, we replaced the single fixed example with a conditional
pattern spanning several different hypothetical subjects at once — "if the intent is about
manufacturers, write register a manufacturer; if about products, write register a product" — so
there was no single decontextualized phrase sitting in the prompt to copy. Second, and this is the
part that turned out to matter more: we added a plain, mechanical check downstream that has nothing
to do with prompt wording at all. Does the entity the model just generated match an entity already
established elsewhere in the project? If the project already knows about "Manufacturer" and this
step produces "Account" with nothing in the text pointing there, the step now fails outright and
saves nothing. We added that check because we didn't trust a better example to be sufficient on its
own.

That turned out to be exactly the right call, for a reason we didn't expect. Several other prompts
in the same system already followed the standing "use Widget, not your real domain" convention —
precisely the fix a reasonable person would reach for after seeing the account incident. If the
convention worked, those prompts should have been safe by construction. One of them used "Widget"
three times in a single set of worked examples. A review of that prompt flagged it anyway — not for
using a domain-specific word, but for using the *same generic word three times*, which was assessed
as recreating the identical anchoring pattern the domain-specific noun had caused, just with a word
that happened to be conventionally safe rather than actually safe. The prediction that a generic
placeholder would be immune to this failed.

So the fix wasn't "pick a better noun." Bracketed placeholders — `<entity>`, `<domain object>`,
`<aggregate>` — can't be copied literally into an answer, because they aren't real words. That's
the difference. "Widget" was an improvement over "Product." It was still a concrete, copyable word.
We now use real nouns only when the structure being taught genuinely needs one — showing a casing
convention, say — and even then, never a project-specific one.

# Evidence

- Three identical runs of the same underlying request, only sampling varying: the worked-example
  phrase "register an account" appeared near-verbatim in the model's own output in all three,
  regardless of the actual subject of the request. (Commit `98c1783`.)
- One of those three runs produced a fully divergent generated schema — username, password, email
  fields — for an entity that both the story and the project's own established vocabulary already
  named "Manufacturer," with no textual basis for "Account" anywhere else in the prompt.
- The fix paired a rewritten, conditional example (no single fixed phrase to copy) with an
  independent, mechanical check comparing the generated entity against already-established project
  vocabulary — failing the operation outright on mismatch rather than trusting the better example
  alone. Covered by dedicated regression tests built from the exact observed failure.
- A separate, already-"safe" prompt using the generic placeholder "Widget" three times in one
  example set was independently flagged for recreating the same anchoring pattern — this time
  triggered by repetition of a generic noun, not the presence of a domain-specific one.
- The conclusion, stated directly afterward: "the reference model treats an example noun as a
  candidate answer, not just an illustration... bracketed placeholders can't be copied literally
  because they aren't real words, which removes the attractor risk entirely rather than just
  weakening it." (Commit `4185038`.)

# Evolution of Understanding

We believed a generic, deliberately fake noun in a worked example was inert — safe by
construction, because no reasonable reader would mistake "Widget" for an actual design decision.

The evidence said otherwise twice, from two different angles. First: a domain-flavored example
noun got echoed into unrelated output at a rate — three runs, three occurrences — that made it
clearly systematic, not a coincidence, and one run turned that echo into a full schema
derailment. Second, and this was the more surprising part: switching to a conventionally-safe
generic noun didn't remove the mechanism. It only removed the domain-specificity. The same
underlying anchoring pattern showed up again, on a word we'd chosen specifically because it was
supposed to be immune to this.

We stopped asking "is this noun domain-specific" and started asking "is this a real, copyable word
at all." A structural, bracketed placeholder — something that reads unambiguously as "fill in the
actual subject" rather than as a word the model might echo — closes the mechanism instead of just
moving it to a different word.

We now treat prompt examples as non-neutral by default. Any concrete noun sitting near an
instruction is a candidate for what the model outputs, independent of whether a human reader would
recognize it as "just an example." We reserve real words for the rare cases where the structure
itself — not the subject matter — genuinely can't be taught without one.

# Engineering Principle

Treat every concrete noun inside a prompt's example text as a candidate the model might copy into
its actual output, not as inert illustration. Prefer a structural placeholder — a bracketed token
that isn't a real word — over even a deliberately generic noun, since a generic noun is still a
word the model can anchor on.

# Why It Generalizes

This applies to any system prompting a language model with few-shot examples, not just ours. The
standard advice — "use a generic placeholder instead of your real domain vocabulary" — is
necessary but not sufficient. It correctly flags that domain-specific example nouns leak into
unrelated output. It doesn't go far enough, because the mechanism isn't "this noun happens to be
domain-specific" — it's "this is a real word sitting near an instruction, and the model can't
always tell illustration from instruction." Any team writing prompts with worked examples — code
generation, structured data extraction, classification tasks with example categories — is exposed
to this the moment an example uses an actual word instead of a structural placeholder, however
generic that word was chosen to be.

# Remaining Questions

We don't know how sensitive this is to repetition count specifically — the "Widget" case that got
flagged used the same noun three times in one example set. Would once or twice have been fine, or
was the risk already there and just not yet observed at lower repetition? We also haven't tested
this across model families and sizes — a larger frontier model might separate illustration from
instruction more reliably, which would change how urgently this matters depending on what model a
given system targets. A direct test — same prompt, varying only the placeholder style across a
fixed set of unrelated inputs — would give us a cleaner measurement than the two consistent but
anecdotal incidents we have so far.
