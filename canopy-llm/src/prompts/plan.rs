use crate::client::{LlmClient, LlmError};
use crate::repair::parse_plan_steps;
use crate::skills::{integration_testing_skill, plan_skill_for_technology, skills_for_architecture};
use canopy_core::*;

fn plan_prompt_for_service(
    service: &ServiceEntry,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    adrs: &[Adr],
    existing_files: &[String],
    service_packages: &std::collections::HashMap<String, String>,
    arch_skills: &str,
    installed_packages: &[String],
) -> String {
    let tech = service.technology.as_deref().unwrap_or("unknown");
    let is_front = service.component_type.as_deref() == Some("frontend");

    // TODO(tech-detection): missing the "javascript"-contains-"java" guard that
    // TechFamily::detect uses — left as-is to avoid a silent behavior change; see
    // refactor plan notes.
    let is_jvm_tech = tech.contains("spring") || tech.contains("java") || tech.contains("kotlin")
        || tech.contains("quarkus") || tech.contains("micronaut");
    let (pkg, pkg_path) = if is_front {
        (String::new(), String::new())
    } else if let Some(detected) = service_packages.get(&service.name) {
        (detected.clone(), detected.replace('.', "/"))
    } else if is_jvm_tech {
        // JVM service without a detected scaffold — fall back to Spring Initializr convention
        let p = service.name.replace('-', "_");
        eprintln!("Warning: no scaffolded package detected for '{}'; using fallback '{}'", service.name, p);
        (p.clone(), p.replace('.', "/"))
    } else {
        // Non-JVM services don't have a Java package — pkg is unused by their skill
        (String::new(), String::new())
    };

    let skill = plan_skill_for_technology(tech, &pkg, &pkg_path, &service.name);

    // Show the directory prefix for every service so the LLM never invents bare /src/ paths.
    let dir_prefix = if is_front {
        format!("frontend/{}/", service.name)
    } else {
        format!("services/{}/", service.name)
    };
    let location_line = format!(
        "Service directory prefix: {dir_prefix}\n\
         ALL file paths in steps MUST start with {dir_prefix} — never use bare src/ or placeholder paths.",
        dir_prefix = dir_prefix,
    );

    let schema_yaml = spec.entity_schema.as_ref()
        .map(|s| serde_yaml::to_string(s).unwrap_or_default())
        .unwrap_or_default();
    let scenarios_yaml = serde_yaml::to_string(&spec.scenarios).unwrap_or_default();

    let adrs_summary: String = adrs.iter()
        .map(|a| format!("  - {}: {}", a.title, a.decision))
        .collect::<Vec<_>>()
        .join("\n");

    // Only show files that belong to this service
    let service_prefix = if is_front {
        format!("frontend/{}/", service.name)
    } else {
        format!("services/{}/", service.name)
    };
    // Strip scaffold/config files from the existing list before it reaches the prompt.
    // The LLM sees these as candidates to modify; removing them prevents leakage upstream.
    let scaffold_names = [
        "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
        "jest.config.js", "jest.config.ts",
        "vite.config.ts", "vite.config.js",
        ".gitignore", "README.md",
    ];
    let service_existing: Vec<&str> = existing_files.iter()
        .filter(|f| f.starts_with(&service_prefix))
        .filter(|f| {
            let base = f.rsplit('/').next().unwrap_or(f);
            !scaffold_names.contains(&base) && !base.starts_with("tsconfig.")
        })
        .map(|f| f.as_str())
        .collect();

    let existing_note = if service_existing.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Existing files\nUse operation: modify for every file in this list; create for all others.\n{}",
            service_existing.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n")
        )
    };

    let packages_note = if installed_packages.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Available packages\nAlready declared — do NOT add npm install steps for these:\n{}\n",
            installed_packages.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
        )
    };

    let skill_section = if skill.is_empty() {
        String::new()
    } else {
        format!("\n{skill}\n")
    };
    let arch_section = if arch_skills.is_empty() {
        String::new()
    } else {
        format!("\n{arch_skills}\n")
    };
    let t_lower = tech.to_lowercase();
    let is_jvm = crate::tech::TechFamily::detect(tech) == crate::tech::TechFamily::Jvm;
    let is_node = crate::tech::TechFamily::detect(tech) == crate::tech::TechFamily::NodeExpress;
    let is_react = is_front || t_lower.contains("react") || t_lower.contains("vite") || t_lower.contains("angular");
    let testing_section = if is_jvm {
        let it_skill = integration_testing_skill(tech);
        format!(
            "\n## Testing plan\n\
             - Unit test files (*Test.java) are auto-generated per class by the TDD loop —\n\
               NEVER include them in this plan.\n\
             - Integration test files (*IT.java) test the full stack end-to-end. ALWAYS include\n\
               them as the LAST step(s) in the plan.\n\
             {it_skill}\n"
        )
    } else if is_node {
        format!(
            "\n## Testing plan\n\
             - jest.config.js and test devDependencies (jest, ts-jest, supertest, etc.) are \
               installed by `canopy scaffold` — NEVER include jest.config.js or package.json in the plan.\n\
             - ALWAYS put test files NEXT TO the file they test, in the SAME directory under src/ — \
               NEVER a separate tests/ directory (JS/TS convention, not Java's).\n\
             - TDD cycle (automatic): EVERY file under src/models/, src/events/, src/repositories/, \
               src/infrastructure/, src/middleware/, src/services/, and src/routes/ gets a co-located \
               test file written automatically — NEVER add a test step for any of them.\n\
               ✗ NEVER write: src/models/Widget.test.ts  src/events/WidgetCreated.test.ts\n\
               ✗ NEVER write: src/repositories/WidgetRepository.test.ts  src/infrastructure/EventPublisher.test.ts\n\
               ✗ NEVER write: src/services/WidgetService.test.ts  src/routes/widgets.test.ts\n\
             - ALWAYS list the implementation file a test step tests in depends_on.\n\
             - Test steps MUST be the last steps in the plan.\n\
             - NEVER include a test file for src/app.ts or src/index.ts.\n"
        )
    } else if is_react {
        format!(
            "\n## Testing plan\n\
             - ALWAYS put test files NEXT TO the file they test, in the SAME directory under src/ — \
               NEVER a separate tests/ directory (JS/TS convention, not Java's).\n\
             - TDD cycle (automatic): EVERY file under src/api/ and EVERY file under src/components/ \
               gets a co-located test file written automatically — NEVER add a test step for them.\n\
               ✗ NEVER write: src/api/WidgetApi.test.ts  src/components/WidgetForm.test.tsx\n\
             - ALWAYS use only libraries from the testing strategy ADR — NEVER msw, Playwright, or Cypress.\n\
             - Test steps MUST be the last steps in the list.\n"
        )
    } else {
        let it_skill = integration_testing_skill(tech);
        format!(
            "\n## Testing plan\n\
             - Include unit test files as needed for each module.\n\
             - Include integration tests as the LAST step(s) in the plan.\n\
             {it_skill}\n"
        )
    };

    let event_scope_rule = if is_front {
        "MUST NOT include event type files, publisher utilities, or any broker/Kafka/Redpanda infrastructure — this is a frontend service; it communicates via HTTP only.\n"
    } else {
        "MUST NOT include event listeners or consumers unless the story explicitly requires consuming an event.\n\
         Broker ADR present + story publishes a domain event → MUST include both the event type file (src/events/) AND the publisher utility (src/infrastructure/).\n\
         No broker ADR → event type file only, no publisher infrastructure.\n"
    };

    format!(
        "Discover every file that must be created or modified to implement story '{story_id}' \
         in service '{sname}'. Do NOT order them — just enumerate what is needed.\n\
         \n\
         ## Story\n\
         As a {as_a}, I want {want}, so that {so_that}.\n\
         \n\
         ## Service\n\
         Name: {sname}  Technology: {tech}\n\
         {location_line}\n\
         {skill_section}\
         {arch_section}\
         ## Entity schema\n\
         {schema_yaml}\n\
         ## BDD scenarios\n\
         {scenarios_yaml}\n\
         ## API contract\n\
         {contract_yaml}\n\
         ## Architecture decisions\n\
         {adrs_summary}\n\
         {existing_note}\
         {packages_note}\
         {testing_section}\n\
         ## Output format\n\
         List ONLY files that belong to service '{sname}'.\n\
         Every field inside a list item MUST be indented by exactly 2 spaces.\n\
         \n\
         steps:\n\
         - id: \"1\"\n\
           service: \"{sname}\"\n\
           file: \"{dir_prefix}src/models/Widget.ts\"\n\
           operation: \"create\"\n\
           description: \"Defines the Widget entity and its factory function.\"\n\
         - id: \"2\"\n\
           service: \"{sname}\"\n\
           file: \"{dir_prefix}src/services/WidgetService.ts\"\n\
           operation: \"create\"\n\
           description: \"Orchestrates widget registration by calling the factory and repository.\"\n\
           depends_on:\n\
           - \"{dir_prefix}src/models/Widget.ts\"\n\
         \n\
         Replace Widget/WidgetService with the actual names for this story. Use {dir_prefix} as the literal prefix for every file path.\n\
         Layer verbs: model → Defines; factory → Constructs; repository → Persists; service → Orchestrates; route → Handles; middleware → Translates.\n\
         \n\
         ### Operations\n\
         MUST use operation: modify for every file in the existing list above; create for all others.\n\
         NEVER use operation: modify for a file that is NOT in the existing list — even if you think it is a scaffold artifact.\n\
         ✗ src/app.ts not in existing list → operation must be create, not modify\n\
         ✗ src/middleware/errorHandler.ts not in existing list → operation must be create, not modify\n\
         ✗ src/index.ts not in existing list → operation must be create, not modify\n\
         One step per file — no duplicates. If two descriptions apply to the same file, merge into one step.\n\
         \n\
         ### Scope\n\
         Include ONLY files with logic for this story.\n\
         MUST NOT include any of these — ever:\n\
         - package.json, package-lock.json (managed by dependency gate)\n\
         - tsconfig.json, tsconfig*.json, vite.config.ts, jest.config.js (scaffold artifacts)\n\
         - README, HELP.md, .gitignore, CSS files\n\
         {event_scope_rule}\
         If in doubt, leave it out.\n\
         \n\
         ### YAML format\n\
         The response MUST start with the root key \"steps:\" — never a bare list.\n\
         ALL string values MUST use double quotes — id, service, file, operation, description.\n\
         Every step MUST use \"file:\" — NEVER use \"tests:\", \"path:\", \"source:\", or any other key.\n\
         MUST NOT use block scalars (>- or |) — one quoted string per line.\n\
         description MUST be a single prose sentence on one line — never a YAML list, never code.\n\
         description MUST NOT contain TypeScript, Java, or any code syntax — prose only.\n\
         EVERY file path MUST start with {dir_prefix} — never a bare path like tests/ or src/.\n\
         File extensions MUST use a dot — e.g. App.tsx ✓  App_tsx ✗  index.ts ✓  index_ts ✗\n\
         depends_on: YAML sequence — NEVER wrap in quotes; use [] for none or a proper list.\n\
         - Each depends_on path is the full project-root path including src/ — e.g. {dir_prefix}src/models/Widget.ts\n\
         - depends_on MUST only list implementation source files — NEVER list test files or package.json.\n\
         - Test steps MUST list the source file they test in depends_on — never empty for a test step.\n\
         \n\
         Your entire response is the YAML — begin with `steps:`, end with the last list item.\n\
         NEVER write ``` anywhere. NEVER add explanations, summaries, or any text before or after the YAML.\n",
        sname = service.name,
        story_id = story.id,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        tech = tech,
        location_line = location_line,
        dir_prefix = dir_prefix,
        skill_section = skill_section,
        arch_section = arch_section,
        schema_yaml = schema_yaml,
        scenarios_yaml = scenarios_yaml,
        contract_yaml = contract_yaml,
        adrs_summary = adrs_summary,
        existing_note = existing_note,
        packages_note = packages_note,
        event_scope_rule = event_scope_rule,
    )
}

    // TODO(tech-detection): is_front here re-derives frontend-ness from the tech string via
    // TechFamily::detect, rather than from the authoritative service.component_type field this
    // same file's event_scope_rule uses (see call site). TechFamily::detect itself doesn't
    // recognize "svelte" or "next.js"/"nextjs" as frontend (both real, scaffold.rs-supported
    // tech strings) — for such a service this function falls through to the generic backend
    // ordering rule ("domain → data layer → service → controller"), which doesn't describe that
    // service's actual file layout at all. Left as-is pending its own small, independently
    // reviewed fix — see project memory project-tech-detection-todo-reconciliation.
