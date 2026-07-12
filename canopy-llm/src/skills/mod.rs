mod architecture;
mod build_system;
mod tech_stack;
mod testing;

pub use architecture::skills_for_architecture;
pub use build_system::skill_for_build_system;
pub use tech_stack::{skill_for_technology, skill_for_technology_all_layers};
pub use testing::testing_skill_for_file_with_adrs;

pub(crate) use tech_stack::plan_skill_for_technology;
pub(crate) use testing::{integration_testing_skill, layer_has_worked_example, testing_skill_from_adrs};

/// Detects which structural layer a generated file belongs to, from its path. Shared by the
/// tech-stack skill (which layer's rules to inject) and the testing skill (which example to
/// show) — previously this exact chain was duplicated ad hoc inside the test-stub prompt.
///
/// `app`/`config` only ever apply to implementation files (app.ts/index.ts/tsconfig.json are
/// never TDD candidates, so the testing skill never sees them) — safe to include unconditionally.
pub fn detect_layer(file_path: &str) -> &'static str {
    if file_path.contains("/services/") { "service" }
    else if file_path.contains("/routes/") { "route" }
    else if file_path.contains("/components/") { "component" }
    else if file_path.contains("/api/") { "api-client" }
    else if file_path.contains("/models/") { "model" }
    else if file_path.contains("/events/") { "event" }
    else if file_path.contains("/repositories/") { "repository" }
    else if file_path.contains("/infrastructure/") { "infrastructure" }
    else if file_path.contains("/middleware/") { "middleware" }
    // Angular names the layer in the file's own suffix instead of a plural directory
    // (src/app/<feature>/<feature>.service.ts, never src/services/...) — without this, every
    // Angular file fell through to the generic "module" branch below, silently disabling every
    // layer-gated rule in unit_test_stub_prompt_ts for the whole Angular family.
    else if file_path.ends_with(".service.ts") { "service" }
    else if file_path.ends_with(".component.ts") { "component" }
    else if file_path.ends_with(".model.ts") { "model" }
    else {
        let base = std::path::Path::new(file_path).file_name().and_then(|n| n.to_str()).unwrap_or("");
        if base == "app.ts" || base == "index.ts" { "app" }
        else if base == "tsconfig.json" { "config" }
        else { "module" }
    }
}

/// Join a header with an ordered list of (heading, body) sections using the
/// separator shape every skill type's render() output already used.
pub(super) fn render_skill(header: &str, sections: &[(&str, &str)]) -> String {
    let mut out = header.to_string();
    for (heading, body) in sections {
        out.push_str("\n\n");
        out.push_str(heading);
        out.push('\n');
        out.push_str(body);
    }
    out
}
