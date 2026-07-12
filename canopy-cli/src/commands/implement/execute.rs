use crate::fix_loop::{dispatch_fix_tool_call, run_fix_loop_logged, run_red_test_sanity_check, RedSanityOutcome};
use crate::project_scan::{
    compile_command_for_service, ensure_npm_installed, scan_service_source_files,
    test_class_command_for_service, test_command_for_service,
};
use crate::roots;
use crate::tdd::{derive_test_file_path, is_tdd_candidate, is_test_file, test_class_name};
use crate::ui::{format_elapsed, print_step_notes, Progress, StepMeta};
use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_core::{Adr, IntentSpec, ServicesRegistry, StepStatus, StoryPlan, UserStory};
use canopy_llm::{
    execute_implementation_step, execute_implementation_stub, execute_implementation_stub_with_tools,
    execute_implementation_with_test, execute_implementation_with_test_and_tools, find_symbol_tool_spec,
    generate_unit_test_stub, generate_unit_test_stub_with_tools, read_file_tool_spec,
    skills_for_architecture, StepResult,
};
use canopy_storage::save_story_plan;
use std::collections::HashMap;

/// Prints the stop notice used whenever a fix loop exhausts its iterations without getting
/// the build green. Stopping here — rather than marking the step done and moving on — matters
/// because every later step's compile/test check assumes everything before it already works;
/// continuing on a broken foundation only compounds errors and burns more LLM calls chasing them.
///
/// Takes `progress` BY VALUE (not `&Progress`) so it can be dropped before printing the closing
/// notice. indicatif's `MultiProgress::println` always renders above every bar it still tracks,
/// no matter when during the run it's called — the only way to make this notice appear genuinely
/// BELOW the finished tree is to let indicatif release the terminal region first (by dropping the
/// whole `Progress`) and then use a plain `println!`, not `Progress::println`.
fn report_broken_build(progress: Progress, idx: usize, step_id: &str, file: &str, story_id: &str) {
    progress.failed(idx);
    // Freeze whatever's still pending/previewed/headered before dropping — otherwise every
    // step that hadn't been reached yet vanishes with no trace the moment `progress` drops.
    progress.freeze();
    drop(progress);
    println!("\n  ✗ Build is broken after step {step_id} ({file}) — stopping so errors don't compound.");
    println!("  Fix the errors above, then re-run `canopy implement {story_id}` to continue.");
}

/// Every earlier "done" step's own file and (if it's a TDD candidate) its test file, in both
/// project-root-relative and canonicalized-absolute form. A fix loop running during a LATER
/// step's processing must never edit a step that's already been verified and marked done, in
/// ANY phase — Red's compile check, Green's own compile check, or Green's test run all need this
/// same protection. Each call site may still append its own additional entries (e.g. the CURRENT
/// step's own test file, which Green phase must also protect once Red has generated it).
fn done_steps_skip_list(plan: &StoryPlan) -> Vec<String> {
    let mut skip = Vec::new();
    for s in plan.steps.iter().filter(|s| s.status == StepStatus::Done) {
        skip.push(s.file.clone());
        if let Ok(abs) = std::fs::canonicalize(&s.file) {
            skip.push(abs.to_string_lossy().to_string());
        }
        if is_tdd_candidate(&s.file) {
            if let Some(prev_test) = derive_test_file_path(&s.file) {
                skip.push(prev_test.clone());
                if let Ok(abs) = std::fs::canonicalize(&prev_test) {
                    skip.push(abs.to_string_lossy().to_string());
                }
            }
        }
    }
    skip
}

pub(crate) fn format_roots_context(packet: &crate::roots::FeatureContextPacket) -> String {
    let mut parts = Vec::new();
    if !packet.symbols.is_empty() {
        let syms: Vec<String> = packet.symbols.iter()
            .map(|s| format!("  {} {} ({}:{})", s.kind, s.fqn, s.file, s.line))
            .collect();
        parts.push(format!("Symbols:\n{}", syms.join("\n")));
    }
    if !packet.facts.is_empty() {
        let facts = packet.facts.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n");
        parts.push(format!("Facts:\n{facts}"));
    }
    parts.join("\n")
}

