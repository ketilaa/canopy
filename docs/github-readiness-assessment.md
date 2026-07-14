# GitHub Readiness Assessment

Prepared 2026-07-14. Read-only review — no repository changes were made as part of this
assessment. All findings below are grounded in direct inspection: a full-history secret scan
(`git log --all -p` across all 255 commits, plus a working-tree pattern scan), a file-by-file
hygiene check, and direct reads of the repository's actual documentation, licensing, and
knowledge-capture assets. Where a finding is a judgment call rather than a verified fact, it's
labeled as such.

---

## 1. Executive Summary

Canopy is closer to GitHub-ready on the security and hygiene axis than on the documentation and
open-source-process axis. The engineering content itself — retrospectives, principles, blog
drafts, narratives — is unusually rich for a project this young, and is the strongest asset this
repository has going for a public launch. But right now, a stranger cloning this repo would find:
no README, no LICENSE, no way to build without already knowing CLAUDE.md exists and reading all
46KB of it, and a git history whose every commit carries a real corporate email address. None of
this is a security emergency — no secrets, keys, or credentials were found anywhere in the full
commit history. It's a readiness gap: the repository currently assumes its only reader is an AI
coding agent working alongside you, not a stranger evaluating whether to trust or use the project.

## 2. Top Risks

Ranked by how hard each is to fix *after* the repository goes public:

1. **No LICENSE file, and no `license` field in any Cargo.toml.** Right now, nobody who clones
   this repository has any explicit legal right to use, modify, or redistribute it — default
   copyright applies. This is trivial to fix before publication and very awkward to fix
   *retroactively* once external forks or clones exist. **Fix before any public push, not after.**
2. **Every commit's author identity is a real corporate email**
   (`ketil.aasarod@no.experis.com`), across all 255 commits, dating back to day one. This becomes
   permanently public and hard to fully scrub once anyone has cloned or forked the repo — GitHub's
   own email-privacy features only protect *future* commits made through their web UI, not
   history that already exists. **This needs a deliberate decision before the first push, not a
   cleanup after.**
3. **One personal-project-name leak**: `docs/retrospectives/2026-07-09.md` named the dogfooding
   test project by its actual name directly, violating a convention established later in the
   project's own history and applied consistently everywhere else. Low severity (it's a project
   name, not a secret), but easy to miss in a skim and easy to fix.
4. **No documentation written for a human reader.** CLAUDE.md is comprehensive, but it is
   explicitly operating instructions for an AI coding agent — it opens with a "Core Design
   Insight" table and workflow syntax, not "here's what this is and why it exists." A strong
   engineer's first five minutes on this repo currently have no good landing page.

None of these are "someone will get hacked" risks. They're "this project will be judged, correctly
or not, within the first five minutes of someone looking at it" risks.

## 3. Quick Wins

Things fixable in under an hour each, with outsized effect on first impressions:

- Add a LICENSE file (needs your decision on which license — see Section 7).
- Reword the dogfooding-project-name mention in `docs/retrospectives/2026-07-09.md` to match the
  established "a dogfooding project" convention.
- Add `description`, `license`, and `repository` fields to the workspace `Cargo.toml` and each
  member crate's `Cargo.toml`.
- Write a genuine top-level `README.md` — see Section 3 (Documentation Readiness) for what it
  needs to contain; this is the single highest-leverage document in the whole readiness effort.
- Add `.DS_Store`, `*.rs.bk`, and other common OS/editor junk patterns to `.gitignore` defensively
  (none have been committed so far, but nothing currently prevents it).

## 4. Detailed Findings

### 4.1 Secrets and Sensitive Information

**Scanned:** working tree (pattern search for API key shapes, AWS keys, GitHub tokens, private key
markers, password/secret assignments) and the *full* git history (`git log --all -p`, all 255
commits, no filtering by branch or date) for the same patterns, plus a separate search for
hardcoded personal file-system paths and email addresses.

**Found:**
- No API keys, tokens, passwords, or private key material anywhere in the working tree or the
  full commit history. `ANTHROPIC_API_KEY` is referenced only as an environment-variable name
  Canopy expects to be set externally — never as a hardcoded value.
