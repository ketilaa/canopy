# A class index from the project's real classpath

Status: built and in the gate (2026-10-11, `5e16ea0`): `check_submission` takes the index and refuses before the build; `run_step` builds it once per run (`--no-classpath` turns it off). Replay and sweep results below. Question it answers: when the model uses or imports a library
class, can the harness check it against what the project can actually load, and say the exact fix?

## Why

A class the model has to place in a package it cannot look up is a recurring Implementer failure, and the
harness has no way to answer it for library classes:

- `LocalServerPort` imported from `org.springframework.boot.web.server` (it moved to
  `...boot.test.web.server` in Boot 3.4; the model's prior is older): a compile error, three attempts, a lost step.
- `javax.validation` against `jakarta.validation`, and `NotBlank`, `JpaRepository`, `@Entity` used in a project
  that has neither library: each time a hand-written fact ("no validation library is available") was the fix.
- JUnit imported in a main source file (`org.junit.jupiter.api` does not exist at compile scope).

The import audit that took the hand-written plan to 10 of 10 knows JDK classes (a table of 14) and the project's
own classes. It cannot say anything about a library class. The same shape of check, refuse before the build and
name the exact line, extends to the classpath.

## What the probes showed (2026-10-11, the widgets baseline, Boot 3.4.5)

| Probe | Result |
|---|---|
| `mvn dependency:build-classpath` | works; 10.7 s cold (the plugin downloads once); 63 entries with the test scope |
| scan the jars | 11,148 top-level classes, 10,257 distinct simple names, 0.7 s in Python |
| `LocalServerPort` | exactly one candidate: `org.springframework.boot.test.web.server.LocalServerPort` |
| `Test` | one: `org.junit.jupiter.api.Test` |
| `Service` | two: `org.apache.catalina.Service` and `org.springframework.stereotype.Service` |
| `NotBlank`, `JpaRepository`, `Entity` | none: not on the classpath |
| ambiguity overall | 686 of 10,257 simple names in more than one package (6.7%), mostly internal classes |
| the JDK | `jimage list $JAVA_HOME/lib/modules` lists about 28,000 entries including nested; `java.util.List` and `java.awt.List` both exist |

So most lookups are unambiguous, a few need ranking, and an empty answer is itself information: the class is
not there. That last case makes the computed absence lines ("no database library", "no validation library")
special cases of one general check.

## The index

`simple name -> candidates`, each with the qualified name, where it comes from (`project`, a `compile` or
`test` jar, or `jdk`) and the artifact. Built from:

1. the project's own classes (what `java::class_index` already reads, plus classes earlier steps created);
2. the JDK, from `jimage`;
3. the build's resolved classpath, with the scope kept apart: compile and runtime classes for production code, plus
   test-scope classes for test code. A production file that imports a class only present at test scope is a
   finding in itself.

Cached by a hash of `pom.xml` (and the classpath file), so it is rebuilt when a dependency changes and not
otherwise. Only top-level public classes at first; nested classes (`Map.Entry`) are imported through their
outer class and are left out.

**No data is not absence.** Only when the classpath was resolved successfully may the harness say "not on the
classpath". With Maven missing or offline the index is empty and every check passes through, as today.

## Uses, in order of value over cost

1. **The import audit, extended.** In `check_submission`, beside the JDK and project checks:
   - a class used without an import, one candidate: refuse with the exact line (as now);
   - several candidates: refuse listing them, ranked (below);
   - an import that resolves to nothing but a class of the same simple name exists elsewhere: refuse and say
     which (the `LocalServerPort` case);
   - a class with no candidate at all: refuse with "X is not available on this project's classpath", which is
     what the hand-written absence facts said, now derived and per class;
   - production code importing a test-scope class: refuse.
2. **A lookup tool** for the model, `find_class(name)`, in the existing tool conversation, for when it wants to
   know before it writes. The refusal in 1 is the default and costs a tool call, not an attempt.
3. **Dependency management.** A catalog entry (see the open question) declares the classes it provides. A refusal
   for an absent class can then name the entry that would provide it ("`NotBlank` comes from
   `spring-boot-starter-validation`, which this project does not have"), and adding a dependency changes the index.
4. **Facts.** The computed absence lines can go once the audit says it per class.

## Ranking among several candidates (deterministic, in order)

1. a candidate already imported by the project's own sources, the most often;
2. a project class over a library class over a JDK class;
3. a package prefix the repository contract prefers (a decision the repository owns,
   `docs/principles/hints-belong-in-repository-metadata.md`);
4. otherwise, all candidates listed and none chosen.

## Where it lives

A new `canopy-implementer/src/classpath.rs` (the index and the jar scan, with `zip` or `unzip -Z1`), the Maven
call beside the other build commands in `build.rs`, and the check in `java.rs`/`gates.rs` where the import audit
already is. Language rules stay in `java.rs`. It does not use Roots: Roots indexes source and signatures, this
indexes what is compiled. (Method and field names, such as `getName()` on a record, are Roots' job, and need its
Java parser extended first.)

## How to know it works, before it is built into the gate

Replay it, as the import audit was replayed: take every stored submission, and for each recorded compile failure of
the kind "package does not exist" or "cannot find symbol: class X", ask what the index would have said.
Pre-registered:

- it names the exact fix (one candidate) in at least 80% of those failures;
- it refuses 0 submissions from steps that passed (no false positive), over the 591-file corpus the first audit used
  and the runs since;
- then a sweep on the widgets plan with the check in the gate, compared with the batches already recorded.

## Not covered

Methods and fields (Roots), Spring wiring, step shape, vulnerabilities and version freshness (the dependency
catalog and its scan), Gradle, multi-module builds, shaded jars. Each is a limit to state when it bites.

## Replay result (2026-10-11, `classpath_replay`, 105 stored widgets runs)

152 compile failures, 107 of them about a missing class or package (the other 45 are of other kinds, such as a
method that does not exist). The index and the existing audit together, run on the submission that failed:

| Failures that involve a library or JDK class (87) | Count | Share |
|---|---|---|
| explained (the right diagnosis) | 84 | 97% |
| of those, the exact import line | 50 | 57% |
| of those, no import fixes it: the class is not on the classpath, or is test scope | 32 | 37% |
| of those, several candidates and none chosen | 2 | 2% |
| not explained | 3 | 3% |

False positives: **0** refusals over the 661 files of steps that passed.

Against what was pre-registered. "No false positive": met. "The exact fix in at least 80%": **not met as
written** (57% had an exact import line). The target conflated two cases, and the split is the finding: in 37%
(`javax.validation.constraints.NotNull`, `org.junit.jupiter.api.Test` in a production file) no import line exists,
because the class is not available to that code; the index says so, where today the model burns attempts on a
compile error it cannot act on. Counting a correct diagnosis, 97%; counting only a ready-to-paste line, 57%.

The three not explained are classes the model uses but nobody declares: `NotNull`/`Size` used with no import and
no library to provide them, and two invented project classes (`WidgetRepositoryImpl`, `WidgetCreateRequest`). They
need a check for a type that is neither declared, imported nor known, which carries a false-positive risk and is
not built.

What the replay corrected in the first version: the Maven scope option was ignored (`includeScope`, not
`mdep.includeScope`), so test jars were on the compile classpath and no test-scope finding could fire; and a class
in an `internal` package (a Mockito matcher) was suggested for `NotNull`. Both fixed, with a test for the second.


## In the gate: a paired rerun (2026-10-11, `5e16ea0`)

Twelve stored plans (four batches: conv, val, routed3, jpa2), each rerun once with the check on, on the same
repository tag as its earlier run without it; one run per plan, so a direction.

| | Without the check | With it |
|---|---|---|
| steps passed, summed over the 12 plans | 15 | 17 |
| runs that stopped at the compile stage | 7 | 3 |
| runs that stopped at the test stage | 3 | 7 |
| classpath refusals | n/a | 6, in 3 runs |

What the refusals said, from the traces: a test-scope `org.junit.jupiter.api.Test` imported in a production file;
`Service` used without an import with two candidates (`org.apache.catalina.Service`,
`org.springframework.stereotype.Service`); `import org.springframework.boot.web.server.LocalServerPort` refused with
the right package named. Each cost a tool call and not an attempt.

Reading it: the compile-stage stops fell from 7 to 3 and became test-stage stops, which is the expected shape (the
code now compiles and fails on behaviour instead); steps passed barely moved, 15 against 17 over twelve single
runs, which is within noise. The check refused nothing wrongly in the replay and nothing visible here. It removes a
class of wasted attempt; it does not by itself raise how far a plan gets, because the next failure is the one
behind it.
