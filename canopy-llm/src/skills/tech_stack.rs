// ── Tech-stack skills ────────────────────────────────────────────────────────
// Every tech-stack skill implements the same three-section contract:
//   1. file_layout    — where files live and what each directory means
//   2. namespace_rules — allowed/forbidden imports with examples
//   3. layer_order    — the sequence in which files must be generated (dependency order)
//
// `notes` is optional: fix-loop guidance, scope constraints, etc.
//
// To add a new stack: implement a builder function that returns TechStackSkill,
// fill all three required fields, add a match arm in skill_for_technology().

/// Layer keys used by `layer_rules`, in the order they should appear when rendering all of
/// them together (`render_all_layers`) — roughly the same order files are generated in.
const LAYER_KEYS: &[&str] = &[
    "model", "event", "repository", "infrastructure", "service",
    "route", "middleware", "app", "config",
];

pub(crate) struct TechStackSkill {
    pub name: String,
    /// Where files live; directory conventions; one-type-per-file rules.
    pub file_layout: String,
    /// Legacy shape: full import/namespace rules text, shown in full regardless of layer.
    /// Populated by skills not yet migrated to the layer-partitioned shape below — leave empty
    /// to opt a skill into `common_rules`/`layer_rules` instead.
    pub namespace_rules: String,
    /// Layer-partitioned shape: rules that apply to every file regardless of layer (import
    /// conventions, exports, etc.).
    pub common_rules: String,
    /// Layer-partitioned shape: rules specific to one layer, keyed by `detect_layer()`'s output.
    /// Only the entry for the file's own layer is injected — a model file never sees repository
    /// or route rules it has no use for.
    pub layer_rules: std::collections::HashMap<&'static str, String>,
    /// Ordered list of layers with rationale — the LLM generates in this sequence.
    pub layer_order: String,
    /// Optional extra rules: scope constraints, fix-loop guidance, etc.
    pub notes: Option<String>,
}

impl TechStackSkill {
    /// Builds the import-rules body for one layer (or, when `layer` is `None`, every layer
    /// concatenated in `LAYER_KEYS` order). Legacy skills that still populate `namespace_rules`
    /// ignore the layer entirely and always return the full text — this keeps skills that
    /// haven't been migrated to the partitioned shape behaving exactly as before.
    fn rules_body(&self, layer: Option<&str>) -> String {
        if !self.namespace_rules.is_empty() {
            return self.namespace_rules.clone();
        }
        let mut parts = Vec::new();
        if !self.common_rules.is_empty() {
            parts.push(self.common_rules.clone());
        }
        match layer {
            Some(l) => {
                if let Some(text) = self.layer_rules.get(l).filter(|s| !s.is_empty()) {
                    parts.push(text.clone());
                }
            }
            None => {
                for key in LAYER_KEYS {
                    if let Some(text) = self.layer_rules.get(key).filter(|s| !s.is_empty()) {
                        parts.push(text.clone());
                    }
                }
            }
        }
        parts.join("\n\n")
    }

    fn render_with_body(&self, body: &str) -> String {
        let mut sections = vec![("### File layout", self.file_layout.as_str())];
        if !body.is_empty() {
            sections.push(("### Import rules", body));
        }
        sections.push(("### Layer order", self.layer_order.as_str()));
        if let Some(n) = self.notes.as_deref().filter(|n| !n.is_empty()) {
            sections.push(("### Additional rules", n));
        }
        super::render_skill(&format!("## Tech stack: {}", self.name), &sections)
    }

    /// Render scoped to one layer — only the rules relevant to the file being generated.
    pub(crate) fn render_for_layer(&self, layer: &str) -> String {
        self.render_with_body(&self.rules_body(Some(layer)))
    }

    /// Render every layer's rules together — for contexts not tied to a single file
    /// (e.g. proposing dependencies for a whole service).
    pub(crate) fn render_all_layers(&self) -> String {
        self.render_with_body(&self.rules_body(None))
    }

    /// Lightweight render for the planning phase.
    /// Omits namespace_rules and notes — those are implementation concerns.
    /// Planning only needs the directory map and layer ordering to enumerate files and depends_on.
    pub(crate) fn render_for_planning(&self) -> String {
        super::render_skill(&format!("## Tech stack: {}", self.name), &[
            ("### File layout", self.file_layout.as_str()),
            ("### Layer order", self.layer_order.as_str()),
        ])
    }
}