- No `.env`, credentials file, `.pem`, `.key`, or similar file has ever been added to the
  repository, at any point in its history.
- No hardcoded `/Users/<name>/...` or `/home/<name>/...` paths in any tracked file.
- No email addresses in tracked documentation content, except the one intentional
  `noreply@anthropic.com` co-author trailer convention already in use for commit messages.
- One personal-project-name mention (the dogfooding project's actual name, in
  `docs/retrospectives/2026-07-09.md`) — see Top Risks #3.
- **Git authorship**: every one of the 255 commits carries `Ketil Aasarød
  <ketil.aasarod@no.experis.com>` as author. Not a leak in the traditional sense (you already
  know your own email), but a decision point: is a corporate work email something you want
  permanently attached to a public repository's entire history? If not, this needs a `git
  filter-repo` (or equivalent) history rewrite *before* the first public push — rewriting after
  the fact, once anyone has cloned, doesn't remove it from clones that already exist.

**Remediation:**
- No secret-removal work is needed — there's nothing to remove.
- Fix the one project-name mention with a one-line edit.
- Decide on commit-author identity before pushing (options: rewrite history with a different
  name/email or a GitHub-provided noreply address; or accept the current identity as a deliberate
  choice — either is legitimate, but it should be a decision, not a default).

### 4.2 Repository Hygiene

**Assessed:** `.gitignore` coverage, tracked file sizes, presence of generated/binary artifacts,
stale or abandoned files.

**Found:**
- `.gitignore` is minimal (4 entries: `/target`, `.claude/settings.local.json`,
  `.claude/worktrees/`, two hash-marker files) but *effective* for what it covers — `target/` is
  correctly untracked despite the on-disk build directory being multiple gigabytes.
- 162 tracked files total, all source/docs/config — no accidentally committed binaries, build
  artifacts, or generated files found. Largest tracked file is 56KB (a prompt-builder source
  file); nothing resembling a stray database dump, log file, or binary blob.
- `Cargo.lock` is tracked, which is correct practice for a workspace producing binaries (not a
  library-only crate), so no change needed there.
- No `.DS_Store`, editor swap files, or other OS/tool junk has ever been committed — but nothing
  in `.gitignore` currently prevents this from happening by accident going forward.
- A previously-deleted experimental command (`try-tools`, mentioned in CLAUDE.md's own history as
  removed "once it did its job") leaves no trace in the current tree — confirmed actually removed,
  not just undocumented dead code.
- No stale or abandoned prototype directories found at any level of the tree.

**Remediation:** Add a handful of defensive `.gitignore` entries (`.DS_Store`, `*.swp`, `*.bak`)
before publishing. Everything else in this category is already in good shape.

### 4.3 Documentation Readiness

**Assessed:** whether a new engineer, with no prior context, could learn what Canopy is, why it
exists, how it works, its current status, and how to run it — from what's actually in the
repository today.

**Found:** There is no `README.md` at all. `CLAUDE.md` (46KB) is the only document a newcomer
would find, and it is explicitly written as operating instructions for an AI coding agent working
inside the project — it opens with "Canopy is an AI software engineering system... NOT a
code-completion tool... NOT a chat interface... NOT a big-bang architecture generator," which is a
reasonable *identity statement* but gives no orientation on: what problem this solves, who it's
for, what stage of maturity it's at, or how to get it running for the first time. Practical setup
information *does* exist (local LLM backend setup via llama-server, environment variable
requirements, the REPL command list) but it's interleaved with agent-facing prompt-engineering
rules (Prompt House Style, tech-stack skill internals) that a newcomer doesn't need on day one and
would have to wade through to find.

`docs/design/behavior-first-planning.md` is a strong, detailed architecture document — but it
documents one specific pipeline redesign, assumes the reader already knows what came before it, and
isn't positioned as an entry point.

None of the 9 library crates (`canopy-core`, `canopy-storage`, `canopy-llm`, `roots-core`,
`roots-parser`, `roots-storage`, `roots-context`, plus the two CLI binaries) have a module-level
doc comment (`//!`) explaining their purpose — `cargo doc` would currently generate documentation
pages with no top-level description for any crate.