fn ordering_prompt_for_service(steps: &[ImplementationStep], service_name: &str, tech: &str) -> String {
    let family = crate::tech::TechFamily::detect(tech);
    let is_front = matches!(family, crate::tech::TechFamily::React | crate::tech::TechFamily::Angular | crate::tech::TechFamily::Vue);
    let is_node = family == crate::tech::TechFamily::NodeExpress;

    let layer_rule = if is_front {
        "Frontend — sort by import dependency:\n\
         src/api/ (no local imports) → src/components/ (imports api/) → App.tsx / main.tsx → tests/\n\
         A file must appear after every file it imports."
    } else if is_node {
        "Backend — sort by layer:\n\
         build config → models → events → repositories → infrastructure → services → routes → middleware → app entry point → tests\n\
         Event type and publisher files must precede the service file that publishes events."
    } else {
        "Backend — sort by layer:\n\
         build config → domain → data layer → service → controller → tests\n\
         Each file must appear after every file it depends on."
    };

    let file_list = steps.iter()
        .map(|s| format!("  - \"{}\"", s.file))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Order these implementation files for service '{service_name}' ({tech}).\n\
         Each file must appear AFTER every file it imports or depends on.\n\
         \n\
         {layer_rule}\n\
         \n\
         Files to order (every file must appear exactly once):\n\
         {file_list}\n\
         \n\
         Return ONLY valid YAML — no prose, no code fences.\n\
         Return ONLY the files listed above — do NOT add or remove any.\n\
         order:\n\
         - \"<file-path>\"\n",
        service_name = service_name,
        tech = tech,
        layer_rule = layer_rule,
        file_list = file_list,
    )
}

