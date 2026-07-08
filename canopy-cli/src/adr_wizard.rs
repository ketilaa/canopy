use anyhow::{Context, Result};
use canopy_core::Adr;
use canopy_storage::save_adr;
use dialoguer::theme::ColorfulTheme;

pub(crate) fn architecture_style_adr(idx: usize) -> Adr {
    match idx {
        _ => Adr {
            title: "Architecture Style".to_string(),
            decision: "Event-driven microservices using Domain-Driven Design".to_string(),
            reason: "Services are bounded by domain context and communicate through domain events. \
                     This enables independent deployability, clear ownership boundaries, \
                     and natural alignment with the domain model."
                .to_string(),
            alternatives: vec![
                "Modular monolith".to_string(),
                "Layered monolith".to_string(),
            ],
        },
    }
}

pub(crate) fn topic_naming_convention_adr(idx: usize) -> Adr {
    match idx {
        0 => Adr {
            title: "Topic Naming Convention".to_string(),
            decision: "One topic per aggregate: <aggregate>-events (e.g. product-events, order-events)".to_string(),
            reason: "Scoping topics to the aggregate gives per-entity ordering guarantees when \
                     partitioned by entity ID, clean schema evolution per aggregate type, and \
                     lets consumers subscribe only to the aggregates they care about. \
                     Finer granularity (one topic per event type) creates subscription sprawl; \
                     coarser granularity (one topic per service or one global topic) loses \
                     ordering and complicates schema management."
                .to_string(),
            alternatives: vec![
                "One topic per service: <service>-events".to_string(),
                "Reverse-DNS style: <domain>.<aggregate>.events".to_string(),
            ],
        },
        1 => Adr {
            title: "Topic Naming Convention".to_string(),
            decision: "One topic per service: <service>-events (e.g. product-service-events)".to_string(),
            reason: "Groups all events from a single deployable service under one topic. \
                     Simpler when a service owns a single aggregate, but conflates service \
                     boundaries with aggregate boundaries as the service grows."
                .to_string(),
            alternatives: vec![
                "One topic per aggregate: <aggregate>-events".to_string(),
            ],
        },
        _ => Adr {
            title: "Topic Naming Convention".to_string(),
            decision: "Reverse-DNS style: <domain>.<aggregate>.events (e.g. commerce.product.events)".to_string(),
            reason: "Namespaced topics prevent collisions in multi-domain Kafka clusters and \
                     make ownership explicit. Common in large organisations sharing a single broker."
                .to_string(),
            alternatives: vec![
                "One topic per aggregate: <aggregate>-events".to_string(),
            ],
        },
    }
}

pub(crate) fn event_broker_adr(idx: usize) -> Adr {
    match idx {
        0 => Adr {
            title: "Event Broker".to_string(),
            decision: "Redpanda as the event broker".to_string(),
            reason: "Redpanda is Kafka-compatible (same producer/consumer API) with no JVM dependency \
                     and first-class Docker Compose support. It starts in milliseconds and requires \
                     no ZooKeeper, making it the lowest-friction choice for local development in an \
                     event-driven microservices architecture."
                .to_string(),
            alternatives: vec![
                "Apache Kafka".to_string(),
                "RabbitMQ".to_string(),
                "NATS".to_string(),
            ],
        },
        1 => Adr {
            title: "Event Broker".to_string(),
            decision: "Apache Kafka as the event broker".to_string(),
            reason: "Kafka is the de facto standard for high-throughput, durable event streaming. \
                     Its log-based model supports event replay and consumer group fan-out, \
                     aligning naturally with event-sourcing and DDD patterns."
                .to_string(),
            alternatives: vec![
                "Redpanda (Kafka-compatible, no JVM)".to_string(),
                "RabbitMQ".to_string(),
            ],
        },
        2 => Adr {
            title: "Event Broker".to_string(),
            decision: "RabbitMQ as the event broker".to_string(),
            reason: "RabbitMQ is a mature AMQP message broker well-suited to flexible routing \
                     patterns (exchanges, queues, bindings). It is simpler to operate than Kafka \
                     when throughput requirements are modest."
                .to_string(),
            alternatives: vec![
                "Apache Kafka".to_string(),
                "Redpanda (Kafka-compatible, no JVM)".to_string(),
            ],
        },
        _ => Adr {
            title: "Event Broker".to_string(),
            decision: "NATS as the event broker".to_string(),
            reason: "NATS is a lightweight, cloud-native messaging system with a tiny footprint. \
                     NATS JetStream adds persistence and replay. Good choice when low latency \
                     and operational simplicity matter more than Kafka's log-retention guarantees."
                .to_string(),
            alternatives: vec![
                "Redpanda (Kafka-compatible, no JVM)".to_string(),
                "Apache Kafka".to_string(),
            ],
        },
    }
}

