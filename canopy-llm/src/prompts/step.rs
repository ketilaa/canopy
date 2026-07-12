use crate::client::{LlmClient, LlmError};
use crate::skills::{detect_layer, layer_has_worked_example, skill_for_build_system, skill_for_technology, testing_skill_from_adrs};
use crate::tools::{tools_hint_section, ChatMessage, ToolCall, ToolSpec, ToolTurn};
use canopy_core::*;

/// A handful of real runs never needed more than 2 (one tool call, one final answer); this just
/// guards against a model that keeps calling tools indefinitely instead of ever producing a
/// final answer. Same bound `fix_file_with_tools` uses.
const MAX_TOOL_ITERATIONS: usize = 4;

fn step_prompt(
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    sibling_section: &str,
    arch_skills: &str,
    test_hint: Option<(&str, &str, bool)>,
    package_constraints: Option<&str>,
    observed_call: Option<&str>,
    tools: &[ToolSpec],
) -> String {
    // The plan LLM sometimes prefixes service names with their directory (e.g. "frontend/admin-portal").
    // Strip any leading path component before looking up in the registry.
    let service_name = step.service.rsplit('/').next().unwrap_or(&step.service);
    let service_entry = services.services.iter()
        .find(|s| s.name == service_name || s.name == step.service);
    let technology = service_entry.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
    let layer = detect_layer(&step.file);

    // Entity schema and OAS contract are only relevant to layers that actually touch domain
    // fields or HTTP shapes — showing them unconditionally to e.g. infrastructure/middleware/
    // app/config files is pure bloat, displacing more decisive content from the model's
    // attention for no benefit (those files have no entity fields or endpoints to align with).
    // Only the model layer constructs fields directly from the entity schema. Every other
    // layer either doesn't touch entity fields at all (infrastructure/middleware/app/config),
    // or gets the same information more precisely from sibling context (repository/service see
    // the model's actual interface via Roots) or from the OAS contract (route/api-client — the
    // contract-generation prompt already maps entity_schema's own validation constraints
    // (max_length → maxLength, etc.) onto the OAS schema, so sending both is redundant).
    let schema_section = if layer == "model" {
        spec.entity_schema.as_ref()
            .map(|s| serde_yaml::to_string(s).unwrap_or_default())
            .map(|yaml| format!("Entity schema:\n{yaml}\n"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    // "app" doesn't need endpoint-level contract detail — it only assembles routes/middleware.
    let contract_section = if matches!(layer, "route" | "api-client") && !contract_yaml.is_empty() {
        format!("OAS Contract:\n{contract_yaml}\n")
    } else {
        String::new()
    };
    // Detect frontend by registry entry OR by file extension (belt-and-suspenders).
    let _is_frontend = service_entry
        .and_then(|s| s.component_type.as_deref())
        .map(|t| t == "frontend")
        .unwrap_or(false)
        || step.file.ends_with(".ts")
        || step.file.ends_with(".tsx");

    let pkg = service_packages.get(service_name)
        .cloned()
        .unwrap_or_else(|| service_name.replace('-', "_"));
    let pkg_path = pkg.replace('.', "/");

    let tech_rules = skill_for_technology(technology, &pkg, &pkg_path, service_name, layer);
    // For build manifest files, also inject the build system skill.
    // The tech skill says WHICH dependencies belong; the build skill says HOW to write the file.
    let build_rules = skill_for_build_system(&step.file);

    // sibling_section is built by the CLI layer using Roots symbol surfaces
    // (falling back to full file content when the index is unavailable).
    let sibling_section = sibling_section;

    let current_section = match current_content {
        Some(content) => format!(
            "\nCurrent file content — preserve what's already correct (e.g. a signature already\n\
             grounded by the test), change only what the description or errors actually require:\n\
             ```\n{content}\n```\n"
        ),
        None => String::new(),
    };

    let roots_section = match roots_context {
        Some(ctx) if !ctx.is_empty() => format!(
            "\nRelated code already in the project (use these exact class names and package paths):\n{ctx}\n"
        ),
        _ => String::new(),
    };

    let is_ts = step.file.ends_with(".ts") || step.file.ends_with(".tsx");
    let is_tsx = step.file.ends_with(".tsx");
    // When Some, this was deterministically parsed from the test file already written (see
    // roots_parser::find_subject_calls) — not a guess. Replaces asking the model to count
    // arguments itself, which has been observed, on a real dogfooding run, to disagree with
    // this exact same test roughly half the time even with an explicit self-check instruction
    // already in place. Falls back to that self-check instruction when extraction couldn't
    // determine a single consistent call shape (None).
    let arity_check_lead = match observed_call {
        Some(shape) => format!(
            "             - ALWAYS declare a signature matching this exact call: `{shape}` —\n\
               same argument count and shape (never split an object into positional\n\
               parameters; never add or drop a parameter).\n"
        ),
        None => "             - ALWAYS count the arguments in each call the test below makes to your export and\n\
               declare EXACTLY that many parameters, in that same shape — NEVER guess a\n\
               different arity from a sibling file's convention.\n".to_string(),
    };
    let test_hint_section = match test_hint {
        Some((tf, tc, true)) if is_tsx => format!(
            "\nSTUB ONLY — return a renderable skeleton, no logic:\n\
             - ALWAYS export the component function with the correct name and props type.\n\
             - Render body: `return null;` — component must mount without errors.\n\
             - NEVER implement any UI, state, or handlers — the Green phase does that.\n\
             \n\
             Unit test this stub must compile and mount against:\n\
             --- {tf} ---\n\
             {tc}\n"
        ),
        Some((tf, tc, true)) if is_ts => format!(
            "\nSTUB ONLY — return a compilable skeleton, no logic:\n\
             - ALWAYS export every class/function/type the test below imports.\n\
             - Every function/method body: ONLY `throw new Error('not implemented');` — no\n\
               validation, no field assignment, no constructed return value, even logic you\n\
               already know is correct. This applies to a standalone factory function exactly\n\
               the same as a class method.\n\
               WRONG — this is a full implementation, not a stub:\n\
                 export function createWidget(name: string, otherField: string): Widget {{\n\
                   if (!name) throw new Error('name-value not provided...')\n\
                   return {{ id: randomUUID(), createdAt: new Date(), name, otherField }}\n\
                 }}\n\
               CORRECT:\n\
                 export function createWidget(name: string, otherField: string): Widget {{\n\
                   throw new Error('not implemented');\n\
                 }}\n\
             - Constructor bodies: empty (no field assignments needed yet).\n\
             {arity_check_lead}\
               WRONG: test calls `subject.registerWidget(widgetData)` — one argument — but the\n\
                 stub declares `registerWidget(name, otherField, optionalField)` — three params  ✗\n\
               CORRECT: `registerWidget(widgetData: Widget)` — one parameter, matching the call\n\
             \n\
             Unit test this stub must compile against:\n\
             --- {tf} ---\n\
             {tc}\n"
        ),
        Some((tf, tc, true)) => format!(
            "\nSTUB ONLY — return a compilable skeleton, no business logic:\n\
             - ALWAYS declare every class, field, constructor, and method the unit test below references.\n\
             - Method bodies: `return null;` for objects, `return 0;` for numbers, `return false;` for booleans, `return List.of();` for collections.\n\
             - NEVER implement any logic — the Green phase replaces this stub with the real implementation.\n\
             \n\
             Unit test this stub must compile against:\n\
             --- {tf} ---\n\
             {tc}\n"
        ),
        Some((tf, tc, false)) => format!(
            "\nGREEN PHASE — implement to make all unit tests below pass.\n\
             Read the test file carefully: every assertion is a requirement.\n\
             ALWAYS use the EXACT method signatures shown in the sibling section — NEVER add\n\
             extra arguments or change parameter order relative to what is declared there.\n\
             {arity_check_lead}\
             \n\
             Unit tests that must pass:\n\
             --- {tf} ---\n\
             {tc}\n"
        ),
        None => String::new(),
    };

    let pkg_section = match package_constraints {
        Some(c) if !c.is_empty() => format!("{c}\n"),
        _ => String::new(),
    };
    let tools_section = tools_hint_section(tools);

    format!(
        "Generate the complete content of file '{file}'.\n\
         \n\
         Operation: {operation}\n\
         Description: {description}\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         Service: {service} ({technology})\n\
         \n\
         {schema_section}\
         {contract_section}\
         {sibling_section}\
         {current_section}\
         {roots_section}\
         {pkg_section}\
         {tools_section}\
         {arch_rules}\n\
         {build_rules}\n\
         {tech_rules}\n\
         {test_hint_section}\n\
         Write the file content first — complete and ready to save.\n\
         {contract}",
        file = step.file,
        operation = step.operation,
        description = step.description,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        service = step.service,
        technology = technology,
        schema_section = schema_section,
        contract_section = contract_section,
        sibling_section = sibling_section,
        current_section = current_section,
        roots_section = roots_section,
        test_hint_section = test_hint_section,
        pkg_section = pkg_section,
        tech_rules = tech_rules,
        build_rules = build_rules,
        arch_rules = arch_skills,
        contract = canopy_summary_contract(),
    )
}

pub use super::summary::StepResult;
use super::summary::{canopy_summary_contract, split_step_response};

pub fn execute_implementation_step(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    sibling_section: &str,
    arch_skills: &str,
    package_constraints: Option<&str>,
) -> Result<StepResult, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, sibling_section, arch_skills, None, package_constraints,
        None, &[],
    );
    Ok(split_step_response(&client.complete_large(&prompt)?))
}

#[allow(clippy::too_many_arguments)]
fn unit_test_stub_prompt(
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    test_file: &str,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    adrs: &[Adr],
    sibling_section: &str,
    tools: &[ToolSpec],
) -> String {
    let impl_file = &step.file;
    if impl_file.ends_with(".ts") || impl_file.ends_with(".tsx") {
        return unit_test_stub_prompt_ts(story, spec, contract_yaml, step, test_file, services, adrs, sibling_section, tools);
    }

    let service_name = step.service.rsplit('/').next().unwrap_or(&step.service);
    let pkg = service_packages.get(service_name)
        .cloned()
        .unwrap_or_else(|| service_name.replace('-', "_"));
    let pkg_path = pkg.replace('.', "/");

    let class_name = std::path::Path::new(impl_file.as_str())
        .file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
    let test_class = format!("{}Test", class_name);

    let layer = if impl_file.contains("/controller/") { "controller" }
        else if impl_file.contains("/service/") { "service" }
        else if impl_file.contains("/dto/") { "dto" }
        else if impl_file.contains("/domain/") { "domain" }
        else { "class" };

    let schema_yaml = spec.entity_schema.as_ref()
        .map(|s| serde_yaml::to_string(s).unwrap_or_default())
        .unwrap_or_default();
    let scenarios_yaml = serde_yaml::to_string(&spec.scenarios).unwrap_or_default();

    let service_entry = services.services.iter()
        .find(|s| s.name == service_name || s.name == step.service);
    let technology = service_entry.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
    // Structural rules (exact class shapes, package layout, jakarta.* namespace rules, etc.) —
    // the same skill the Green phase sees. Without this, the Red-phase test is written blind
    // to the class shape the skill mandates and has to guess before the implementation exists
    // to check against. Mirrors the TS path's tech_rules wiring below.
    let tech_rules = skill_for_technology(technology, &pkg, &pkg_path, service_name, layer);
    let test_skill = testing_skill_from_adrs(adrs, technology, layer);

    format!(
        "Generate a JUnit 5 unit test class '{test_class}' to drive TDD for '{impl_class}'.\n\
         \n\
         Implementation file : {impl_file}\n\
         Test file to create : {test_file}\n\
         Layer               : {layer}\n\
         Package base        : {pkg}\n\
         Service             : {service_name}\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         \n\
         Entity schema:\n\
         {schema_yaml}\n\
         BDD scenarios — one @Test method per scenario:\n\
         {scenarios_yaml}\n\
         \n\
         {tech_rules}\n\
         {test_skill}\n\
         \n\
         Method naming: should_<expected_outcome>_when_<condition>  (snake_case)\n\
         \n\
         Body structure:\n\
         // Arrange — build minimal valid inputs from entity schema field definitions\n\
         // Act     — call the method under test\n\
         // Assert  — verify the 'then' clause of the BDD scenario\n\
         \n\
         IMPORTANT:\n\
         - Write REAL assertions that verify actual behaviour.\n\
         - Tests will be Red naturally because the stub returns null/0/false.\n\
           The Green phase makes them pass. Do NOT use Assertions.fail().\n\
         - Package declaration: derive sub-package from the test file path.\n\
         - Import {impl_class} from its package under {pkg}.\n\
         \n\
         Write the raw Java file content first.\n\
         {contract}",
        test_class = test_class,
        impl_class = class_name,
        impl_file = impl_file,
        test_file = test_file,
        layer = layer,
        pkg = pkg,
        service_name = service_name,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        schema_yaml = schema_yaml,
        scenarios_yaml = scenarios_yaml,
        tech_rules = tech_rules,
        test_skill = test_skill,
        contract = canopy_summary_contract(),
    )
}

#[allow(clippy::too_many_arguments)]
fn unit_test_stub_prompt_ts(
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    test_file: &str,
    services: &ServicesRegistry,
    adrs: &[Adr],
    sibling_section: &str,
    tools: &[ToolSpec],
) -> String {
    let impl_file = &step.file;
    let is_component = impl_file.ends_with(".tsx");
    let service_name = step.service.rsplit('/').next().unwrap_or(&step.service);

    let module_name = std::path::Path::new(impl_file.as_str())
        .file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");

    let layer = detect_layer(impl_file);

    // Only the model layer constructs fields directly from the entity schema — every other
    // layer's test either doesn't touch entity fields at all, or (route) can derive boundary-
    // condition test data from the OAS contract instead, which already carries the same
    // validation constraints mapped onto its schema (max_length → maxLength, etc.).
    let schema_section = if layer == "model" {
        spec.entity_schema.as_ref()
            .map(|s| serde_yaml::to_string(s).unwrap_or_default())
            .map(|yaml| format!("Entity schema:\n{yaml}\n"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let scenarios_yaml = serde_yaml::to_string(&spec.scenarios).unwrap_or_default();

    let service_entry = services.services.iter()
        .find(|s| s.name == service_name || s.name == step.service);
    let technology = service_entry.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
    let test_skill = testing_skill_from_adrs(adrs, technology, layer);
    // Structural rules (exact class shapes, file layout, domain-event thin-shape rule, etc.) —
    // the same skill the Green phase sees. Without this, the Red-phase test is written blind
    // to any class shape the skill mandates (e.g. EventPublisher's constructor, event fields)
    // and has to guess, often wrongly, before the implementation exists to check against.
    let tech_rules = skill_for_technology(technology, "", "", service_name, layer);

    // Co-located: the test file lives in the SAME directory as its implementation, so the
    // import is always same-directory relative — no more crossing a tests/-to-src/ boundary.
    let import_path = format!("./{module_name}");

    let red_reason = if is_component {
        "Tests MUST be Red: the stub renders null so queries like getByRole/getByText will fail."
    } else {
        "Tests MUST be Red: the stub throws Error('not implemented') so all calls will reject."
    };

    let test_structure = if is_component {
        format!(
            "Test structure (React Testing Library):\n\
             describe('{module_name}', () => {{\n\
               it('should render <element> when <condition>', () => {{\n\
                 // Arrange — prepare any props or mock handlers\n\
                 // Act     — render(<{module_name} />)\n\
                 // Assert  — screen.getByRole(...) / screen.getByText(...)\n\
               }})\n\
             }})",
            module_name = module_name,
        )
    } else if layer_has_worked_example(technology, layer) {
        // This layer already gets a complete, correct worked example (imports, beforeEach,
        // mocks, and the exact assertion pattern) from the tech-stack testing skill above —
        // ask the skill itself rather than hand-copying a layer list here, so this stays
        // correct automatically as new stacks or new layer examples are added. Checked BEFORE
        // the "event"/"model" hand-written skeletons below: if the testing skill ever grows a
        // dedicated example for either (e.g. Node/Express gains an "event" entry), this branch
        // must win so the two don't end up sent to the model side by side — exactly the "Model
        // unit test example" + "Test structure (model...)" duplication this comment is fixing.
        // Do NOT also show a generic Arrange/Act/Assert skeleton here — it has no assertion
        // content of its own, and being the LAST structural template before the final
        // instruction, it displaces the specific example instead of reinforcing it.
        "Test structure: follow the worked example above EXACTLY — same imports, same \
         beforeEach shape, same assertion pattern (objectContaining/toMatchObject, never a \
         second factory call compared by deep-equality). Do not fall back to a generic \
         Arrange/Act/Assert skeleton; the example above is the structure.".to_string()
    // These two hand-written skeletons assume the Node/Express factory-function convention
    // (createWidget with positional args, "interface not a class") — gated on tech family, not
    // bare layer name, because detect_layer() returns the same "event"/"model" string for every
    // stack (e.g. Angular's own <feature>.model.ts is a plain interface with no factory at all,
    // per angular_skill() in tech_stack.rs, and gets its own TestBed-based skill as test_skill
    // instead). Without this gate, fixing detect_layer() to recognize Angular's file-suffix
    // convention would have silently routed Angular model/event files into this Node.js-specific
    // skeleton — the same unconditional-vs-gated contradiction shape as the other fixes above,
    // just reachable through a layer-detection fix instead of a missing gate.
    } else if layer == "event" && matches!(crate::tech::TechFamily::detect(technology), crate::tech::TechFamily::NodeExpress) {
        format!(
            "Test structure (domain event — THIN factory function, NOT `new`):\n\
             import {{ create{module_name} }} from '{import_path}'\n\
             \n\
             describe('create{module_name}', () => {{\n\
               it('should create with an eventId, the aggregate id, and occurredAt', () => {{\n\
                 // Act — call the factory with ONLY the aggregate's id (e.g. widgetId) as the argument.\n\
                 // A domain event takes ONE argument: the id of the aggregate it is about.\n\
                 const result = create{module_name}('aggregate-id-value')\n\
                 // Assert — a domain event has exactly these fields: eventId, <entity>Id, occurredAt\n\
                 expect(result.eventId).toEqual(expect.any(String))\n\
                 expect(result.occurredAt).toBeInstanceOf(Date)\n\
               }})\n\
             }})\n\
             CRITICAL: {module_name} is a THIN record — eventId (its own identity), the aggregate's\n\
             id (e.g. widgetId), and occurredAt ONLY. Do NOT pass or assert on the aggregate's\n\
             other fields (name, description, etc.) here, even ones you already know from\n\
             context — a consumer that needs them fetches the aggregate by its id. Do NOT add a\n\
             modifiedAt/updatedAt field — an event is a fact about one instant.\n\
             `new {module_name}()` will NOT compile — {module_name} is an interface, not a class.",
            module_name = module_name,
            import_path = import_path,
        )
    } else if layer == "model" && matches!(crate::tech::TechFamily::detect(technology), crate::tech::TechFamily::NodeExpress) {
        format!(
            "Test structure (model — factory function, NOT `new`):\n\
             import {{ create{module_name} }} from '{import_path}'\n\
             \n\
             describe('create{module_name}', () => {{\n\
               it('should create with all mandatory fields', () => {{\n\
                 // Act — call the factory function directly\n\
                 const result = create{module_name}(/* mandatory args */)\n\
                 // Assert\n\
                 expect(result.id).toEqual(expect.any(String))\n\
                 expect(result.createdAt).toBeInstanceOf(Date)\n\
               }})\n\
               it('should create with optional field included', () => {{\n\
                 const result = create{module_name}(/* mandatory args */, /* optional arg */)\n\
                 expect(result.optionalField).toBe('expected-value')\n\
               }})\n\
               it('should throw when a mandatory field is missing', () => {{\n\
                 // Same POSITIONAL call as every test above — cast ONLY the missing argument,\n\
                 // in its own position. NEVER collapse the call into a single object literal.\n\
                 expect(() => create{module_name}(undefined as any, /* remaining mandatory args, same order */)).toThrow('...')\n\
               }})\n\
             }})\n\
             CRITICAL: `new {module_name}()` will NOT compile — {module_name} is an interface, not a class.\n\
             WRONG for the missing-field test — collapses the positional call into one object:\n\
               const invalidPayload = {{ /* fields */ }} as any\n\
               expect(() => create{module_name}(invalidPayload)).toThrow(...)   ✗ this factory takes positional args, not one object",
            module_name = module_name,
            import_path = import_path,
        )
    } else {
        format!(
            "Test structure:\n\
             describe('{module_name}', () => {{\n\
               beforeEach(() => {{ /* set up mocks and subject */ }})\n\
               it('should <expected_outcome> when <condition>', async () => {{\n\
                 // Arrange\n\
                 // Act\n\
                 // Assert\n\
               }})\n\
             }})",
            module_name = module_name,
        )
    };

    let sibling_block = if sibling_section.is_empty() {
        String::new()
    } else {
        format!("Dependency types (use these exact field names in test data):\n{sibling_section}\n\n")
    };
    let tools_section = tools_hint_section(tools);

    // Infrastructure/repository/event/middleware files never receive invalid input in practice —
    // by the time an aggregate reaches EventPublisher.publish(), a repository's save(), or an
    // event factory, it has already been validated at the route (zod) or model (factory) layer;
    // middleware (errorHandler.ts) only formats whatever error already occurred, the same "no
    // validation responsibility" reasoning. Blindly telling the model to "cover every BDD
    // scenario" causes it to write a validation test against a layer that has no validation
    // responsibility at all — for an event specifically, it forces a "missing name" test that
    // can only pass if the event's factory accepts and re-validates the whole aggregate's fields,
    // directly violating the thin-event rule (eventId + <entity>Id + occurredAt only) and leaving
    // no implementation that can ever satisfy both the test and the skill at once (confirmed:
    // this produced an unwinnable Red fix loop with a real dogfooding project's ProductCreated.ts
    // — see CLAUDE.md's Diagnosing Dogfooding Runs section). The implementation otherwise
    // correctly does NOT throw, and the test fails for a reason that has nothing to do with a
    // real defect.
    let scenario_coverage_note = if layer == "infrastructure" || layer == "repository" || layer == "event" || layer == "middleware" {
        let job = match layer {
            "repository" => "the database access layer — store the data you receive as-is",
            "infrastructure" => "an infrastructure wrapper — pass the data you receive through to the external client unchanged",
            "event" => "the domain event's payload — describe a thin fact about the aggregate, not the aggregate itself",
            "middleware" => "the error-handling middleware — format whatever error already occurred, unchanged",
            _ => unreachable!(),
        };
        format!(
            "You are implementing {job} for this story.\n\
             ALWAYS assume the data you receive has already been validated upstream (by the model\n\
             factory or route boundary) — if you need to confirm exactly what's already enforced\n\
             there, use find_symbol/read_file on the model file instead of guessing. Your only job\n\
             is this layer's own responsibility, nothing more.\n\
             NEVER write a test for a missing/invalid-field or error-message scenario at this\n\
             layer, even though the BDD scenario list below includes one — skip it entirely.\n\
               WRONG — re-testing the factory's own validation at this layer:\n\
                 it('throws an error when name is not provided', async () => {{\n\
                   const invalid = createWidget(undefined as any, 'other-field-value')\n\
                   await expect(subject.saveWidget(invalid)).rejects.toThrow('name-value not provided...')\n\
                 }})\n\
               CORRECT: simply do not write a test for that scenario at this layer — it already has\n\
               one at the model layer, and calling the factory with invalid input above throws\n\
               immediately, before subject.saveWidget is ever reached, so the assertion never even\n\
               executes what it claims to test."
        )
    } else {
        "One describe/it block per scenario below — cover every scenario, do not skip or merge any.".to_string()
    };

    let route_rule = if layer == "route" {
        "- NEVER import from 'app.ts' or '../src/app' in a route test. ALWAYS import the router\n\
  from the implementation file — it is a Router INSTANCE, never a factory. Mount it on a local\n\
  Express instance and inject the mocked service via app.locals BEFORE mounting:\n\
    import router from '../src/routes/...'\n\
    const app = express()\n\
    app.use(express.json())\n\
    app.locals.widgetService = mockWidgetService\n\
    app.use('/widgets', router)\n\
  NEVER write `router(mockWidgetService)` — the route module has no factory to call.\n\
- ALWAYS match the mount path (e.g. '/widgets' above) to the OAS Contract and app.ts exactly —\n\
  NEVER invent a different prefix (e.g. '/api/...') unless the contract specifies one.\n"
    } else {
        ""
    };

    let contract_section = if (layer == "route" || layer == "api-client") && !contract_yaml.is_empty() {
        format!("OAS Contract — the route/endpoint path in your test MUST match this exactly:\n{contract_yaml}\n\n")
    } else {
        String::new()
    };

    // Boundary-condition (max length / max item count) validation only ever happens at the
    // model factory or the route's zod schema — the same reasoning scenario_coverage_note above
    // already applies to skip missing/invalid-field tests entirely for infrastructure/repository/
    // event layers. Sending this there would be dead weight: an instruction for an assertion the
    // model was just told, a few paragraphs prior in this same prompt, never to write. "service"
    // is included for the same reason missing_field_exception_rule includes it: the service
    // worked example calls the factory positionally (tech_stack.rs's Service rules), so a
    // boundary-condition BDD scenario reaching a service test transitively hits the same
    // real-data-not-mocked-length requirement.
    let boundary_rule = if layer == "model" || layer == "route" || layer == "service" {
        "         - Boundary conditions (max length, max items, etc.): ALWAYS construct REAL data that\n\
           naturally satisfies the condition. NEVER mock a language built-in to fake it —\n\
           String.prototype.length / Array.prototype.length are non-configurable; jest.spyOn(...,\n\
           'length', 'get') always throws \"Property 'length' is not declared configurable\":\n\
             WRONG: jest.spyOn(String.prototype, 'length', 'get').mockReturnValue(201)\n\
             CORRECT: const name = 'x'.repeat(201)   // an actual 201-character string\n\
             CORRECT: const tags = Array.from({ length: 6 }, (_, i) => `tag-${i}`)   // actually 6 items\n\
           An array field (e.g. `type: '[string]'`) can carry TWO independent validations —\n\
           `max_length` (each element's string length) and `max_items` (the array's item count).\n\
           ALWAYS write a separate test per declared condition, with boundary data matching that\n\
           condition. ALWAYS phrase the asserted message from the template for the constraint\n\
           under test — NEVER the other constraint's number or wording:\n\
             max_items test  → '<field> exceed maximum array item count of <max_items>'\n\
             max_length test → '<field> exceed maximum item length of <max_length>'\n\
             WRONG: testing max_items: 5 with 6 tags, asserting\n\
               'Tags exceed maximum item length of 100 characters'   // borrowed max_length's number+text\n\
             CORRECT: testing max_items: 5 with 6 tags, asserting\n\
               'Tags exceed maximum array item count of 5'\n".to_string()
    } else {
        String::new()
    };

    // Only the service layer takes a single options-object parameter (see the Service skill's
    // `Omit<Widget, 'id' | 'createdAt' | 'modifiedAt'>` shape in tech_stack.rs) — the model
    // factory takes positional args (its own worked example is under the "model" layer branch
    // above) and every other layer either has no missing-field scenario to test at all
    // (scenario_coverage_note above already forbids one for infrastructure/repository/event) or
    // isn't calling a typed in-process function with an object literal (route tests post an
    // untyped JSON body via supertest, not a TS-checked call). Sending this unconditionally
    // handed every layer a worked example of exactly the test scenario_coverage_note tells
    // infrastructure/repository/event layers never to write — a real contradiction confirmed
    // live: a repository-layer test copied this pattern almost verbatim, produced an unfixable
    // Red-phase failure (the factory throws before the repository method is ever reached), and
    // burned the whole Green-phase fix budget on an implementation that could never satisfy it.
    let missing_field_exception_rule = if layer == "service" {
        "         - EXCEPTION — testing a \"missing mandatory field\" scenario on a subject that takes ONE\n\
options-object parameter (e.g. a service method): TypeScript rejects an omitted required property\n\
at COMPILE time, before the test can even run the RUNTIME check it's meant to test. ALWAYS cast\n\
`as any` on the object literal to allow the deliberately-invalid call:\n\
             const invalidPayload = { manufacturer: 'Acme', model: 'X1' } as any\n\
             await expect(subject.createWidget(invalidPayload)).rejects.toThrow('name-value not provided...')\n\
           The cast only affects the value being constructed — it does NOT weaken the real runtime\n\
           check you are testing. (A positional-argument subject, e.g. a model factory, has its own\n\
           worked example above.)\n".to_string()
    } else {
        String::new()
    };

    // A pure factory (model/event) has no injected dependencies to mock — testing.rs's own
    // "model" worked example says so explicitly ("A pure factory has nothing to mock: NEVER
    // jest.mock(), jest.fn()..."). An unconditional "ALWAYS mock dependencies with jest.fn()"
    // bullet a few lines below would directly contradict that same-prompt instruction for these
    // two layers — the same unconditional-vs-gated contradiction shape as the other rules above.
    let mock_dependencies_rule = if layer == "model" || layer == "event" {
        String::new()
    } else {
        "         - ALWAYS mock dependencies with jest.fn().\n".to_string()
    };

    format!(
        "Generate a Jest test file to drive TDD for '{module_name}'.\n\
         \n\
         Implementation file : {impl_file}\n\
         Test file to create : {test_file}\n\
         Layer               : {layer}\n\
         Service             : {service_name}\n\
         Import path         : {import_path}\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         \n\
         {schema_section}\
         BDD scenarios:\n\
         {scenario_coverage_note}\n\
         {scenarios_yaml}\n\
         \n\
         {contract_section}\
         {sibling_block}\
         {tools_section}\
         {tech_rules}\n\
         {test_skill}\n\
         \n\
         {test_structure}\n\
         \n\
         IMPORTANT:\n\
         - Import the subject from '{import_path}'.\n\
         - The tech-stack rules above describe the EXACT shape of any file this skill governs\n\
           (e.g. a constructor signature, a domain event's fields). If the implementation file\n\
           you are testing is covered by one of those rules, your test data and mocks MUST\n\
           match that exact shape — do not guess a plausible-looking alternative.\n\
         - NEVER mock the subject under test, in either form:\n\
           (1) module-level: `jest.mock('{import_path}', ...)`.\n\
           (2) instance-level: `jest.spyOn(subject, 'methodName').mockResolvedValue(...)` on the\n\
               real '{module_name}' instance you constructed —\n\
             WRONG: jest.spyOn(subject, 'saveWidget').mockResolvedValue(fakeResult)\n\
                    const result = await subject.saveWidget(widget)\n\
                    expect(result).toEqual(fakeResult)  // proves nothing — you replaced the method\n\
           ALWAYS call the real method and assert on what it ACTUALLY does. Mocking/spying on\n\
           '{module_name}' is only correct in tests for its CONSUMERS — never its own test file.\n\
         - Write REAL assertions.\n\
         - {red_reason}\n\
           NEVER use expect.assertions(0) or skip assertions.\n\
{mock_dependencies_rule}\
         - jest is a global — NEVER import it.\n\
         - ALWAYS use plain string literals for test IDs (e.g. 'widget-1') — NEVER crypto or uuid imports.\n\
{boundary_rule}\
         - Test data objects MUST include every MANDATORY field from the dependency types above.\n\
{missing_field_exception_rule}\
         {route_rule}\
         \n\
         Write the raw TypeScript test file content first.\n\
         {contract}",
        module_name = module_name,
        impl_file = impl_file,
        test_file = test_file,
        layer = layer,
        service_name = service_name,
        import_path = import_path,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        schema_section = schema_section,
        scenarios_yaml = scenarios_yaml,
        contract_section = contract_section,
        tech_rules = tech_rules,
        test_skill = test_skill,
        test_structure = test_structure,
        red_reason = red_reason,
        route_rule = route_rule,
        scenario_coverage_note = scenario_coverage_note,
        boundary_rule = boundary_rule,
        missing_field_exception_rule = missing_field_exception_rule,
        mock_dependencies_rule = mock_dependencies_rule,
        contract = canopy_summary_contract(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn generate_unit_test_stub(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    test_file: &str,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    adrs: &[Adr],
    sibling_section: &str,
) -> Result<StepResult, LlmError> {
    let prompt = unit_test_stub_prompt(story, spec, contract_yaml, step, test_file, service_packages, services, adrs, sibling_section, &[]);
    Ok(split_step_response(&client.complete_large(&prompt)?))
}

/// Same contract as `generate_unit_test_stub`, but gives the model tools to call before
/// producing its final answer — e.g. checking a dependency's real signature via `find_symbol`
/// rather than inferring it purely from the entity schema. `dispatch` executes one tool call and
/// returns its result content; canopy-llm has no Roots dependency itself, so the actual lookup
/// logic stays in canopy-cli, called back through here.
#[allow(clippy::too_many_arguments)]
pub fn generate_unit_test_stub_with_tools(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    test_file: &str,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    adrs: &[Adr],
    sibling_section: &str,
    tools: &[ToolSpec],
    mut dispatch: impl FnMut(&ToolCall) -> String,
) -> Result<StepResult, LlmError> {
    let prompt = unit_test_stub_prompt(story, spec, contract_yaml, step, test_file, service_packages, services, adrs, sibling_section, tools);
    let mut messages = vec![ChatMessage::User(prompt)];

    for _ in 0..MAX_TOOL_ITERATIONS {
        match client.complete_with_tools(&messages, tools)? {
            ToolTurn::ToolCalls(calls) => {
                messages.push(ChatMessage::Assistant { content: None, tool_calls: calls.clone() });
                for call in &calls {
                    let result = dispatch(call);
                    messages.push(ChatMessage::Tool { tool_call_id: call.id.clone(), content: result });
                }
            }
            ToolTurn::FinalText(text) => return Ok(split_step_response(&text)),
        }
    }

    Err(LlmError::UnexpectedShape(format!(
        "exhausted {MAX_TOOL_ITERATIONS} tool-call iterations without a final answer for {test_file}"
    )))
}

pub fn execute_implementation_stub(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    sibling_section: &str,
    arch_skills: &str,
    test_file: &str,
    test_content: &str,
    package_constraints: Option<&str>,
    observed_call: Option<&str>,
) -> Result<StepResult, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, sibling_section, arch_skills,
        Some((test_file, test_content, true)), package_constraints,
        observed_call, &[],
    );
    Ok(split_step_response(&client.complete_large(&prompt)?))
}

/// Same contract as `execute_implementation_stub`, but gives the model tools to call before
/// producing its final answer — by stub time, the test and any sibling files already exist on
/// disk, so a real lookup (e.g. confirming a dependency's exact signature via `find_symbol`)
/// replaces another round of prompt-guessing. `dispatch` executes one tool call and returns its
/// result content; canopy-llm has no Roots dependency itself, so the actual lookup logic stays
/// in canopy-cli, called back through here.
#[allow(clippy::too_many_arguments)]
pub fn execute_implementation_stub_with_tools(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    sibling_section: &str,
    arch_skills: &str,
    test_file: &str,
    test_content: &str,
    package_constraints: Option<&str>,
    observed_call: Option<&str>,
    tools: &[ToolSpec],
    mut dispatch: impl FnMut(&ToolCall) -> String,
) -> Result<StepResult, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, sibling_section, arch_skills,
        Some((test_file, test_content, true)), package_constraints,
        observed_call, tools,
    );
    let mut messages = vec![ChatMessage::User(prompt)];

    for _ in 0..MAX_TOOL_ITERATIONS {
        match client.complete_with_tools(&messages, tools)? {
            ToolTurn::ToolCalls(calls) => {
                messages.push(ChatMessage::Assistant { content: None, tool_calls: calls.clone() });
                for call in &calls {
                    let result = dispatch(call);
                    messages.push(ChatMessage::Tool { tool_call_id: call.id.clone(), content: result });
                }
            }
            ToolTurn::FinalText(text) => return Ok(split_step_response(&text)),
        }
    }

    Err(LlmError::UnexpectedShape(format!(
        "exhausted {MAX_TOOL_ITERATIONS} tool-call iterations without a final answer for {}", step.file
    )))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_implementation_with_test(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    sibling_section: &str,
    arch_skills: &str,
    test_file: &str,
    test_content: &str,
    package_constraints: Option<&str>,
    observed_call: Option<&str>,
) -> Result<StepResult, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, sibling_section, arch_skills,
        Some((test_file, test_content, false)), package_constraints,
        observed_call, &[],
    );
    Ok(split_step_response(&client.complete_large(&prompt)?))
}

