//! View definition: describes how a materialized view is computed.

use serde::{Deserialize, Serialize};

/// Kind of join operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

/// Aggregation function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Aggregation {
    Count,
    Sum(String),     // field name
    Avg(String),
    Min(String),
    Max(String),
}

/// A join clause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinClause {
    pub kind: JoinKind,
    pub target: String,
    pub on_left: String,
    pub on_right: String,
}

/// Definition of a materialized view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefinition {
    /// Source collection names.
    pub sources: Vec<String>,
    /// Optional filter (KQL-style query as JSON).
    pub filter: Option<serde_json::Value>,
    /// Projection: which fields to include.
    pub projection: Vec<String>,
    /// Optional joins.
    pub joins: Vec<JoinClause>,
    /// Optional aggregations.
    pub aggregations: Vec<Aggregation>,
    /// Group-by fields.
    pub group_by: Vec<String>,
    /// Optional ordering.
    pub order_by: Vec<(String, bool)>, // (field, ascending)
    /// Optional limit.
    pub limit: Option<usize>,
}

impl Default for ViewDefinition {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            filter: None,
            projection: Vec::new(),
            joins: Vec::new(),
            aggregations: Vec::new(),
            group_by: Vec::new(),
            order_by: Vec::new(),
            limit: None,
        }
    }
}
