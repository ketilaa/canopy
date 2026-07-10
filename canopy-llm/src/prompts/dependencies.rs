use crate::client::{LlmClient, LlmError};
use crate::repair::{repair_list_item_indentation, strip_code_fences};
use canopy_core::*;

pub fn propose_dependencies(
    client: &LlmClient,
    service_name: &str,
    tech: &str,
    story: &UserStory,
    plan_steps: &[ImplementationStep],
    installed: &[String],
    previously_rejected: &[String],
    adrs: &[Adr],
    tech_skill: &str,
) -> Result<Vec<ProposedDependency>, LlmError> {
    let steps_summary: String = plan_steps.iter()
        .map(|s| format!("  - {} ({}): {}", s.file, s.operation, s.description))
        .collect::<Vec<_>>()
        .join("\n");
    let installed_list = if installed.is_empty() {
        "none".to_string()
    } else {
        installed.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
    };
    let adrs_section = if adrs.is_empty() {
        String::new()
    } else {
        let lines = adrs.iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n## Architecture decisions\n{lines}\nPropose ONLY dependencies consistent with these decisions.\n")
    };

    let rejected_section = if previously_rejected.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Previously rejected dependencies (NEVER propose these again)\n{}\n",
            previously_rejected.iter().map(|p| format!("- {}", p)).collect::<Vec<_>>().join("\n")
        )
    };

    let (stack_desc, coord_format, scope_note, manifest_name, builtin_note) = match canopy_core::TechFamily::classify(tech) {
        canopy_core::TechFamily::Npm => (
            "Node.js / TypeScript",
            "npm package name (e.g. \"zod\", \"kafkajs\")",
            "dev: true for devDependencies (build tools, @types/* stubs, test libraries); dev: false for runtime dependencies",
            "package.json",
            "Built-in Node.js APIs (crypto, fs, path, etc.) are always available — NEVER propose them.",
        ),
        canopy_core::TechFamily::JvmGradle => (
            "Java / Gradle",
            "groupId:artifactId (e.g. \"org.springframework.boot:spring-boot-starter-security\")",
            "dev: true for testImplementation scope; dev: false for implementation scope",
            "build.gradle",
            "Spring Boot BOM-managed dependencies do not need a version. JDK standard library is always available.",
        ),
        canopy_core::TechFamily::JvmMaven => (
            "Java / Maven",
            "groupId:artifactId (e.g. \"org.springframework.boot:spring-boot-starter-security\")",
            "dev: true for <scope>test</scope>; dev: false for compile scope (no <scope> tag needed)",
            "pom.xml",
            "Spring Boot parent BOM manages versions — do not include version strings. JDK standard library is always available.",
        ),
    };

    let skill_section = if tech_skill.is_empty() {
        String::new()
    } else {
        format!("\n## Tech stack rules (MUST follow)\n{tech_skill}\n")
    };

    let prompt = format!(
        "You are reviewing a {stack_desc} implementation plan for service '{service}' ({tech}).\n\
         \n\
         ## Implementation plan\n\
         {steps}\n\
         \n\
         ## Story\n\
         As a {as_a}, I want {want}, so that {so_that}.\n\
         {adrs_section}\
         {skill_section}\
         ## Already declared dependencies ({manifest_name})\n\
         {installed}\n\
         STOP — do NOT propose any package from the list above. They are already installed.\n\
         {rejected_section}\
         ## Task\n\
         Identify NEW external dependencies NOT in the already-declared list above.\n\
         A package already listed above MUST NOT appear in proposed_dependencies — ever.\n\
         {builtin_note}\n\
         For each proposed dependency:\n\
         - State the coordinate in the format: {coord_format}\n\
         - Explain precisely why it is needed for this story (not just \"for X functionality\")\n\
         - List alternatives that were considered, including built-in options, and explain why each was rejected\n\
         - {scope_note}\n\
         \n\
         If no new dependencies are needed, return: proposed_dependencies: []\n\
         \n\
         Return ONLY valid YAML — no prose, no code fences.\n\
         \n\
         ## Output format\n\
         CRITICAL — indentation: every field after the dash MUST be indented by exactly 2 spaces.\n\
         WRONG (fields at column 0 — will not parse):\n\
         proposed_dependencies:\n\
         - package: \"example\"\n\
         justification: \"reason\"\n\
         alternatives: \"other options\"\n\
         dev: false\n\
         \n\
         CORRECT (fields indented 2 spaces under the dash):\n\
         proposed_dependencies:\n\
         - package: \"example\"\n\
           justification: \"reason\"\n\
           alternatives: \"other options\"\n\
           dev: false\n\
         \n\
         Additional rules:\n\
         - All string values MUST use double quotes.\n\
         - justification and alternatives MUST be on the same line as their key — no block scalars.\n\
         - dev MUST be a bare boolean: true or false (no quotes).",
        stack_desc = stack_desc,
        service = service_name,
        tech = tech,
        steps = steps_summary,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        installed = installed_list,
        adrs_section = adrs_section,
        skill_section = skill_section,
        rejected_section = rejected_section,
        manifest_name = manifest_name,
        builtin_note = builtin_note,
        coord_format = coord_format,
        scope_note = scope_note,
    );

    let raw = client.complete(&prompt)?;
    let stripped = strip_code_fences(&raw);

    #[derive(serde::Deserialize)]
    struct Wrapper { proposed_dependencies: Vec<ProposedDependency> }

    let installed_set: std::collections::HashSet<String> =
        installed.iter().map(|s| s.to_lowercase()).collect();

    // Try parsing as-is; if it fails (common: LLM omits list-item indentation),
    // repair the indentation and try again before giving up.
    serde_yaml::from_str::<Wrapper>(&stripped)
        .or_else(|_| serde_yaml::from_str::<Wrapper>(&repair_list_item_indentation(&stripped)))
        .map(|w| {
            w.proposed_dependencies
                .into_iter()
                .filter(|d| !installed_set.contains(&d.package.to_lowercase()))
                .collect()
        })
        .map_err(|e| LlmError::UnexpectedShape(format!("dependency proposals: {e}")))
}