fn spring_boot_skill(pkg: &str, pkg_path: &str, service_name: &str) -> TechStackSkill {
    TechStackSkill {
        name: "Spring Boot 3 (Jakarta EE)".to_string(),
        file_layout: format!(
            "  Build file:  services/{sn}/pom.xml\n\
             Source root: services/{sn}/src/main/java/{pp}/\n\
             Test root:   services/{sn}/src/test/java/{pp}/\n\
             Layers:      {p}.domain  {p}.repository  {p}.dto  {p}.service  {p}.controller\n\
             One public type per .java file; file name must match the class name exactly.\n\
             @SpringBootApplication lives in {p} directly — never inside a sub-package.",
            sn = service_name, pp = pkg_path, p = pkg
        ),
        namespace_rules: format!(
            "  jakarta.* everywhere — NEVER import javax.* (will not compile under Jakarta EE 9+)\n\
             - jakarta.servlet.http.HttpServletRequest  (NOT javax.servlet.http.HttpServletRequest)\n\
             - jakarta.validation.constraints.*  (@NotBlank, @NotNull, @Positive, ...)\n\
             - jakarta.persistence.*  (@Entity, @Id, @GeneratedValue, @Column, ...)\n\
             - jakarta.annotation.*  (@PostConstruct, ...)\n\
             Every package declaration must be exactly {p} or a sub-package of it.",
            p = pkg
        ),
        common_rules: String::new(),
        layer_rules: std::collections::HashMap::new(),
        layer_order: format!(
            "  1. services/{sn}/pom.xml     — complete Maven POM; must end with </project>\n\
             2. {pp}/domain/         — @Entity classes with @Id and @GeneratedValue\n\
             3. {pp}/repository/     — JpaRepository interfaces\n\
             4. {pp}/dto/            — request/response classes with validation annotations\n\
             5. {pp}/service/        — @Service business logic\n\
             6. {pp}/controller/     — @RestController endpoints matching OAS contract\n\
             7. src/test/**/*IT.java — @SpringBootTest integration tests (end-to-end only)\n\
                Do NOT plan *Test.java files — the TDD loop generates them automatically.\n\
             Reason: each layer imports from the one above; generate strictly in this order.",
            sn = service_name, pp = pkg_path
        ),
        notes: Some(format!(
            "  pom.xml required starters: spring-boot-starter-web, spring-boot-starter-data-jpa,\n\
             spring-boot-starter-validation, h2 (runtime scope), spring-boot-starter-test (test scope).\n\
             (Maven structure and dependency validity rules are in the Maven build skill below.)\n\
             Integration tests: import DTOs from {p}.dto — never define local classes that shadow them.\n\
             Include all java.util.* and annotation imports. Test only OAS-declared endpoints.\n\
             Validation annotation type safety:\n\
             - @Positive / @Min / @Max / @DecimalMin / @DecimalMax: ALWAYS numeric types\n\
               (int, Integer, long, Long, BigDecimal, Double). NEVER String, List, Set, Collection.\n\
             - Non-null, non-empty collection: ALWAYS @NotNull + @NotEmpty. NEVER @Positive.\n\
             - Non-blank string: ALWAYS @NotBlank. NEVER @NotNull alone.\n\
             - Non-null object reference: ALWAYS @NotNull.",
            p = pkg
        )),
    }
}

fn react_vite_skill() -> TechStackSkill {
    TechStackSkill {
        name: "React + TypeScript (Vite)".to_string(),
        file_layout:
            "  All .ts/.tsx files live under <service-prefix>/src/\n\
             Canonical layout for one story:\n\
             - <prefix>/src/api/<Entity>Api.ts         — typed fetch() client + interfaces\n\
             - <prefix>/src/components/<Entity>Form.tsx — controlled form component\n\
             - <prefix>/src/App.tsx                    — renders the form\n\
             File paths in plan steps are relative to the PROJECT ROOT;\n\
             always include the full prefix (e.g. frontend/admin-portal/src/api/WidgetApi.ts).\n\
             Test files (*.test.ts/.tsx) are co-located NEXT TO the file they test, in the same\n\
             directory — e.g. src/api/WidgetApi.test.ts, never a separate tests/ root.\n\
             \n\
             FILE EXTENSION RULES — STRICTLY ENFORCED:\n\
             .ts  files are PURE TYPESCRIPT: interfaces, types, and plain functions ONLY.\n\
                  NO JSX, NO React.FC, NO HTML elements, NO useState/useEffect.\n\
                  src/api/<Entity>Api.ts contains only:\n\
                    export interface WidgetRegistrationRequest { ... }\n\
                    export async function registerWidget(data: WidgetRegistrationRequest): Promise<Widget> { ... }\n\
             .tsx files contain JSX — components, form elements, event handlers.\n\
                  EVERY file that contains <JSX> syntax MUST have the .tsx extension.\n\
             Mixing JSX into a .ts file causes: error TS1005: '>' expected (unrecoverable parse error)."
            .to_string(),
        namespace_rules:
            "  Imports are relative to the file's position inside src/:\n\
             - App.tsx:          import ProductForm from './components/ProductForm'\n\
             - ProductForm.tsx:  import { registerProduct } from '../api/ProductApi'\n\
             Never use '../../' — all source files are siblings or children within src/.\n\
             HTTP: use fetch() only — no axios, ky, or any other HTTP library.\n\
             Do not import a file that does not exist yet.\n\
             A file MUST NOT import from its own path — no self-imports.\n\
             NEVER import React explicitly (\"import React from 'react'\") — the automatic JSX transform puts it in scope already."
            .to_string(),
        common_rules: String::new(),
        layer_rules: std::collections::HashMap::new(),
        layer_order:
            "  1. src/api/<Entity>Api.ts         — request/response interfaces + fetch function\n\
             2. src/components/<Entity>Form.tsx  — controlled form; manages its own state; accepts NO props\n\
             3. src/App.tsx                      — imports and renders the form with NO props: <WidgetForm />\n\
             Test files are not a separate layer step — the TDD cycle generates one\n\
             automatically, co-located next to each file above, as that file's own step runs.\n\
             Reason: each file imports from the previous; generating out of order causes type mismatches."
            .to_string(),
        notes: Some(
            "  STRICT SCOPE — do NOT add unless the story explicitly requires it:\n\
             custom hooks, page components, route files, Redux/Zustand slices,\n\
             utility modules, CSS files, or any abstraction not named in the acceptance criteria.\n\
             The form component handles its own state and calls the API client directly.\n\
             App.tsx ONLY renders the form — it does NOT manage form state or pass props:\n\
               return <div><h1>...</h1><WidgetForm /></div>   ✓  no props\n\
               return <div><WidgetForm formData={...} onSubmit={...} /></div>  ✗  form owns its state\n\
             The FIRST LINE of every file MUST be valid TypeScript/TSX code.\n\
             NEVER write a language label ('tsx', 'typescript', 'ts') as the first line.\n\
             TS2322 on a JSX element: ALWAYS remove the offending props (and any state/handlers\n\
             that only fed them) from the caller in THIS file. NEVER modify the component to\n\
             accept them — React.FC / React.FC<{}> with no type parameter accepts NO props."
            .to_string()
        ),
    }
}

