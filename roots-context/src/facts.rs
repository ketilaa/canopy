use roots_storage::{RelationshipRow, Store};

use crate::error::ContextError;

pub fn from_relationships(rels: &[RelationshipRow]) -> Vec<String> {
    rels.iter().map(|r| {
        let verb = match r.kind.as_str() {
            "CALLS"      => "calls",
            "EXTENDS"    => "extends",
            "IMPLEMENTS" => "implements",
            "IMPORTS"    => "imports",
            "REFERENCES" => "references",
            other        => return format!("{} {} {}", r.from_symbol, other.to_lowercase(), r.to_symbol),
        };
        format!("{} {} {}", r.from_symbol, verb, r.to_symbol)
    }).collect()
}

pub fn symbol_facts(
    store: &Store,
    workspace_id: &str,
    fqn: &str,
) -> Result<Vec<String>, ContextError> {
    let graph = store.query_graph(workspace_id, fqn)?;
    let mut all: Vec<RelationshipRow> = graph.outgoing;
    all.extend(graph.incoming);
    all.dedup_by(|a, b| a.from_symbol == b.from_symbol && a.to_symbol == b.to_symbol && a.kind == b.kind);
    Ok(from_relationships(&all))
}

#[cfg(test)]
mod tests {
    use super::*;
    use roots_storage::RelationshipRow;

    fn rel(from: &str, to: &str, kind: &str) -> RelationshipRow {
        RelationshipRow {
            from_symbol:  from.into(),
            to_symbol:    to.into(),
            kind:         kind.into(),
            file:         "Test.java".into(),
            line:         Some(1),
            workspace_id: "ws".into(),
        }
    }

    #[test]
    fn calls_fact() {
        let facts = from_relationships(&[rel("A", "B", "CALLS")]);
        assert_eq!(facts[0], "A calls B");
    }

    #[test]
    fn extends_fact() {
        let facts = from_relationships(&[rel("Dog", "Animal", "EXTENDS")]);
        assert_eq!(facts[0], "Dog extends Animal");
    }

    #[test]
    fn implements_fact() {
        let facts = from_relationships(&[rel("OrderService", "IOrderService", "IMPLEMENTS")]);
        assert_eq!(facts[0], "OrderService implements IOrderService");
    }

    #[test]
    fn unknown_kind_lowercased() {
        let facts = from_relationships(&[rel("A", "B", "PUBLISHES")]);
        assert_eq!(facts[0], "A publishes B");
    }
}
