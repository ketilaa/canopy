use canopy_core::Adr;

// ── Testing Skills ───────────────────────────────────────────────────────────
//
// Testing skills encode the exact framework choices, annotation patterns, and
// assertion style for each technology. They are injected at three points:
//   1. unit_test_stub_prompt  — drives TDD Red phase test generation
//   2. fix_prompt             — guides the fix loop when repairing test files
//   3. plan discovery prompt    — tells the planner which test files to include
//
// Adding a new skill: write a const (or fn for dynamic content), add a match arm
// in unit_testing_skill() / integration_testing_skill() / testing_skill_for_file().

const SPRING_BOOT_UNIT_TEST_COMMON: &str = "\
=== Testing Skill: Spring Boot unit tests (JUnit Jupiter + AssertJ + Mockito) ===

Framework stack — all available from spring-boot-starter-test, no extra deps needed:
  JUnit Jupiter 5     org.junit.jupiter.api.{Test,BeforeEach,AfterEach,Nested,DisplayName}
  AssertJ             static import org.assertj.core.api.Assertions.*
  Mockito             static import org.mockito.Mockito.* + org.mockito.ArgumentMatchers.*
  MockMvc             org.springframework.test.web.servlet.{MockMvc,MockMvcRequestBuilders,ResultMatchers}
  Jakarta Validation  jakarta.validation.Validation.buildDefaultValidatorFactory().getValidator()

Static imports — include all relevant ones in every test file:
  import static org.assertj.core.api.Assertions.*;
  import static org.mockito.ArgumentMatchers.*;
  import static org.mockito.Mockito.*;
  import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
  import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

Assertion style — AssertJ everywhere; never bare JUnit assertions:
  assertThat(response.getId()).isNotNull()
  assertThat(response.getName()).isEqualTo(\"Widget\")
  assertThat(violations).isEmpty()
  assertThat(violations).extracting(v -> v.getPropertyPath().toString()).contains(\"name\")
  assertThatThrownBy(() -> service.method(arg)).isInstanceOf(ResponseStatusException.class)

Forbidden — these indicate a mistake, fix them immediately:
  - @SpringBootTest in unit tests → use @WebMvcTest / @DataJpaTest / @ExtendWith(MockitoExtension)
  - org.junit.Test or @RunWith    → JUnit 4; use org.junit.jupiter.api.Test and @ExtendWith
  - assertEquals / assertTrue     → use assertThat() from AssertJ
  - javax.*                       → jakarta.* only (Spring Boot 3 / Jakarta EE 9+)";

const SPRING_BOOT_INTEGRATION_TEST_SKILL: &str = "\
=== Testing Skill: Spring Boot integration tests ===

Guiding principle: prefer focused slice tests; use @SpringBootTest sparingly.

@SpringBootTest — full application context, all beans wired, real HTTP or RANDOM_PORT:
  @SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
  @AutoConfigureMockMvc
  class ProductRegistrationIT {
    @Autowired MockMvc mockMvc;
    // Tests the full stack: controller → service → repository → H2 in one shot.
    // Reserve for end-to-end scenarios that slice tests cannot cover.
  }

Prefer focused slice tests for targeted scenarios (faster, more isolated):
  @WebMvcTest(FooController.class)  → controller + HTTP layer; no JPA, no full context
  @DataJpaTest                      → repository + H2 only; no web or service layer
  @JsonTest                         → Jackson serialization only
  @RestClientTest                   → REST client only

Integration test file naming convention: *IT.java (not *Test.java).
These are the LAST steps in the implementation plan — they exercise the full stack.

Assertions: AssertJ + MockMvc (same as unit tests, see unit test skill above).";

const REACT_VITEST_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: React + TypeScript — Vitest + React Testing Library ===
Skill trigger keyword: vitest\n\
=== Testing Skill: React + TypeScript (Vitest + React Testing Library) ===

Framework stack:
  vitest                      — test runner (replaces Jest in Vite projects)
  @testing-library/react      — render(), screen, waitFor()
  @testing-library/user-event — userEvent.type(), userEvent.click()
  @testing-library/jest-dom   — .toBeInTheDocument(), .toHaveValue(), etc.

Standard imports:
  import { describe, it, expect, vi, beforeEach } from 'vitest'
  import { render, screen, waitFor } from '@testing-library/react'
  import userEvent from '@testing-library/user-event'

Component test pattern:
  describe('WidgetForm', () => {
    it('should submit form data when all required fields are filled', async () => {
      const onSubmit = vi.fn()
      render(<WidgetForm onSubmit={onSubmit} />)
      await userEvent.type(screen.getByLabelText(/name/i), 'Widget')
      await userEvent.click(screen.getByRole('button', { name: /submit/i }))
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: 'Widget' }))
    })
  })

API function test pattern (mock fetch globally):
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true, status: 201,
      headers: { get: (h: string) => h === 'Location' ? '/api/widgets/uuid' : null },
      json: async () => ({ id: 'uuid', name: 'Widget' })
    }))
  })

Forbidden:
  - jest.fn() → vi.fn()
  - enzyme → use @testing-library/react
  - testing implementation details (state, refs) → test via the DOM only";

const REACT_JEST_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: React + TypeScript — Jest + React Testing Library ===
Skill trigger keyword: jest (react)

Framework stack:
  jest                        — test runner (jest.fn(), jest.mock())
  @testing-library/react      — render(), screen, waitFor()
  @testing-library/user-event — userEvent.type(), userEvent.click()
  @testing-library/jest-dom   — .toBeInTheDocument(), .toHaveValue(), etc.

