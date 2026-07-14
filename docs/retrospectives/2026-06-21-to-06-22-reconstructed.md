# 2026-06-21 to 2026-06-22 — The scaffold-generation rollback

**source: reconstructed** — written 2026-07-14 from commit history only (24 commits across these
two days). No first-hand session record exists for this period. Inferred entirely from commit
messages and diffs, cross-referenced with `git show` on the key pivot commit — nothing here is
invented beyond what those artifacts support.

---

# What Changed

The planning phase shipped in full (`1b68f87`): domain registry, intent specs, implementation
plans, a scaffold command, an interactive REPL, and integration with Roots (a separate
repository-intelligence tool, merged into the same monorepo the same day, `fb9ba3b`). The
architecture schema itself became schema-free (`8d53abd`, `e900514`) — a deliberate continuation of
day 0's pivot away from rigid typed fields.

Scaffolding — turning a chosen architecture into an actual runnable project skeleton — was built as
another LLM-driven step: given the architecture, the model would generate the shell commands to run.
Over the following two days, at least 13 separate commits fixed scaffold-generation bugs: an invented
non-existent Vite template, wrong flag ordering, an invented `npm init` scaffolder that doesn't exist,
incompatible Angular CLI invocations, wrong Maven archetype versions, incorrect Spring Boot
initializer parameters, and more.

That churn ended in `1c759bf`: "Scaffold commands are fully deterministic given component type — the
LLM added latency, cost, and hallucination risk. Replace with static templates." LLM-based scaffold
generation was removed and replaced with static, code-driven templates keyed on component type
(`arch_needs_jvm`, `vite_template_for`).

# What We Learned

Two different LLM roles got resolved in opposite directions in the same short window. Deriving
*architecture* from an intent — a task with genuine, story-specific ambiguity — stayed LLM-driven
and even got looser (schema-free) rather than more constrained. Turning an already-decided
architecture into shell *commands* — a mapping with no real ambiguity once the component type is
known — got pulled out of the LLM entirely.

The distinguishing factor wasn't "is this task hard" — architecture derivation is arguably harder —
it was "is the mapping from input to output actually enumerable." Scaffold commands for a given
component type are the same every time; there was no reason to pay an LLM's latency, cost, and
hallucination risk for a lookup table.

# What Surprised Us

The volume of hallucination was higher and more varied than a single bad example would suggest —
non-existent npm packages, invalid CLI flags, wrong version numbers, incompatible template names,
spread across at least 5 different scaffold targets (Vite, Angular, Spring Boot, Node/Express). No
single fix addressed the pattern; each fix addressed one specific hallucinated command, and the
volume of near-identical patch commits is itself the evidence that the *approach*, not any single
prompt wording, was the problem.

# What We Believe Now

*(Reconstructed inference from what `1c759bf` states directly, not a verbatim broader belief
statement from the commits.)* Use the model where the mapping from input to output is genuinely
underdetermined and needs judgment. Once a mapping becomes fully deterministic given already-decided
inputs, replace generation with code — not because the model can't do it, but because there's no
value in accepting hallucination risk for something enumerable in advance.

# Possible Next Steps

*(Inferred from what the evidence suggests remained unresolved, not stated directly.)* Even after
the static-template rewrite, residual scaffold bugs surfaced (`9dc84b7`) — the deterministic
templates still needed their own debugging, suggesting "replace with code" traded one class of bug
(hallucination) for another (template edge cases), which is a reasonable trade but not a free one.
Whether the same enumerable-mapping test should be applied more broadly elsewhere in the pipeline is
not addressed in this period's commits.
