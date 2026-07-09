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
    else {
        let base = std::path::Path::new(file_path).file_name().and_then(|n| n.to_str()).unwrap_or("");
        if base == "app.ts" || base == "index.ts" { "app" }
        else if base == "tsconfig.json" { "config" }
        else { "module" }
    }
}

/// The one fix pattern for every `exactOptionalPropertyTypes: true` violation this project has
/// hit — declaring an optional field, a factory forwarding its own optional parameter, a route
/// forwarding a validator's parsed output, and test fixture data all reduce to the same rule.
/// Defined once and referenced from each layer/prompt that needs it, instead of a separate
/// case-specific WRONG/CORRECT block per call site (which is how this grew the first three times).
pub(crate) const EXACT_OPTIONAL_PROPERTY_RULE: &str =
    "Under `exactOptionalPropertyTypes: true`, a property declared `field?: T` may be ABSENT \
     from an object, but if the key IS present its value must be exactly `T` — never `undefined`. \
     This applies no matter where the value comes from: a hand-written literal, a parameter \
     forwarded by shorthand, or a third-party library's parsed/validated output (e.g. Zod's \
     `.optional()` infers `T | undefined`, a wider type that is NOT assignable to `field?: T`).\n\
     The fix is always the same shape — never assign `undefined` to the key; omit it instead:\n\
       WRONG:   { ...rest, field: value }              // value may be `undefined`\n\
       CORRECT: { ...rest, ...(value !== undefined && { field: value }) }\n\
     `rest` in that CORRECT line means the source object WITHOUT `field` — destructure it out\n\
     first (`const { field, ...rest } = source`). Spreading the ORIGINAL object first and only\n\
     conditionally re-adding `field` after does NOT fix anything: the earlier spread already\n\
     copied `field` (however it was — including a literal `undefined` value) onto the result,\n\
     and a conditional spread that evaluates false is a no-op — it cannot remove a key another\n\
     spread already added:\n\
       WRONG:   const { name } = source                                    // field not destructured out\n\
                { ...source, ...(source.field !== undefined && { field: source.field }) }\n\
                // source.field is still on the result via `...source`, whether or not the second spread runs\n\
       CORRECT: const { field, ...rest } = source\n\
                { ...rest, ...(field !== undefined && { field }) }\n\
     Apply this everywhere a possibly-undefined value flows into an optional-typed field: object \
     literals, factory functions forwarding their own optional parameters, route handlers \
     forwarding a validator's parsed output into a service call, and test fixture data.";

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