Standard imports:
  import { describe, it, expect, jest, beforeEach } from '@jest/globals'
  import { render, screen, waitFor } from '@testing-library/react'
  import userEvent from '@testing-library/user-event'

Component test pattern:
  describe('WidgetForm', () => {
    it('should submit form data when all required fields are filled', async () => {
      const onSubmit = jest.fn()
      render(<WidgetForm onSubmit={onSubmit} />)
      await userEvent.type(screen.getByLabelText(/name/i), 'Widget')
      await userEvent.click(screen.getByRole('button', { name: /submit/i }))
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: 'Widget' }))
    })
  })

API function test pattern (mock fetch globally):
  beforeEach(() => {
    global.fetch = jest.fn().mockResolvedValue({
      ok: true, status: 201,
      headers: { get: (h: string) => h === 'Location' ? '/api/widgets/uuid' : null },
      json: async () => ({ id: 'uuid', name: 'Widget' })
    } as Response)
  })

Forbidden:
  - vi.fn() → jest.fn()
  - enzyme → use @testing-library/react
  - testing implementation details (state, refs) → test via the DOM only";

const ANGULAR_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: Angular (TestBed + Jasmine / Jest) ===

Component tests via TestBed:
  beforeEach(() => TestBed.configureTestingModule({
    declarations: [WidgetFormComponent],
    imports: [ReactiveFormsModule, HttpClientTestingModule],
    providers: [{ provide: WidgetService, useValue: mockWidgetService }]
  }).compileComponents())
  const fixture = TestBed.createComponent(WidgetFormComponent)
  fixture.detectChanges()
  const el: HTMLElement = fixture.nativeElement

Service tests:
  beforeEach(() => TestBed.configureTestingModule({
    imports: [HttpClientTestingModule],
    providers: [WidgetService]
  }))
  service = TestBed.inject(WidgetService)
  httpMock = TestBed.inject(HttpTestingController)
  afterEach(() => httpMock.verify())

Assertion: jasmine expect() or jest expect() depending on project config.
Prefer Angular Testing Library (@testing-library/angular) for behaviour-driven tests.";

const NODE_VITEST_UNIT_TEST_COMMON: &str = "\
=== Testing Skill: Node.js / Express (Vitest + Supertest) ===

Trigger keyword: vitest (node)

Framework stack:
  vitest    — test runner and mocking (vi.fn(), vi.mock())
  supertest — HTTP integration: request(app).post('/api/...')";

/// Mirrors `node_express_layer_examples()`'s shape — only "route" and "service" have a
/// dedicated Vitest worked example today. Keying this the same way keeps
/// `node_vitest_unit_test_skill()` able to defer to `layer_has_worked_example()` per layer
/// instead of bundling both examples into every layer's prompt unconditionally (the bug this
/// split fixes: a repository/model/infrastructure-layer test-gen call used to receive the Route
/// example's "returns 400 when mandatory field is missing" pattern regardless of its own layer,
/// the same unconditional-vs-gated contradiction shape fixed elsewhere for the Jest skill).
fn node_vitest_layer_examples() -> std::collections::HashMap<&'static str, String> {
    std::collections::HashMap::from([
        ("route",
         "  Route / integration test pattern:\n\
            import request from 'supertest'\n\
            import { app } from '../app'\n\
            import { describe, it, expect, vi, beforeEach } from 'vitest'\n\
            \n\
            describe('POST /api/widgets', () => {\n\
              it('returns 201 with Location header when payload is valid', async () => {\n\
                const res = await request(app).post('/api/widgets')\n\
                  .send({ name: 'Widget', price: 29.99 })\n\
                  .set('Content-Type', 'application/json')\n\
                expect(res.status).toBe(201)\n\
                expect(res.headers.location).toMatch(/\\/api\\/widgets\\//)\n\
              })\n\
              it('returns 400 when mandatory field is missing', async () => {\n\
                const res = await request(app).post('/api/widgets').send({})\n\
                expect(res.status).toBe(400)\n\
              })\n\
            })"
         .to_string()),
        ("service",
         "  Service unit test pattern (mock repository):\n\
            vi.mock('../repository/WidgetRepository')\n\
            import { WidgetRepository } from '../repository/WidgetRepository'\n\
            const mockRepo = vi.mocked(WidgetRepository)\n\
            mockRepo.save.mockResolvedValue({ id: 'uuid', ...payload })"
         .to_string()),
    ])
}

fn node_vitest_unit_test_skill(layer: &str) -> String {
    match node_vitest_layer_examples().get(layer) {
        Some(example) => format!("{NODE_VITEST_UNIT_TEST_COMMON}\n\n{example}"),
        None => NODE_VITEST_UNIT_TEST_COMMON.to_string(),
    }
}

const NODE_EXPRESS_UNIT_TEST_COMMON: &str = "\
## Testing: Jest + Supertest (Node.js / Express)

