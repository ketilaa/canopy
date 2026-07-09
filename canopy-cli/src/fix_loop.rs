use crate::build_output::{
    errors_for_file, extract_error_files, extract_missing_packages,
    extract_pom_validation_errors, extract_unresolvable_dependencies, parse_ts_imports, strip_ansi,
};
use crate::project_scan::collect_files;
use crate::roots;
use crate::ui::{print_step_notes, Progress};
use canopy_core::{Adr, ServiceEntry};
use canopy_llm::{detect_layer, fix_file, skill_for_build_system, skill_for_technology, testing_skill_for_file_with_adrs, FixAttempt, LlmClient};

/// Outcome of one fix loop run — `iterations` feeds the run-level closing summary
/// (`execute_steps` sums it across every step) rather than just a pass/fail bool.
pub(crate) struct FixOutcome {
    pub(crate) passed: bool,
    pub(crate) iterations: usize,
}

/// Runs a build/test command and iterates an LLM fix loop until it succeeds or max_iterations is hit.
/// `skip_files` lists files that must not be modified (e.g. the unit test in the Green phase).
/// `adrs` is used to resolve the correct testing skill when fixing test files.
/// `progress`/`step_idx` anchor fix-attempt spinners under the calling step's checklist line.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_fix_loop_logged(
    client: &LlmClient,
    service: &ServiceEntry,
    service_dir: &str,
    build_cmd: &str,
    service_source_files: &[String],
    skip_files: &[String],
    adrs: &[Adr],
    arch_skills: &str,
    max_iterations: usize,
    fix_log_dir: Option<&std::path::Path>,
    step_label: &str,
    progress: &Progress,
    step_idx: usize,
) -> FixOutcome {
    let mut telemetry_iterations: Vec<String> = Vec::new();
    let mut total_iterations = 0usize;

    let passed = run_fix_loop_inner(
        client, service, service_dir, build_cmd, service_source_files,
        skip_files, adrs, arch_skills, max_iterations,
        &mut telemetry_iterations, &mut total_iterations,
        progress, step_idx,
    );

    if let Some(log_dir) = fix_log_dir {
        let label = step_label.rsplit('/').next().unwrap_or(step_label)
            .replace('.', "_");
        let log_path = log_dir.join(format!("{}-{}.yaml", service.name, label));
        let tech = service.technology.as_deref().unwrap_or("unknown");
        let passed_str = if passed { "true" } else { "false" };
        let iterations_yaml = if telemetry_iterations.is_empty() {
            "  - iteration: 1\n    errors: []\n    result: pass\n".to_string()
        } else {
            telemetry_iterations.join("")
        };
        let content = format!(
            "service: \"{}\"\ntechnology: \"{}\"\nstep: \"{}\"\ntotal_iterations: {}\npassed: {}\niterations:\n{}",
            service.name, tech, step_label, total_iterations, passed_str, iterations_yaml
        );
        let _ = std::fs::write(&log_path, content);
    }

    FixOutcome { passed, iterations: total_iterations }
}

