use canopy_core::Adr;

// ── Architecture skills ───────────────────────────────────────────────────────
// Architecture skills are orthogonal to tech-stack skills.
// They capture cross-cutting patterns that apply regardless of language or framework.
// Contract: every architecture skill fills three required sections:
//   vocabulary      — terms this pattern introduces and their precise meaning in code
//   structural_rules — naming, layering, and dependency rules
//   anti_patterns   — explicit prohibitions that prevent the most common mistakes
//
// Skills are derived from ADR decisions (keyword matching on the architecture-style ADR).
// To add a new architecture skill: implement a builder, add keyword detection below.

pub(crate) struct ArchitectureSkill {
    pub name: String,
    /// Terms this pattern introduces and their precise meaning in code.
    pub vocabulary: String,
    /// Naming, layering, and dependency rules.
    pub structural_rules: String,
    /// Explicit prohibitions — what NOT to do.
    pub anti_patterns: String,
}

impl ArchitectureSkill {
    pub(crate) fn render(&self) -> String {
        super::render_skill(&format!("## Architecture: {}", self.name), &[
            ("### Vocabulary", self.vocabulary.as_str()),
            ("### Structural rules", self.structural_rules.as_str()),
            ("### Anti-patterns", self.anti_patterns.as_str()),
        ])
    }
}

fn ddd_skill_for_tech(tech: &str) -> ArchitectureSkill {
    let t = tech.to_lowercase();
    let is_node = t.contains("node") || t.contains("express") || t.contains("nest");
    let is_front = t.contains("react") || t.contains("angular") || t.contains("vite");

    if is_node && !is_front {
        ArchitectureSkill {
            name: "Domain-Driven Design (DDD)".to_string(),
            vocabulary:
                "  Aggregate root: the single entry point to a cluster of related entities; holds invariants.\n\
                 Entity: has identity (id field), mutable state, lifecycle — modelled as a TypeScript interface or class.\n\
                 Value object: no identity, equality by value, immutable — use a readonly TypeScript interface.\n\
                 Repository: one per aggregate root; returns fully-constructed aggregates.\n\
                 Application service: orchestrates use cases, translates domain ↔ DTO; a plain class, not a framework annotation.\n\
                 Domain service: stateless; expresses a business operation that spans multiple entities."
                .to_string(),
            structural_rules:
                "  Use the ubiquitous language from the stories and domain registry in all identifiers —\n\
                 class names, method names, field names. No technical synonyms (WidgetData, WidgetInfo)\n\
                 when the agreed term is Widget.\n\
                 Business invariants live in the aggregate, not in the application service or route handler.\n\
                 DTOs live at the API boundary (returned from routes); never expose domain entities directly in REST responses.\n\
                 Access nested entities only through their aggregate root.\n\
                 Repositories return domain objects; the service layer maps them to response shapes for callers.\n\
                 Aggregate lifecycle — three distinct responsibilities, never mixed:\n\
                   Factory (model file): constructs a new aggregate instance, assigns id and createdAt.\n\
                   Repository: receives a fully-constructed aggregate and persists it unchanged — never assigns ids or timestamps.\n\
                   Application service: calls the factory to construct, then the repository to persist."
                .to_string(),
            anti_patterns:
                "  No business logic in route handlers or repositories — route handlers translate HTTP;\n\
                 repositories translate persistence.\n\
                 No anemic domain model — a type that is only fields with all logic in services is not DDD;\n\
                 move invariants into the entity or service.\n\
                 No findById that silently returns undefined without handling — throw a typed domain error\n\
                 (e.g. WidgetNotFoundError) or return a discriminated union with explicit handling at the call site."
                .to_string(),
        }
    } else {
        // Spring / JVM variant (also used as default)
        ArchitectureSkill {
            name: "Domain-Driven Design (DDD)".to_string(),
            vocabulary:
                "  Aggregate root: the single entry point to a cluster of related entities; holds invariants.\n\
                 Entity: has identity (@Id), mutable state, lifecycle — modelled in domain/.\n\
                 Value object: no identity, equality by value, immutable — use Java records.\n\
                 Repository: one per aggregate root; returns fully-constructed aggregates.\n\
                 Application service (@Service): orchestrates use cases, translates domain ↔ DTO.\n\
                 Domain service: stateless; expresses a business operation that spans multiple entities."
                .to_string(),
            structural_rules:
                "  Use the ubiquitous language from the stories and domain registry in all identifiers —\n\
                 class names, method names, field names. No technical synonyms (WidgetData, WidgetInfo)\n\
                 when the agreed term is Widget.\n\
                 Business invariants live in the aggregate, not in the application service or controller.\n\
                 DTOs live at the API boundary (dto/); never expose domain entities in REST responses.\n\
                 Access nested entities only through their aggregate root — never inject a nested entity's\n\
                 repository directly.\n\
                 Repositories return domain objects; the service layer maps them to DTOs for callers.\n\
                 Aggregate lifecycle — three distinct responsibilities, never mixed:\n\
                   Factory (domain class or static method): constructs a new aggregate instance, assigns id and createdAt.\n\
                   Repository: receives a fully-constructed aggregate and persists it unchanged — never assigns ids or timestamps.\n\
                   Application service: calls the factory to construct, then the repository to persist."
                .to_string(),
            anti_patterns:
                "  No business logic in controllers or repositories — controllers translate HTTP;\n\
                 repositories translate persistence.\n\
                 No anemic domain model — an entity that is only getters/setters with all logic in\n\
                 services is not DDD; move invariants into the entity.\n\
                 No getById that silently returns null — throw a domain exception (WidgetNotFoundException)\n\
                 or return Optional with explicit handling at the call site."
                .to_string(),
        }
    }
}