### File locations
  service root/
    src/
      app.ts     ← exports the Express app as DEFAULT export
      services/
        WidgetService.ts
        WidgetService.test.ts    ← co-located, SAME directory as what it tests
      repositories/
      infrastructure/
      models/
  ALWAYS put test files NEXT TO the file they test, in the SAME directory — NEVER a separate
  tests/ directory (JS/TS convention, not Java's mirrored tree).

### Import and jest.mock paths — same depth rule as production code: same-directory for the
### file under test, one dot-dot for a sibling folder.
  import request from 'supertest'                                          ✓
  import { WidgetService } from './WidgetService'                         ✓  (same directory — the file under test)
  import { WidgetRepository } from '../repositories/WidgetRepository'     ✓  (sibling directory)
  import { EventPublisher } from '../infrastructure/EventPublisher'       ✓  (sibling directory)
  jest.mock('../repositories/WidgetRepository')                           ✓
  jest.mock('../infrastructure/EventPublisher', () => ({ ... }))         ✓
  NEVER import app.ts in a route test — mount the router on a local Express instance instead
  (see the Route example below).

### Forbidden paths (imports AND jest.mock)
These will cause module-not-found errors at runtime:
  import ... from '../src/...'                    ✗  there is no src/ segment to cross — the test is ALREADY inside src/
  import ... from '../../services/...'           ✗  too many dot-dots — siblings under src/ are ONE dot-dot away, not two
  import ... from '@services/...'               ✗  no path aliases configured in tsconfig
  require(...)                                   ✗  use ES import syntax
  uuid(), faker()                                ✗  not in devDependencies — use plain strings

### jest is a global — NEVER import it
ts-jest injects jest as a global automatically:
  jest.fn()                                      ✓  jest is already in scope
  jest.Mocked<T>                                 ✓  type reference, no import
  import { jest } from '@jest/globals'           ✗  causes TS2305: Module has no exported member 'jest'
  import jest from '@jest/globals'               ✗  same — ts-jest globals mode does not export jest

### Test IDs and test data — ALWAYS plain string literals, NEVER real UUIDs:
  const id = 'test-id-123'                       ✓  simple, readable, predictable
  import { v4 as uuid } from 'uuid'              ✗  uuid not in devDependencies
  import { v4 as uuid } from 'crypto'            ✗  crypto has no v4 export — RUNTIME ERROR
  crypto.randomUUID()                            ✗  non-deterministic — use a literal string
Use literal dates too: new Date('2024-01-01') instead of new Date().

### Variable scope: mocks used across it() blocks
ALWAYS declare a mock/subject/fixture used in more than one it() block with `let` at
describe()-level, assigning it only inside beforeEach. NEVER declare it with `const`/`let`
INSIDE beforeEach — that scopes it to the callback, and every it() below fails with TS2304
\"Cannot find name '<var>'\":
  WRONG — mockConnection only exists inside beforeEach's closure:
    describe('WidgetGateway', () => {
      let gateway: WidgetGateway
      beforeEach(() => {
        const mockConnection = { getClient: jest.fn().mockReturnValue({ open: jest.fn() }) } as any   ✗
        gateway = new WidgetGateway(mockConnection)
      })
      it('opens', async () => {
        await gateway.open()
        expect(mockConnection.getClient().open).toHaveBeenCalled()   // TS2304: mockConnection not found
      })
    })
  CORRECT — declare at describe-level, assign inside beforeEach:
    describe('WidgetGateway', () => {
      let mockConnection: any
      let gateway: WidgetGateway
      beforeEach(() => {
        mockConnection = { getClient: jest.fn().mockReturnValue({ open: jest.fn() }) }
        gateway = new WidgetGateway(mockConnection)
      })
      it('opens', async () => {
        await gateway.open()
        expect(mockConnection.getClient().open).toHaveBeenCalled()   // ✓ mockConnection is in scope
      })
    })

### Jest assertion rules
  ALWAYS .toThrow() — NEVER .toThrowError() (removed in Jest 30).
  ALWAYS pass an argument to mockResolvedValue() — use mockResolvedValue(undefined) for void.
  NEVER expect.any(X) for a TypeScript interface (TS2693 — no runtime representation).
  ALWAYS expect.objectContaining({field: value}) instead.

### Imports in test files
ALWAYS explicitly import every class/function used in a test, even when mocked — jest.mock()
replaces the module but does NOT create the binding:
  import { EventPublisher } from '../infrastructure/EventPublisher'   ✓ (after jest.mock; sibling directory)
  new EventPublisher('', '')  without importing EventPublisher            ✗ ReferenceError";

/// Layer-specific examples for the Node/Express unit test skill — only the entry matching the
/// file's own layer is injected alongside `NODE_EXPRESS_UNIT_TEST_COMMON`. A model test has no
/// use for the Repository/Service/Route examples, so it no longer sees them.
fn node_express_layer_examples() -> std::collections::HashMap<&'static str, String> {
    std::collections::HashMap::from([
        ("route",
         "  ### Route test example (mock ONLY the service layer, never the repository or event\n\
  publisher directly; this isolates the route test from every infrastructure concern, including\n\
  any Kafka/DB connection attempt)\n\
  import request from 'supertest'\n\
  import express from 'express'\n\
  import router from './widgets'\n\
  import { WidgetService } from '../services/WidgetService'\n\
\n\
  jest.mock('../services/WidgetService')\n\
\n\
  let app: express.Express\n\
  let mockWidgetService: jest.Mocked<WidgetService>\n\
\n\
  beforeEach(() => {\n\
    mockWidgetService = new WidgetService({} as any, {} as any) as jest.Mocked<WidgetService>\n\
    app = express()\n\
    app.use(express.json())\n\
    app.locals.widgetService = mockWidgetService\n\
    app.use('/widgets', router)\n\
  })\n\
\n\
  describe('POST /widgets', () => {\n\
    it('returns 201 with Location header when payload is valid', async () => {\n\
      mockWidgetService.createWidget.mockResolvedValue({\n\
        id: 'widget-1', createdAt: new Date(), name: 'name-value', otherField: 'other-field-value',\n\
      })\n\
      const res = await request(app).post('/widgets')\n\
        .send({ name: 'name-value', otherField: 'other-field-value' })\n\
        .set('Content-Type', 'application/json')\n\
      expect(res.status).toBe(201)\n\
      expect(res.headers.location).toMatch(/\\/widgets\\//)\n\
    })\n\
    it('returns 400 when a mandatory field is missing', async () => {\n\
      const res = await request(app).post('/widgets').send({})\n\
      expect(res.status).toBe(400)\n\
    })\n\
  })\n\
\n\
  Mocking a CLASS constructed with `new` differs from mocking a plain-object interface\n\
  (Repository/EventPublisher examples use `{ method: jest.fn() } as any` since the service\n\
  builds those as plain objects). ALWAYS pair `jest.mock('../services/WidgetService')` with a\n\
  cast on the constructed instance — its static type is still the real class:\n\
    WRONG:   let mockWidgetService: WidgetService = new WidgetService(a, b)\n\
             mockWidgetService.createWidget.mockResolvedValue(...)   ✗ TS2339: no .mockResolvedValue\n\
    CORRECT: let mockWidgetService: jest.Mocked<WidgetService>\n\
             mockWidgetService = new WidgetService(a, b) as jest.Mocked<WidgetService>\n\
  jest.mock() no-ops the real constructor — ALWAYS pass `{} as any` for each constructor\n\
  argument, matching however many the real constructor declares.\n\
\n\
### Scope discipline\n\
  ALWAYS write tests only for the HTTP methods/operations the story describes. NEVER add\n\
  GET/DELETE/PUT route tests the acceptance criteria don't require."
         .to_string()),
        ("model",
         "  ### Model unit test example\n\
  Models are interfaces + factory functions — NEVER classes. Tests call the factory, not `new`.\n\
  import { createWidget } from './Widget'\n\
  import type { Widget } from './Widget'\n\
\n\
  describe('createWidget', () => {\n\
    it('creates a widget with all mandatory fields', () => {\n\
      const widget = createWidget('name-value', 'other-field-value')\n\
      expect(widget.id).toEqual(expect.any(String))\n\
      expect(widget.createdAt).toBeInstanceOf(Date)\n\
      expect(widget.name).toBe('name-value')\n\
      expect(widget.optionalField).toBeUndefined()\n\
    })\n\
    it('creates a widget with optional field included', () => {\n\
      const widget = createWidget('name-value', 'other-field-value', 'optional-value')\n\
      expect(widget.optionalField).toBe('optional-value')\n\
    })\n\
    it('throws when a mandatory field is missing', () => {\n\
      // Same POSITIONAL call as every test above — cast ONLY the missing argument, in its own\n\
      // position. NEVER collapse the call into a single object literal.\n\
      expect(() => createWidget(undefined as any, 'other-field-value')).toThrow('name-value not provided...')\n\
    })\n\
  })\n\
  RULES for model tests:\n\
  - A pure factory has nothing to mock: NEVER `jest.mock()`, `jest.fn()`, or reference a\n\
    repository/service/event publisher in this file — call the real factory with plain arguments.\n\
  - NEVER write `new Widget(...)` — the model is an interface, not a class; `new` will not compile\n\
  - NEVER call `widget.save()` or any persistence method — models have no such methods\n\
  - ALWAYS `import { createWidget }` for the factory call, `import type { Widget }` for types only\n\
  - WRONG for the missing-field test — collapses the positional call into one object:\n\
      const invalidPayload = { otherField: 'other-field-value' } as any\n\
      expect(() => createWidget(invalidPayload)).toThrow(...)   ✗ this factory takes positional args, not one object"
         .to_string()),
        ("service",
         "  ### Service unit test example\n\
  import { WidgetService } from './WidgetService'\n\
  import { WidgetRepository } from '../repositories/WidgetRepository'\n\
  import { EventPublisher } from '../infrastructure/EventPublisher'\n\
  // The event factory lives in its OWN file under src/events/ — NEVER in the model file.\n\
  // WRONG: import { createWidgetCreated } from '../models/Widget'          ✗ TS2305\n\
\n\
  let mockRepo: jest.Mocked<WidgetRepository>\n\
  let mockPublisher: jest.Mocked<EventPublisher>\n\
  let service: WidgetService\n\
\n\
  beforeEach(() => {\n\
    mockRepo = { saveWidget: jest.fn(), getWidgetById: jest.fn() } as any\n\
    mockPublisher = { connect: jest.fn(), disconnect: jest.fn(), publish: jest.fn() } as any\n\
    service = new WidgetService(mockRepo, mockPublisher)\n\
  })\n\
\n\
  it('creates a widget and publishes an event', async () => {\n\
    // The repository persists whatever the service's factory constructed and returns it\n\
    // UNCHANGED (repository contract: never assigns ids or timestamps) — never a separate\n\
    // hardcoded literal. This also means the mock naturally echoes back the SAME id the\n\
    // factory generated, instead of racing it against an unrelated one.\n\
    mockRepo.saveWidget.mockImplementation(async (widget) => widget)\n\
    mockPublisher.connect.mockResolvedValue(undefined)\n\
    mockPublisher.publish.mockResolvedValue(undefined)\n\
    mockPublisher.disconnect.mockResolvedValue(undefined)\n\
\n\
    // Call the service with ONE options object — match the method's ACTUAL declared\n\
    // signature (check the referenced/sibling surface above), never guess positional args:\n\
    const result = await service.createWidget({ name: 'name-value', otherField: 'other-field-value' })\n\
\n\
    // NEVER build a second, separate object (via the factory or an event factory) and\n\
    // deep-equal it against what the service produced — EVERY randomUUID()/new Date() call\n\
    // creates a DIFFERENT value, including a domain event's own eventId and occurredAt.\n\
    // Assert only the fields you actually control, using objectContaining/toMatchObject:\n\
    expect(result).toMatchObject({ name: 'name-value', otherField: 'other-field-value' })\n\
    expect(result.id).toEqual(expect.any(String))\n\
    expect(result.createdAt).toBeInstanceOf(Date)\n\
    expect(mockRepo.saveWidget).toHaveBeenCalledWith(expect.objectContaining({ name: 'name-value' }))\n\
    expect(mockPublisher.publish).toHaveBeenCalledWith(expect.objectContaining({ widgetId: result.id }))\n\
  })\n\
  ALWAYS assert via objectContaining/toMatchObject on specific fields, exactly as above. NEVER\n\
  build a second object (factory or event factory called again) and deep-equal it — every\n\
  randomUUID()/new Date() call produces a different value, even when the code is correct. This\n\
  includes id/createdAt/modifiedAt inside a toMatchObject(...) literal — a matching fake value\n\
  in the mock does not make them equal either:\n\
    WRONG: mockRepo.saveWidget.mockResolvedValue({ ...widget, id: 'fake-id' })\n\
           expect(result).toMatchObject({ id: 'fake-id', name: 'name-value' })   ✗ id differs\n\
  CORRECT: never put id/createdAt/modifiedAt inside toMatchObject(...) — check them separately\n\
  with expect.any(String) / toBeInstanceOf(Date), exactly as shown above.\n\
  Same mistake, standalone assertion form (the mock invents an unrequested value):\n\
    WRONG: const widgetId = 'test-id-123'\n\
           mockRepo.saveWidget.mockImplementation(async (widget) => ({ ...widget, id: widgetId }))\n\
           const result = await service.createWidget({ name: 'name-value' })\n\
           expect(result.id).toEqual(widgetId)   ✗ the service returns the FACTORY's product,\n\
           never whatever the repository mock resolved to.\n\
  CORRECT: mock the repository to resolve with the SAME object passed in; assert result.id with\n\
  expect.any(String), never a mock-invented literal."
         .to_string()),
        ("repository",
         "  ### Repository unit test example (real method, mocked Pool — never mock the method itself)\n\
  ALWAYS call the repository's REAL methods and assert real behavior — mock only the injected\n\
  pg.Pool. NEVER jest.spyOn the repository's own method:\n\
    import { Pool } from 'pg'\n\
    import { WidgetRepository } from './WidgetRepository'\n\
    import { createWidget } from '../models/Widget'\n\
\n\
    NEVER declare the mock as `jest.Mocked<Pool>` — pg.Pool.query's overloads collapse to\n\
    `never` under Mocked<>, causing TS2345 on every mockResolvedValue() call. ALWAYS declare a\n\
    minimal plain shape instead, cast only at construction:\n\
    let mockPool: { query: jest.Mock }\n\
    let subject: WidgetRepository\n\
\n\
    beforeEach(() => {\n\
      mockPool = { query: jest.fn() }\n\
      subject = new WidgetRepository(mockPool as unknown as Pool)\n\
    })\n\
\n\
    it('saves a widget by running a query against the pool', async () => {\n\
      const widget = createWidget('name-value', 'other-field-value')\n\
      mockPool.query.mockResolvedValue({ rows: [] })\n\
\n\
      const result = await subject.saveWidget(widget)\n\
\n\
      expect(mockPool.query).toHaveBeenCalledWith(expect.any(String), expect.arrayContaining([widget.id]))\n\
      expect(result).toEqual(widget)\n\
    })\n\
    WRONG — replaces the method under test instead of exercising it, proves nothing:\n\
      jest.spyOn(subject, 'saveWidget').mockResolvedValue(widget)   ✗\n\
    WRONG — collapses query's mock type to `never`, every mockResolvedValue() call fails TS2345:\n\
      let mockPool: jest.Mocked<Pool>   ✗"
         .to_string()),
        ("infrastructure",
         "  ### Wrapper unit test example (SUT obtains its real collaborator via a factory method)\n\
  When the constructor obtains its real collaborator via a factory/getter (e.g.\n\
  `this.client = connection.getClient()`), ALWAYS make the mocked factory return the SAME mock\n\
  object the test asserts against — NEVER two disconnected mocks:\n\
    WRONG — the SUT never touches mockClient, so every assertion on it sees 0 calls:\n\
      const mockConnection = { getClient: jest.fn().mockReturnValue({ open: jest.fn(), ... }) } as any  ✗\n\
      const mockClient = { open: jest.fn(), ... } as any                                                ✗ never wired in\n\
      expect(mockClient.open).toHaveBeenCalled()   // fails — SUT called the OTHER object\n\
    CORRECT — construct the mock once, then return that exact instance from the factory:\n\
      const mockClient = { open: jest.fn().mockResolvedValue(undefined),\n\
                            close: jest.fn().mockResolvedValue(undefined),\n\
                            send: jest.fn().mockResolvedValue(undefined) } as any\n\
      const mockConnection = { getClient: jest.fn().mockReturnValue(mockClient) } as any\n\
      const gateway = new WidgetGateway(mockConnection)\n\
      await gateway.open()\n\
      expect(mockClient.open).toHaveBeenCalled()   // passes — same object\n\
  ALWAYS build the mock instance as its own variable first, then pass it into a stubbed factory\n\
  method (getClient(), getConnection(), createClient(), ...) via mockReturnValue(...) — it must\n\
  BE the object asserted against later.\n\
\n\
  When hoisted to describe-level (per the variable-scope rule above), NEVER declare these mocks\n\
  with the real SDK type (Client/Connection) — a partial mock won't satisfy its full member list\n\
  (TS2322 \"Property '<member>' is missing\"). ALWAYS declare hoisted mocks as `any`, cast only\n\
  at construction:\n\
    WRONG — partial object can't satisfy the full interface:\n\
      let mockClient: ExternalClient          ✗\n\
      let mockConnection: ExternalConnection  ✗\n\
    CORRECT:\n\
      let mockClient: any\n\
      let mockConnection: any\n\
      beforeEach(() => {\n\
        mockClient = { open: jest.fn().mockResolvedValue(undefined),\n\
                       close: jest.fn().mockResolvedValue(undefined),\n\
                       send: jest.fn().mockResolvedValue(undefined) }\n\
        mockConnection = { getClient: jest.fn().mockReturnValue(mockClient) } as unknown as ExternalConnection\n\
      })\n\
\n\
  ### kafkajs Producer (same factory pattern as above — kafka.producer() is getClient()).\n\
  ALWAYS build mockProducer with jest.fn() methods FIRST. NEVER declare\n\
  `mockProducer = {} as Producer` and then jest.spyOn() it — spyOn requires an EXISTING method,\n\
  and an empty object has none:\n\
    WRONG:\n\
      mockProducer = {} as Producer                                    ✗ no methods to spy on\n\
      mockKafka.producer = jest.fn().mockReturnValue(mockProducer)\n\
      const connectSpy = jest.spyOn(mockProducer, 'connect')            ✗ throws immediately\n\
    CORRECT — mockProducer's methods ARE jest.fn()s already; assert on them directly, no spyOn:\n\
      let mockKafka: any\n\
      let mockProducer: any\n\
      beforeEach(() => {\n\
        mockProducer = {\n\
          connect: jest.fn().mockResolvedValue(undefined),\n\
          disconnect: jest.fn().mockResolvedValue(undefined),\n\
          send: jest.fn().mockResolvedValue([{ topicName: 'widget-events', partition: 0, errorCode: 0 }]),\n\
        }\n\
        mockKafka = { producer: jest.fn().mockReturnValue(mockProducer) } as unknown as Kafka\n\
        eventPublisher = new EventPublisher(mockKafka, 'widget-events')\n\
      })\n\
      it('connects to the producer', async () => {\n\
        await eventPublisher.connect()\n\
        expect(mockProducer.connect).toHaveBeenCalled()                 ✓ already a jest.fn()\n\
      })\n\
  `Producer.send()` resolves to `RecordMetadata[]` (`topicName`/`partition`/`errorCode`, plus\n\
  optional `offset`/`timestamp`/`baseOffset`/`logAppendTime`/`logStartOffset`) — NEVER the\n\
  `{ topic, messages }` shape you passed IN:\n\
    WRONG — mirrors send()'s own ARGUMENT shape, not its RETURN type:\n\
      send: jest.fn().mockResolvedValue([{ topic: 'widget-events', messages: [] }])   ✗ TS2353\n\
    CORRECT: send: jest.fn().mockResolvedValue([{ topicName: 'widget-events', partition: 0, errorCode: 0 }])\n\
  The assertion on what was SENT still uses the argument shape (this part is unrelated and correct):\n\
    expect(mockProducer.send).toHaveBeenCalledWith({ topic: 'widget-events', messages: [{ value: expect.any(String) }] })"
         .to_string()),
    ])
}