#[allow(clippy::too_many_arguments)]
fn run_fix_loop_inner(
    client: &LlmClient,
    service: &ServiceEntry,
    service_dir: &str,
    build_cmd: &str,
    service_source_files: &[String],
    skip_files: &[String],
    adrs: &[Adr],
    arch_skills: &str,
    max_iterations: usize,
    telemetry: &mut Vec<String>,
    total_iterations: &mut usize,
    progress: &Progress,
    step_idx: usize,
) -> bool {
    let mut attempt_history: std::collections::HashMap<String, Vec<FixAttempt>> = std::collections::HashMap::new();
    for iteration in 0..max_iterations {
        let output = crate::shell::run_capture_in_dir("bash", build_cmd, service_dir);

        let output = match output {
            Ok(o) => o,
            Err(e) => { progress.println(format!("  failed to run command: {e}")); return false; }
        };

        let combined = strip_ansi(format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));

        if output.status.success() {
            progress.println(format!("  {} {}", crate::ui::green("✓"), service.name));
            return true;
        }

        let missing_pkgs = extract_missing_packages(&combined);
        let mut fixed_any = false;
        let broken_files: Vec<String> = extract_error_files(&combined, service_dir)
            .into_iter()
            .filter(|f| !skip_files.contains(f))
            .collect();

        if missing_pkgs.iter().any(|p| p.starts_with("javax.")) {
            let n = migrate_javax_to_jakarta(service_dir);
            if n > 0 {
                progress.println(format!("  migrated javax.* → jakarta.* in {n} file(s)"));
                fixed_any = true;
            }
        }

        let pom_validation = extract_pom_validation_errors(&combined);
        let unresolvable = extract_unresolvable_dependencies(&combined);
        let non_javax: Vec<_> = missing_pkgs.iter().filter(|p| !p.starts_with("javax.")).collect();
        if !pom_validation.is_empty() || !unresolvable.is_empty() || !non_javax.is_empty() {
            let build_file = format!("{service_dir}/pom.xml");
            if std::path::Path::new(&build_file).exists() {
                let content = std::fs::read_to_string(&build_file).unwrap_or_default();
                if !content.is_empty() {
                    let mut error_lines = Vec::new();
                    if !pom_validation.is_empty() {
                        error_lines.push(format!(
                            "Maven POM validation errors (artifact not in parent BOM — remove or replace with correct artifactId):\n{}",
                            pom_validation.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join("\n")
                        ));
                    }
                    if !unresolvable.is_empty() {
                        error_lines.push(format!(
                            "Unresolvable dependencies — do not exist on Maven Central. Remove from pom.xml:\n{}",
                            unresolvable.iter().map(|c| format!("  - {c}")).collect::<Vec<_>>().join("\n")
                        ));
                    }
                    if !non_javax.is_empty() {
                        error_lines.push(format!(
                            "Missing packages not in pom.xml:\n{}",
                            non_javax.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n")
                        ));
                    }
                    let errors = error_lines.join("\n\n");
                    progress.println(format!("  fixing pom.xml ({} pom, {} unresolvable, {} missing)",
                        pom_validation.len(), unresolvable.len(), non_javax.len()));
                    let pom_skill = skill_for_build_system(&build_file);
                    match fix_file(client, &build_file, &content, &errors, service_source_files, &[], &pom_skill, arch_skills, &[]) {
                        Ok(result) => {
                            let _ = std::fs::write(&build_file, &result.content);
                            print_step_notes(progress, step_idx, &result.summary, &result.deviations);
                            fixed_any = true;
                        }
                        Err(e) => progress.println(format!("    LLM fix failed for pom.xml: {e}")),
                    }
                }
            }
        }

        // Collect telemetry for this iteration
        *total_iterations += 1;
        {
            let error_patterns: Vec<String> = combined.lines()
                .filter(|l| {
                    let lo = l.to_lowercase();
                    lo.contains("error") || lo.contains("cannot find") || lo.contains("is not assignable")
                })
                .take(5)
                .map(|l| format!("    - \"{}\"", l.trim().replace('"', "'")))
                .collect();
            let files_yaml = broken_files.iter()
                .map(|f| format!("    - \"{}\"", f))
                .collect::<Vec<_>>()
                .join("\n");
            let patterns_yaml = if error_patterns.is_empty() {
                "    - \"(no matching error lines)\"".to_string()
            } else {
                error_patterns.join("\n")
            };
            telemetry.push(format!(
                "  - iteration: {}\n    files_with_errors:\n{}\n    error_patterns:\n{}\n",
                iteration + 1,
                if files_yaml.is_empty() { "    []".to_string() } else { files_yaml },
                patterns_yaml,
            ));
        }

        if broken_files.is_empty() && !fixed_any {
            progress.println("  No fixable errors found — manual fix needed.");
            progress.println(&combined);
            return false;
        }

        if !broken_files.is_empty() {
            let short: Vec<_> = broken_files.iter()
                .map(|f| std::path::Path::new(f).file_name()
                    .and_then(|n| n.to_str()).unwrap_or(f.as_str()))
                .collect();
            progress.println(format!("  fix [{}/{}]  {} error(s) in {}",
                iteration + 1, max_iterations, broken_files.len(), short.join(", ")));
        }

        for file_path in &broken_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => { progress.println(format!("  cannot read {file_path}: {e}")); continue; }
            };
            let errors = errors_for_file(&combined, file_path);
            if errors.trim().is_empty() {
                progress.println(format!("  skipping {} — no matching error lines", file_path));
                continue;
            }
            let short_name = std::path::Path::new(file_path).file_name()
                .and_then(|n| n.to_str()).unwrap_or(file_path.as_str());

            // The errors just extracted are the direct outcome of whichever attempt last
            // touched this file — backfill them onto that attempt now that they're known.
            if let Some(history) = attempt_history.get_mut(file_path) {
                if let Some(last) = history.last_mut() {
                    if last.resulting_error.is_none() {
                        last.resulting_error = Some(errors.clone());
                    }
                }
            }

            // Same error signature (first line) as what the PREVIOUS attempt left behind —
            // a real (non-noop) code change that didn't actually move the error is easy to
            // miss when every "fixing" line looks alike; call it out explicitly instead of
            // making the reader diff two truncated strings themselves.
            let error_signature = |e: &str| e.lines().next().unwrap_or("").trim().to_string();
            let same_error_as_before = attempt_history.get(file_path)
                .and_then(|history| history.len().checked_sub(2).map(|i| &history[i]))
                .and_then(|prev| prev.resulting_error.as_deref())
                .map(|prev_err| error_signature(prev_err) == error_signature(&errors))
                .unwrap_or(false);

            // Two consecutive no-op attempts mean the model is stuck on this file — stop
            // burning iterations on it rather than asking a third time for the same result.
            if let Some(history) = attempt_history.get(file_path) {
                if history.len() >= 2 && history[history.len() - 1].is_noop && history[history.len() - 2].is_noop {
                    progress.println(format!("  {file_path} made no changes on two consecutive attempts — giving up on it for now."));
                    continue;
                }
            }

            let prior_attempts = attempt_history.get(file_path).cloned().unwrap_or_default();

            let referenced: Vec<(String, String)> =
                if file_path.ends_with(".ts") || file_path.ends_with(".tsx") {
                    let imports = parse_ts_imports(&content, file_path, service_dir);
                    if let Some(surface) = roots::get_ts_module_surface(&imports, service_dir) {
                        vec![("module-surface (roots index)".to_string(), surface)]
                    } else {
                        // Roots not available or files not yet indexed — read the imported files.
                        imports.iter()
                            .filter_map(|rel| {
                                std::fs::read_to_string(format!("{service_dir}/{rel}"))
                                    .ok()
                                    .map(|c| (rel.clone(), c))
                            })
                            .collect()
                    }
                } else if file_path.ends_with(".java") {
                    let imported: Vec<&str> = content.lines()
                        .filter(|l| l.starts_with("import ") && !l.contains('*'))
                        .filter_map(|l| l.trim_end_matches(';').rsplit('.').next())
                        .collect();
                    if let Some(surface) = roots::get_class_surface(&imported, service_dir) {
                        vec![("type-surface (roots index)".to_string(), surface)]
                    } else {
                        service_source_files.iter()
                            .filter(|f| f.ends_with(".java"))
                            .filter_map(|rel| {
                                let full = format!("{service_dir}/{rel}");
                                if full == *file_path { return None; }
                                let stem = std::path::Path::new(rel).file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                if !imported.contains(&stem) { return None; }
                                std::fs::read_to_string(&full).ok().map(|c| (rel.clone(), c))
                            })
                            .collect()
                    }
                } else {
                    vec![]
                };

            let tech = service.technology.as_deref().unwrap_or("");
            let base_skill = skill_for_technology(tech, "", "", &service.name, detect_layer(file_path));
            let test_skill = testing_skill_for_file_with_adrs(file_path, tech, adrs, service_source_files);
            let fix_skill = if test_skill.is_empty() {
                base_skill
            } else {
                format!("{base_skill}\n\n{test_skill}")
            };
            // The bare "fixing {file}" label gave no indication of what was actually wrong, or
            // which attempt this was — the real error is already sitting in `errors`, just
            // never shown, and the iteration/max is only ever printed on a separate line that
            // scrolls away from the per-file history. Both now travel with the label itself.
            let first_error_line = errors.lines().next().unwrap_or("").trim();
            let error_summary: String = if first_error_line.chars().count() > 70 {
                first_error_line.chars().take(70).chain(std::iter::once('…')).collect()
            } else {
                first_error_line.to_string()
            };
            let same_error_note = if same_error_as_before { " [same error persists]" } else { "" };
            let fix_result = progress.timed(
                step_idx,
                format!("fixing  {short_name} (attempt {}/{max_iterations}){same_error_note} — {error_summary}", iteration + 1),
                client,
                || fix_file(client, file_path, &content, &errors, service_source_files, &referenced, &fix_skill, arch_skills, &prior_attempts),
            );
            match fix_result {
                Ok(result) => {
                    let is_noop = result.content.trim() == content.trim();
                    attempt_history.entry(file_path.clone()).or_default().push(FixAttempt {
                        summary: result.summary.clone(),
                        resulting_error: None,
                        is_noop,
                    });
                    if is_noop {
                        // Don't also print the model's self-reported summary here — it has
                        // claimed a fix ("Fixed the TypeScript error by...") on the exact same
                        // attempt that changed nothing, which reads as directly contradicting
                        // the line above it. "model made no changes" is the trustworthy, byte-
                        // verified status; the self-report adds nothing but confusion when it's
                        // already known to be wrong.
                        progress.println(format!("    model made no changes to {file_path}"));
                    } else {
                        let _ = std::fs::write(file_path, &result.content);
                        print_step_notes(progress, step_idx, &result.summary, &result.deviations);
                    }
                }
                Err(e) => progress.println(format!("    LLM fix failed for {file_path}: {e}")),
            }
        }
    }
    false
}
fn migrate_javax_to_jakarta(service_dir: &str) -> usize {
    let mut count = 0;
    let mut java_files = Vec::new();
    collect_files(std::path::Path::new(service_dir), &mut java_files);
    for path in java_files.iter().filter(|p| p.ends_with(".java")) {
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.contains("javax.") {
                let fixed = content
                    .replace("javax.persistence", "jakarta.persistence")
                    .replace("javax.validation", "jakarta.validation")
                    .replace("javax.servlet", "jakarta.servlet")
                    .replace("javax.annotation", "jakarta.annotation")
                    .replace("javax.transaction", "jakarta.transaction");
                if fixed != content {
                    let _ = std::fs::write(path, fixed);
                    count += 1;
                }
            }
        }
    }
    count
}
