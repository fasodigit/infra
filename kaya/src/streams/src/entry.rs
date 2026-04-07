//! Stream entry type.

use bytes::Bytes;

use crate::StreamId;

/// A single entry in a stream: an ID plus a list of field-value pairs.
#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub id: StreamId,
    pub fields: Vec<(Bytes, Bytes)>,
}

impl StreamEntry {
    pub fn new(id: StreamId, fields: Vec<(Bytes, Bytes)>) -> Self {
        Self { id, fields }
    }

    /// Get a field value by name.
    pub fn get_field(&self, name: &[u8]) -> Option<&Bytes> {
        self.fields
            .iter()
            .find(|(k, _)| k.as_ref() == name)
            .map(|(_, v)| v)
    }

    /// Number of fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}