/// Renders the Node/Express unit test skill scoped to one layer — common rules plus the one
/// example relevant to the file's layer (or common-only when the layer has no dedicated example).
fn node_express_unit_test_skill(layer: &str) -> String {
    match node_express_layer_examples().get(layer) {
        Some(example) => format!("{NODE_EXPRESS_UNIT_TEST_COMMON}\n\n{example}"),
        None => NODE_EXPRESS_UNIT_TEST_COMMON.to_string(),
    }
}

/// True when the testing skill for this technology already provides a complete worked example
/// for this layer (imports, mocks, and the exact assertion pattern) — in which case a caller
/// building its own generic test-structure skeleton should defer to that example instead of
/// showing a second, content-free one. This is the single source of truth for "which layers
/// have a dedicated example"; callers must not hand-copy the layer list, or it silently goes
/// stale the moment a new example is added here.
pub(crate) fn layer_has_worked_example(adrs: &[Adr], tech: &str, layer: &str) -> bool {
    use crate::tech::TechFamily;
    match TechFamily::detect(tech) {
        // Node/Express's example set differs by which testing framework the project's ADR
        // picked — resolved via the SAME node_testing_adr() helper testing_skill_from_adrs
        // uses (not an independently-written lookup), so the two can never disagree about
        // which framework is in play and defer to a worked example the other function never
        // actually rendered. Defaults to the Jest set when no matching ADR exists, same as
        // testing_skill_from_adrs's own fallback (unit_testing_skill always resolves
        // NodeExpress to node_express_unit_test_skill).
        TechFamily::NodeExpress => {
            let uses_vitest = node_testing_adr(adrs)
                .map(|a| a.decision.to_lowercase().contains("vitest"))
                .unwrap_or(false);
            if uses_vitest {
                node_vitest_layer_examples().contains_key(layer)
            } else {
                node_express_layer_examples().contains_key(layer)
            }
        }
        // Other stacks don't partition their testing skill by layer yet — nothing to defer to.
        TechFamily::Jvm | TechFamily::React | TechFamily::Angular | TechFamily::Vue | TechFamily::Other => false,
    }
}