**Recommendations:**
- Write a `README.md` covering: what Canopy is (one paragraph, human-facing, not the CLAUDE.md
  identity statement verbatim), the problem it solves, current status/maturity (be explicit that
  this is an active research/engineering project, not a finished product — see Section 4.4), a
  quickstart (clone, build, run against a local model or Anthropic API), and pointers to
  `docs/design/`, `docs/narratives/`, and `docs/principles/` for anyone who wants the deeper story.
- Add a one-paragraph `//!` module doc to each library crate's `lib.rs` — cheap, and it's the
  difference between `cargo doc` being useless and being a real map of the codebase.
- Consider a short `docs/ARCHITECTURE.md` (or promote a trimmed version of the existing design doc)
  that a newcomer can read in 10 minutes, distinct from CLAUDE.md's agent-operating-instructions
  role.
- A contributor guide is premature until Section 7's open-source process questions are answered —
  don't write one yet.

### 4.4 Research vs. Product Clarity

**Assessed:** where the repository currently blurs experimental/research material with
product-level claims about what works.

**Found:** This is the area where a public reader's trust is most at stake, and where the
existing knowledge-capture assets (retrospectives, principles, narratives) are simultaneously the
project's greatest strength and its biggest disclosure risk if presented without framing.

The repository's own artifacts are *honest* — reproducibility sweeps report real incidence rates
(duplicate ADRs at "roughly 2 of 3 runs" before a fix, "1–2 of 6" policy questions still
occasionally mis-resolved after a fix), and several principle documents are explicitly marked
`confidence: medium` / `maturity: emerging` rather than inflated to "validated." That honesty is
valuable and should be preserved, not softened. But none of that framing currently reaches a
stranger, because none of it is summarized anywhere outside `docs/`. A reader who only sees
`CLAUDE.md`'s confident, prescriptive tone (which is appropriate for its actual audience — an AI
agent that needs firm instructions) with no counterbalancing "here's what's still experimental and
unproven" statement would reasonably read the project as more finished than the evidence in its own
`docs/principles/` and `docs/narratives/` actually supports.

Concretely: `docs/design/behavior-first-planning.md` opens with "Status: Proposed — not yet
implemented" for the pipeline it describes, which is exactly the right pattern — but that pattern
isn't applied consistently. There's no single place stating, plainly, "here is what currently
works end-to-end, here is what's designed but not wired up yet (contract-driven implementation,
per `docs/narratives/the-road-to-contracts.md`), here is what's exploratory."

**Recommendations:**
- Add a "Project Status" section to the new README, stating plainly: what runs today
  (`intent`/`spec`/`behaviors` through Stage 4, scaffold, implement against the older
  ADR-driven planner), what's designed but not yet consumed by implementation
  (`contracts.yaml` wiring), and what's exploratory (reproducibility methodology itself, several
  `emerging`-maturity principles).
- Consider a `docs/STATUS.md` or a status table that gets updated as narratives/principles change
  maturity — this is cheap to maintain given the `maturity:`/`confidence:` front matter already
  exists on every principle document; it just needs to be surfaced somewhere a newcomer will see
  it.

### 4.5 Prompt and Model Assets

**Assessed:** whether design intent behind prompts/skills is understandable, whether house rules
are documented, whether prompt-engineering rationale is discoverable, and where relevant knowledge
is currently trapped in artifacts a newcomer wouldn't think to check.

**Found:** This is unusually well-documented *for an internal audience*. CLAUDE.md's "Prompt
House Style," "Tech-Stack Skills," and the extensive doc comments throughout
`canopy-llm/src/prompts/*.rs` and `canopy-llm/src/skills/*.rs` explain not just what each rule
does but why it exists, often citing the specific bug or reproducibility finding that motivated it.
The `canopy-prompt-reviewer` subagent definition itself documents the house rules as a checkable
process, not just a style preference.

The gap is discoverability for someone who doesn't already know this apparatus exists. A newcomer
reading `canopy-llm/src/skills/tech_stack.rs` cold would find well-reasoned, well-commented code —
but nothing that says "there is a documented review process for changes to this file, here's
where to find it" unless they happen to already be reading CLAUDE.md's Tech-Stack Skills section
in full.

