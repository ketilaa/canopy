use std::collections::HashSet;

use roots_storage::{RelationshipRow, Store, SymbolRow};

use crate::error::ContextError;
use crate::facts;
use crate::packet::FeatureContextPacket;

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "of", "in", "for", "to", "and", "or",
    "with", "from", "at", "by", "as", "on", "that", "this",
    "it", "is", "are", "was", "were", "be", "been",
];

pub fn extract_keywords(goal: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    goal.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .filter(|w| seen.insert(w.clone()))
        .collect()
}

pub fn feature_context(
    store: &Store,
    workspace_id: &str,
    goal: &str,
) -> Result<FeatureContextPacket, ContextError> {
    let keywords = extract_keywords(goal);

    let mut seen_fqns: HashSet<String> = HashSet::new();
    let mut symbols: Vec<SymbolRow> = Vec::new();
    for keyword in &keywords {
        for sym in store.query_prefix(workspace_id, keyword)? {
            if seen_fqns.insert(sym.fqn.clone()) {
                symbols.push(sym);
            }
        }
    }

    let matched_fqns: HashSet<String> = symbols.iter().map(|s| s.fqn.clone()).collect();
    let mut all_rels: Vec<RelationshipRow> = Vec::new();
    for fqn in &matched_fqns {
        for dep in store.query_deps(workspace_id, fqn)? {
            all_rels.push(dep);
        }
    }
    all_rels.dedup_by(|a, b| {
        a.from_symbol == b.from_symbol && a.to_symbol == b.to_symbol && a.kind == b.kind
    });

    let facts = facts::from_relationships(&all_rels);

    Ok(FeatureContextPacket {
        goal: goal.to_string(),
        keywords,
        symbols,
        relationships: all_rels,
        facts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_extracted_and_deduplicated() {
        let kws = extract_keywords("order placement flow and order processing");
        assert!(kws.contains(&"order".to_string()));
        assert!(kws.contains(&"placement".to_string()));
        assert!(kws.contains(&"flow".to_string()));
        assert!(kws.contains(&"processing".to_string()));
        assert!(!kws.contains(&"and".to_string()));
        let count = kws.iter().filter(|k| k.as_str() == "order").count();
        assert_eq!(count, 1, "order should appear only once");
    }

    #[test]
    fn stopwords_filtered() {
        let kws = extract_keywords("the order for the customer");
        assert!(!kws.contains(&"the".to_string()));
        assert!(!kws.contains(&"for".to_string()));
        assert!(kws.contains(&"order".to_string()));
        assert!(kws.contains(&"customer".to_string()));
    }

    #[test]
    fn short_words_filtered() {
        let kws = extract_keywords("get id by name");
        assert!(!kws.contains(&"id".to_string()));
        assert!(!kws.contains(&"by".to_string()));
        assert!(kws.contains(&"get".to_string()));
        assert!(kws.contains(&"name".to_string()));
    }
}