fn angular_skill() -> TechStackSkill {
    TechStackSkill {
        name: "Angular".to_string(),
        file_layout:
            "  Source root: <service-prefix>/src/app/\n\
             Feature folder per domain concept (one folder per entity/use-case):\n\
             - src/app/<feature>/<feature>.module.ts\n\
             - src/app/<feature>/<feature>.service.ts\n\
             - src/app/<feature>/<feature>.component.ts / .html\n\
             - src/app/<feature>/<feature>.model.ts\n\
             File paths in plan steps are relative to the PROJECT ROOT."
            .to_string(),
        namespace_rules:
            "  Import only from Angular packages and local files:\n\
             - @angular/core        (@Component, @Injectable, @Input, @OnInit, ...)\n\
             - @angular/common/http (HttpClient, HttpClientModule)\n\
             - @angular/forms       (FormBuilder, Validators, ReactiveFormsModule)\n\
             Never call fetch() directly — inject HttpClient and use typed generics:\n\
               this.http.post<ProductResponse>('/products', body)\n\
             Services: @Injectable({ providedIn: 'root' }) unless feature-lazy-loaded."
            .to_string(),
        common_rules: String::new(),
        layer_rules: std::collections::HashMap::new(),
        layer_order:
            "  1. <feature>.model.ts      — TypeScript interfaces (no Angular deps)\n\
             2. <feature>.service.ts     — @Injectable; imports HttpClient and model\n\
             3. <feature>.module.ts      — NgModule; imports HttpClientModule, ReactiveFormsModule\n\
             4. <feature>.component.ts   — @Component; injects service\n\
             5. <feature>.component.html — template; no logic, only bindings\n\
             Reason: component depends on service; service depends on model."
            .to_string(),
        notes: Some(
            "  Prefer reactive forms (FormBuilder) over template-driven for non-trivial inputs.\n\
             Use RxJS operators (map, catchError) in service methods; subscribe in components.\n\
             Unsubscribe in ngOnDestroy or use the async pipe to avoid memory leaks."
            .to_string()
        ),
    }
}