Real prompt-engineering lessons currently live scattered across `docs/retrospectives/`,
`docs/principles/`, `docs/blog-drafts/`, and `docs/narratives/` — which is appropriate for their
different purposes (see Section 4.9), but none of it is cross-linked *from* the source code itself.
A doc comment in `canopy-llm/src/prompts/spec.rs` explaining a specific rule's origin doesn't
currently point to the fuller narrative in `docs/narratives/from-prompt-engineering-to-mechanical-
facts.md` that explains the general pattern it's an instance of.

**Recommendations:**
- Add a short "Prompt Engineering & Design Rationale" pointer section to the new README, directing
  readers to CLAUDE.md's Prompt House Style section, `docs/principles/`, and
  `docs/narratives/from-prompt-engineering-to-mechanical-facts.md` specifically — this is one of
  the project's strongest, most differentiated pieces of intellectual property and currently has
  no public entry point at all.
- Consider (not urgent) adding a one-line `// see docs/principles/<file>.md` comment at the most
  load-bearing rule sites (e.g., the Entity/Event Continuity checks, the enumeration-based
  checklists) — low cost, meaningfully improves discoverability for a code reader specifically.

### 4.6 Public-Facing Narrative

**"What is Canopy?"** — a draft framing, grounded specifically in what the evidence in this
repository actually supports, not aspirational language:

> Canopy is an experiment in AI-assisted software planning that treats specification and
> architecture as things that should emerge incrementally from described behavior, rather than be
> decided upfront in one large pass. It generates BDD-style specifications, extracts atomic
> behaviors, surfaces business decisions that need a human answer instead of letting a model guess
> at them, and produces per-component "contracts" meant to bound what an implementation step is
> allowed to touch. Canopy is also, deliberately, a record of its own development process — every
> reliability fix is backed by a reproducibility sweep, and the reasoning behind major design
> pivots is preserved rather than discarded once the pivot is made.

**What problem is it solving?** The gap between "an LLM can generate plausible code" and "an LLM
can be trusted to make architecture and business-rule decisions without a human noticing it did
so." Canopy's specific bet, evidenced repeatedly in its own history, is that this gap is closed
less by better prompts and more by mechanically computing whatever the system can determine on its
own, and reserving model judgment for genuine ambiguity — see
`docs/principles/compute-facts-mechanically.md` and
`docs/narratives/from-prompt-engineering-to-mechanical-facts.md`.

**What makes it different?** Two things the evidence actually supports claiming: (1) a deliberate
separation between planning and implementation, gated at every stage, rather than one-shot
generation; (2) an unusually rigorous internal practice of validating reliability fixes with
repeated, controlled reproducibility sweeps rather than single-run confirmation — itself
documented as a narrative (`docs/narratives/how-reproducibility-became-first-class.md`).

**What should NOT be claimed yet:**
- That contract-driven implementation works end-to-end — it doesn't yet; `canopy implement` still
  runs on the older ADR/architecture-skill-driven planner. Say "designed, not yet wired up," not
  "supported."
- That any given principle is broadly proven — several are explicitly `maturity: emerging` with
  a single supporting instance. Say so.
- That the reproducibility methodology generalizes beyond the specific stories it's been tested
  against (`manufacturer-001`, `product-001`) — it's evidenced on a small number of stories, not a
  large corpus.
- Anything implying production-readiness. This is, honestly and by its own internal evidence, a
  research-and-development-stage project with real, working pieces and real, open questions — that
  framing is more credible than overclaiming, and the project's own artifacts already model that
  honesty internally.

### 4.7 Open-Source Readiness

**Found:** No LICENSE file, no per-crate license metadata, no CONTRIBUTING.md, no `.github/`
directory (no issue templates, no PR template, no CI workflow definitions), no CODEOWNERS.

**Recommendations, scoped to *minimal* hygiene rather than a full OSS program:**
- **License**: this needs your decision, not mine — but note the deadline pressure is
  asymmetric: an unlicensed public repo technically grants no reuse rights, which will confuse or
  deter exactly the technically sophisticated readers you'd most want engaging with it. Pick
  before publishing, not after.
