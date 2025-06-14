//! JSON to Arrow conversion for streaming event processing
//!
//! This module provides efficient conversion from JSON events to Arrow RecordBatches
//! for columnar processing of Sigma rules.

use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::{SchemaRef, ArrowError};
use arrow_json::reader::{infer_json_schema_from_iterator, ReaderBuilder};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader as AsyncBufReader};

use crate::error::{Result, SigmaError};

/// Converts JSON events to Arrow RecordBatches
pub struct JsonToArrowConverter {
    /// Target batch size
    batch_size: usize,
    /// Schema for the RecordBatches
    schema: Option<SchemaRef>,
    /// Whether to infer schema from data
    infer_schema: bool,
}

impl JsonToArrowConverter {
    /// Create a new converter with default settings
    pub fn new() -> Self {
        Self {
            batch_size: 10_000,
            schema: None,
            infer_schema: true,
        }
    }

    /// Set the batch size
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Set a fixed schema
    pub fn with_schema(mut self, schema: SchemaRef) -> Self {
        self.schema = Some(schema);
        self.infer_schema = false;
        self
    }

    /// Convert a reader of JSON lines to Arrow RecordBatches
    pub fn convert_reader<R: Read>(
        &self,
        reader: R,
    ) -> Result<Vec<RecordBatch>> {
        let buf_reader = BufReader::new(reader);
        let lines: Vec<String> = buf_reader
            .lines()
            .filter_map(|line| line.ok())
            .filter(|line| !line.trim().is_empty())
            .collect();

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        // Get or infer schema
        let schema = if let Some(schema) = &self.schema {
            schema.clone()
        } else {
            self.infer_schema_from_lines(&lines)?
        };

        // Convert in batches
        let mut batches = Vec::new();
        for chunk in lines.chunks(self.batch_size) {
            let batch = self.lines_to_batch(chunk, &schema)?;
            batches.push(batch);
        }

        Ok(batches)
    }

    /// Convert JSON lines to a single RecordBatch
    fn lines_to_batch(&self, lines: &[String], schema: &SchemaRef) -> Result<RecordBatch> {
        // Create a concatenated JSON array string
        let json_array = format!("[{}]", lines.join(","));
        
        // Use arrow-json reader
        let reader = ReaderBuilder::new(schema.clone())
            .build_decoder()
            .map_err(|e| SigmaError::Arrow(format!("Failed to build decoder: {}", e)))?;

        let mut decoder = reader;
        decoder
            .decode(json_array.as_bytes())
            .map_err(|e| SigmaError::Arrow(format!("Failed to decode JSON: {}", e)))?;

        let batch = decoder
            .flush()
            .map_err(|e| SigmaError::Arrow(format!("Failed to flush decoder: {}", e)))?
            .ok_or_else(|| SigmaError::Arrow("No batch produced".to_string()))?;

        Ok(batch)
    }

    /// Infer schema from JSON lines
    fn infer_schema_from_lines(&self, lines: &[String]) -> Result<SchemaRef> {
        let values: Vec<Value> = lines
            .iter()
            .take(100) // Sample first 100 lines for schema inference
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        if values.is_empty() {
            return Err(SigmaError::Parse("No valid JSON found for schema inference".to_string()));
        }

        // Convert to Results for the iterator
        let value_results: Vec<std::result::Result<&Value, ArrowError>> = values.iter()
            .map(|v| Ok(v))
            .collect();
        
        let schema = infer_json_schema_from_iterator(value_results.into_iter())
            .map_err(|e| SigmaError::Arrow(format!("Schema inference failed: {}", e)))?;

        Ok(Arc::new(schema))
    }
}

/// Async streaming JSON to Arrow converter
pub struct StreamingJsonToArrow {
    converter: JsonToArrowConverter,
    buffer: Vec<String>,
}

impl StreamingJsonToArrow {
    /// Create a new streaming converter
    pub fn new(batch_size: usize) -> Self {
        Self {
            converter: JsonToArrowConverter::new().with_batch_size(batch_size),
            buffer: Vec::with_capacity(batch_size),
        }
    }

    /// Add a JSON line to the buffer
    pub fn add_line(&mut self, line: String) {
        if !line.trim().is_empty() {
            self.buffer.push(line);
        }
    }

    /// Check if buffer is ready to flush
    pub fn should_flush(&self) -> bool {
        self.buffer.len() >= self.converter.batch_size
    }

    /// Flush the buffer to a RecordBatch
    pub fn flush(&mut self) -> Result<Option<RecordBatch>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }

        // Infer schema from first batch if needed
        if self.converter.schema.is_none() && self.converter.infer_schema {
            let schema = self.converter.infer_schema_from_lines(&self.buffer)?;
            self.converter.schema = Some(schema);
        }

        let schema = self.converter.schema.as_ref()
            .ok_or_else(|| SigmaError::Arrow("No schema available".to_string()))?;

        let batch = self.converter.lines_to_batch(&self.buffer, schema)?;
        self.buffer.clear();

        Ok(Some(batch))
    }

    /// Process a stream of JSON lines
    pub async fn process_stream<R: AsyncBufRead + Unpin>(
        &mut self,
        reader: R,
    ) -> Result<Vec<RecordBatch>> {
        let mut reader = AsyncBufReader::new(reader);
        let mut line = String::new();
        let mut batches = Vec::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    self.add_line(line.clone());
                    
                    if self.should_flush() {
                        if let Some(batch) = self.flush()? {
                            batches.push(batch);
                        }
                    }
                }
                Err(e) => return Err(SigmaError::Io(e)),
            }
        }

        // Flush remaining
        if let Some(batch) = self.flush()? {
            batches.push(batch);
        }

        Ok(batches)
    }
}

impl Default for JsonToArrowConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_json_to_arrow_conversion() -> Result<()> {
        let json_data = r#"
{"EventID": 1, "CommandLine": "cmd.exe /c whoami", "ProcessId": 1234}
{"EventID": 4104, "CommandLine": "Get-Process", "ProcessId": 5678}
{"EventID": 1, "CommandLine": "powershell.exe -encoded", "ProcessId": 9012}
"#;

        let converter = JsonToArrowConverter::new().with_batch_size(2);
        let batches = converter.convert_reader(Cursor::new(json_data))?;

        assert_eq!(batches.len(), 2); // 3 lines with batch size 2
        assert_eq!(batches[0].num_rows(), 2);
        assert_eq!(batches[1].num_rows(), 1);

        // Check schema
        let schema = batches[0].schema();
        assert!(schema.field_with_name("EventID").is_ok());
        assert!(schema.field_with_name("CommandLine").is_ok());
        assert!(schema.field_with_name("ProcessId").is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_streaming_conversion() -> Result<()> {
        let json_data = r#"{"EventID": 1, "CommandLine": "test"}
{"EventID": 2, "CommandLine": "test2"}"#;

        let cursor = Cursor::new(json_data.as_bytes());
        let mut converter = StreamingJsonToArrow::new(2);
        
        let batches = converter.process_stream(cursor).await?;
        
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 2);

        Ok(())
    }
}