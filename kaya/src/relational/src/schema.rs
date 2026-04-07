//! Schema definitions for typed collections.

use serde::{Deserialize, Serialize};

/// Supported field types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Json,
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Integer => write!(f, "integer"),
            Self::Float => write!(f, "float"),
            Self::Boolean => write!(f, "boolean"),
            Self::Timestamp => write!(f, "timestamp"),
            Self::Json => write!(f, "json"),
        }
    }
}

/// A field definition within a schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub indexed: bool,
}

/// A collection schema: ordered list of field definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub fields: Vec<FieldDef>,
}

impl Schema {
    pub fn new(fields: Vec<FieldDef>) -> Self {
        Self { fields }
    }

    /// Validate a JSON document against this schema.
    pub fn validate(&self, doc: &serde_json::Value) -> Result<(), crate::RelationalError> {
        let obj = doc
            .as_object()
            .ok_or_else(|| crate::RelationalError::SchemaValidation("document must be an object".into()))?;

        for field in &self.fields {
            match obj.get(&field.name) {
                None if field.required => {
                    return Err(crate::RelationalError::SchemaValidation(format!(
                        "missing required field: {}",
                        field.name
                    )));
                }
                Some(val) => {
                    self.validate_type(&field.name, &field.field_type, val)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn validate_type(
        &self,
        field_name: &str,
        expected: &FieldType,
        value: &serde_json::Value,
    ) -> Result<(), crate::RelationalError> {
        let ok = match expected {
            FieldType::String => value.is_string(),
            FieldType::Integer => value.is_i64() || value.is_u64(),
            FieldType::Float => value.is_f64() || value.is_i64(),
            FieldType::Boolean => value.is_boolean(),
            FieldType::Timestamp => value.is_string(), // ISO-8601 string
            FieldType::Json => true, // any JSON value is valid
        };

        if !ok {
            return Err(crate::RelationalError::TypeMismatch {
                field: field_name.into(),
                expected: expected.to_string(),
                actual: format!("{}", value),
            });
        }

        Ok(())
    }

    /// Get field def by name.
    pub fn get_field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Names of indexed fields.
    pub fn indexed_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.indexed)
            .map(|f| f.name.as_str())
            .collect()
    }
}
