# Dogfooding repositories and tasks

The Implementer is run against small Java / Spring Boot 3.4 / Maven repositories under `dogfooding/` in
the canopy checkout. That directory is git-ignored by canopy. Each repository inside it is its own git
repository, hand-coded (not generated), and its tags are its starting states. A state is always
`<repo>@<tag>`.

Reset a repository to a state: `git checkout -q -f --detach <tag> && git clean -fdq` (no `-x`, so Maven's
`target/` survives and builds stay fast). The runner does this before every run.

## Repositories

| Repo | Tags | What it is |
|---|---|---|
| `java-greeter` | `baseline-00-scaffold`, `candidate-01-greeter` | The empty scaffold, and a `Greeter` with `GreeterTest` that the Implementer itself produced (a real run's patch, applied unchanged). |
| `java-naming` | `baseline-00-naming` | `Greeter` delegates how a name is shown to `format.NameFormatter`. `legacy.LegacyGreeter` looks similar, is marked do-not-change, and its test pins its exact output. |
| `java-shop` | `baseline-00-shop` | `PriceCalculator` computes an order total that `InvoiceFormatter` prints and `OrderService` charges; the invoice test pins the exact output. |
| `java-widgets` | `baseline-00-widgets`, `baseline-01-widgets-grain` | An empty Spring Boot web application (application class and a web starter). The base for a full vertical slice built as a plan: domain object, in-memory repository, service, REST controller. | `baseline-01-widgets-grain` is the same application with `spring-boot-starter-data-jpa`, `spring-boot-starter-validation` and `h2` added to the pom (Boot 3.4.5, versions managed by the BOM), so the libraries a model reaches for are available; it is for comparing a project that fits the model's habits with one that forbids them.

## Tasks

Run with `cargo run -p canopy-implementer --example run_step -- --task <name> --runs N`.

Runner options that matter for a measurement: `--url <base url>` (which llama-server the batch runs against; a
separate GPU-accelerated server gives the same pass rates and about 4x shorter steps), `--attempts N` (default 3),
and `--declare-imports` (the model lists its imports before its files; built, off by default, unswept). Every batch
writes `batch.json` with the harness commit and its dirty flag, the server's properties, the sampling and the
`mvn -version`. Run Maven on **Java 21**: on Java 25 Mockito cannot mock a class (Byte Buddy), and a model-written
`@MockBean` test then fails with an error the model cannot act on. Do not change the JDK, the plan or the harness
while a sweep is running: a batch that straddles a change is discarded.

| Task | State | Request | What it puts under pressure |
|---|---|---|---|
| `create` | `java-greeter@baseline-00-scaffold` | Create `Greeter` and `GreeterTest` (stated in full). | Whole-file generation in an empty repo. Single call, no tools. |
| `trim` | `java-greeter@candidate-01-greeter` | Trim the name; keep existing tests; add a test (stated in full). | Changing an existing class, shown its current files. Single call, no tools. |
| `nav` | `java-greeter@candidate-01-greeter` | "The Greeter should trim the name before returning the greeting in the greet method." | Finding and reading the files with `glob` and `read_file`. |
| `pasted` | `java-greeter@candidate-01-greeter` | The same change, with the line to change pasted into the request. | Temptation to submit without reading the file. |
| `hi` | `java-greeter@candidate-01-greeter` | "Greet with `Hi` instead of `Hello`." | A change that contradicts existing tests, which must be updated, not deleted. |
| `naming` | `java-naming@baseline-00-naming` | "Names should be trimmed before the greeting is built." | Indirection through a helper; a do-not-change look-alike with its own test; a null-handling trap in a careless fix. |
| `vat` | `java-shop@baseline-00-shop` | "Orders should include VAT at 25 percent: show it as its own line on the invoice, and make sure the total includes it." | A cross-file ripple (calculator, invoice, charge) that contradicts three existing tests. |

The planned tasks run a hand-written plan (`tasks/<name>/plan.yaml`) of small steps, one after another, each on top of the last, instead of one request:

| Task | State | Plan |
|---|---|---|
| `naming-planned` | `java-naming@baseline-00-naming` | 2 steps: change `NameFormatter`, then add a `Greeter` test. Same hidden test and scoring as `naming`. |
| `vat-planned` | `java-shop@baseline-00-shop` | 4 steps in expand-then-contract order: add the new calculator methods, switch the invoice, switch the charge, then switch the old total. Same hidden test and scoring as `vat`. |
| `widgets-planned` | `java-widgets@baseline-00-widgets` | 5 steps: domain record, in-memory repository, service, controller (201 path only), then 400 validation. Hidden test: the real application over HTTP. Plan v6; 10/10 with the import audit (see the model file for the path there). |

A plan is validated before any model run by applying a reference change step by step in a scratch copy: the build must be green after every step and the hidden test must pass at the end.

What a plan has to state (learned from the widgets plan, each item fixed a failure): the exact test inputs and
expected results (which names are invalid, and that a name with spaces is valid), how a test reaches what it
tests (constructors, how two entities get one id, the exact test setup line and request body), and a step small
enough that its answer fits the 2048-token completion cap (the controller was split in two). Hand-written plans
carry this knowledge today; a Planner will have to produce it.

### The step contract

The planner, not the Implementer, decides which files a step touches and where new classes go. A step lists:

- `modify`: existing files to change. Their content is shown to the Implementer up front.
- `create`: new files, at the exact paths the planner chose.
- `read_only`: existing files the step depends on, shown but not changeable.

The Implementer keeps its tools and may still explore, but it may only *write* files listed under `modify` or
`create`, and must create every `create` file. A submission with any other file, without a required new
file, or that leaves a `modify` file identical to what is on disk (or omits it), is refused with the reason: `modify`
means the planner decided the file has to change, and a file that only needs to be looked at is `read_only`. Files shown up front count as read. A step with none of these lists is
unscoped and behaves as before.

Before any step runs, the orchestrator checks the plan against the repository as it is: every `modify` and
`read_only` file exists (or an earlier step creates it), and no `create` file exists. A plan that fails is
not run; the problems are in the trace. This is the deterministic half of what a plan reviewer ("sceptic")
would check.

`nav`, `pasted`, `hi`, `naming` and `vat` use tools. Every request is a plain sentence with no file list.

### Gates and audits on a submission

In order, before anything is written: unsafe paths, files read before change, scope and `modify`/`create` contract, an empty or unchanged submission, the **import audit** (a used class from another package without its import is refused with the exact line to add), and the no-test gate (changed production code needs a changed test that mentions it, or is reached through a covered changed class; a refactor step is exempt). A refusal inside the tool conversation costs a tool call, not an attempt. After a failed build the workspace is restored and the retry text says the previous answer was undone.

## Scoring

A run is scored as a pass only when all of these hold:

1. The loop finished successfully (the model's own tests and the whole suite pass).
2. A **hidden acceptance test** passes. It is copied into the repository after the run and never shown to
   the model. It only uses API that exists in the starting state.
3. Every test method that existed in the starting state still exists by name (`preserved_tests`).
4. Every file in `untouched` is unchanged (a do-not-change trap).
5. At least `min_tests_added` new test methods were added. Updating an existing test does not count.

Hidden tests live in `canopy-implementer/tasks/<task>/`, outside every dogfooding repository, so no tool
the model has can reach them.

## Adding a repository or a task

1. Hand-code a small repository, make `mvn test` pass, commit, and tag it `baseline-NN-<name>`.
2. Write the hidden acceptance test. **It must fail in the starting state.** If it passes without a
   change, a pass proves nothing.
3. **It must be passable.** Apply a correct change in a scratch copy of the starting state and confirm the
   hidden test passes. Discard that solution; it is never committed anywhere the model can read.
4. Prefer hidden assertions that tolerate reasonable variation (for example, a line that starts with `VAT`
   and contains the amount) over exact output the request does not specify.
5. Add the task to `examples/run_step.rs` and record its results in the model file.