fn spring_boot_unit_test_skill(layer: &str) -> String {
    let layer_pattern = match layer {
        "controller" =>
            "Layer pattern — @WebMvcTest (web slice only, no JPA or service beans):\n\
             \n  @WebMvcTest(FooController.class)\n\
               class FooControllerTest {\n\
                 @Autowired MockMvc mockMvc;\n\
                 @Autowired ObjectMapper objectMapper;\n\
                 @MockBean FooService fooService;\n\
             \n    @Test\n\
                 void should_return_201_and_location_when_data_is_valid() throws Exception {\n\
                   when(fooService.create(any())).thenReturn(savedDto);\n\
                   mockMvc.perform(post(\"/api/foos\")\n\
                           .contentType(MediaType.APPLICATION_JSON)\n\
                           .content(objectMapper.writeValueAsString(validRequest)))\n\
                       .andExpect(status().isCreated())\n\
                       .andExpect(header().exists(\"Location\"))\n\
                       .andExpect(jsonPath(\"$.id\").isNotEmpty());\n\
                 }\n\
             \n    @Test\n\
                 void should_return_400_when_mandatory_field_is_missing() throws Exception {\n\
                   mockMvc.perform(post(\"/api/foos\")\n\
                           .contentType(MediaType.APPLICATION_JSON).content(\"{}\"))\n\
                       .andExpect(status().isBadRequest());\n\
                 }\n\
               }",
        "service" =>
            "Layer pattern — @ExtendWith(MockitoExtension.class) (pure unit, no Spring context):\n\
             \n  @ExtendWith(MockitoExtension.class)\n\
               class FooServiceTest {\n\
                 @Mock FooRepository fooRepository;\n\
                 @InjectMocks FooService fooService;\n\
             \n    @Test\n\
                 void should_persist_entity_and_return_response_when_data_is_valid() {\n\
                   Foo saved = new Foo(); saved.setId(UUID.randomUUID());\n\
                   when(fooRepository.save(any(Foo.class))).thenReturn(saved);\n\
                   FooResponse response = fooService.create(request);\n\
                   assertThat(response.getId()).isNotNull();\n\
                   verify(fooRepository).save(any(Foo.class));\n\
                 }\n\
               }",
        "dto" | "domain" =>
            "Layer pattern — plain JUnit 5, no Spring context; test Bean Validation constraints:\n\
             \n  class FooRequestTest {\n\
                 private Validator validator;\n\
             \n    @BeforeEach void setUp() {\n\
                   validator = Validation.buildDefaultValidatorFactory().getValidator();\n\
                 }\n\
             \n    @Test void should_pass_when_all_mandatory_fields_are_present() {\n\
                   FooRequest req = buildValidRequest(); // set all mandatory fields\n\
                   assertThat(validator.validate(req)).isEmpty();\n\
                 }\n\
             \n    @Test void should_fail_when_name_is_blank() {\n\
                   FooRequest req = buildValidRequest(); req.setName(\"\");\n\
                   assertThat(validator.validate(req))\n\
                       .extracting(v -> v.getPropertyPath().toString()).contains(\"name\");\n\
                 }\n\
               }",
        _ => "Layer pattern: choose @WebMvcTest / @ExtendWith(MockitoExtension) / plain JUnit 5 based on what the class does.",
    };
    format!("{SPRING_BOOT_UNIT_TEST_COMMON}\n\n{layer_pattern}")
}