fn event_orientation_skill_for_tech(tech: &str) -> ArchitectureSkill {
    let t = tech.to_lowercase();
    let is_node = t.contains("node") || t.contains("express") || t.contains("nest");

    if is_node {
        ArchitectureSkill {
            name: "Event Orientation (Node.js / Kafka)".to_string(),
            vocabulary:
                "  Domain event: a fact that happened — immutable, past tense, e.g. WidgetCreated.\n\
                 Event type file: TypeScript interface in src/events/ describing the message payload.\n\
                 Publisher utility: thin module in src/infrastructure/ that wraps the Kafka/Redpanda\n\
                 client and exposes a typed publish(topic, event) function.\n\
                 Transactional boundary: persist first; publish only after the database write succeeds."
                .to_string(),
            structural_rules:
                "  Name events in past tense: WidgetCreated, OrderPlaced.\n\
                 Event type (src/events/) and publisher (src/infrastructure/) must precede the service step — follow the tech skill layer order.\n\
                 Topic: use the Topic Naming Convention ADR value (e.g. widget-events).\n\
                 Payload: eventId (own identity) + <entity>Id (aggregate reference) + occurredAt —\n\
                 never copy the aggregate's other fields onto the event, never add a modifiedAt.\n\
                 Add kafkajs to the package.json step if not already listed."
                .to_string(),
            anti_patterns:
                "  Never publish before the database write completes.\n\
                 Never import the Kafka client directly into the service — always through the publisher utility.\n\
                 Never plan an event consumer/listener unless the story explicitly requires consuming an event.\n\
                 Never omit the event type file when the service publishes an event — the type must be defined\n\
                 before the service that uses it."
                .to_string(),
        }
    } else {
        // Spring / JVM variant
        ArchitectureSkill {
            name: "Event Orientation".to_string(),
            vocabulary:
                "  Domain event: a fact that happened — immutable, past tense, e.g. WidgetCreated.\n\
                 Event publisher: the service layer that emits events after successful persistence.\n\
                 Event listener: a separate class that reacts to one event; one concern per listener.\n\
                 Transactional boundary: the unit-of-work that must complete before an event is visible."
                .to_string(),
            structural_rules:
                "  Name events in past tense using domain language: WidgetCreated, OrderPlaced.\n\
                 Define event classes in the domain layer alongside the aggregate they describe.\n\
                 Publish events from the service layer after the aggregate is persisted — never before.\n\
                 Use @TransactionalEventListener(phase = AFTER_COMMIT) so listeners fire only on\n\
                 successful commit; this prevents phantom events from rolled-back transactions.\n\
                 Event payload: eventId (own identity) + <entity>Id (aggregate reference) + occurredAt —\n\
                 never copy the aggregate's other fields onto the event, never add a modifiedAt/updatedAt.\n\
                 One listener class per consuming concern; listeners must not call back into the\n\
                 publishing service (no circular event chains)."
                .to_string(),
            anti_patterns:
                "  Never publish events before the database write commits — a rollback after publish\n\
                 creates phantom events that consumers act on against data that was never saved.\n\
                 Never use events for synchronous responses — if the caller needs a return value,\n\
                 use a direct service call, not an event.\n\
                 Never import ApplicationEventPublisher into the domain model — it is infrastructure;\n\
                 the domain emits events as return values or via a domain service; the application\n\
                 service calls the publisher.\n\
                 ApplicationEventPublisher is included via spring-boot-starter — no extra Maven\n\
                 dependency is needed or should be added."
                .to_string(),
        }
    }
}

