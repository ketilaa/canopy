use roots_storage::{RelationshipRow, SymbolRow};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ImpactSummary {
    pub fan_in:           usize,
    pub fan_out:          usize,
    pub score:            u32,
    pub risk:             String,
    pub transitive_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SymbolContextPacket {
    pub symbol:  SymbolRow,
    pub callers: Vec<RelationshipRow>,
    pub callees: Vec<RelationshipRow>,
    pub deps:    Vec<RelationshipRow>,
    pub impact:  ImpactSummary,
    pub facts:   Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectContextPacket {
    pub project:  String,
    pub language: String,
    pub symbols:  Vec<SymbolRow>,
    pub facts:    Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FeatureContextPacket {
    pub goal:          String,
    pub keywords:      Vec<String>,
    pub symbols:       Vec<SymbolRow>,
    pub relationships: Vec<RelationshipRow>,
    pub facts:         Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FileContextPacket {
    pub file:          String,
    pub language:      String,
    pub symbols:       Vec<SymbolRow>,
    pub relationships: Vec<RelationshipRow>,
    pub facts:         Vec<String>,
}