/// Returns the unit testing skill for the given technology and layer.
/// Used in: TDD Red phase test generation, fix loop for unit test files.
/// Layer values: "controller" | "service" | "dto" | "domain" | "" (generic)
pub(crate) fn unit_testing_skill(tech: &str, layer: &str) -> String {
    use crate::tech::TechFamily;
    match TechFamily::detect(tech) {
        TechFamily::Jvm => spring_boot_unit_test_skill(layer),
        TechFamily::React => REACT_VITEST_UNIT_TEST_SKILL.to_string(),
        TechFamily::Angular => ANGULAR_UNIT_TEST_SKILL.to_string(),
        TechFamily::NodeExpress => node_express_unit_test_skill(layer),
        TechFamily::Vue | TechFamily::Other => String::new(),
    }
}

/// Returns the integration testing skill for the given technology.
/// Used in: plan prompt (for *IT.java steps), fix loop for integration test files.
// TODO(tech-detection): the Node/Express branch below checks only "node"/"express",
// NOT "nest" — this differs from TechFamily::detect's NodeExpress variant. Left as-is
// to avoid a silent behavior change for NestJS services; see refactor plan notes.
pub(crate) fn integration_testing_skill(tech: &str) -> String {
    let t = tech.to_lowercase();
    if t.contains("spring") || t.contains("quarkus") || t.contains("micronaut")
        || (t.contains("java") && !t.contains("javascript")) || t.contains("kotlin")
    {
        SPRING_BOOT_INTEGRATION_TEST_SKILL.to_string()
    } else if t.contains("react") || t.contains("vite") {
        "Integration tests: use Playwright or Cypress for full end-to-end browser tests. \
         Use msw (Mock Service Worker) for API-level integration tests within Vitest.".to_string()
    } else if t.contains("node") || t.contains("express") {
        "Integration tests: use Jest + Supertest against the full Express app (see unit test skill — \
         Supertest tests already exercise the HTTP + service + repository stack).".to_string()
    } else {
        String::new()
    }
}