/// Builds the sibling context section for an implementation step prompt.
///
/// Tries Roots symbol surfaces first (compact). Falls back to reading full file
/// content from session_written or disk when Roots is unavailable.
pub(crate) fn build_sibling_section(
    deps: &[String],
    service_dir: &str,
    session_written: &std::collections::HashMap<String, String>,
) -> String {
    if deps.is_empty() {
        return String::new();
    }
    let prefix = format!("{}/", service_dir);
    let rel_paths: Vec<String> = deps.iter()
        .filter_map(|d| d.strip_prefix(&prefix).map(|s| s.to_string()))
        .collect();

    if let Some(surface) = roots::get_ts_module_surface(&rel_paths, service_dir) {
        return surface;
    }

    let mut parts: Vec<String> = Vec::new();
    for dep in deps {
        let rel = dep.strip_prefix(&prefix).unwrap_or(dep.as_str());
        let content = session_written.get(dep)
            .cloned()
            .or_else(|| std::fs::read_to_string(dep).ok());
        if let Some(c) = content {
            parts.push(format!("// {}\n{}", rel, c));
        }
    }
    parts.join("\n\n")
}

/// Executes every pending step in `plan`, running the TDD Red/Green cycle for
/// TDD-candidate files and direct generation for everything else, then runs a
/// final integration test pass across all implementable services.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_steps(
    story_id: &str,
    debug: bool,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    services: &ServicesRegistry,
    adrs: &[Adr],
    service_packages: &HashMap<String, String>,
    fix_log_dir: &std::path::Path,
    mut plan: StoryPlan,
) -> Result<()> {
    // Load package constraints (written during plan/gate phase, also available on resume).
    let constraints_path = canopy_storage::storage_dir()
        .join(format!("stories/{}/pkg_constraints.yaml", story_id));
    let pkg_constraints_by_service: std::collections::HashMap<String, String> =
        std::fs::read_to_string(&constraints_path)
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or_default();

    let client = build_client("developer", debug)?;
    let total = plan.steps.len();
    let mut written = 0usize;
    let mut total_fix_iterations = 0usize;
    let run_start = std::time::Instant::now();
    let mut session_written: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    const MAX_FIX_ITERATIONS: usize = 5;

    roots::ensure_indexed();

    let step_metas: Vec<StepMeta> = plan.steps.iter().map(|s| {
        let service_name = s.service.rsplit('/').next().unwrap_or(&s.service).to_string();
        let is_frontend = services.services.iter()
            .find(|svc| svc.name == service_name || svc.name == s.service)
            .and_then(|svc| svc.component_type.as_deref()) == Some("frontend");
        StepMeta { file: s.file.clone(), group: service_name, is_frontend }
    }).collect();
    let progress = Progress::new(&step_metas);
    progress.println(format!(
        "Implementing {story_id}: As a {}, I want {}, so that {}.",
        story.as_a, story.want, story.so_that
    ));
    // A resumed plan already has some steps marked done from a prior run — freeze their
    // checklist lines immediately so the whole run's history is visible from the start,
    // not just the steps executed in this particular invocation.
    for (idx, step) in plan.steps.iter().enumerate() {
        if step.status == StepStatus::Done {
            progress.done(idx, "already done");
        }
    }

    // A pending "create" step's file should not exist yet — if it does, it's a leftover
    // Red-phase artifact from a prior run that was interrupted before this step reached
    // "done" (e.g. Ctrl-C, or a step earlier in the plan being reset for a re-run). Left in
    // place, it pollutes every whole-project compile check between now and when this step
    // is actually (re-)executed, surfacing as fix-loop noise attributed to the wrong step.
    for step in plan.steps.iter().filter(|s| s.status == StepStatus::Pending && s.operation == "create") {
        if std::path::Path::new(&step.file).exists() {
            progress.println(format!("  removing stale artifact (leftover from an interrupted run): {}", step.file));
            let _ = std::fs::remove_file(&step.file);
        }
        if is_tdd_candidate(&step.file) {
            if let Some(test_file) = derive_test_file_path(&step.file) {
                if std::path::Path::new(&test_file).exists() {
                    progress.println(format!("  removing stale artifact (leftover from an interrupted run): {}", test_file));
                    let _ = std::fs::remove_file(&test_file);
                }
            }
        }
    }

    for i in 0..total {
        if plan.steps[i].status != StepStatus::Pending { continue; }

        let step = &plan.steps[i];

        // If this plan step targets a test file that the TDD cycle already wrote, skip it.
        if is_test_file(&step.file) && std::path::Path::new(&step.file).exists() {
            progress.skipped(i, "already written by TDD cycle");
            plan.steps[i].status = StepStatus::Done;
            save_story_plan(story_id, &plan).context("failed to save plan progress")?;
            continue;
        }

        // Resolve the service entry and directory for this step.
        let step_service_name = step.service.rsplit('/').next().unwrap_or(&step.service).to_string();
        let step_service = services.services.iter()
            .find(|s| s.name == step_service_name || s.name == step.service);
        let step_tech = step_service.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
        let arch_skills = skills_for_architecture(adrs, step_tech);
        let step_service_dir = match step_service.and_then(|s| s.component_type.as_deref()) {
            Some("frontend") => format!("frontend/{}", step_service_name),
            _ => format!("services/{}", step_service_name),
        };

        if is_tdd_candidate(&step.file) {
            let test_file = match derive_test_file_path(&step.file) {
                Some(p) => p,
                None => {
                    progress.note(i, format!("cannot derive test path for {} — skipping TDD", step.file));
                    continue;
                }
            };
            let test_class = test_class_name(&test_file).unwrap_or_else(|| "Test".to_string());

            // ── RED PHASE ────────────────────────────────────────────────────────
            progress.phase(i, "TDD 🔴 — write failing test + compilable stub");
            // Build sibling context first — the test prompt needs the type surfaces
            // so it can produce test data with all required fields.
            let stub_siblings = build_sibling_section(&step.depends_on, &step_service_dir, &session_written);

            let is_ts_family = step.file.ends_with(".ts") || step.file.ends_with(".tsx");
            let StepResult { content: test_content, summary: test_summary, deviations: test_deviations } = progress.timed(
                i,
                format!("generating test    {test_file}"),
                &client,
                Some(&test_file),
                || {
                    if is_ts_family {
                        let tools = vec![read_file_tool_spec(), find_symbol_tool_spec()];
                        generate_unit_test_stub_with_tools(
                            &client, story, spec, contract_yaml, step, &test_file,
                            service_packages, services, adrs, &stub_siblings,
                            &tools,
                            |call| dispatch_fix_tool_call(call, &test_file, &step_service_dir, &progress, i, &client),
                        )
                    } else {
                        generate_unit_test_stub(
                            &client, story, spec, contract_yaml, step, &test_file,
                            service_packages, services, adrs, &stub_siblings,
                        )
                    }
                },
            ).with_context(|| format!("LLM call failed generating test for step {}", step.id))?;

            let test_dest = std::path::Path::new(&test_file);
            if let Some(parent) = test_dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(test_dest, &test_content)
                .with_context(|| format!("failed to write {}", test_file))?;
            progress.annotate_last_child(i, &format!("→ {test_file}"));
            print_step_notes(&progress, i, &test_summary, &test_deviations);
            let pkg_constraints = pkg_constraints_by_service.get(&step_service_name).map(|s| s.as_str());
            // Deterministically parsed from the test just written, not asked of the model —
            // see roots::find_test_call_shape's doc for why this exists (a self-check asking
            // the model to count the test's own call-site arguments has been observed, on a
            // real run, to disagree with this exact fact roughly half the time).
            let class_name = std::path::Path::new(&step.file)
                .file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let observed_call = roots::find_test_call_shape(&test_content, class_name);
            let StepResult { content: stub_content, summary: stub_summary, deviations: stub_deviations } = progress.timed(
                i,
                format!("generating stub    {}", step.file),
                &client,
                Some(&step.file),
                || {
                    if is_ts_family {
                        let tools = vec![read_file_tool_spec(), find_symbol_tool_spec()];
                        execute_implementation_stub_with_tools(
                            &client, story, spec, contract_yaml,
                            step, None, None,
                            service_packages, services, &stub_siblings, &arch_skills,
                            &test_file, &test_content, pkg_constraints,
                            observed_call.as_deref(),
                            &tools,
                            |call| dispatch_fix_tool_call(call, &step.file, &step_service_dir, &progress, i, &client),
                        )
                    } else {
                        execute_implementation_stub(
                            &client, story, spec, contract_yaml,
                            step, None, None,
                            service_packages, services, &stub_siblings, &arch_skills,
                            &test_file, &test_content, pkg_constraints,
                            observed_call.as_deref(),
                        )
                    }
                },
            ).with_context(|| format!("LLM call failed generating stub for step {}", step.id))?;

            let dest = std::path::Path::new(&step.file);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, &stub_content)
                .with_context(|| format!("failed to write {}", step.file))?;
            progress.annotate_last_child(i, &format!("→ {}", step.file));
            print_step_notes(&progress, i, &stub_summary, &stub_deviations);
            written += 1;
            session_written.insert(step.file.clone(), stub_content);

            if let Some(svc) = step_service {
                if std::path::Path::new(&step_service_dir).exists() {
                    ensure_npm_installed(&step_service_dir);
                    let compile_cmd = compile_command_for_service(svc, &step_service_dir);
                    progress.phase(i, &format!("TDD 🔴 — compile stub    $ {compile_cmd}"));
                    let src_files = scan_service_source_files(&step_service_dir);
                    // Red fix loop protects done-step impl files and test files from
                    // earlier TDD steps — but NOT the current step's test, which the
                    // fix loop is allowed to repair (e.g. wrong constructor pattern).
                    let red_skip = done_steps_skip_list(&plan);
                    let red = run_fix_loop_logged(&client, svc, &step_service_dir, &compile_cmd,
                        &src_files, &red_skip, adrs, &arch_skills, MAX_FIX_ITERATIONS,
                        Some(fix_log_dir), &format!("red-{}", step.file), &progress, i);
                    total_fix_iterations += red.iterations;
                    if !red.passed {
                        report_broken_build(progress, i, &step.id, &step.file, story_id);
                        return Ok(());
                    }

                    // tsc has no way to catch a test that compiles fine but crashes at Jest
                    // RUNTIME (e.g. jest.spyOn on an empty mock object) — it only surfaces once
                    // Jest actually runs the test, and by Green phase the test file is protected
                    // from edits. Catch it here instead, while the test is still freely
                    // editable. Scoped to non-component TS: a .tsx component's Red state
                    // ("renders null") has no single deterministic failure string to check.
                    if test_file.ends_with(".ts") && !test_file.ends_with(".tsx") {
                        progress.phase(i, "TDD 🔴 — sanity-check the test actually fails as expected");
                        let sane = run_red_test_sanity_check(
                            &client, &step_service_dir, &test_class_command_for_service(svc, &test_class),
                            &test_file, &src_files, step_tech, adrs, &arch_skills, MAX_FIX_ITERATIONS,
                            &progress, i,
                        );
                        match sane {
                            RedSanityOutcome::Broken => {
                                report_broken_build(progress, i, &step.id, &test_file, story_id);
                                return Ok(());
                            }
                            // Red's own compile check plus this sanity check already proved the
                            // stub compiles and passes — nothing left for Green to verify, and
                            // regenerating from scratch would only risk replacing a working
                            // answer with a fresh gamble. session_written already has the stub's
                            // content (inserted above, before the compile check ran).
                            RedSanityOutcome::AlreadyImplemented => {
                                progress.done(i, "done");
                                plan.steps[i].status = StepStatus::Done;
                                save_story_plan(story_id, &plan).context("failed to save plan progress")?;
                                roots::reindex();
                                continue;
                            }
                            RedSanityOutcome::ExpectedRed => {}
                        }
                    }
                }
            }
            roots::reindex();

            // ── GREEN PHASE ──────────────────────────────────────────────────────
            progress.phase(i, "TDD 🟢 — implement to pass the test");
            let roots_context = roots::get_feature_context(&step.description)
                .map(|p| format_roots_context(&p))
                .filter(|s| !s.is_empty());

            // Re-read test in case the Red fix loop modified it.
            let test_content = std::fs::read_to_string(&test_file)
                .unwrap_or(test_content);
            // Re-read the stub too — Green phase was generating "from scratch" with no visibility
            // into the Red-phase stub it's replacing, even though that stub's signature was
            // already grounded by observed_call. Confirmed on a real run: Green regenerated
            // registerProduct(productData: Omit<Product, ...>) as five positional parameters,
            // matching the entity schema's field count instead of the test's actual one-argument
            // call — the same arity bug observed_call fixed for stubs, reappearing one phase
            // later because neither the fact nor the stub's own content ever reached this call.
            let stub_content_for_green = std::fs::read_to_string(&step.file).ok();

            let green_siblings = build_sibling_section(&step.depends_on, &step_service_dir, &session_written);
            let StepResult { content: impl_content, summary: impl_summary, deviations: impl_deviations } = progress.timed(
                i,
                format!("implementing      {}", step.file),
                &client,
                Some(&step.file),
                || {
                    if is_ts_family {
                        let tools = vec![read_file_tool_spec(), find_symbol_tool_spec()];
                        execute_implementation_with_test_and_tools(
                            &client, story, spec, contract_yaml,
                            step, stub_content_for_green.as_deref(), roots_context.as_deref(),
                            service_packages, services, &green_siblings, &arch_skills,
                            &test_file, &test_content, pkg_constraints,
                            observed_call.as_deref(),
                            &tools,
                            |call| dispatch_fix_tool_call(call, &step.file, &step_service_dir, &progress, i, &client),
                        )
                    } else {
                        execute_implementation_with_test(
                            &client, story, spec, contract_yaml,
                            step, stub_content_for_green.as_deref(), roots_context.as_deref(),
                            service_packages, services, &green_siblings, &arch_skills,
                            &test_file, &test_content, pkg_constraints,
                            observed_call.as_deref(),
                        )
                    }
                },
            ).with_context(|| format!("LLM call failed for Green phase step {}", step.id))?;

            std::fs::write(dest, &impl_content)
                .with_context(|| format!("failed to write {}", step.file))?;
            progress.annotate_last_child(i, &format!("→ {}", step.file));
            print_step_notes(&progress, i, &impl_summary, &impl_deviations);
            session_written.insert(step.file.clone(), impl_content);

            if let Some(svc) = step_service {
                if std::path::Path::new(&step_service_dir).exists() {
                    ensure_npm_installed(&step_service_dir);
                    let test_cmd = test_class_command_for_service(svc, &test_class);
                    progress.phase(i, &format!("TDD 🟢 — run tests      $ {test_cmd}"));
                    let src_files = scan_service_source_files(&step_service_dir);
                    let abs_test = std::fs::canonicalize(&test_file)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or(test_file.clone());
                    // Protect every earlier done step's file/test in ADDITION to this step's own
                    // test — without this, a stack trace merely mentioning a done step's file
                    // (e.g. a test that triggers that file's own legitimate validation) gets
                    // misread as "this file is broken," and the fix loop can silently rewrite an
                    // already-verified step. Confirmed: a repository test calling a factory with
                    // deliberately-invalid input surfaced the factory's own file in the stack
                    // trace, and this fix loop tried (and got lucky not) to "fix" it.
                    let mut green_skip = done_steps_skip_list(&plan);
                    green_skip.push(test_file.clone());
                    green_skip.push(abs_test.clone());
                    let green = run_fix_loop_logged(&client, svc, &step_service_dir, &test_cmd,
                        &src_files, &green_skip, adrs, &arch_skills, MAX_FIX_ITERATIONS,
                        Some(fix_log_dir), &format!("green-{}", step.file), &progress, i);
                    total_fix_iterations += green.iterations;
                    if !green.passed {
                        report_broken_build(progress, i, &step.id, &step.file, story_id);
                        return Ok(());
                    }

                    // The test run above only proves this file's tests pass at Jest RUNTIME —
                    // ts-jest under this project's isolatedModules transpile mode does not
                    // necessarily diagnose real type errors (confirmed: an
                    // exactOptionalPropertyTypes violation passed 11/11 Jest tests cleanly but
                    // `tsc --noEmit` caught it immediately). Left unchecked, a step can be
                    // marked done with code that doesn't actually type-check, and by the time a
                    // LATER step's own compile check surfaces it, this file is already protected
                    // as a done-step file and permanently unfixable automatically. Run the same
                    // project-wide compile check Red phase already used, while this step's own
                    // file is still fixable — protect only earlier done steps' files and this
                    // step's OWN test (still off-limits per the Green-phase invariant), not
                    // step.file itself.
                    let compile_cmd = compile_command_for_service(svc, &step_service_dir);
                    progress.phase(i, &format!("TDD 🟢 — compile check   $ {compile_cmd}"));
                    let compile_check = run_fix_loop_logged(&client, svc, &step_service_dir, &compile_cmd,
                        &src_files, &green_skip, adrs, &arch_skills, MAX_FIX_ITERATIONS,
                        Some(fix_log_dir), &format!("green-compile-{}", step.file), &progress, i);
                    total_fix_iterations += compile_check.iterations;
                    if !compile_check.passed {
                        report_broken_build(progress, i, &step.id, &step.file, story_id);
                        return Ok(());
                    }
                }
            }
            roots::reindex();
        } else {
            // ── DIRECT IMPLEMENTATION (non-TDD candidates) ───────────────────────
            let current_content = if step.operation == "modify" {
                std::fs::read_to_string(&step.file).ok()
            } else {
                None
            };
            let roots_context = roots::get_feature_context(&step.description)
                .map(|p| format_roots_context(&p))
                .filter(|s| !s.is_empty());

            progress.phase(i, "generating");
            let step_siblings = build_sibling_section(&step.depends_on, &step_service_dir, &session_written);
            let pkg_constraints = pkg_constraints_by_service.get(&step_service_name).map(|s| s.as_str());
            let StepResult { content, summary, deviations } = progress.timed(
                i,
                format!("generating  {}", step.file),
                &client,
                Some(&step.file),
                || execute_implementation_step(
                    &client, story, spec, contract_yaml,
                    step, current_content.as_deref(), roots_context.as_deref(),
                    service_packages, services, &step_siblings, &arch_skills, pkg_constraints,
                ),
            ).with_context(|| format!("LLM call failed for step {}", step.id))?;

            let dest = std::path::Path::new(&step.file);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, &content)
                .with_context(|| format!("failed to write {}", step.file))?;
            progress.annotate_last_child(i, &format!("→ {}", step.file));
            print_step_notes(&progress, i, &summary, &deviations);
            written += 1;
            session_written.insert(step.file.clone(), content);

            // Compile check after each direct step — catch errors before the next
            // step inherits them. Only allow fixing the newly written file.
            if let Some(svc) = step_service {
                if std::path::Path::new(&step_service_dir).exists() {
                    ensure_npm_installed(&step_service_dir);
                    let compile_cmd = compile_command_for_service(svc, &step_service_dir);
                    progress.phase(i, &format!("compile   $ {compile_cmd}"));
                    let src_files = scan_service_source_files(&step_service_dir);
                    let direct_skip = done_steps_skip_list(&plan);
                    let direct = run_fix_loop_logged(&client, svc, &step_service_dir, &compile_cmd,
                        &src_files, &direct_skip, adrs, &arch_skills, MAX_FIX_ITERATIONS,
                        Some(fix_log_dir), &format!("direct-{}", step.file), &progress, i);
                    total_fix_iterations += direct.iterations;
                    if !direct.passed {
                        report_broken_build(progress, i, &step.id, &step.file, story_id);
                        return Ok(());
                    }
                }
            }

            roots::reindex();
        }

        progress.done(i, "done");
        plan.steps[i].status = StepStatus::Done;
        save_story_plan(story_id, &plan).context("failed to save plan progress")?;
    }

    // Drop `progress` before printing the closing summary — see report_broken_build's doc for
    // why this is the only way to make the message land below the tree instead of above it.
    progress.freeze();
    drop(progress);
    println!(
        "\n{written} file(s) written, {total_fix_iterations} fix-loop iteration(s), {} total.",
        format_elapsed(run_start.elapsed())
    );

    // Final integration test pass — catches e2e tests and cross-service interaction issues.
    let implementable: Vec<_> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .filter(|s| s.technology.is_some())
        .collect();

    // Each service is its own group of exactly one "step" — reusing the same grouped
    // checklist the main loop uses gives the final pass the same live-bar/collapse treatment,
    // and the header still makes backend vs frontend obvious at a glance.
    let final_metas: Vec<StepMeta> = implementable.iter().map(|s| StepMeta {
        file: "no regressions".to_string(),
        group: s.name.clone(),
        is_frontend: s.component_type.as_deref() == Some("frontend"),
    }).collect();
    let final_progress = Progress::new(&final_metas);
    for (i, service) in implementable.iter().enumerate() {
        let service_dir = match service.component_type.as_deref().unwrap_or("service") {
            "frontend" => format!("frontend/{}", service.name),
            _ => format!("services/{}", service.name),
        };
        if !std::path::Path::new(&service_dir).exists() {
            final_progress.skipped(i, "not scaffolded");
            continue;
        }

        if !std::path::Path::new(&format!("{service_dir}/node_modules")).exists()
            && std::path::Path::new(&format!("{service_dir}/package.json")).exists()
        {
            final_progress.println(format!("  running npm install in {service_dir}..."));
            let _ = crate::shell::npm_install(&service_dir, &[], false);
        }

        let svc_tech = service.technology.as_deref().unwrap_or("unknown");
        let arch_skills = skills_for_architecture(adrs, svc_tech);
        let test_cmd = test_command_for_service(service, &service_dir);
        let service_source_files = scan_service_source_files(&service_dir);
        // Protect test files in the final pass — they are specs, not targets to simplify.
        // Suffix-based: TS/TSX tests are co-located next to source, not under a separate dir.
        let test_files: Vec<String> = service_source_files.iter()
            .filter(|f| is_test_file(f))
            .cloned()
            .collect();
        let abs_test_files: Vec<String> = test_files.iter()
            .filter_map(|f| std::fs::canonicalize(f).ok()
                .map(|p| p.to_string_lossy().to_string()))
            .collect();
        let skip_tests: Vec<String> = test_files.into_iter()
            .chain(abs_test_files.into_iter())
            .collect();
        final_progress.phase(i, &format!("Verifying — no regressions    $ {test_cmd}"));
        let final_result = run_fix_loop_logged(&client, service, &service_dir, &test_cmd,
            &service_source_files, &skip_tests, adrs, &arch_skills, MAX_FIX_ITERATIONS,
            Some(fix_log_dir), &format!("final-{}", service.name), &final_progress, i);
        if final_result.passed {
            final_progress.done(i, "no regressions");
        } else {
            final_progress.failed(i);
            eprintln!("  ✗ Final validation failed for '{}' — fix the errors above.", service.name);
        }
    }

    Ok(())
}
