//! Technology-family detection shared across skill dispatch, planning, and
//! scaffold prompts.
//!
//! NOTE: the various `t.contains("spring") || ...`-style predicates scattered
//! throughout this crate are NOT all identical — some guard against
//! "javascript" containing "java", some don't; some include "nest", some
//! don't. `TechFamily::detect` below matches the majority ("variant B")
//! behavior used by the skill dispatchers. Do not silently reroute an
//! outlier call site onto it without checking its exact existing predicate
//! first — see the module-split plan for the enumerated exceptions.

use canopy_core::ServicesRegistry;

/// Coarse technology family used by skill/testing-strategy dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TechFamily {
    Jvm,
    React,
    Angular,
    NodeExpress,
    Vue,
    Other,
}

impl TechFamily {
    /// Case-insensitive; guards "javascript" (which contains the substring "java").
    pub(crate) fn detect(tech: &str) -> TechFamily {
        let t = tech.to_lowercase();
        if t.contains("spring") || t.contains("quarkus") || t.contains("micronaut")
            || (t.contains("java") && !t.contains("javascript")) || t.contains("kotlin") {
            TechFamily::Jvm
        } else if t.contains("angular") {
            TechFamily::Angular
        } else if t.contains("react") || t.contains("vite") {
            TechFamily::React
        } else if t.contains("node") || t.contains("express") || t.contains("nest") {
            TechFamily::NodeExpress
        } else if t.contains("vue") {
            TechFamily::Vue
        } else {
            TechFamily::Other
        }
    }
}

fn is_jvm_technology(tech: &str) -> bool {
    canopy_core::TechFamily::classify(tech).is_jvm()
}

pub(crate) fn infer_working_dir(technology: &str) -> &'static str {
    let t = technology.to_lowercase();
    if t.contains("angular") || t.contains("react") || t.contains("vue")
        || t.contains("next") || t.contains("vite") || t.contains("svelte")
        || t.contains("nuxt")
    {
        "frontend"
    } else {
        "services"
    }
}

pub fn services_need_jvm(services: &ServicesRegistry) -> bool {
    services
        .services
        .iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .any(|s| s.technology.as_deref().map(is_jvm_technology).unwrap_or(false))
}
