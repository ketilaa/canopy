---
title: "Stable, Repository-Specific Hints Belong in Repository Metadata, Not in the Prompt"
status: draft

confidence: medium

maturity: emerging

themes:
  - prompt-engineering
  - system-design
  - repository-context

evidence_strength: medium

source_artifacts:
  - "Planner hardening sessions 2026-10-10 and 2026-10-11, docs/reports/widgets-planner.md"
  - "commits 071aeb4 (storage line), 803171a (validation line), 699ca12 (stereotype line), ae0805f (namespace line), fcbdac5 (classes created by earlier steps)"
  - "docs/open-questions/dependency-management-and-supply-chain.md"

related_principles:
  - compute-facts-mechanically

cluster: "Compute, Don't Ask"
---

# Principle

When a hint is stable, specific to one repository, and true, do not accumulate it as prompt prose. Give it a
source. If it can be derived from the repository, compute it (`compute-facts-mechanically`). If it is a decision
a person made (a storage choice, a naming or package convention, a test style, a framework rule), keep it in
structured repository metadata (an ADR, a repository contract, a decisions file) that a person approves and the
harness only renders into the prompt. The agent stays general; the repository carries what is specific to it.

# Problem That Revealed It

Hardening the Planner against one story took a growing list of lines in the model's context, each added after
a run failed: "no database library is available", "no bean validation library is available", "Spring Boot 3
uses jakarta.*", "a class named *Service is @Service", "existing classes", "classes created by earlier
steps". None of them was wrong, and each removed the class of error it targeted. But six lines about one
project, written by the harness author in Rust, are a stack profile with no owner, no review, no way to
differ per repository, and no check that it is still true.

# Evidence

- Three of the six lines are derived from the repository (storage, validation, existing classes) and are
  recomputed on every run, so they cannot drift; they are the cheap case and already follow
  `compute-facts-mechanically`.
- Three are decisions or conventions no computation can supply (stereotypes by class name; "validate in code,
  not with annotations"; "keep data in memory"). They are hard-coded in `canopy-planner/src/facts.rs`. A
  different repository with a different convention would need a code change.
- The baseline experiment (`baseline-01-widgets-grain`) shows the same hint changing meaning with the
  repository: adding the libraries removed two lines automatically (derived) and left the convention question
  ("how does this project validate?") unanswered, and the model's next failure was exactly that.
- The project already has the homes for such decisions: ADRs, `services.yaml` (technology and
  responsibilities), contracts, and tech-stack skills ("a concise, authoritative rules block"), which
  `CLAUDE.md` says must not accumulate bug workarounds.

# Counter-Evidence

- Moving a hint out of the prompt does not change what the model sees; the metadata is rendered into the
  prompt. The gain is governance and testability, not model behaviour. Added context alone has not raised
  steps passed in these runs ("present is not consumed"), so the principle makes no claim about compliance.
- Metadata can go stale where a derived fact cannot. A contract that says "JPA" in a repository that dropped it
  is worse than no contract. A contract needs an audit against the repository (declared stereotypes against the
  annotations actually present) or it is a new source of drift.
- "Objectively true" is carrying weight. A hint that is only true for this story, or true by one person's
  taste, is a plan decision or a preference, not repository metadata.
- No experiment has compared a hard-coded line with a repository-owned one. The claim is a design direction
  supported by an accumulation of cases, not a measured effect.

# Applicability

- Storage, dependency, framework, naming, package and wiring rules, and the test style a stack needs.
- Not story-specific content, which belongs in the plan, and not what can be derived, which belongs in a
  computed fact.

# Confidence Assessment

Medium. Six instances in one project, one story and one model, all pointing the same way, and consistent with
the project's own structure (ADRs, skills, `services.yaml`). Low on effect: untested. It would move to high
with a comparison on a second repository where only the contract differs, and the same harness code.

# Future Validation

Build a minimal repository contract (the conventions above as data, approved once, checked by an audit against
the repository) and run the same sweep on two repositories with different contracts and no harness change.
Count how many hard-coded lines disappear from the code, and whether drift is caught by the audit.
