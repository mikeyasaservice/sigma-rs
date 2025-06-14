//! Arrow-based event representation for columnar processing
//!
//! This module provides RecordBatch-based event processing for high-performance
//! columnar evaluation of Sigma rules using Apache Arrow.

use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use arrow_select::filter as arrow_filter;

use crate::error::{Result, SigmaError};

/// Arrow-based event batch for columnar processing
pub struct ArrowEventBatch {
    /// The underlying Arrow RecordBatch
    batch: RecordBatch,
    /// Cached schema for quick field lookups
    schema: SchemaRef,
}

impl ArrowEventBatch {
    /// Create a new ArrowEventBatch from a RecordBatch
    pub fn new(batch: RecordBatch) -> Self {
        let schema = batch.schema();
        Self { batch, schema }
    }

    /// Get the number of events in this batch
    pub fn num_events(&self) -> usize {
        self.batch.num_rows()
    }

    /// Get the schema of this batch
    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    /// Get the underlying RecordBatch
    pub fn record_batch(&self) -> &RecordBatch {
        &self.batch
    }

    /// Get a column by name
    pub fn column(&self, name: &str) -> Option<&ArrayRef> {
        self.schema
            .index_of(name)
            .ok()
            .and_then(|idx| self.batch.column(idx).into())
    }

    /// Get a string column by name
    pub fn string_column(&self, name: &str) -> Result<&StringArray> {
        self.column(name)
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' not found", name)))?
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' is not a string", name)))
    }

    /// Get an integer column by name
    pub fn int_column(&self, name: &str) -> Result<&Int64Array> {
        self.column(name)
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' not found", name)))?
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' is not an integer", name)))
    }

    /// Get a float column by name
    pub fn float_column(&self, name: &str) -> Result<&Float64Array> {
        self.column(name)
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' not found", name)))?
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' is not a float", name)))
    }

    /// Get a boolean column by name
    pub fn bool_column(&self, name: &str) -> Result<&BooleanArray> {
        self.column(name)
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' not found", name)))?
            .as_any()
            .downcast_ref::<BooleanArray>()
            .ok_or_else(|| SigmaError::Field(format!("Column '{}' is not a boolean", name)))
    }

    /// Filter this batch by a boolean mask
    pub fn filter(&self, mask: &BooleanArray) -> Result<RecordBatch> {
        arrow_filter::filter_record_batch(&self.batch, mask)
            .map_err(|e| SigmaError::Arrow(e.to_string()))
    }

    /// Create a match result batch with rule information
    pub fn create_match_batch(
        &self,
        mask: &BooleanArray,
        rule_id: &str,
        rule_title: &str,
    ) -> Result<RecordBatch> {
        // Filter the original batch
        let filtered = self.filter(mask)?;
        
        if filtered.num_rows() == 0 {
            return Ok(filtered);
        }

        // Add rule metadata columns
        let num_rows = filtered.num_rows();
        let rule_id_vec: Vec<&str> = vec![rule_id; num_rows];
        let rule_id_array = Arc::new(StringArray::from(rule_id_vec)) as ArrayRef;
        let rule_title_vec: Vec<&str> = vec![rule_title; num_rows];
        let rule_title_array = Arc::new(StringArray::from(rule_title_vec)) as ArrayRef;

        // Build new schema with rule columns
        let mut fields = filtered.schema().fields().to_vec();
        fields.push(Arc::new(Field::new("rule_id", DataType::Utf8, false)));
        fields.push(Arc::new(Field::new("rule_title", DataType::Utf8, false)));
        let schema = Arc::new(Schema::new(fields));

        // Build new record batch
        let mut columns = filtered.columns().to_vec();
        columns.push(rule_id_array);
        columns.push(rule_title_array);

        RecordBatch::try_new(schema, columns)
            .map_err(|e| SigmaError::Arrow(e.to_string()))
    }
}

/// Builder for creating Arrow schemas from Sigma rules
pub struct ArrowSchemaBuilder {
    fields: Vec<Arc<Field>>,
}

impl ArrowSchemaBuilder {
    /// Create a new schema builder
    pub fn new() -> Self {
        Self {
            fields: vec![
                // Common fields that are always present
                Arc::new(Field::new("timestamp", DataType::Utf8, true)),
                Arc::new(Field::new("hostname", DataType::Utf8, true)),
            ],
        }
    }

    /// Add a field to the schema
    pub fn add_field(&mut self, name: &str, data_type: DataType) -> &mut Self {
        // Check if field already exists
        if !self.fields.iter().any(|f| f.name() == name) {
            self.fields.push(Arc::new(Field::new(name, data_type, true)));
        }
        self
    }

    /// Add a string field
    pub fn add_string_field(&mut self, name: &str) -> &mut Self {
        self.add_field(name, DataType::Utf8)
    }

    /// Add an integer field
    pub fn add_int_field(&mut self, name: &str) -> &mut Self {
        self.add_field(name, DataType::Int64)
    }

    /// Add a float field
    pub fn add_float_field(&mut self, name: &str) -> &mut Self {
        self.add_field(name, DataType::Float64)
    }

    /// Add a boolean field
    pub fn add_bool_field(&mut self, name: &str) -> &mut Self {
        self.add_field(name, DataType::Boolean)
    }

    /// Build the schema
    pub fn build(self) -> SchemaRef {
        Arc::new(Schema::new(self.fields))
    }
}

impl Default for ArrowSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::Int64Array;

    #[test]
    fn test_arrow_event_batch() -> Result<()> {
        // Create a simple record batch
        let schema = Arc::new(Schema::new(vec![
            Arc::new(Field::new("EventID", DataType::Int64, false)),
            Arc::new(Field::new("CommandLine", DataType::Utf8, true)),
        ]));

        let event_ids = Arc::new(Int64Array::from(vec![1, 4104, 1])) as ArrayRef;
        let command_lines = Arc::new(StringArray::from(vec![
            "cmd.exe /c whoami",
            "Get-Process",
            "powershell.exe -encoded",
        ])) as ArrayRef;

        let batch = RecordBatch::try_new(schema, vec![event_ids, command_lines])?;
        let arrow_batch = ArrowEventBatch::new(batch);

        assert_eq!(arrow_batch.num_events(), 3);

        // Test column access
        let event_id_col = arrow_batch.int_column("EventID")?;
        assert_eq!(event_id_col.value(0), 1);
        assert_eq!(event_id_col.value(1), 4104);

        let cmd_col = arrow_batch.string_column("CommandLine")?;
        assert_eq!(cmd_col.value(0), "cmd.exe /c whoami");

        Ok(())
    }

    #[test]
    fn test_schema_builder() {
        let schema = ArrowSchemaBuilder::new()
            .add_string_field("EventID")
            .add_string_field("CommandLine")
            .add_int_field("ProcessId")
            .add_float_field("CPU")
            .add_bool_field("Elevated")
            .build();

        assert_eq!(schema.fields().len(), 7); // 2 default + 5 added
        assert_eq!(schema.field(0).name(), "timestamp");
        assert_eq!(schema.field(1).name(), "hostname");
    }
}