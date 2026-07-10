// ── Build system skills ───────────────────────────────────────────────────────
// Build system skills are orthogonal to tech-stack and architecture skills.
// They capture how to write and fix build manifests correctly.
// The pom.xml fix loop only needs the build skill — not the tech-stack skill.
// The step prompt for a build file gets both: tech skill (which deps) + build skill (how to write).
//
// Contract: every build system skill fills three required sections:
//   manifest_contract — required sections, completeness rules, validity constraints
//   dependency_rules  — allowed registries/coordinates, scope keywords, version management
//   anti_patterns     — hallucination patterns this build system is prone to
//
// Detection: skill_for_build_system(file_path) matches by file name.
// To add a new build system: implement a builder, add a match arm in skill_for_build_system().

pub(crate) struct BuildSystemSkill {
    pub name: String,
    /// Required sections, completeness rules, structural validity constraints.
    pub manifest_contract: String,
    /// Allowed registries, coordinate rules, scope keywords, version management.
    pub dependency_rules: String,
    /// Hallucination patterns to avoid — specific to this build system.
    pub anti_patterns: String,
}

impl BuildSystemSkill {
    pub(crate) fn render(&self) -> String {
        super::render_skill(&format!("Build system — {}:", self.name), &[
            ("Manifest contract:", self.manifest_contract.as_str()),
            ("Dependency rules:", self.dependency_rules.as_str()),
            ("Anti-patterns (never do these):", self.anti_patterns.as_str()),
        ])
    }
}

fn maven_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: "Maven (pom.xml)".to_string(),
        manifest_contract:
            "  The pom.xml must be a complete, well-formed XML file ending with </project>.\n\
             A truncated POM is a fatal parse error — Maven refuses to read it.\n\
             Required sections in order:\n\
             - <modelVersion>4.0.0</modelVersion>\n\
             - <parent> block (Spring Boot BOM, if applicable)\n\
             - <groupId>, <artifactId>, <version>, <name>\n\
             - <properties> with <java.version>17</java.version>\n\
             - <dependencies> with all required starters\n\
             - <build> with <plugins> containing spring-boot-maven-plugin\n\
             NEVER add <version> on managed starters when spring-boot-starter-parent is the\n\
             <parent> — the BOM manages them."
            .to_string(),
        dependency_rules:
            "  Only use groupIds that exist on Maven Central:\n\
             - org.springframework.boot  (starters — no explicit version needed with BOM)\n\
             - com.h2database            (h2, scope: runtime)\n\
             - org.projectlombok        (lombok, optional: true)\n\
             - com.fasterxml.jackson.*  (jackson-databind etc.)\n\
             - org.junit.*              (scope: test)\n\
             - org.assertj.*            (scope: test)\n\
             - org.mockito.*            (scope: test)\n\
             Scope keywords: omit for compile (default), <scope>test</scope> for test-only,\n\
             <scope>runtime</scope> for runtime-only."
            .to_string(),
        anti_patterns:
            "  Never add a <dependency> whose <groupId> matches or is derived from the project's\n\
             own <groupId> — your own classes are not published JARs.\n\
             Domain event classes (WidgetCreated, OrderPlaced) live in the service's own\n\
             domain/ package — never add them as a Maven dependency.\n\
             ApplicationEventPublisher is in spring-context, already on the classpath via\n\
             spring-boot-starter — no extra <dependency> is needed or should be added.\n\
             Never truncate the file — the closing </project> tag is mandatory."
            .to_string(),
    }
}

fn gradle_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: "Gradle (Groovy/Kotlin DSL)".to_string(),
        manifest_contract:
            "  Required block order: plugins {}, java {} toolchain, repositories {}, dependencies {}.\n\
             java {\n\
               toolchain { languageVersion = JavaLanguageVersion.of(17) }\n\
             }\n\
             repositories { mavenCentral() }\n\
             The file must be syntactically complete — Gradle fails silently on unterminated blocks."
            .to_string(),
        dependency_rules:
            "  Configuration keywords: implementation (compile), testImplementation (test-only),\n\
             runtimeOnly (runtime-only), annotationProcessor (APT — e.g. Lombok).\n\
             Spring Boot Gradle plugin manages versions — omit explicit version strings for\n\
             Spring Boot managed dependencies.\n\
             Same groupId restrictions as Maven: only well-known Maven Central groupIds."
            .to_string(),
        anti_patterns:
            "  Never use the deprecated compile configuration (removed in Gradle 7) — use implementation.\n\
             Same invented-coordinate prohibitions as Maven: no groupIds derived from the project,\n\
             no domain event JARs, no ApplicationEventPublisher dependency."
            .to_string(),
    }
}

