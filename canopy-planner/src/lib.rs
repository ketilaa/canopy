//! The Planner: turns a story and a Gherkin feature into a plan the Implementer can run.
//!
//! Developed in isolation, like the Implementer. The model is an unreliable component inside a
//! deterministic harness: the feature is parsed, the repository facts are computed, the draft is
//! schema-constrained, and a verifier checks it before anything runs.

pub mod draft;
pub mod facts;
pub mod gherkin;
pub mod planner;
pub mod prompt;
pub mod verify;

#[cfg(test)]
mod testing;
