use crate::client::{LlmClient, LlmError};
use crate::skills::{detect_layer, layer_has_worked_example, skill_for_build_system, skill_for_technology, testing_skill_from_adrs};
use canopy_core::*;

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
    let schema_yaml = if matches!(layer, "infrastructure" | "middleware" | "app" | "config" | "module") {
        String::new()
    } else {
        spec.entity_schema.as_ref()
            .map(|s| serde_yaml::to_string(s).unwrap_or_default())
            .unwrap_or_default()
    };
    let contract_yaml = if matches!(layer, "route" | "api-client" | "app") {
        contract_yaml
    } else {
        ""
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
            "\nCurrent file content (modify operation — preserve what stays, change what the description requires):\n\
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
               validation, no field assignment, no constructed return value. This applies to\n\
               a standalone factory function exactly the same as a class method.\n\
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
             - NEVER implement any logic, even logic you already know is correct — the Green\n\
               phase replaces this stub in a separate step.\n\
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

    format!(
        "Generate the complete content of file '{file}'.\n\
         \n\
         Operation: {operation}\n\
         Description: {description}\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         Service: {service} ({technology})\n\
         \n\
         Entity schema:\n\
         {schema_yaml}\n\
         OAS Contract:\n\
         {contract_yaml}\n\
         {sibling_section}\
         {current_section}\
         {roots_section}\
         {pkg_section}\
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
        schema_yaml = schema_yaml,
        contract_yaml = contract_yaml,
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
        None,
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
) -> String {
    let impl_file = &step.file;
    if impl_file.ends_with(".ts") || impl_file.ends_with(".tsx") {
        return unit_test_stub_prompt_ts(story, spec, contract_yaml, step, test_file, services, adrs, sibling_section);
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
         - Use jakarta.* everywhere — never javax.*\n\
         \n\
         Write the raw Java file content first — no code fences.\n\
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
) -> String {
    let impl_file = &step.file;
    let is_component = impl_file.ends_with(".tsx");
    let service_name = step.service.rsplit('/').next().unwrap_or(&step.service);

    let module_name = std::path::Path::new(impl_file.as_str())
        .file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");

    let layer = detect_layer(impl_file);

    let schema_yaml = spec.entity_schema.as_ref()
        .map(|s| serde_yaml::to_string(s).unwrap_or_default())
        .unwrap_or_default();
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
    } else if layer == "event" {
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
             id (e.g. widgetId), and occurredAt ONLY. The Entity schema below describes the\n\
             AGGREGATE, not this event — do NOT pass or assert on the aggregate's other fields\n\
             (name, description, etc.) here; a consumer that needs them fetches the aggregate by\n\
             its id. Do NOT add a modifiedAt/updatedAt field — an event is a fact about one instant.\n\
             `new {module_name}()` will NOT compile — {module_name} is an interface, not a class.",
            module_name = module_name,
            import_path = import_path,
        )
    } else if layer == "model" {
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
             }})\n\
             CRITICAL: `new {module_name}()` will NOT compile — {module_name} is an interface, not a class.",
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

    // Infrastructure/repository/event files never receive invalid input in practice — by the
    // time an aggregate reaches EventPublisher.publish(), a repository's save(), or an event
    // factory, it has already been validated at the route (zod) or model (factory) layer.
    // Blindly telling the model to "cover every BDD scenario" causes it to write a validation
    // test against a layer that has no validation responsibility at all — for an event
    // specifically, it forces a "missing name" test that can only pass if the event's factory
    // accepts and re-validates the whole aggregate's fields, directly violating the thin-event
    // rule (eventId + <entity>Id + occurredAt only) and leaving no implementation that can ever
    // satisfy both the test and the skill at once (confirmed: this produced an unwinnable Red
    // fix loop with a real dogfooding project's ProductCreated.ts — see CLAUDE.md's Diagnosing
    // Dogfooding Runs section). The implementation otherwise correctly does NOT throw, and the
    // test fails for a reason that has nothing to do with a real defect.
    let scenario_coverage_note = if layer == "infrastructure" || layer == "repository" || layer == "event" {
        "This layer never validates input — that happens upstream (model factory or route\n\
         boundary). NEVER write a test for a missing/invalid-field or error-message scenario at\n\
         this layer, even though the BDD scenario list below includes one — skip it entirely and\n\
         write tests only for this layer's own job.\n\
           WRONG — re-testing the factory's own validation at this layer:\n\
             it('throws an error when name is not provided', async () => {\n\
               const invalid = createWidget(undefined as any, 'other-field-value')\n\
               await expect(subject.saveWidget(invalid)).rejects.toThrow('name-value not provided...')\n\
             })\n\
           CORRECT: simply do not write a test for that scenario at this layer — it already has\n\
           one at the model layer, and calling the factory with invalid input above throws\n\
           immediately, before subject.saveWidget is ever reached, so the assertion never even\n\
           executes what it claims to test.".to_string()
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
  NEVER invent a different prefix (e.g. '/api/...') unless the contract specifies one.\n\
- ALWAYS mock only the service layer in a route test — NEVER repository or event publisher directly.\n"
    } else {
        ""
    };

    let contract_section = if (layer == "route" || layer == "api-client") && !contract_yaml.is_empty() {
        format!("OAS Contract — the route/endpoint path in your test MUST match this exactly:\n{contract_yaml}\n\n")
    } else {
        String::new()
    };

    let entity_schema_label = if layer == "event" {
        "Entity schema (describes the AGGREGATE only — this event's test data must NOT mirror\n\
         these fields; see the \"### Domain events\" section for the payload shape — eventId,\n\
         the aggregate's id, occurredAt ONLY):"
    } else {
        "Entity schema:"
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
         {entity_schema_label}\n\
         {schema_yaml}\n\
         BDD scenarios:\n\
         {scenario_coverage_note}\n\
         {scenarios_yaml}\n\
         \n\
         {contract_section}\
         {sibling_block}\
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
         - ALWAYS mock dependencies with jest.fn().\n\
         - jest is a global — NEVER import it.\n\
         - ALWAYS use plain string literals for test IDs (e.g. 'widget-1') — NEVER crypto or uuid imports.\n\
         - Boundary conditions (max length, max items, etc.): ALWAYS construct REAL data that\n\
           naturally satisfies the condition. NEVER mock a language built-in to fake it —\n\
           String.prototype.length / Array.prototype.length are non-configurable; jest.spyOn(...,\n\
           'length', 'get') always throws \"Property 'length' is not declared configurable\":\n\
             WRONG: jest.spyOn(String.prototype, 'length', 'get').mockReturnValue(201)\n\
             CORRECT: const name = 'x'.repeat(201)   // an actual 201-character string\n\
             CORRECT: const categories = Array.from({{ length: 6 }}, (_, i) => `cat-${{i}}`)   // actually 6 items\n\
           An array field (e.g. `type: '[string]'`) can carry TWO independent validations —\n\
           `max_length` (each element's string length) and `max_items` (the array's item count).\n\
           ALWAYS write a separate test per declared condition, with boundary data matching that\n\
           condition — NEVER borrow the other constraint's number or message text.\n\
         - Test data objects MUST include every MANDATORY field from the dependency types above.\n\
         - EXCEPTION — testing a \"missing mandatory field\" scenario: TypeScript rejects an\n\
omitted required property or `undefined` positional argument at COMPILE time, before the test\n\
can even run the RUNTIME check it's meant to test. ALWAYS cast `as any` at the narrowest point\n\
to allow the deliberately-invalid call — check the subject's ACTUAL signature first, then match\n\
ONE of these two shapes (never mix them):\n\
             Single options-object subject (e.g. a service method taking one payload param):\n\
               const invalidPayload = {{ manufacturer: 'Acme', model: 'X1' }} as any\n\
               await expect(subject.createWidget(invalidPayload)).rejects.toThrow('name-value not provided...')\n\
             Positional-argument subject (e.g. a model factory `createWidget(name, otherField, optionalField?)`):\n\
               cast ONLY the missing argument in its own position — do NOT collapse the call\n\
               into a single object literal:\n\
                 expect(() => createWidget(undefined as any, 'other-field-value')).toThrow('name-value not provided...')\n\
           The cast only affects the value(s) being constructed — it does NOT weaken the real\n\
           runtime check you are testing.\n\
         {route_rule}\
         \n\
         Write the raw TypeScript test file content first — no code fences.\n\
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
        entity_schema_label = entity_schema_label,
        schema_yaml = schema_yaml,
        scenarios_yaml = scenarios_yaml,
        contract_section = contract_section,
        tech_rules = tech_rules,
        test_skill = test_skill,
        test_structure = test_structure,
        red_reason = red_reason,
        route_rule = route_rule,
        scenario_coverage_note = scenario_coverage_note,
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
    let prompt = unit_test_stub_prompt(story, spec, contract_yaml, step, test_file, service_packages, services, adrs, sibling_section);
    Ok(split_step_response(&client.complete_large(&prompt)?))
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
        observed_call,
    );
    Ok(split_step_response(&client.complete_large(&prompt)?))
}

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
) -> Result<StepResult, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, sibling_section, arch_skills,
        Some((test_file, test_content, false)), package_constraints,
        None,
    );
    Ok(split_step_response(&client.complete_large(&prompt)?))
}

