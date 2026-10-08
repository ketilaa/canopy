//! The Implementer: turns one plan step into files, checked by the build.
//!
//! Developed in isolation from the rest of canopy. The model is an unreliable component inside a
//! deterministic harness: its output is schema-constrained, the harness writes the files and runs
//! the checks, and nothing the model says about its own work is trusted.
//!
//! Everything that happens is reported as a typed [`observe::RunEvent`]; consumers (console, trace
//! file, metrics, tests) are [`observe::RunObserver`]s of the same stream.

pub mod build;
pub mod files;
pub mod gates;
pub mod hints;
pub mod implementer;
pub mod llm;
pub mod observe;
pub mod prompt;
pub mod sandbox;
pub mod snapshot;
pub mod tools;

#[cfg(test)]
mod testing;