fn microservices_skill() -> ArchitectureSkill {
    ArchitectureSkill {
        name: "Microservices".to_string(),
        vocabulary:
            "  Bounded context: the domain scope of one service — it owns its data and its language.\n\
             Service contract: the OAS API surface and the domain events a service publishes;\n\
             the only things other services may depend on.\n\
             Anti-corruption layer: an adapter that translates between two bounded contexts so\n\
             their models stay independent."
            .to_string(),
        structural_rules:
            "  Each service owns exactly one database schema — no other service reads or writes\n\
             its tables directly.\n\
             Cross-service state changes: prefer async domain events over synchronous HTTP calls.\n\
             Synchronous HTTP (OAS contract) is acceptable for queries needing an immediate response.\n\
             A service's domain model classes are never imported by another service — duplicate\n\
             the fields you need as local DTOs rather than sharing a domain library.\n\
             Service names are kebab-case and match the bounded context they represent."
            .to_string(),
        anti_patterns:
            "  No shared database between services — even read-only access couples services to\n\
             each other's schema evolution.\n\
             No distributed transactions — use eventual consistency and compensating events.\n\
             No shared domain model library — a common-domain JAR couples release cycles and\n\
             violates bounded context autonomy.\n\
             No direct method calls into another service's internal classes — only through its\n\
             published OAS contract or domain events."
            .to_string(),
    }
}

/// Derive active architecture skills from the project's ADR decisions.
/// Scans each ADR for keywords and maps them to the corresponding skill.
/// Returns the rendered skills joined as a single string for prompt injection.
pub fn skills_for_architecture(adrs: &[Adr], tech: &str) -> String {
    let text: String = adrs.iter()
        .map(|a| format!("{} {}", a.title, a.decision).to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let t = tech.to_lowercase();
    let is_frontend = t.contains("react") || t.contains("angular") || t.contains("vite") || t.contains("vue");

    let mut skills: Vec<ArchitectureSkill> = Vec::new();
    // DDD and event orientation are backend concerns — frontends post to APIs, they do not own
    // aggregates or publish domain events directly to a broker.
    if !is_frontend {
        if text.contains("domain-driven") || text.contains("domain driven") || text.contains("ddd") {
            skills.push(ddd_skill_for_tech(tech));
        }
        if text.contains("event-driven") || text.contains("event driven") || text.contains("domain event") {
            skills.push(event_orientation_skill_for_tech(tech));
        }
    }
    if text.contains("microservice") {
        skills.push(microservices_skill());
    }

    if skills.is_empty() {
        return String::new();
    }
    skills.iter().map(|s| s.render()).collect::<Vec<_>>().join("\n\n")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_produces_expected_literal_output() {
        let skill = ArchitectureSkill {
            name: "Sample".to_string(),
            vocabulary: "vocab-body".to_string(),
            structural_rules: "rules-body".to_string(),
            anti_patterns: "anti-body".to_string(),
        };
        assert_eq!(
            skill.render(),
            "## Architecture: Sample\n\n\
             ### Vocabulary\nvocab-body\n\n\
             ### Structural rules\nrules-body\n\n\
             ### Anti-patterns\nanti-body"
        );
    }
}
