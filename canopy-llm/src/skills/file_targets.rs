//! Mechanical file-target resolution: `Contract.kind` + `Contract.entity` (see
//! docs/contract-readiness-assessment.md, Option 2) plus a tech-stack's own directory
//! conventions -> a concrete implementation file path, computed without any LLM call. This is
//! the structured counterpart to `TechStackSkill.file_layout`, which is prose meant for an LLM
//! to read, not data a Rust function can query — this module never renders anything into a
//! prompt and never touches `TechStackSkill`.
//!
//! Two real limits found while building this, disclosed rather than smoothed over:
//! - An event-shape contract's file identity is NOT determined by `entity` alone — the event's
//!   own name names the file (`ManufacturerRegistered.ts`, not `Manufacturer.ts`). A publication
//!   contract's file is a fixed, entity-independent name (`EventPublisher.ts`) shared by every
//!   entity in the service. (`Contract` itself has no `subject` field to read this from directly
//!   — the caller recovers it from `Contract.name`; see `contract_plan.rs`.)
//! - Not every (tech family, abstract layer) pair has an established convention yet — React's
//!   skill has no model/repository/service concept for a form-only story, and JVM's own
//!   `spring_boot_skill` has no per-layer *content* rules yet for "event"/"infrastructure" (its
//!   `layer_rules` only has a "domain" entry) even though a file-target convention for both now
//!   exists below (added 2026-07-15, docs/design/contract-composition-assessment.md §8) — a real,
//!   named, deliberately-deferred gap: file *placement* for a JVM event-driven service is now
//!   established, file *content guidance* isn't yet, since no experiment has needed it. Returns
//!   `None` for genuinely unconventioned pairs rather than inventing an unverified one.

use crate::tech::TechFamily;
use canopy_core::BehaviorKind;

/// Fixed, tech-agnostic mapping from a behavior's `kind` to its abstract architectural layer —
/// a `kind` never changes meaning across tech stacks; only which directory/file realizes it
/// does. Not the same string space as `detect_layer()` in every case (e.g. React's HTTP boundary
/// is "api-client", not "route") — that's fine: what matters is that the file path this module
/// computes, when re-classified by `detect_layer()`, lands back on the right concept, not that
/// the two vocabularies are spelled identically.
pub fn abstract_layer_for_kind(kind: &BehaviorKind) -> &'static str {
    match kind {
        BehaviorKind::Validation | BehaviorKind::Construction => "model",
        BehaviorKind::Persistence => "repository",
        BehaviorKind::EventShape => "event",
        BehaviorKind::Publication => "infrastructure",
        BehaviorKind::Orchestration => "service",
        BehaviorKind::HttpRequest | BehaviorKind::HttpResponse => "route",
        BehaviorKind::ErrorTranslation => "middleware",
    }
}

/// Naive English pluralization covering the common regular cases (widget -> widgets, category ->
/// categories, box -> boxes) — not a full inflector. An irregular noun (person -> people) will
/// come out wrong; disclosed here rather than silently assumed correct.
fn naive_plural_lower(entity: &str) -> String {
    let lower = entity.to_lowercase();
    if let Some(stem) = lower.strip_suffix('y') {
        if !stem.ends_with(|c: char| "aeiou".contains(c)) {
            return format!("{stem}ies");
        }
    }
    if lower.ends_with('s') || lower.ends_with('x') || lower.ends_with("ch") || lower.ends_with("sh") {
        return format!("{lower}es");
    }
    format!("{lower}s")
}