/// ADR-aware variant of testing_skill_for_file.
/// When a "Testing Strategy" ADR exists its decision overrides the technology-based default
/// (e.g. "jest" keyword routes React to the Jest skill instead of the Vitest skill).
///
/// `service_source_files` is a fallback for an old-style flat `tests/Foo.test.ts` (no
/// subdirectory of its own) — matched back to its implementation file by stem so the correct
/// layer's example still gets injected. A co-located test's own path already carries the
/// layer signal directly and doesn't need this.
pub fn testing_skill_for_file_with_adrs(file_path: &str, tech: &str, adrs: &[Adr], service_source_files: &[String]) -> String {
    let is_test_file =
        file_path.ends_with("Test.java")
        || file_path.ends_with("IT.java")
        || file_path.ends_with(".test.ts")
        || file_path.ends_with(".test.tsx")
        || file_path.ends_with(".spec.ts")
        || file_path.ends_with(".spec.tsx");
    if !is_test_file {
        return String::new();
    }
    if file_path.ends_with("IT.java") {
        return integration_testing_skill(tech);
    }
    let layer = if file_path.ends_with(".ts") || file_path.ends_with(".tsx") {
        layer_for_ts_test_file(file_path, service_source_files)
    } else {
        String::new()
    };
    testing_skill_from_adrs(adrs, tech, &layer)
}

