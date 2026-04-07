//! KQL (Kaya Query Language) parser and evaluation.
//!
//! Simple query language for COLLECTION.FIND:
//! `{ "field": "value", "age": { "$gt": 18 } }`


use crate::RelationalError;

/// A parsed KQL query.
#[derive(Debug, Clone)]
pub struct KqlQuery {
    pub filters: Vec<Filter>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// A single filter condition.
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}

/// Filter operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Contains,
    Exists,
}

impl KqlQuery {
    /// Parse a KQL query from a JSON value.
    ///
    /// Format: `{ "field": "value" }` for equality,
    /// or `{ "field": { "$gt": 10 } }` for operators.
    pub fn parse(value: &serde_json::Value) -> Result<Self, RelationalError> {
        let obj = value
            .as_object()
            .ok_or_else(|| RelationalError::QueryParse("query must be a JSON object".into()))?;

        let mut filters = Vec::new();
        let mut limit = None;
        let mut offset = None;

        for (key, val) in obj {
            match key.as_str() {
                "$limit" => {
                    limit = val.as_u64().map(|n| n as usize);
                }
                "$offset" => {
                    offset = val.as_u64().map(|n| n as usize);
                }
                field => {
                    if let Some(inner) = val.as_object() {
                        // Operator query: { "$gt": 10 }
                        for (op_key, op_val) in inner {
                            let op = match op_key.as_str() {
                                "$eq" => FilterOp::Eq,
                                "$ne" => FilterOp::Ne,
                                "$gt" => FilterOp::Gt,
                                "$gte" => FilterOp::Gte,
                                "$lt" => FilterOp::Lt,
                                "$lte" => FilterOp::Lte,
                                "$in" => FilterOp::In,
                                "$contains" => FilterOp::Contains,
                                "$exists" => FilterOp::Exists,
                                other => {
                                    return Err(RelationalError::QueryParse(format!(
                                        "unknown operator: {other}"
                                    )));
                                }
                            };
                            filters.push(Filter {
                                field: field.to_string(),
                                op,
                                value: op_val.clone(),
                            });
                        }
                    } else {
                        // Equality shorthand: { "field": "value" }
                        filters.push(Filter {
                            field: field.to_string(),
                            op: FilterOp::Eq,
                            value: val.clone(),
                        });
                    }
                }
            }
        }

        Ok(Self {
            filters,
            limit,
            offset,
        })
    }

    /// Check if a document matches all filters.
    pub fn matches(&self, doc: &serde_json::Value) -> bool {
        self.filters.iter().all(|f| f.matches(doc))
    }
}

impl Filter {
    /// Check if a document matches this filter.
    pub fn matches(&self, doc: &serde_json::Value) -> bool {
        let field_val = doc.get(&self.field);

        match self.op {
            FilterOp::Exists => {
                let should_exist = self.value.as_bool().unwrap_or(true);
                field_val.is_some() == should_exist
            }
            _ => {
                let field_val = match field_val {
                    Some(v) => v,
                    None => return false,
                };
                match self.op {
                    FilterOp::Eq => field_val == &self.value,
                    FilterOp::Ne => field_val != &self.value,
                    FilterOp::Gt => compare_json(field_val, &self.value) == Some(std::cmp::Ordering::Greater),
                    FilterOp::Gte => {
                        matches!(compare_json(field_val, &self.value), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))
                    }
                    FilterOp::Lt => compare_json(field_val, &self.value) == Some(std::cmp::Ordering::Less),
                    FilterOp::Lte => {
                        matches!(compare_json(field_val, &self.value), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))
                    }
                    FilterOp::In => {
                        if let Some(arr) = self.value.as_array() {
                            arr.contains(field_val)
                        } else {
                            false
                        }
                    }
                    FilterOp::Contains => {
                        if let (Some(haystack), Some(needle)) = (field_val.as_str(), self.value.as_str()) {
                            haystack.contains(needle)
                        } else {
                            false
                        }
                    }
                    FilterOp::Exists => unreachable!(),
                }
            }
        }
    }
}

/// Compare two JSON values numerically or lexicographically.
fn compare_json(
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Option<std::cmp::Ordering> {
    // Try numeric comparison first.
    if let (Some(a_f), Some(b_f)) = (as_f64(a), as_f64(b)) {
        return a_f.partial_cmp(&b_f);
    }
    // Fall back to string comparison.
    if let (Some(a_s), Some(b_s)) = (a.as_str(), b.as_str()) {
        return Some(a_s.cmp(b_s));
    }
    None
}

fn as_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_i64().map(|i| i as f64))
}