fn node_express_skill() -> TechStackSkill {
    TechStackSkill {
        name: "Node.js / Express (TypeScript)".to_string(),
        file_layout:
            "  Source root: <service-prefix>/src/\n\
             - src/models/           — one interface + one factory function per aggregate; \
factory assigns id via randomUUID() from Node.js built-in 'crypto'; NO imports from npm packages\n\
             - src/events/           — domain event interfaces (one file per event, e.g. WidgetCreated.ts)\n\
             - src/repositories/     — data access layer; all database calls live here; no Express imports\n\
             - src/infrastructure/   — infrastructure utilities (e.g. EventPublisher.ts wrapping kafkajs)\n\
             - src/services/         — business logic; depends on repositories and infrastructure; no Express imports\n\
             - src/routes/           — Express routers; thin request/response handling; validate with zod\n\
             - src/middleware/       — cross-cutting (errorHandler, auth, logging)\n\
             - src/app.ts            — builds and exports the Express app; MUST NOT call app.listen()\n\
             - src/index.ts          — entry point; imports app and calls app.listen()\n\
             Test files (*.test.ts) are co-located NEXT TO the file they test, in the same\n\
             directory — e.g. src/services/WidgetService.test.ts, never a separate tests/ root.\n\
             File paths in plan steps are relative to the PROJECT ROOT."
            .to_string(),
        namespace_rules: String::new(),
        common_rules:
            "  ES module imports throughout — never use require().\n\
             NEVER use TypeScript path aliases (@services/..., @app/..., @src/..., etc.) — \
             there is no paths config in tsconfig.json; use only relative imports.\n\
             \n\
             #### Deriving paths from depends_on\n\
             depends_on entries are PROJECT-ROOT paths, not import specifiers.\n\
             Example: generating src/repositories/WidgetRepository.ts with\n\
               depends_on: [\"services/<name>/src/models/Widget.ts\"]\n\
             Correct import:  import { Widget } from '../models/Widget'     ✓  relative from repositories/ to models/\n\
             WRONG:           import { Widget } from '@services/<name>/src/models/Widget'  ✗  path alias — invalid\n\
             WRONG:           import { Widget } from 'services/<name>/src/models/Widget'   ✗  not a node module\n\
             ALWAYS strip the service prefix (e.g. services/<name>/) from both paths first, then\n\
             compute the relative path between the two src/ locations.\n\
             \n\
             #### Import depth within src/\n\
             All src/ subdirectories are siblings — one dot-dot only:\n\
               src/services/WidgetService.ts → import { Widget } from '../models/Widget'       ✓\n\
               src/services/WidgetService.ts → import { WidgetRepository } from '../repositories/WidgetRepository' ✓\n\
             WRONG — two dots leaves src/ entirely and reaches the project root:\n\
               src/services/WidgetService.ts → import { Widget } from '../../models/Widget'    ✗\n\
               src/services/WidgetService.ts → import ... from '../../infrastructure/...'      ✗\n\
             \n\
             #### isolatedModules — import type\n\
             tsconfig has `isolatedModules: true`. Any import used ONLY as a type annotation\n\
             MUST use `import type`:\n\
               import type { Product } from '../models/Product'   ✓  (type-only use)\n\
               import { createProduct } from '../models/Product'  ✓  (value use — factory call)\n\
             Using a plain `import { T }` for a type-only symbol causes TS1484.\n\
             \n\
             #### No external utilities\n\
             RUNTIME ERROR — importing moment, uuid, nanoid, or any package absent from package.json\n\
             will crash the process. Check package.json before using any package.\n\
               Timestamps: new Date()             ✓  (built-in)\n\
               IDs:        randomUUID()           ✓  — import { randomUUID } from 'crypto'  (Node.js built-in, no npm install)\n\
             \n\
             ### Exports\n\
             All source files use NAMED exports:\n\
               export class WidgetService { ... }      ✓\n\
               export interface Widget { ... }          ✓\n\
               export const errorHandler = ...         ✓\n\
             EXCEPTION: src/app.ts uses default export: export default app\n\
             NEVER: export default class WidgetService  ✗  causes TS2613/TS2614 in importers"
            .to_string(),
        layer_rules: std::collections::HashMap::from([
            ("model",
             "  ### Models\n\
             A model file exports one interface AND one standalone factory function:\n\
               import { randomUUID } from 'crypto'\n\
               export interface Widget { id: string; createdAt: Date; ... }\n\
               export function createWidget(name: string, ...): Widget {\n\
                 return { id: randomUUID(), createdAt: new Date(), name, ... }\n\
               }\n\
             ALWAYS include only fields the entity schema lists. NEVER default a system-generated\n\
             field (e.g. modifiedAt) to null — derive it from the story's acceptance criteria\n\
             (e.g. \"system sets modifiedAt to the current timestamp\" means `new Date()` at\n\
             construction). NEVER import npm packages here — built-in Node APIs only ('crypto', 'path').\n\
             \n\
             #### Optional fields\n\
             Declare optional fields with `?: Type` — NEVER with `field: Type | undefined`:\n\
               description?: string             ✓\n\
               description: string | undefined  ✗  a REQUIRED key, not an optional one\n\
             NEVER call Widget.create() — Widget is an interface; interfaces have no static methods.\n\
             Callers import and call the factory function: import { createWidget } from '../models/Widget'"
             .to_string()),
            ("event",
             "  ### Domain events\n\
             ALWAYS treat an event as a thin, immutable record that something happened. NEVER\n\
             copy the aggregate's schema into it — ignore the Entity schema's field list here;\n\
             it describes the aggregate, not the event payload.\n\
               import { randomUUID } from 'crypto'\n\
               export interface WidgetCreated {\n\
                 eventId: string;\n\
                 widgetId: string;\n\
                 occurredAt: Date;\n\
               }\n\
               export function createWidgetCreated(widgetId: string): WidgetCreated {\n\
                 return { eventId: randomUUID(), widgetId, occurredAt: new Date() };\n\
               }\n\
             RULES:\n\
             - Fields are exactly: eventId (the event's own identity), <entity>Id (reference to the\n\
               aggregate, e.g. widgetId — a plain id, not a URI), and occurredAt.\n\
             - NEVER copy the aggregate's other fields (name, description, etc.) into the event —\n\
               a consumer that needs them fetches the aggregate by <entity>Id.\n\
             - NEVER add a modifiedAt/updatedAt field — an event is a fact about one instant, it is\n\
               never updated after it occurs."
             .to_string()),
            ("repository",
             "  ### Repository\n\
             RESPONSIBILITY: persistence ONLY.\n\
             ALWAYS generate ONLY the method(s) the current story requires — NEVER add a\n\
             find/list/delete method speculatively just because the entity could support one.\n\
             A story that only registers a new entity needs exactly ONE method:\n\
               async saveWidget(widget: Widget): Promise<Widget>          // create / update\n\
             A LATER story that needs a lookup, listing, or delete names it accordingly — but\n\
             ONLY generate that method when such a story actually requires it:\n\
               async findWidgetById(id: string): Promise<Widget | null>   // read by id — only if needed\n\
               async findWidgets(): Promise<Widget[]>                     // list — only if needed\n\
               async deleteWidget(id: string): Promise<void>              // delete — only if needed\n\
             Method naming: save<Entity>, find<Entity>ById, find<Entity>s, delete<Entity>.\n\
             ALWAYS use real 'pg' persistence against PostgreSQL. NEVER use an in-memory\n\
             array/object/Map or a stub comment (\"// Simulate database save\") in its place.\n\
             ALWAYS accept a pg.Pool via the constructor — app.ts owns and injects it, the same\n\
             way it injects EventPublisher:\n\
               import { Pool } from 'pg'\n\
               export class WidgetRepository {\n\
                 constructor(private pool: Pool) {}\n\
                 async saveWidget(widget: Widget): Promise<Widget> {\n\
                   await this.pool.query(\n\
                     'INSERT INTO widgets (id, name, optional_field, created_at, modified_at) ' +\n\
                     'VALUES ($1, $2, $3, $4, $5) ' +\n\
                     'ON CONFLICT (id) DO UPDATE SET name = $2, optional_field = $3, modified_at = $5',\n\
                     [widget.id, widget.name, widget.optionalField ?? null, widget.createdAt, widget.modifiedAt]\n\
                   )\n\
                   return widget\n\
                 }\n\
               }\n\
             RULES:\n\
             - NEVER generate ids or timestamps — the factory already assigned them.\n\
             - NEVER have a createWidget() or create() method — that is the factory function in the model file.\n\
             - NEVER publish events — that is the service's responsibility.\n\
             - NEVER call EventPublisher — repositories are unaware of events.\n\
             - Always use parameterized queries ($1, $2, ...) — never string-concatenate values into a query.\n\
             - Public method names MUST match what the service's unit test mocks declare."
             .to_string()),
            ("infrastructure",
             "  ### EventPublisher\n\
             ALWAYS use this exact class shape — it wraps kafkajs:\n\
             import { Kafka, Producer } from 'kafkajs';\n\
             export class EventPublisher {\n\
               private producer: Producer;\n\
               constructor(private kafka: Kafka, private topic: string) {\n\
                 this.producer = kafka.producer();\n\
               }\n\
               async connect(): Promise<void> { await this.producer.connect(); }\n\
               async disconnect(): Promise<void> { await this.producer.disconnect(); }\n\
               async publish<T>(event: T): Promise<void> {\n\
                 await this.producer.send({ topic: this.topic, messages: [{ value: JSON.stringify(event) }] });\n\
               }\n\
             }\n\
             RULES:\n\
             - publish<T>(event: T) takes ONE argument — the event. Topic is in the constructor.\n\
             - NO domain-type imports (no ProductCreated, no entity interfaces) — EventPublisher is generic.\n\
             - NEVER add topic as a parameter of publish().\n\
             Callers: await this.eventPublisher.publish(event)  ✓\n\
             WRONG:   await this.eventPublisher.publish('topic', event)  ✗"
             .to_string()),
            ("service",
             "  ### Service\n\
             RESPONSIBILITY: business logic — orchestrates the factory, repository, and event\n\
             publisher for one use case. ALWAYS accept ONLY the fields the caller supplies —\n\
             NEVER the full entity type (it also carries factory-assigned fields: id, createdAt,\n\
             modifiedAt):\n\
               WRONG:   async createWidget(widgetData: Widget): Promise<Widget> { ... }  ✗\n\
               CORRECT: async createWidget(\n\
                          widgetData: Omit<Widget, 'id' | 'createdAt' | 'modifiedAt'>\n\
                        ): Promise<Widget> { ... }\n\
             ALWAYS match the parameter type to the factory's actual signature (check the\n\
             sibling context above) — NEVER guess a different shape.\n\
             \n\
             ALWAYS call the factory with POSITIONAL arguments (see the Model rule above),\n\
             destructuring the method's own parameter — NEVER a single object. NEVER generate or\n\
             pass id/createdAt/modifiedAt yourself, and NEVER import randomUUID/crypto/uuid into\n\
             the service — only the factory touches id generation:\n\
               WRONG:\n\
                 import { v4 as randomUUID } from 'crypto';  ✗ crypto has no v4 export\n\
                 const widget = createWidget({ ...widgetData, id: randomUUID(), createdAt: new Date() });  ✗ wrong call shape AND wrong responsibility\n\
               CORRECT:\n\
                 const widget = createWidget(widgetData.name, widgetData.otherField, widgetData.optionalField);\n\
             RULES:\n\
             - Call the factory to construct the aggregate, call the repository to persist it,\n\
               then call the event publisher — in that order.\n\
             - NEVER assign an id or timestamp directly in a service method — only the factory does.\n\
             - connect() the event publisher, publish(), then disconnect() — do not leave it\n\
               connected after the method returns."
             .to_string()),
            ("route",
             "  ### Route handlers\n\
             ALWAYS export a Router instance as the default export — NEVER a factory function.\n\
             app.ts imports it as a default import and mounts it directly:\n\
               const router = Router()       ✓\n\
               export default router         ✓\n\
               export const registerRoutes = (router: Router) => { ... }   ✗ app.ts and\n\
                 tests both do `import router from './widgets'` — a factory breaks that import\n\
             ALWAYS declare next in the handler signature:\n\
               router.post('/', async (req: Request, res: Response, next: NextFunction) => {\n\
             NEVER repeat the resource name in this file's own route path — it's relative to\n\
             wherever app.ts mounts this router, not the resource name again:\n\
               WRONG:   router.post('/widgets', ...)   ✗ mounted at /widgets → accessible at /widgets/widgets\n\
               CORRECT: router.post('/', ...)          ✓ mounted at /widgets → accessible at /widgets\n\
               router.get('/:id', ...)                 ✓ sub-resource — mounted at /widgets → /widgets/:id\n\
             ALWAYS pass errors to next(err) — NEVER catch-and-respond in the route body.\n\
             ALWAYS validate input at the route boundary with zod:\n\
             - Define a zod schema in the route file; field names MUST match the domain\n\
               interface exactly (e.g. `categories`, not `categoryIds`, if the domain uses `categories`).\n\
             - ALWAYS use .optional() for optional fields. NEVER .nullable() or .nullable().optional().\n\
             - ALWAYS call .optional() LAST in the chain, after all constraints (ZodOptional has\n\
               no .max()/.min()):\n\
               z.string().max(1000).optional()                    ✓\n\
               z.string().optional().max(1000)                    ✗  RUNTIME ERROR: no .max() on ZodOptional\n\
             - For array fields: z.array(z.string().max(100)).max(5) — zod arrays have no\n\
               .maxLength(); ALWAYS use .max() on both the array and its elements.\n\
             - Call schema.parse(req.body); pass errors to next(err).\n\
             ALWAYS use async/await — NEVER raw .then() chains.\n\
             NEVER instantiate EventPublisher or any infrastructure class in a route handler —\n\
             the service (from req.app.locals) owns all business logic, including publishing.\n\
             Route responsibility: validate input → call service → return HTTP response. Nothing else."
             .to_string()),
            ("middleware",
             "  ### Error handling\n\
             src/middleware/errorHandler.ts exports a named ErrorRequestHandler:\n\
               import { ErrorRequestHandler } from 'express'\n\
               export const errorHandler: ErrorRequestHandler = (err, req, res, next) => { ... }\n\
             app.ts imports it as: import { errorHandler } from './middleware/errorHandler'\n\
             NEVER use default export for errorHandler — app.ts must destructure it by name.\n\
             import { ZodError } from 'zod' — use instanceof ZodError,\n\
             NOT z.ZodError (z is not imported in middleware; ZodError is a named export from 'zod').\n\
             NEVER store EventPublisher as a private field the route accesses. ALWAYS construct\n\
             it as a local variable in the route handler — connect/disconnect belong to the caller."
             .to_string()),
            ("config",
             "  ### tsconfig.json\n\
             ALWAYS use EXACTLY this structure. NEVER add, remove, or \"improve\" any\n\
             compilerOption, even a stricter one. Test files are co-located under src/, so one\n\
             include entry covers everything:\n\
             {\n\
               \"compilerOptions\": {\n\
                 \"target\": \"ES2020\",\n\
                 \"module\": \"node16\",\n\
                 \"lib\": [\"ES2020\"],\n\
                 \"strict\": true,\n\
                 \"isolatedModules\": true,\n\
                 \"esModuleInterop\": true,\n\
                 \"skipLibCheck\": true,\n\
                 \"resolveJsonModule\": true,\n\
                 \"moduleResolution\": \"node16\",\n\
                 \"types\": [\"jest\", \"node\"]\n\
               },\n\
               \"include\": [\"src/**/*\"],\n\
               \"exclude\": [\"node_modules\"]\n\
             }"
             .to_string()),
            ("app",
             "  ### App structure\n\
             ALWAYS import and register every router/middleware module created by this plan —\n\
             an unwired module is dead code.\n\
             ALWAYS export the Express app instance directly — NEVER wrap it in a factory\n\
             function or class:\n\
               const app = express()         ✓\n\
               export default app            ✓\n\
               export default function createApp() { ... }   ✗  callers get a function, not an app\n\
             ALWAYS use express.json() for body parsing. NEVER import 'body-parser' (not in\n\
             package.json — MODULE_NOT_FOUND at runtime):\n\
               app.use(express.json())      ✓  built into Express\n\
               import bodyParser from 'body-parser'  ✗  not in package.json\n\
             Every router is a default export (see Route handlers) — import it as\n\
             `import widgetRouter from './routes/widgets'`, NEVER as a named import.\n\
             ALWAYS import middleware using its actual export style:\n\
               import { errorHandler } from './middleware/errorHandler'   ✓  (named export)\n\
               import errorHandler from './middleware/errorHandler'        ✗  default import of named export → undefined\n\
             Middleware order: routers first, error-handling middleware last.\n\
             ALWAYS build/export the app in app.ts WITHOUT calling app.listen() — ONLY index.ts\n\
             calls it, so Supertest can import app without starting a server.\n\
             ALWAYS create service/repository instances (via new) in app.ts and register them on\n\
             app.locals — routers access them via req.app.locals, no constructor arguments needed.\n\
             A repository backed by PostgreSQL needs the shared Pool passed in — app.ts creates\n\
             ONE Pool from the connection string and injects it into every repository:\n\
               import { Pool } from 'pg'\n\
               import { WidgetRepository } from './repositories/WidgetRepository'\n\
               const pool = new Pool({ connectionString: process.env.DATABASE_URL })\n\
               app.locals.widgetRepository = new WidgetRepository(pool)\n\
             \n\
             ### Router mount paths\n\
             ALWAYS mount every router with its resource path — NEVER without one:\n\
               app.use('/widgets', widgetRouter)    ✓  POST /widgets → router.post('/', ...)\n\
               app.use(widgetRouter)                ✗  router.post('/') responds to POST /, not POST /widgets → 404\n\
             Router handler paths are relative to the mount point:\n\
               router.post('/', ...)           ✓  mount at '/widgets' → responds to POST /widgets\n\
               router.post('/widgets', ...)    ✗  mount at '/widgets' → responds to POST /widgets/widgets"
             .to_string()),
        ]),
        layer_order:
            "  1. src/models/           — interfaces only; no deps\n\
             2. src/events/           — domain event interfaces; no deps\n\
             3. src/repositories/     — imports models; all DB calls; no Express deps\n\
             4. src/infrastructure/   — imports events and external clients (e.g. kafkajs); no Express deps\n\
             5. src/services/         — imports models, events, repositories, infrastructure; no Express deps\n\
             6. src/routes/           — imports services; mounts on Express router; validates with zod\n\
             7. src/middleware/errorHandler.ts — depends on nothing; must be created before app.ts\n\
             8. src/app.ts            — assembles the Express app; imports routes and middleware\n\
             9. src/index.ts          — starts the server; imports app; calls app.listen()\n\
             Test files are not a separate layer step — the TDD cycle generates one\n\
             automatically, co-located next to each file above, as that file's own step runs.\n\
             Reason: services must not import from routes; app.ts must not call listen()."
            .to_string(),
        notes: None,
    }
}