/// A co-located test file (e.g. `src/repositories/Foo.test.ts`) carries the layer signal
/// directly in its own path — `detect_layer` resolves it with no extra work. The stem-match
/// against sibling source files is a fallback for a flat `tests/Foo.test.ts` (no subdirectory
/// of its own), which older/pre-existing test files may still use.
fn layer_for_ts_test_file(file_path: &str, service_source_files: &[String]) -> String {
    let direct = crate::skills::detect_layer(file_path);
    if direct != "module" {
        return direct.to_string();
    }
    let stem = std::path::Path::new(file_path)
        .file_stem().and_then(|s| s.to_str()).unwrap_or("")
        .trim_end_matches(".test").trim_end_matches(".spec");
    service_source_files.iter()
        .find(|f| {
            (f.ends_with(".ts") || f.ends_with(".tsx"))
                && !is_test_file_path(f)
                && std::path::Path::new(f).file_stem().and_then(|s| s.to_str()) == Some(stem)
        })
        .map(|f| crate::skills::detect_layer(f).to_string())
        .unwrap_or_default()
}

/// Suffix-based test-file check, local to this module — TS/TSX tests are co-located next to
/// source, so a directory-based check can't distinguish them.
fn is_test_file_path(f: &str) -> bool {
    f.ends_with(".test.ts") || f.ends_with(".test.tsx") || f.ends_with(".spec.ts") || f.ends_with(".spec.tsx")
}

/// Resolves the unit testing skill for the given technology, consulting the Testing Strategy ADRs.
///
/// ADR titles are service-scoped (e.g. "Admin Portal Testing Strategy", "Product Testing Strategy")
/// so each service can choose its framework independently. The lookup therefore matches on the
/// decision field, not the title — making it robust to any naming convention.
///
/// Decision keywords select the skill:
///   React/Vite + "vitest"               → REACT_VITEST_UNIT_TEST_SKILL
///   React/Vite + "jest"                 → REACT_JEST_UNIT_TEST_SKILL
///   Angular                             → ANGULAR_UNIT_TEST_SKILL (implicit, no ADR needed)
///   Node/Express + "jest"  + "supertest" → NODE_EXPRESS_UNIT_TEST_SKILL
///   Node/Express + "vitest"+ "supertest" → node_vitest_unit_test_skill(layer)
///
/// The Node/Express ADR (if any) that decided this project's unit-test framework — first match
/// wins, same as testing_skill_from_adrs always resolved it. Shared with
/// `layer_has_worked_example` so the two can never disagree about which framework (and thus
/// which layer's worked example) applies to the same `adrs`/`tech` input: an independently
/// written second lookup previously used `.any()` over the whole ADR list instead of this exact
/// `.find()`, so a multi-service project with one Jest ADR and one unrelated Vitest ADR could
/// have `testing_skill_from_adrs` pick Jest while `layer_has_worked_example` "saw" Vitest —
/// reintroducing the same unconditional-vs-gated contradiction this pair of functions exists to
/// prevent.
fn node_testing_adr(adrs: &[Adr]) -> Option<&Adr> {
    adrs.iter().find(|a| {
        if !a.title.to_lowercase().contains("testing") { return false; }
        let d = a.decision.to_lowercase();
        (d.contains("jest") || d.contains("vitest")) && d.contains("supertest")
    })
}

/// Falls back to unit_testing_skill(tech, layer) when no relevant ADR exists.
pub(crate) fn testing_skill_from_adrs(adrs: &[Adr], tech: &str, layer: &str) -> String {
    let t = tech.to_lowercase();
    let family = crate::tech::TechFamily::detect(tech);
    let is_react = family == crate::tech::TechFamily::React;
    let is_node = family == crate::tech::TechFamily::NodeExpress;

    if is_node {
        if let Some(adr) = node_testing_adr(adrs) {
            let d = adr.decision.to_lowercase();
            if d.contains("vitest") {
                return node_vitest_unit_test_skill(layer);
            }
            if d.contains("jest") {
                return node_express_unit_test_skill(layer);
            }
        }
        return unit_testing_skill(tech, layer);
    }

    // Match on decision content — titles are service-scoped and vary across projects.
    let adr = adrs.iter().find(|a| {
        if !a.title.to_lowercase().contains("testing") { return false; }
        let d = a.decision.to_lowercase();
        if is_react {
            (d.contains("vitest") || d.contains("jest")) && d.contains("react testing")
        } else {
            false
        }
    });

    if let Some(adr) = adr {
        let d = adr.decision.to_lowercase();
        if is_react && d.contains("vitest") {
            return REACT_VITEST_UNIT_TEST_SKILL.to_string();
        }
        if is_react && d.contains("jest") && !d.contains("vitest") {
            return REACT_JEST_UNIT_TEST_SKILL.to_string();
        }
        if t.contains("angular") || d.contains("angular testbed") {
            return ANGULAR_UNIT_TEST_SKILL.to_string();
        }
    }
    unit_testing_skill(tech, layer)
}

