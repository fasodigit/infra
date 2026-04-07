//! View engine: computes and refreshes materialized views.


use crate::{MaterializedView, ViewError};
use kaya_relational::CollectionManager;

/// Engine that refreshes materialized views from source collections.
pub struct ViewEngine;

impl ViewEngine {
    /// Refresh a materialized view by re-computing from sources.
    pub fn refresh(
        view: &MaterializedView,
        collections: &CollectionManager,
    ) -> Result<(), ViewError> {
        // For each source, query all documents.
        let mut all_docs = Vec::new();

        for source in &view.definition.sources {
            let query_val = view
                .definition
                .filter
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            let query = kaya_relational::KqlQuery::parse(&query_val)
                .map_err(|e| ViewError::Internal(e.to_string()))?;
            let docs = collections
                .find(source, &query)
                .map_err(|e| ViewError::SourceNotFound(e.to_string()))?;
            for doc in docs {
                all_docs.push(doc.data);
            }
        }

        // Apply projection if specified.
        if !view.definition.projection.is_empty() {
            all_docs = all_docs
                .into_iter()
                .map(|doc| {
                    if let serde_json::Value::Object(map) = &doc {
                        let filtered: serde_json::Map<String, serde_json::Value> = map
                            .iter()
                            .filter(|(k, _)| view.definition.projection.contains(k))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        serde_json::Value::Object(filtered)
                    } else {
                        doc
                    }
                })
                .collect();
        }

        // Apply limit.
        if let Some(limit) = view.definition.limit {
            all_docs.truncate(limit);
        }

        view.set_data(all_docs);
        Ok(())
    }
}
