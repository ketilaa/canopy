use crate::{ProposedAdr, ServiceEntry, ServicesRegistry};

impl ServicesRegistry {
    /// Merge an accepted architecture-decision proposal into the services registry —
    /// creating a new service/infrastructure entry or updating an existing one.
    pub fn apply_adr_proposal(&mut self, proposal: &ProposedAdr) {
        let is_infra = proposal.component_type.as_deref() == Some("infrastructure");

        if is_infra {
            // Infrastructure proposals (DB, event broker) describe a shared component, not the owning
            // service. Derive the component name from its technology so it gets its own entry.
            if let Some(ref tech) = proposal.technology {
                let infra_name = tech
                    .split_whitespace()
                    .next()
                    .unwrap_or(tech)
                    .to_lowercase();
                if !infra_name.is_empty() && !self.services.iter().any(|s| s.name == infra_name) {
                    self.services.push(ServiceEntry {
                        name: infra_name,
                        responsibilities: vec![],
                        technology: Some(tech.clone()),
                        component_type: Some("infrastructure".to_string()),
                    });
                }
            }
            return;
        }

        // Frontend proposals often have service: null with the component name in decision instead.
        // Derive the name: for a naming proposal (no technology) use decision; for a tech stack
        // proposal (technology set) find an existing untyped frontend entry.
        let derived_name: Option<String>;
        let name: &str = if let Some(ref svc) = proposal.service {
            if svc.is_empty() { return; }
            svc.as_str()
        } else if proposal.component_type.as_deref() == Some("frontend") {
            if proposal.technology.is_none() {
                // Component-naming proposal: decision holds the frontend name.
                let candidate = proposal.decision.trim();
                if candidate.is_empty() { return; }
                derived_name = Some(candidate.to_string());
                derived_name.as_deref().unwrap()
            } else {
                // Tech stack proposal: apply to the most recent frontend entry without technology.
                if let Some(entry) = self.services.iter_mut().find(|s| {
                    s.component_type.as_deref() == Some("frontend") && s.technology.is_none()
                }) {
                    entry.technology = proposal.technology.clone();
                }
                return;
            }
        } else {
            return;
        };

        let filtered_responsibilities: Vec<String> = proposal
            .service_responsibilities
            .iter()
            .filter(|r| r.as_str() != "<none>")
            .cloned()
            .collect();

        if let Some(entry) = self.services.iter_mut().find(|s| s.name == *name) {
            for r in &filtered_responsibilities {
                let normalized = r.trim().trim_end_matches('.').to_lowercase();
                let already_present = entry.responsibilities.iter().any(|existing| {
                    existing.trim().trim_end_matches('.').to_lowercase() == normalized
                });
                if !already_present {
                    entry.responsibilities.push(r.clone());
                }
            }
            // A proposal with an explicit component_type is a tech stack ADR and is authoritative
            // for technology — overrides any accidental earlier setting (e.g. a database ADR that
            // leaked its technology onto the service entry because component_type was not set).
            if entry.technology.is_none() || proposal.component_type.is_some() {
                entry.technology = proposal.technology.clone();
            }
            if entry.component_type.is_none() || proposal.component_type.is_some() {
                entry.component_type = Some(
                    proposal.component_type.clone().unwrap_or_else(|| "service".to_string())
                );
            }
        } else {
            self.services.push(ServiceEntry {
                name: name.to_string(),
                responsibilities: filtered_responsibilities,
                technology: proposal.technology.clone(),
                component_type: Some(
                    proposal.component_type.clone().unwrap_or_else(|| "service".to_string())
                ),
            });
        }
    }
}