fn npm_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: "npm (package.json)".to_string(),
        manifest_contract:
            "  Required fields: name, version, \"private\": true (for apps), scripts,\n\
             dependencies, devDependencies.\n\
             scripts must include \"build\" and \"dev\".\n\
             TypeScript projects must include \"type-check\": \"tsc --noEmit\" in scripts.\n\
             Must be valid JSON — trailing commas cause a parse failure."
            .to_string(),
        dependency_rules:
            "  dependencies: runtime packages that ship to production (react, react-dom).\n\
             devDependencies: build tools and type stubs (vite, typescript, @types/*).\n\
             @types/* packages always belong in devDependencies, never dependencies.\n\
             Never use file: or link: references unless this is a configured monorepo workspace.\n\
             Only reference packages published to registry.npmjs.org."
            .to_string(),
        anti_patterns:
            "  Never add a package reference for files in your own src/ — those are TypeScript\n\
             imports, not npm packages.\n\
             Never add axios, node-fetch, or other HTTP client libraries unless the story\n\
             explicitly requires them — use the built-in fetch().\n\
             Never add @types/react to dependencies — it belongs in devDependencies."
            .to_string(),
    }
}

fn dotnet_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: ".NET (csproj / MSBuild)".to_string(),
        manifest_contract:
            "  Opening tag for web APIs: <Project Sdk=\"Microsoft.NET.Sdk.Web\">\n\
             Required <PropertyGroup> elements:\n\
             - <TargetFramework>net8.0</TargetFramework>\n\
             - <Nullable>enable</Nullable>\n\
             - <ImplicitUsings>enable</ImplicitUsings>\n\
             <PackageReference> elements go inside an <ItemGroup>.\n\
             Must be well-formed XML."
            .to_string(),
        dependency_rules:
            "  Only packages from NuGet.org.\n\
             <ProjectReference> only for .csproj files that actually exist in the solution —\n\
             verify the path before adding.\n\
             Use Microsoft.AspNetCore.* packages — never Microsoft.AspNet.* (legacy, pre-.NET Core)."
            .to_string(),
        anti_patterns:
            "  Never add a <PackageReference> for types defined within the same project or solution.\n\
             Never reference a <ProjectReference> path that does not exist on disk.\n\
             Never use Microsoft.AspNet.* — the correct namespace is Microsoft.AspNetCore.*"
            .to_string(),
    }
}

/// Return the rendered build system skill for the given build file path.
/// Detected by file name; returns empty string if no built-in skill matches.
/// To add a new build system: implement a builder, add a match arm here.
pub fn skill_for_build_system(file_path: &str) -> String {
    let name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let skill: Option<BuildSystemSkill> = if name == "pom.xml" {
        Some(maven_skill())
    } else if name == "build.gradle" || name == "build.gradle.kts"
           || name == "settings.gradle" || name == "settings.gradle.kts" {
        Some(gradle_skill())
    } else if name == "package.json" {
        Some(npm_skill())
    } else if file_path.ends_with(".csproj") {
        Some(dotnet_skill())
    } else {
        None
    };
    skill.map(|s| s.render()).unwrap_or_default()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_produces_expected_literal_output() {
        let skill = BuildSystemSkill {
            name: "Sample".to_string(),
            manifest_contract: "manifest-body".to_string(),
            dependency_rules: "deps-body".to_string(),
            anti_patterns: "anti-body".to_string(),
        };
        assert_eq!(
            skill.render(),
            "Build system — Sample:\n\n\
             Manifest contract:\nmanifest-body\n\n\
             Dependency rules:\ndeps-body\n\n\
             Anti-patterns (never do these):\nanti-body"
        );
    }
}