- **CONTRIBUTING.md**: premature to write in detail before deciding how open you want contribution
  to be initially (solo project accepting issues only vs. actively welcoming PRs) — a single
  paragraph stating current intent ("issues and discussion welcome; this is currently a
  single-maintainer research project, PR process TBD") is enough for launch.
- **Issue templates**: skip for an initial launch; add if/when issue volume justifies structure.
- **Code ownership**: not needed yet for a single-author repository — revisit if a second
  maintainer joins.
- **Release/versioning**: `Cargo.toml` versions are all `0.1.0` across every crate, which is
  honest and fine for a pre-1.0 research project — no change needed unless you want to start
  tagging releases, which isn't urgent.

### 4.8 Technical Credibility Review

**Likely questions from a strong engineer landing on this repo, and what currently answers them:**

- *"Does this actually work?"* — Partially answerable today: `docs/reports/manufacturer-001.md`
  and the retrospectives document real, live runs against a real dogfooding project. This evidence
  exists but currently has no visibility from a README or landing page.
- *"How reproducible is it?"* — Answerable, and this is a genuine strength: multiple 3-run
  reproducibility sweeps with quantified before/after incidence rates exist and are documented in
  `docs/narratives/how-reproducibility-became-first-class.md` and the underlying reports. Most AI
  coding tools do not publish this kind of self-scrutiny; surfacing it prominently would be a real
  differentiator.
- *"What evidence supports the design?"* — `docs/principles/` answers this directly, with explicit
  counter-evidence sections rather than one-sided advocacy — genuinely unusual and worth
  highlighting rather than hiding.
- *"How much of this is prompt engineering vs. real architecture?"* — Honestly, both, and the
  project's own narrative (`from-prompt-engineering-to-mechanical-facts.md`) is the best possible
  answer to this exact question — it documents the project's own shift from relying on the former
  toward the latter. This should be a featured pointer, not a buried file.

**Recommendation:** the single highest-leverage move for technical credibility is surfacing
`docs/narratives/`, `docs/principles/`, and `docs/reports/` from the README, with one sentence each
on what a skeptical reader will find there. Burying genuinely strong evidence in an undiscoverable
directory costs more credibility than having no evidence at all — a skeptical reader who doesn't
find it will assume it doesn't exist.

### 4.9 Knowledge Capture Assets

**Assessed:** what should stay internal, what should go public in this repository, and what should
eventually move to a separate site — reviewing all four artifact types together
(`retrospectives`, `principles`, `blog-drafts`, `narratives`).

**Recommendation by type:**
- **Retrospectives** (`docs/retrospectives/`, 11 files, including 6 explicitly marked `source:
  reconstructed`) — keep in the repository, public. These are the rawest, most honest record of
  the intellectual journey the user specifically wants preserved, and the "source: reconstructed"
  marking already models exactly the right honesty (distinguishing first-hand record from
  inference). No redaction found necessary beyond the one project-name fix already noted.
- **Engineering Principles** (`docs/principles/`, 9 files) — keep in the repository, public,
  unedited. These are already written for external generalization ("Why It Generalizes" /
  "Generalization" sections exist specifically for this), evidence-graded, and include honest
  counter-evidence. This is likely the single most reusable asset in the entire knowledge-capture
  system for an outside reader.
- **Blog Drafts** (`docs/blog-drafts/`, 5 files) — these are explicitly `status: draft` and were
  written to a house style, not yet reviewed for external publication tone the way the other three
  types were. Recommend a final editorial pass (removing internal-only phrasing, confirming no
  reference to anything that should stay private) before these specifically move to a public blog
  or Hugo site — treat these as the one category needing a genuine "are we ready to publish this
  externally" review pass, distinct from "is this safe to have in a public repo" (it already is
  safe; it's an editorial readiness question, not a security one).
- **Project Narratives** (`docs/narratives/`, 6 files, plus `docs/narrative-analysis.md`) — keep
  in the repository, public. `docs/narrative-analysis.md`'s own proposed reading order is a ready-
  made table of contents for exactly the kind of "how did the thinking evolve" story the user wants
  preserved and surfaced.

**What should move to a separate Hugo/blog site eventually (per the already-existing backlog
idea):** the *polished, human-edited* output derived from `blog-drafts/`, once through an
editorial pass — not the raw retrospectives/principles/narratives themselves, which are better
kept as primary-source repository documentation (versioned alongside the code they describe,
linkable by commit) than migrated elsewhere. The existing backlog note on this
(draft-generation-only, human-gated publishing) already reflects the right level of caution; no
change recommended to that plan.

## 5. Recommended GitHub Readiness Backlog

Ordered roughly by dependency (earlier items unblock or inform later ones), not strictly by
priority:

1. Decide on license; add LICENSE file and per-crate Cargo.toml metadata.
2. Decide on commit-author identity for public history; rewrite history before first push if a
   change is wanted (this is the one item that becomes materially harder after publication).
3. Fix the one dogfooding-project-name mention.
4. Add defensive `.gitignore` entries for OS/editor junk.
5. Write `README.md` (what Canopy is, problem solved, current status/maturity, quickstart, pointers
   to `docs/`).
6. Add a one-paragraph `//!` module doc to each library crate.
7. Add a "Project Status" section distinguishing what works today, what's designed-not-wired-up,
   and what's exploratory.
8. Add a one-paragraph CONTRIBUTING.md stating current contribution posture.
9. Editorial pass on `docs/blog-drafts/` specifically, if/when moving toward external publication
   of that content.
10. (Optional, not blocking) Cross-link key prompt/skill source files to the principle/narrative
    documents that motivated them.

## 6. Recommended Publication Sequence

**Stage 1 — Private Cleanup**
Goals: remove any remaining ambiguity before anything becomes visible.
Actions: items 1–4 above (license decision, authorship decision, project-name fix, gitignore).
Risks: the authorship decision is the one irreversible-after-the-fact item in this whole plan — get
it right here, not later.
Recommended order: license and authorship decisions first (they may take longer to think through
than to execute), then the mechanical fixes.

**Stage 2 — Documentation Hardening**
Goals: make the repository legible to a stranger.
Actions: items 5–8 above (README, crate docs, status section, CONTRIBUTING).
Risks: rushing the README's "what is Canopy" framing risks either overclaiming (credibility risk
once readers check the evidence themselves) or underselling genuinely strong work (the
reproducibility methodology and principles library). Use Section 4.6's draft framing as a starting
point, not a final answer — this deserves your own pass.
Recommended order: README first (everything else is discoverable from it once it exists), then
crate docs and status section in parallel, CONTRIBUTING last.