/// Resolves a mechanical implementation-file target for one (tech, layer, entity) combination.
/// `event_name` is required — and only used — when `layer == "event"`: an event's file identity
/// is its own name (e.g. `ManufacturerRegistered`, i.e. `Contract.subject` for an event-shape
/// contract), not the owning entity's. Returns `None` when no established mechanical convention
/// exists yet for this (family, layer) pair, rather than guessing one.
pub fn resolve_implementation_target(
    tech: &str,
    pkg_path: &str,
    service_name: &str,
    layer: &str,
    entity: &str,
    event_name: Option<&str>,
) -> Option<String> {
    match TechFamily::detect(tech) {
        TechFamily::NodeExpress => {
            let prefix = format!("services/{service_name}/src/");
            match layer {
                "model" => Some(format!("{prefix}models/{entity}.ts")),
                "repository" => Some(format!("{prefix}repositories/{entity}Repository.ts")),
                "service" => Some(format!("{prefix}services/{entity}Service.ts")),
                "route" => Some(format!("{prefix}routes/{}.ts", naive_plural_lower(entity))),
                "event" => event_name.map(|e| format!("{prefix}events/{e}.ts")),
                "infrastructure" => Some(format!("{prefix}infrastructure/EventPublisher.ts")),
                "middleware" => Some(format!("{prefix}middleware/errorHandler.ts")),
                _ => None,
            }
        }
        TechFamily::Jvm => {
            let prefix = format!("services/{service_name}/src/main/java/{pkg_path}/");
            match layer {
                "model" => Some(format!("{prefix}domain/{entity}.java")),
                "repository" => Some(format!("{prefix}repository/{entity}Repository.java")),
                "service" => Some(format!("{prefix}service/{entity}Service.java")),
                "route" => Some(format!("{prefix}controller/{entity}Controller.java")),
                // Reuses the same plural "events"/"infrastructure" directory names the Node.js
                // arm above uses (not a JVM-singular convention like domain/repository/service/
                // controller above) specifically because `detect_layer()`
                // (canopy-llm/src/skills/mod.rs) already recognizes `/events/` and
                // `/infrastructure/` as generic, tech-family-independent layer names, checked
                // before its JVM-singular block — reusing them means this needs no detect_layer()
                // change. `spring_boot_skill` has no per-layer content rules for either yet (see
                // this module's own doc comment) — file placement only, not file content
                // guidance, added 2026-07-15 (docs/design/contract-composition-assessment.md §8).
                "event" => event_name.map(|e| format!("{prefix}events/{e}.java")),
                "infrastructure" => Some(format!("{prefix}infrastructure/EventPublisher.java")),
                // spring_boot_skill doesn't define middleware layout yet — see this module's own
                // doc comment.
                _ => None,
            }
        }
        TechFamily::React => {
            let prefix = format!("frontend/{service_name}/src/");
            match layer {
                "route" => Some(format!("{prefix}api/{entity}Api.ts")),
                // react_vite_skill has no model/repository/service/event/infrastructure/
                // middleware convention — a form-only frontend story has nothing behind the
                // api client.
                _ => None,
            }
        }
        TechFamily::Angular => {
            let feature = entity.to_lowercase();
            let prefix = format!("frontend/{service_name}/src/app/{feature}/");
            match layer {
                "model" => Some(format!("{prefix}{feature}.model.ts")),
                // Angular's HttpClient calls happen inside the service file itself
                // (angular_skill) — there's no separate api-client layer, so an orchestration
                // contract and an http-request/response contract for the same entity resolve to
                // the same file. Real, not a bug: this stack doesn't split the two concerns the
                // way Node/React do.
                "service" | "route" => Some(format!("{prefix}{feature}.service.ts")),
                _ => None,
            }
        }
        TechFamily::Vue | TechFamily::Other => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstract_layer_covers_every_kind() {
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::Validation), "model");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::Construction), "model");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::Persistence), "repository");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::EventShape), "event");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::Publication), "infrastructure");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::Orchestration), "service");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::HttpRequest), "route");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::HttpResponse), "route");
        assert_eq!(abstract_layer_for_kind(&BehaviorKind::ErrorTranslation), "middleware");
    }

    #[test]
    fn naive_plural_covers_regular_cases() {
        assert_eq!(naive_plural_lower("Widget"), "widgets");
        assert_eq!(naive_plural_lower("Category"), "categories");
        assert_eq!(naive_plural_lower("Box"), "boxes");
        assert_eq!(naive_plural_lower("Manufacturer"), "manufacturers");
    }

    #[test]
    fn node_express_resolves_every_documented_layer() {
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "model", "Manufacturer", None),
            Some("services/manufacturer-service/src/models/Manufacturer.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "repository", "Manufacturer", None),
            Some("services/manufacturer-service/src/repositories/ManufacturerRepository.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "service", "Manufacturer", None),
            Some("services/manufacturer-service/src/services/ManufacturerService.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "route", "Manufacturer", None),
            Some("services/manufacturer-service/src/routes/manufacturers.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "infrastructure", "Manufacturer", None),
            Some("services/manufacturer-service/src/infrastructure/EventPublisher.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "middleware", "Manufacturer", None),
            Some("services/manufacturer-service/src/middleware/errorHandler.ts".to_string())
        );
    }

    #[test]
    fn node_express_event_layer_requires_event_name() {
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "event", "Manufacturer", Some("ManufacturerRegistered")),
            Some("services/manufacturer-service/src/events/ManufacturerRegistered.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Node.js", "", "manufacturer-service", "event", "Manufacturer", None),
            None
        );
    }

    #[test]
    fn jvm_resolves_documented_layers_and_declines_the_rest() {
        assert_eq!(
            resolve_implementation_target("Spring Boot", "com.example.manufacturer", "manufacturer-service", "model", "Manufacturer", None),
            Some("services/manufacturer-service/src/main/java/com.example.manufacturer/domain/Manufacturer.java".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Spring Boot", "com.example.manufacturer", "manufacturer-service", "route", "Manufacturer", None),
            Some("services/manufacturer-service/src/main/java/com.example.manufacturer/controller/ManufacturerController.java".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Spring Boot", "com.example.manufacturer", "manufacturer-service", "event", "Manufacturer", Some("ManufacturerRegistered")),
            Some("services/manufacturer-service/src/main/java/com.example.manufacturer/events/ManufacturerRegistered.java".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Spring Boot", "com.example.manufacturer", "manufacturer-service", "event", "Manufacturer", None),
            None
        );
        assert_eq!(
            resolve_implementation_target("Spring Boot", "com.example.manufacturer", "manufacturer-service", "infrastructure", "Manufacturer", None),
            Some("services/manufacturer-service/src/main/java/com.example.manufacturer/infrastructure/EventPublisher.java".to_string())
        );
        assert_eq!(
            resolve_implementation_target("Spring Boot", "com.example.manufacturer", "manufacturer-service", "middleware", "Manufacturer", None),
            None
        );
    }

    #[test]
    fn jvm_events_directory_is_recognized_by_detect_layer() {
        // Ties this module's new JVM event/infrastructure paths to `detect_layer()`
        // (canopy-llm/src/skills/mod.rs) directly, so a future change to either side that breaks
        // the other fails a test here rather than silently reproducing the exact "layer-scoped
        // rule never reaches the file" bug already found once for JVM's singular directories.
        let event_path = resolve_implementation_target(
            "Spring Boot", "com.example.manufacturer", "manufacturer-service", "event", "Manufacturer", Some("ManufacturerRegistered"),
        ).unwrap();
        assert_eq!(crate::skills::detect_layer(&event_path), "event");
        let infra_path = resolve_implementation_target(
            "Spring Boot", "com.example.manufacturer", "manufacturer-service", "infrastructure", "Manufacturer", None,
        ).unwrap();
        assert_eq!(crate::skills::detect_layer(&infra_path), "infrastructure");
    }

    #[test]
    fn react_resolves_only_the_api_client_layer() {
        assert_eq!(
            resolve_implementation_target("React", "", "manufacturer-registration-portal", "route", "Manufacturer", None),
            Some("frontend/manufacturer-registration-portal/src/api/ManufacturerApi.ts".to_string())
        );
        assert_eq!(
            resolve_implementation_target("React", "", "manufacturer-registration-portal", "model", "Manufacturer", None),
            None
        );
    }

    #[test]
    fn angular_service_and_route_share_one_file() {
        let service = resolve_implementation_target("Angular", "", "admin-portal", "service", "Manufacturer", None);
        let route = resolve_implementation_target("Angular", "", "admin-portal", "route", "Manufacturer", None);
        assert_eq!(service, route);
        assert_eq!(service, Some("frontend/admin-portal/src/app/manufacturer/manufacturer.service.ts".to_string()));
    }

    #[test]
    fn vue_and_other_families_resolve_nothing() {
        assert_eq!(resolve_implementation_target("Vue", "", "svc", "model", "Manufacturer", None), None);
        assert_eq!(resolve_implementation_target("COBOL", "", "svc", "model", "Manufacturer", None), None);
    }
}