fn apply_ordering(raw: &str, mut steps: Vec<ImplementationStep>) -> Vec<ImplementationStep> {
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    #[derive(serde::Deserialize)]
    struct OrderResponse { order: Vec<String> }
    let Ok(parsed) = serde_yaml::from_str::<OrderResponse>(stripped) else {
        return steps; // ordering failed — fall back to discovery order; code sort corrects it
    };
    let mut ordered: Vec<ImplementationStep> = Vec::with_capacity(steps.len());
    for path in &parsed.order {
        if let Some(pos) = steps.iter().position(|s| &s.file == path) {
            ordered.push(steps.remove(pos));
        }
    }
    ordered.extend(steps); // any file the LLM omitted goes at the end
    ordered
}

fn layer_weight(file: &str) -> u8 {
    let f = file.to_lowercase();
    // Build manifests first.
    if f.ends_with("pom.xml") || f.ends_with("build.gradle") { return 0; }
    // Tests last — suffix-based; TS/TSX tests are co-located next to source, not under a
    // separate directory, so this must fire before any layer-directory check below matches.
    // Dot-delimited substrings only (".test.", ".spec.") — a bare `contains("test")` or
    // `contains("spec")` misclassifies any real implementation file whose name merely contains
    // that substring (e.g. AttestationService.ts, SpecialOfferService.ts) as a test file.
    if f.contains(".test.") || f.contains(".spec.") || f.ends_with("test.java") { return 10; }
    // Node.js / Express layer order (plural directory names).
    if f.contains("/models/")         { return 1; }
    if f.contains("/events/")         { return 2; }
    if f.contains("/repositories/")   { return 3; }
    if f.contains("/infrastructure/") { return 4; }
    if f.contains("/src/services/") { return 5; }
    if f.contains("/routes/")     { return 6; }
    if f.contains("/middleware/") { return 7; }
    // app.ts / app.js — assembles the app; must come after routes and middleware.
    if f.rsplit('/').next().map_or(false, |n| n.starts_with("app.")) { return 8; }
    // index.ts / index.js — entry point; calls listen(); must follow app.ts.
    if f.rsplit('/').next().map_or(false, |n| n.starts_with("index.")) { return 9; }
    // JVM / Spring layer order (singular directory names).
    if f.contains("/domain/") || f.contains("entity") { return 1; }
    if f.contains("/repository/")  { return 3; }
    if f.contains("/dto/") || f.contains("request") || f.contains("response") { return 3; }
    if f.contains("/service/")     { return 5; }
    if f.contains("/controller/")  { return 6; }
    3 // unknown — treat as mid-stack
}