/// Resolve the tech-stack skill for a technology string, if one exists.
/// Shared by `skill_for_technology` and `plan_skill_for_technology` — they differ only
/// in which render method they call on the result.
fn resolve_tech_stack_skill(tech: &str, pkg: &str, pkg_path: &str, service_name: &str) -> Option<TechStackSkill> {
    match crate::tech::TechFamily::detect(tech) {
        crate::tech::TechFamily::Jvm => Some(spring_boot_skill(pkg, pkg_path, service_name)),
        crate::tech::TechFamily::React => Some(react_vite_skill()),
        crate::tech::TechFamily::Angular => Some(angular_skill()),
        crate::tech::TechFamily::NodeExpress => Some(node_express_skill()),
        crate::tech::TechFamily::Vue | crate::tech::TechFamily::Other => None,
    }
}

/// Return the rendered skill block for the given technology, scoped to one layer.
/// Returns an empty string if no built-in skill matches (LLM gets no extra rules).
/// JVM skills receive dynamic package context; others are technology-only.
/// To add a new stack: implement a builder function, add a match arm in resolve_tech_stack_skill().
pub fn skill_for_technology(tech: &str, pkg: &str, pkg_path: &str, service_name: &str, layer: &str) -> String {
    resolve_tech_stack_skill(tech, pkg, pkg_path, service_name).map(|s| s.render_for_layer(layer)).unwrap_or_default()
}