**Stage 3 — Public GitHub Launch**
Goals: make the repository visible.
Actions: push to GitHub, verify Stage 1/2 items actually landed as intended (re-check the pushed
repo, not just the local one), set repository visibility and basic settings (topics, description
matching the README's framing).
Risks: this is the point of no return for anything not caught in Stage 1 — treat Stage 1 as a hard
gate, not a suggestion.
Recommended order: do a final full re-run of this assessment's secret/hygiene scans against the
exact commit that will be pushed, immediately before pushing.

**Stage 4 — Blog / Knowledge Site Launch**
Goals: publish the editorially-reviewed subset of `docs/blog-drafts/` externally, per the existing
backlog plan.
Actions: editorial pass on blog drafts (item 9), Hugo/GitHub Pages setup (already scoped as a
later step in the existing knowledge-to-blog backlog note — don't build this until draft-only
output has proven useful over time, per that note's own reasoning).
Risks: none specific to readiness beyond what's already captured in the existing backlog plan.
Recommended order: after Stage 3, once the repository itself has had time to be seen and the blog
drafts' relevance can be reassessed against any public feedback.

**Stage 5 — Community Onboarding**
Goals: decide how open to make contribution, once there's evidence anyone external is interested.
Actions: revisit CONTRIBUTING.md with real specifics, consider issue templates if issue volume
justifies it, consider CODEOWNERS if a second maintainer joins.
Risks: building community process before there's a community is wasted effort — this stage should
be reactive to actual interest, not front-loaded.
Recommended order: deliberately last, and deliberately not planned in detail until Stage 3 has run
long enough to show whether there's real external interest to onboard.