fn frontend_tier(file: &str) -> u8 {
    let f = file.to_lowercase();
    // Suffix-based — co-located next to source, not under a separate directory.
    if f.contains(".test.") { return 3; }
    if f.contains("/api/") { return 0; }
    if f.ends_with("app.tsx") || f.ends_with("app.ts")
        || f.ends_with("main.tsx") || f.ends_with("main.ts") { return 2; }
    if f.contains("/components/") { return 1; }
    1 // unknown frontend file — treat as component tier
}


fn inject_missing_app_tsx(steps: &mut Vec<ImplementationStep>, service: &ServiceEntry) {
    let prefix = format!("frontend/{}/", service.name);
    let app_tsx = format!("{}src/App.tsx", prefix);
    // Only inject if there is at least one component step and App.tsx is not already present.
    let has_component = steps.iter().any(|s| s.file.contains("/src/components/"));
    let has_app = steps.iter().any(|s| s.file == app_tsx);
    if !has_component || has_app {
        return;
    }
    // Collect the component files this App.tsx will render.
    let component_deps: Vec<String> = steps.iter()
        .filter(|s| s.file.contains("/src/components/"))
        .map(|s| s.file.clone())
        .collect();
    // Insert before the first test step so it sits in the right layer position.
    let insert_pos = steps.iter()
        .position(|s| s.file.to_lowercase().contains(".test."))
        .unwrap_or(steps.len());
    steps.insert(insert_pos, ImplementationStep {
        id: String::new(),
        service: service.name.clone(),
        file: app_tsx,
        operation: "modify".to_string(),
        description: "Render the form component and display it as the main page content".to_string(),
        depends_on: component_deps,
        status: StepStatus::Pending,
    });
}

