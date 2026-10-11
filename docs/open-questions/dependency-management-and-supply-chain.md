---
title: Dependency management, signatures from Roots, and the supply chain
status: open
origin: the Planner hardening sessions of 2026-10-10 (docs/reports/widgets-planner.md)
date_discovered: 2026-10-10
related_principles: [compute-facts-mechanically.md]
related_narratives: []
related_reports: [widgets-planner.md]
related_design_docs: [planner-first-increment.md, run-analysis-playbook.md, contract-routing.md, classpath-class-index.md]
---

# Question

The Planner and Implementer treat the project's dependencies as fixed: `pom.xml` is off limits, and the
facts tell the model what is missing. How should dependencies be chosen, added, kept current and checked,
and how should what earlier steps produced (class and method signatures) reach later steps?

# Why It Matters

Much of the Planner hardening was spent steering the model away from libraries the project does not have.
That works against the model's strongest priors, and it shapes the environment to the story instead of the
other way round. In the product, the stack is a human decision (an ADR); the harness should receive it, not
forbid its absence. The old pipeline had a dependency decision log and `propose_dependencies`; the new
harness dropped both.

# Evidence So Far

- The Implementer wrote JPA classes in 9 of 9 generated plans that reached an entity or repository step, and
  `javax.validation` imports in 2 of 7 runs, in a project with only the web and test starters. A computed
  "no database library" line and a computed "no validation library" line removed those classes (JPA mentions
  30 to 0, validation errors 2 of 7 runs to 0 of 3) without raising steps passed: the stop points moved.
- A probe compile showed `jakarta.validation` is also missing from that project, so the obvious fix ("use
  jakarta, not javax") would have been wrong. In Spring Boot 3 the namespace is `jakarta.*`, which the model's
  prior (older versions) does not favour even when the library is present.
- Failures from what earlier steps produced: a controller calling a method its service never defined, and
  `getName()` called on a record. `read_only` is computed from class names an instruction mentions, and the
  Implementer is shown whole files, not signatures.
- What Roots gives for Java today (probe 2026-10-11): constructors and methods with parameter lists. Missing:
  records and their components, return types, annotations. `roots facts` and `roots context` failed on a bare
  class name.
- The scaffold uses Spring Boot 3.4.5. Nothing scans dependencies for known vulnerabilities, and nothing
  keeps versions current.

# Options

1. **Signatures from Roots.** Extend `roots-parser`'s Java support (records and components, return types,
   annotations), then reindex after each step, shell out to `roots` (never link it), and give the next step the
   compact surface of the classes it depends on as a stated fact. A lookup tool for the rest, in the shape of
   `find_symbol`. Fall back to reading files when Roots is unavailable.
2. **Dependencies as a closed set.** A curated catalog of managed starters (no versions chosen by the model).
   At plan level, a `dependencies` field with an enum from the catalog and a derived first step that edits the
   pom deterministically; the facts are recomputed afterwards. Possibly an Implementer-side tool later, with the
   same catalog, if a step discovers a need mid-way (a tool cannot share a turn with the Planner's schema).
   Each decision is recorded like an ADR; unattended runs approve catalog entries only.
3. **Go with the model's grain.** Give the benchmark project the libraries models expect (data and validation
   starters) and state the namespace by Boot version, instead of forbidding them. Cheap to test: a new baseline
   tag, same sweep, compare steps passed and failure classes.
4. **Freshness and security.** Version policy (newest the model still knows well, versus newest), a scan for
   known vulnerabilities (a registry or advisory source, a build-plugin scan), and checks on new dependencies
   (name look-alikes, provenance, maintenance). Belongs with option 2, since the catalog is where versions
   and approvals live.

- A wrong-package import of a library class (`LocalServerPort` taken from the wrong package, 2026-10-11; earlier
  `javax` against `jakarta`). The Implementer's import audit knows JDK classes only. A class index built from the
  project's real classpath (the dependency jars the build resolves) would let the harness refuse an unknown
  import with the exact qualified name, the same shape as the audit that took the widgets plan to 10 of 10.
  Designed, with probe numbers: `docs/design/classpath-class-index.md`.

# What We Don't Know

- Whether going with the grain beats steering away from it: the experiment has not been run.
- Whether newer Boot versions cost model accuracy (training priors lag releases) against the security gain of
  staying current.
- How much of the remaining failure rate is dependency-related once the Implementer is shown signatures.
- Which second story needs a real dependency, to benchmark any of this; widgets needs none.

# The contract and ADRs

The repository contract (`.canopy/contract.yaml`, first version 2026-10-11) is meant to be a projection of
accepted ADRs, not a hand-edited file. The ADR is the record (decision, reason, alternatives, who accepted);
the contract is what the harness renders from the accepted ones, regenerable, with each entry citing its ADR
in `source`. A later ADR that supersedes an earlier one replaces its entry, and nothing is lost.

What stands in the way: today's ADR (`canopy-core::Adr`) is `title`, `decision`, `reason`, `alternatives`, all
free text, so there is nothing to render deterministically. An ADR proposal would need a typed `effects`
section in a schema-constrained response (a stereotype, a convention with a closed set of topics, a
dependency from the catalog) that the person accepts together with the prose. That is also where a dependency
decision (option 2 above) naturally lives: accepting it renders both a catalog entry and a contract line.

Until then the contract is a hand-written seed. Seeds are allowed and carry no `source`; the audit and the
`source` field are the seam where ADR rendering plugs in.

# Suggested Order (a proposal, not a decision)

Signatures first (measurable on widgets). Option 3 as a single cheap comparison with versions held fixed.
Dependency catalog and scanning designed together, once a second benchmark story exists. Do not bump the
scaffold's versions inside an experiment that is testing something else.

# Exit Criteria

Graduates to a design doc when option 1 or 2 starts as real work (mark `status: active`, link it here), and
to a principle or narrative when answered with a generalizable result.