pub(crate) fn deployment_style_adr(idx: usize) -> Adr {
    match idx {
        _ => Adr {
            title: "Deployment Style".to_string(),
            decision: "Docker Compose for local development".to_string(),
            reason: "All services, databases, and event infrastructure run locally in Docker Compose. \
                     This provides a consistent, portable local development environment without \
                     requiring a Kubernetes cluster. Production deployment strategy is decided separately."
                .to_string(),
            alternatives: vec![
                "Kubernetes with local cluster (minikube or kind)".to_string(),
                "Native processes per service".to_string(),
            ],
        },
    }
}

/// Prompts for a testing framework and saves the ADR when the accepted tech stack is React/Vite
/// or Node.js/Express and no testing strategy ADR for this service exists yet.
/// Title and slug are scoped to the service so each frontend or backend can choose independently:
///   admin-portal → "Admin Portal Testing Strategy", adr-NNN-admin-portal-testing-strategy.yaml
///   product      → "Product Testing Strategy",      adr-NNN-product-testing-strategy.yaml
/// Angular TestBed and Spring Boot JUnit 5 are implicit — no prompt needed.
pub(crate) fn maybe_prompt_testing_strategy(
    theme: &ColorfulTheme,
    existing_adrs: &mut Vec<Adr>,
    technology: &str,
    service_name: &str,
) -> Result<()> {
    let t = technology.to_lowercase();
    let is_react = t.contains("react") || t.contains("vite");
    let is_node  = t.contains("node") || t.contains("express") || t.contains("nest");
    if !is_react && !is_node {
        return Ok(());
    }
    let display_name: String = service_name.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let slug = format!("{}-testing-strategy", service_name);
    let title = format!("{} Testing Strategy", display_name);
    if existing_adrs.iter().any(|a| a.title.to_lowercase() == title.to_lowercase()) {
        return Ok(());
    }
    let (prompt, options, adr_offset) = if is_react {
        (
            format!("{} testing framework", display_name),
            vec![
                "Vitest + React Testing Library  (recommended for Vite)",
                "Jest  + React Testing Library",
            ],
            0usize,
        )
    } else {
        (
            format!("{} testing framework", display_name),
            vec![
                "Jest  + Supertest  (recommended — de facto standard)",
                "Vitest + Supertest",
            ],
            2usize,
        )
    };
    let idx = crate::ui::select_required(theme, &prompt, &options, 0, "failed to read testing framework selection")?;
    let adr = testing_strategy_adr(adr_offset + idx, &display_name);
    let index = existing_adrs.len() + 1;
    save_adr(index, &slug, &adr)
        .context("failed to save testing-strategy ADR")?;
    println!("  Saved: adr-{:03}-{}.yaml", index, slug);
    existing_adrs.push(adr);
    Ok(())
}

/// Pre-authored testing strategy ADRs. Title is scoped to the service name.
/// 0 = React + Vitest, 1 = React + Jest, 2 = Node + Jest, 3 = Node + Vitest.
/// Angular TestBed and Spring Boot JUnit 5 are implicit — no ADR needed.
fn testing_strategy_adr(idx: usize, service: &str) -> Adr {
    match idx {
        0 => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Vitest + React Testing Library for unit tests".to_string(),
            reason: "Vitest is the natural testing companion for Vite-based React projects. \
                     It shares the Vite config, runs natively in ES modules, and is significantly \
                     faster than Jest for component tests. React Testing Library enforces \
                     user-behaviour-oriented assertions over implementation details."
                .to_string(),
            alternatives: vec![
                "Jest + React Testing Library".to_string(),
                "Playwright (component mode)".to_string(),
            ],
        },
        1 => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Jest + React Testing Library for unit tests".to_string(),
            reason: "Jest is the most widely adopted JavaScript test runner and works well with \
                     non-Vite React setups. It has the broadest ecosystem of matchers and utilities."
                .to_string(),
            alternatives: vec![
                "Vitest + React Testing Library".to_string(),
                "Playwright (component mode)".to_string(),
            ],
        },
        2 => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Jest + Supertest for unit and route tests".to_string(),
            reason: "Jest is the de facto standard test runner for Node.js projects. \
                     Supertest provides a clean HTTP-layer assertion API that exercises \
                     the full Express middleware stack without starting a real server."
                .to_string(),
            alternatives: vec![
                "Vitest + Supertest".to_string(),
                "Mocha + Chai + Supertest".to_string(),
            ],
        },
        _ => Adr {
            title: format!("{} Testing Strategy", service),
            decision: "Vitest + Supertest for unit and route tests".to_string(),
            reason: "Vitest offers a faster, ES-module-native alternative to Jest for Node.js \
                     projects. Its API is Jest-compatible, so migration is low-risk. \
                     Supertest provides the same HTTP-layer assertions."
                .to_string(),
            alternatives: vec![
                "Jest + Supertest".to_string(),
            ],
        },
    }
}
