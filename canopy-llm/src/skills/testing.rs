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
  describe('ProductForm', () => {
    it('should submit form data when all required fields are filled', async () => {
      const onSubmit = vi.fn()
      render(<ProductForm onSubmit={onSubmit} />)
      await userEvent.type(screen.getByLabelText(/name/i), 'Widget')
      await userEvent.click(screen.getByRole('button', { name: /submit/i }))
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: 'Widget' }))
    })
  })

API function test pattern (mock fetch globally):
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true, status: 201,
      headers: { get: (h: string) => h === 'Location' ? '/api/products/uuid' : null },
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
  describe('ProductForm', () => {
    it('should submit form data when all required fields are filled', async () => {
      const onSubmit = jest.fn()
      render(<ProductForm onSubmit={onSubmit} />)
      await userEvent.type(screen.getByLabelText(/name/i), 'Widget')
      await userEvent.click(screen.getByRole('button', { name: /submit/i }))
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: 'Widget' }))
    })
  })

API function test pattern (mock fetch globally):
  beforeEach(() => {
    global.fetch = jest.fn().mockResolvedValue({
      ok: true, status: 201,
      headers: { get: (h: string) => h === 'Location' ? '/api/products/uuid' : null },
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
    declarations: [ProductFormComponent],
    imports: [ReactiveFormsModule, HttpClientTestingModule],
    providers: [{ provide: ProductService, useValue: mockProductService }]
  }).compileComponents())
  const fixture = TestBed.createComponent(ProductFormComponent)
  fixture.detectChanges()
  const el: HTMLElement = fixture.nativeElement

Service tests:
  beforeEach(() => TestBed.configureTestingModule({
    imports: [HttpClientTestingModule],
    providers: [ProductService]
  }))
  service = TestBed.inject(ProductService)
  httpMock = TestBed.inject(HttpTestingController)
  afterEach(() => httpMock.verify())

Assertion: jasmine expect() or jest expect() depending on project config.
Prefer Angular Testing Library (@testing-library/angular) for behaviour-driven tests.";

const NODE_VITEST_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: Node.js / Express (Vitest + Supertest) ===

Trigger keyword: vitest (node)

Framework stack:
  vitest    — test runner and mocking (vi.fn(), vi.mock())
  supertest — HTTP integration: request(app).post('/api/...')

Route / integration test pattern:
  import request from 'supertest'
  import { app } from '../app'
  import { describe, it, expect, vi, beforeEach } from 'vitest'

  describe('POST /api/products', () => {
    it('returns 201 with Location header when payload is valid', async () => {
      const res = await request(app).post('/api/products')
        .send({ name: 'Widget', price: 29.99 })
        .set('Content-Type', 'application/json')
      expect(res.status).toBe(201)
      expect(res.headers.location).toMatch(/\\/api\\/products\\//)
    })
    it('returns 400 when mandatory field is missing', async () => {
      const res = await request(app).post('/api/products').send({})
      expect(res.status).toBe(400)
    })
  })

Service unit test pattern (mock repository):
  vi.mock('../repository/ProductRepository')
  import { ProductRepository } from '../repository/ProductRepository'
  const mockRepo = vi.mocked(ProductRepository)
  mockRepo.save.mockResolvedValue({ id: 'uuid', ...payload })";

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
  Test files live NEXT TO the file they test, in the SAME directory — never a separate
  tests/ directory. This is the standard JS/TS convention (not Java's mirrored-tree one).

### Import and jest.mock paths — co-located, so imports follow the SAME depth rule as
### production code (see the tech-stack rules' Import depth within src/ section): the
### file being tested is same-directory; everything else is one dot-dot to a sibling folder.
  import request from 'supertest'                                          ✓
  import { ProductService } from './ProductService'                       ✓  (same directory — the file under test)
  import { ProductRepository } from '../repositories/ProductRepository'   ✓  (sibling directory)
  import { EventPublisher } from '../infrastructure/EventPublisher'       ✓  (sibling directory)
  jest.mock('../repositories/ProductRepository')                          ✓
  jest.mock('../infrastructure/EventPublisher', () => ({ ... }))         ✓
  Route tests do NOT import app.ts at all — see the Route test example below, which mounts
  the router on a local Express instance instead.

### Forbidden paths (imports AND jest.mock)
These will cause module-not-found errors at runtime:
  import ... from '../src/...'                    ✗  there is no src/ segment to cross — the test is ALREADY inside src/
  import ... from '../../services/...'           ✗  too many dot-dots — siblings under src/ are ONE dot-dot away, not two
  import ... from '@services/...'               ✗  no path aliases configured in tsconfig
  require(...)                                   ✗  use ES import syntax
  uuid(), faker()                                ✗  not in devDependencies — use plain strings

### jest is a global — NEVER import it
ts-jest with CommonJS injects jest as a global — no import is needed or allowed:
  jest.fn()                                      ✓  jest is already in scope
  jest.Mocked<T>                                 ✓  type reference, no import
  import { jest } from '@jest/globals'           ✗  causes TS2305: Module has no exported member 'jest'
  import jest from '@jest/globals'               ✗  same — ts-jest globals mode does not export jest
Do NOT add any import statement for jest. Use jest.fn(), jest.mock(), jest.Mocked<T> directly.

### Test IDs and test data — use plain string literals
Tests do NOT need real UUIDs. Use plain string literals:
  const id = 'test-id-123'                       ✓  simple, readable, predictable
  import { v4 as uuid } from 'uuid'              ✗  uuid not in devDependencies
  import { v4 as uuid } from 'crypto'            ✗  crypto has no v4 export — RUNTIME ERROR
  crypto.randomUUID()                            ✗  non-deterministic — use a literal string
Use literal dates too: new Date('2024-01-01') instead of new Date().

### Variable scope: mocks used across it() blocks
Any mock, subject, or fixture referenced in more than one it() block MUST be declared with
`let` at the describe()-level — OUTSIDE beforeEach — and only ASSIGNED inside beforeEach.
Declaring it with `const`/`let` INSIDE the beforeEach callback scopes it to that callback;
every it() block below then fails with TS2304 \"Cannot find name '<var>'\" because the variable
never existed outside beforeEach's own function body.
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
  Use .toThrow() not .toThrowError() — toThrowError was removed in Jest 30.
  mockResolvedValue always requires an argument — use mockResolvedValue(undefined) for void.
  NEVER use expect.any(X) where X is a TypeScript interface — interfaces have no runtime
  representation and this causes TS2693. Use expect.objectContaining({field: value}) instead.

### Imports in test files
Every class or function used in a test MUST be explicitly imported — even when mocked.
jest.mock() replaces the module at runtime but does NOT create the binding; without the
import, `new EventPublisher(...)` throws ReferenceError at parse time.
  import { EventPublisher } from '../infrastructure/EventPublisher'   ✓ (after jest.mock; sibling directory)
  new EventPublisher('', '')  without importing EventPublisher            ✗ ReferenceError";

/// Layer-specific examples for the Node/Express unit test skill — only the entry matching the
/// file's own layer is injected alongside `NODE_EXPRESS_UNIT_TEST_COMMON`. A model test has no
/// use for the Repository/Service/Route examples, so it no longer sees them.
fn node_express_layer_examples() -> std::collections::HashMap<&'static str, String> {
    std::collections::HashMap::from([
        ("route",
         "  ### Route test example (mock ONLY the service layer — per the Route rules above,\n\
  never the repository or event publisher directly; this isolates the route test from every\n\
  infrastructure concern, including any Kafka/DB connection attempt)\n\
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
  CRITICAL — mocking a CLASS is a different pattern from mocking an interface (the Repository\n\
  and EventPublisher examples elsewhere use a plain `{ method: jest.fn() } as any` object because\n\
  those are constructed by the service via a plain object, not `new`'d directly by the test).\n\
  A class the test itself constructs with `new` needs `jest.mock('../services/WidgetService')`\n\
  (auto-mocks every prototype method) PLUS a cast on the constructed instance, because\n\
  TypeScript's static type of `new WidgetService(...)` is still the REAL class, which has no\n\
  `.mockResolvedValue`:\n\
    WRONG:   let mockWidgetService: WidgetService = new WidgetService(a, b)\n\
             mockWidgetService.createWidget.mockResolvedValue(...)   ✗ TS2339: no .mockResolvedValue\n\
    CORRECT: let mockWidgetService: jest.Mocked<WidgetService>\n\
             mockWidgetService = new WidgetService(a, b) as jest.Mocked<WidgetService>\n\
  The constructor arguments passed to `new WidgetService(...)` are never used — jest.mock replaces\n\
  the real constructor body with a no-op — so pass `{} as any` for each one, matching however many\n\
  the real constructor declares.\n\
\n\
### Scope discipline\n\
  Only write tests for the HTTP methods and service operations described in the story.\n\
  Do NOT generate GET/DELETE/PUT route tests unless the story's acceptance criteria require them."
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
  })\n\
  RULES for model tests:\n\
  - NEVER write `new Widget(...)` — the model is an interface, not a class; `new` will not compile\n\
  - NEVER call `widget.save()` or any persistence method — models have no such methods\n\
  - Use `import { createWidget }` for the factory call; use `import type { Widget }` for type annotations only"
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
  RULE — this is the ONLY correct pattern, do not deviate from it: every assertion above\n\
  matches specific fields via objectContaining/toMatchObject. NEVER build a second, separate\n\
  object (by calling the aggregate factory or the event factory a second time) and compare it\n\
  by deep equality — every randomUUID()/new Date() call produces a DIFFERENT value, so two\n\
  independently-built objects never match, even when the code is completely correct.\n\
  This includes id/createdAt/modifiedAt inside a toMatchObject(...) literal — the factory\n\
  assigns these BEFORE any repository mock is even called, so setting a fake id/date on the\n\
  mock's resolved value and putting that SAME fake value inside toMatchObject(...) does not\n\
  make them match either:\n\
    WRONG: mockRepo.saveWidget.mockResolvedValue({ ...widget, id: 'fake-id' })\n\
           expect(result).toMatchObject({ id: 'fake-id', name: 'name-value' })   ✗ id differs\n\
  CORRECT: never put id/createdAt/modifiedAt inside the toMatchObject(...) literal at all —\n\
  check them separately with expect.any(String) / toBeInstanceOf(Date), exactly as shown above.\n\
  The same mistake also appears as a STANDALONE assertion, not just inside toMatchObject(...) —\n\
  same root cause (the mock invents a value never asked for), different shape:\n\
    WRONG: const widgetId = 'test-id-123'\n\
           mockRepo.saveWidget.mockImplementation(async (widget) => ({ ...widget, id: widgetId }))\n\
           const result = await service.createWidget({ name: 'name-value' })\n\
           expect(result.id).toEqual(widgetId)   ✗ the service returns the FACTORY's product,\n\
           never whatever the repository mock resolved to — reassigning id/createdAt/modifiedAt\n\
           in the mock's return value cannot make the service's real return value match it.\n\
  CORRECT: mock the repository to resolve with the SAME object it was called with (as in the\n\
  worked example above), then assert result.id with expect.any(String) — never against a\n\
  literal the mock invented."
         .to_string()),
        ("repository",
         "  ### Repository unit test example (real method, mocked Pool — never mock the method itself)\n\
  The repository's OWN test must call its REAL methods and assert on their REAL behavior. Mock\n\
  the injected pg.Pool (an external dependency) — never jest.spyOn the repository's own method:\n\
    import { Pool } from 'pg'\n\
    import { WidgetRepository } from './WidgetRepository'\n\
    import { createWidget } from '../models/Widget'\n\
\n\
    NEVER declare the mock as `jest.Mocked<Pool>` — pg.Pool.query is a heavily overloaded\n\
    generic method, and TypeScript's Mocked<> utility collapses its parameter type to `never`\n\
    for overloaded signatures it can't resolve, causing TS2345 on every mockResolvedValue() call\n\
    even though the code is correct. Declare a minimal plain shape instead and cast only at the\n\
    point you hand it to the constructor:\n\
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
  When the constructor calls a factory/getter method on an injected dependency to obtain the\n\
  REAL object it uses internally (e.g. `this.client = connection.getClient()`), the mocked\n\
  factory method MUST return the SAME mock object the test asserts against — never two\n\
  disconnected mocks for the same collaborator:\n\
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
  Rule: whenever a factory method (getClient(), getConnection(), createClient(), etc.) is stubbed\n\
  with mockReturnValue({...}), that argument must BE the mock instance asserted against later —\n\
  build it as its own variable first, then pass it into mockReturnValue(...).\n\
\n\
  When these mocks are ALSO hoisted to describe-level scope (per the variable-scope rule above,\n\
  needed as soon as more than one it() block uses them), do NOT declare them with the real\n\
  external-library type (its SDK's Client/Connection type) — a partial mock only implementing\n\
  the methods this test needs will NOT satisfy that type's full member list (most SDK client\n\
  interfaces require far more members than any one class ever calls), causing TS2322\n\
  \"Property '<member>' is missing\". Declare hoisted mocks as `any` instead, and cast only the\n\
  outer object at construction time:\n\
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
      })"
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
pub(crate) fn layer_has_worked_example(tech: &str, layer: &str) -> bool {
    use crate::tech::TechFamily;
    match TechFamily::detect(tech) {
        TechFamily::NodeExpress => node_express_layer_examples().contains_key(layer),
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
///   Node/Express + "vitest"+ "supertest" → NODE_VITEST_UNIT_TEST_SKILL
///
/// Falls back to unit_testing_skill(tech, layer) when no relevant ADR exists.
pub(crate) fn testing_skill_from_adrs(adrs: &[Adr], tech: &str, layer: &str) -> String {
    let t = tech.to_lowercase();
    let family = crate::tech::TechFamily::detect(tech);
    let is_react = family == crate::tech::TechFamily::React;
    let is_node = family == crate::tech::TechFamily::NodeExpress;

    // Match on decision content — titles are service-scoped and vary across projects.
    let adr = adrs.iter().find(|a| {
        if !a.title.to_lowercase().contains("testing") { return false; }
        let d = a.decision.to_lowercase();
        if is_react {
            (d.contains("vitest") || d.contains("jest")) && d.contains("react testing")
        } else if is_node {
            (d.contains("jest") || d.contains("vitest")) && d.contains("supertest")
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
        if is_node && d.contains("vitest") {
            return NODE_VITEST_UNIT_TEST_SKILL.to_string();
        }
        if is_node && d.contains("jest") && !d.contains("vitest") {
            return node_express_unit_test_skill(layer);
        }
    }
    unit_testing_skill(tech, layer)
}