/// Same as `skill_for_technology`, but with every layer's rules included — for contexts not
/// tied to a single file (e.g. proposing dependencies for a whole service).
pub fn skill_for_technology_all_layers(tech: &str, pkg: &str, pkg_path: &str, service_name: &str) -> String {
    resolve_tech_stack_skill(tech, pkg, pkg_path, service_name).map(|s| s.render_all_layers()).unwrap_or_default()
}

/// Planner skill — file layout and layer order only.
/// Strips namespace_rules and notes (implementation concerns) to keep planning prompts lean.
pub(crate) fn plan_skill_for_technology(tech: &str, pkg: &str, pkg_path: &str, service_name: &str) -> String {
    resolve_tech_stack_skill(tech, pkg, pkg_path, service_name).map(|s| s.render_for_planning()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_for_layer_produces_expected_literal_output_with_notes_for_legacy_skill() {
        let skill = TechStackSkill {
            name: "Sample".to_string(),
            file_layout: "layout-body".to_string(),
            namespace_rules: "ns-body".to_string(),
            common_rules: String::new(),
            layer_rules: std::collections::HashMap::new(),
            layer_order: "order-body".to_string(),
            notes: Some("notes-body".to_string()),
        };
        // A legacy (non-partitioned) skill ignores the layer argument entirely.
        assert_eq!(
            skill.render_for_layer("model"),
            "## Tech stack: Sample\n\n\
             ### File layout\nlayout-body\n\n\
             ### Import rules\nns-body\n\n\
             ### Layer order\norder-body\n\n\
             ### Additional rules\nnotes-body"
        );
    }

    #[test]
    fn render_for_layer_omits_notes_section_when_none() {
        let skill = TechStackSkill {
            name: "Sample".to_string(),
            file_layout: "layout-body".to_string(),
            namespace_rules: "ns-body".to_string(),
            common_rules: String::new(),
            layer_rules: std::collections::HashMap::new(),
            layer_order: "order-body".to_string(),
            notes: None,
        };
        assert_eq!(
            skill.render_for_layer("model"),
            "## Tech stack: Sample\n\n\
             ### File layout\nlayout-body\n\n\
             ### Import rules\nns-body\n\n\
             ### Layer order\norder-body"
        );
    }

    #[test]
    fn render_for_planning_omits_namespace_rules_and_notes() {
        let skill = TechStackSkill {
            name: "Sample".to_string(),
            file_layout: "layout-body".to_string(),
            namespace_rules: "ns-body".to_string(),
            common_rules: String::new(),
            layer_rules: std::collections::HashMap::new(),
            layer_order: "order-body".to_string(),
            notes: Some("notes-body".to_string()),
        };
        assert_eq!(
            skill.render_for_planning(),
            "## Tech stack: Sample\n\n\
             ### File layout\nlayout-body\n\n\
             ### Layer order\norder-body"
        );
    }

    #[test]
    fn render_for_layer_scopes_a_partitioned_skill_to_one_layer() {
        let skill = TechStackSkill {
            name: "Sample".to_string(),
            file_layout: "layout-body".to_string(),
            namespace_rules: String::new(),
            common_rules: "common-body".to_string(),
            layer_rules: std::collections::HashMap::from([
                ("model", "model-body".to_string()),
                ("route", "route-body".to_string()),
            ]),
            layer_order: "order-body".to_string(),
            notes: None,
        };
        // Only the requested layer's rules are included — never a sibling layer's.
        assert_eq!(
            skill.render_for_layer("model"),
            "## Tech stack: Sample\n\n\
             ### File layout\nlayout-body\n\n\
             ### Import rules\ncommon-body\n\nmodel-body\n\n\
             ### Layer order\norder-body"
        );
    }

    #[test]
    fn render_all_layers_concatenates_every_layer_in_order() {
        let skill = TechStackSkill {
            name: "Sample".to_string(),
            file_layout: "layout-body".to_string(),
            namespace_rules: String::new(),
            common_rules: "common-body".to_string(),
            layer_rules: std::collections::HashMap::from([
                ("route", "route-body".to_string()),
                ("model", "model-body".to_string()),
            ]),
            layer_order: "order-body".to_string(),
            notes: None,
        };
        // LAYER_KEYS orders "model" before "route" regardless of map insertion order.
        assert_eq!(
            skill.render_all_layers(),
            "## Tech stack: Sample\n\n\
             ### File layout\nlayout-body\n\n\
             ### Import rules\ncommon-body\n\nmodel-body\n\nroute-body\n\n\
             ### Layer order\norder-body"
        );
    }
}