fn inject_missing_frontend_tests(steps: &mut Vec<ImplementationStep>, _service_name: &str) {
    // The TDD loop generates a co-located test file for every src/api/ and src/components/
    // step automatically — strip out any explicit test step the planner added anyway, since
    // it would just duplicate that automatic file. Location no longer distinguishes a
    // legitimate test step from a redundant one now that co-located IS the correct location;
    // the planner should never add one at all.
    steps.retain(|s| !s.file.to_lowercase().contains(".test."));
}

pub fn generate_story_plan(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    services: &ServicesRegistry,
    adrs: &[Adr],
    existing_files: &[String],
    service_packages: &std::collections::HashMap<String, String>,
    installed_deps_by_service: &std::collections::HashMap<String, Vec<String>>,
) -> Result<StoryPlan, LlmError> {
    let active: Vec<&ServiceEntry> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .collect();

    let mut all_steps: Vec<ImplementationStep> = Vec::new();
    for service in &active {
        let tech = service.technology.as_deref().unwrap_or("unknown");
        let arch_skills = skills_for_architecture(adrs, tech);
        let installed = installed_deps_by_service.get(&service.name).map(|v| v.as_slice()).unwrap_or(&[]);
        // Phase 1: Discover what files are needed (no ordering pressure)
        let disc_prompt = plan_prompt_for_service(
            service, story, spec, contract_yaml, adrs, existing_files, service_packages, &arch_skills, installed,
        );
        let disc_raw = client.complete_large(&disc_prompt)?;
        let mut steps = parse_plan_steps(&disc_raw)?;
        // Strip scaffold / lock-file / config artifacts that must never appear in a plan.
        let t_lower = tech.to_lowercase();
        let is_npm_service = t_lower.contains("node") || t_lower.contains("express")
            || t_lower.contains("react") || t_lower.contains("angular")
            || t_lower.contains("vite") || t_lower.contains("nest");
        steps.retain(|s| {
            let f = s.file.rsplit('/').next().unwrap_or(&s.file);
            // Lock files and config scaffolding are never valid plan steps.
            if matches!(f,
                "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml"
                | "jest.config.js" | "jest.config.ts"
                | "vite.config.ts" | "vite.config.js"
                | "tsconfig.json" | ".gitignore" | "README.md"
            ) || f.starts_with("tsconfig.") {
                return false;
            }
            // npm services: package.json is managed by the dependency gate — never a plan step.
            if is_npm_service && f == "package.json" {
                return false;
            }
            true
        });
        for step in &mut steps {
            step.service = service.name.clone();
            if step.operation.to_lowercase() != "modify" {
                step.operation = "create".to_string();
            } else {
                step.operation = "modify".to_string();
            }
        }

        let is_front_svc = service.component_type.as_deref() == Some("frontend");

        // Phase 2: Order by import dependency (small, focused prompt)
        let order_prompt = ordering_prompt_for_service(&steps, &service.name, tech);
        if let Ok(order_raw) = client.complete(&order_prompt) {
            steps = apply_ordering(&order_raw, steps);
        }

        if is_front_svc {
            inject_missing_app_tsx(&mut steps, service);
            inject_missing_frontend_tests(&mut steps, &service.name);
        }
        all_steps.extend(steps);
    }

    // Sort: backend services first, frontend last; within each group by architectural layer
    let is_frontend_service = |name: &str| {
        services.services.iter()
            .find(|s| s.name == name)
            .and_then(|s| s.component_type.as_deref())
            .map(|t| t == "frontend")
            .unwrap_or(false)
    };
    all_steps.sort_by_key(|s| {
        let is_fe = is_frontend_service(&s.service);
        let service_tier = if is_fe { 1u8 } else { 0u8 };
        let file_tier = if is_fe { frontend_tier(&s.file) } else { layer_weight(&s.file) };
        (service_tier, file_tier)
    });

    for (i, step) in all_steps.iter_mut().enumerate() {
        step.id = (i + 1).to_string();
    }

    Ok(StoryPlan { story_id: story.id.clone(), steps: all_steps })
}

