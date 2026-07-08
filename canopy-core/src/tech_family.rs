/// Coarse technology family used to select build/test tooling and manifest format.
/// Not exhaustive by design — anything unmatched defaults to `Npm`, mirroring every
/// existing call site's implicit "else" branch (dependency/build-command dispatch in
/// canopy-cli never had a fourth branch; a .NET service tech string falls through to
/// this default today too — that pre-existing gap is preserved, not fixed, here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechFamily {
    Npm,
    JvmMaven,
    JvmGradle,
}

impl TechFamily {
    pub fn classify(tech: &str) -> Self {
        let t = tech.to_lowercase();
        let is_npm = t.contains("node") || t.contains("express") || t.contains("react")
            || t.contains("angular") || t.contains("vite") || t.contains("nest");
        if is_npm {
            return TechFamily::Npm;
        }
        if t.contains("gradle") {
            return TechFamily::JvmGradle;
        }
        if t.contains("spring") || t.contains("java") || t.contains("kotlin")
            || t.contains("maven") || t.contains("quarkus") || t.contains("micronaut")
        {
            return TechFamily::JvmMaven;
        }
        TechFamily::Npm
    }

    pub fn is_jvm(self) -> bool {
        matches!(self, TechFamily::JvmMaven | TechFamily::JvmGradle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Characterization test: locks in classification for every tech string this
    // codebase's LLM prompts/scaffold generation actually produces today, so a
    // reviewer can confirm intent before call sites are switched onto this type.
    #[test]
    fn classifies_known_tech_strings() {
        assert_eq!(TechFamily::classify("Spring Boot 3"), TechFamily::JvmMaven);
        assert_eq!(TechFamily::classify("Spring Boot 3 (Kotlin)"), TechFamily::JvmMaven);
        assert_eq!(TechFamily::classify("Angular"), TechFamily::Npm);
        assert_eq!(TechFamily::classify("React + Vite"), TechFamily::Npm);
        assert_eq!(TechFamily::classify("Node.js / Express"), TechFamily::Npm);
    }

    #[test]
    fn gradle_keyword_routes_to_jvm_gradle_even_with_java_keyword_present() {
        assert_eq!(TechFamily::classify("Spring Boot (Gradle)"), TechFamily::JvmGradle);
    }

    #[test]
    fn is_jvm_true_for_both_maven_and_gradle() {
        assert!(TechFamily::JvmMaven.is_jvm());
        assert!(TechFamily::JvmGradle.is_jvm());
        assert!(!TechFamily::Npm.is_jvm());
    }
}
