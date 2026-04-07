//! KAYA Views: Materialized views with incremental maintenance.
//!
//! VIEW.CREATE / VIEW.REFRESH / VIEW.QUERY commands.

pub mod definition;
pub mod engine;

use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;
use thiserror::Error;

pub use definition::{ViewDefinition, JoinKind, Aggregation};
pub use engine::ViewEngine;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ViewError {
    #[error("view not found: {0}")]
    ViewNotFound(String),

    #[error("view already exists: {0}")]
    ViewExists(String),

    #[error("invalid view definition: {0}")]
    InvalidDefinition(String),

    #[error("source collection not found: {0}")]
    SourceNotFound(String),

    #[error("view error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Materialized View
// ---------------------------------------------------------------------------

/// A materialized view: cached query results that update incrementally.
pub struct MaterializedView {
    pub name: String,
    pub definition: ViewDefinition,
    /// Cached results.
    data: RwLock<Vec<serde_json::Value>>,
    /// Whether the view needs a full refresh.
    stale: RwLock<bool>,
}

impl MaterializedView {
    pub fn new(name: String, definition: ViewDefinition) -> Self {
        Self {
            name,
            definition,
            data: RwLock::new(Vec::new()),
            stale: RwLock::new(true),
        }
    }

    /// Get the cached data.
    pub fn data(&self) -> Vec<serde_json::Value> {
        self.data.read().clone()
    }

    /// Replace the cached data (full refresh).
    pub fn set_data(&self, data: Vec<serde_json::Value>) {
        *self.data.write() = data;
        *self.stale.write() = false;
    }

    /// Mark as needing refresh.
    pub fn invalidate(&self) {
        *self.stale.write() = true;
    }

    /// Check if stale.
    pub fn is_stale(&self) -> bool {
        *self.stale.read()
    }

    /// Number of rows in the view.
    pub fn len(&self) -> usize {
        self.data.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.read().is_empty()
    }
}

// ---------------------------------------------------------------------------
// View Manager
// ---------------------------------------------------------------------------

/// Manages all materialized views.
pub struct ViewManager {
    views: DashMap<String, Arc<MaterializedView>>,
}

impl ViewManager {
    pub fn new() -> Self {
        Self {
            views: DashMap::new(),
        }
    }

    /// VIEW.CREATE: register a new materialized view.
    pub fn create(
        &self,
        name: &str,
        definition: ViewDefinition,
    ) -> Result<(), ViewError> {
        if self.views.contains_key(name) {
            return Err(ViewError::ViewExists(name.into()));
        }
        let view = Arc::new(MaterializedView::new(name.to_string(), definition));
        self.views.insert(name.to_string(), view);
        Ok(())
    }

    /// VIEW.DROP: remove a view.
    pub fn drop_view(&self, name: &str) -> Result<(), ViewError> {
        self.views
            .remove(name)
            .ok_or_else(|| ViewError::ViewNotFound(name.into()))?;
        Ok(())
    }

    /// Get a view by name.
    pub fn get(&self, name: &str) -> Result<Arc<MaterializedView>, ViewError> {
        self.views
            .get(name)
            .map(|v| v.value().clone())
            .ok_or_else(|| ViewError::ViewNotFound(name.into()))
    }

    /// List all view names.
    pub fn list(&self) -> Vec<String> {
        self.views.iter().map(|e| e.key().clone()).collect()
    }

    /// Invalidate views that depend on a given collection.
    pub fn invalidate_for_collection(&self, collection_name: &str) {
        for entry in self.views.iter() {
            let view = entry.value();
            if view.definition.sources.contains(&collection_name.to_string()) {
                view.invalidate();
            }
        }
    }
}

impl Default for ViewManager {
    fn default() -> Self {
        Self::new()
    }
}
