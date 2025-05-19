# Sigma-rs Examples

This directory contains example applications demonstrating how to use the Sigma-rs rule engine.

## Examples Overview

### 1. Simple Detection
Basic example showing how to create events and apply Sigma rules.

```bash
cargo run --example simple_detection
```

### 2. Rule Validator
Validates Sigma rule files and reports parsing statistics. This is useful for checking rule compatibility with the Rust implementation.

```bash
# Validate rules in a single directory
cargo run --example rule_validator -- --rule-dirs /path/to/sigma/rules

# Validate rules from multiple directories
cargo run --example rule_validator -- --rule-dirs ./rules/windows;./rules/linux

# Increase verbosity to see detailed errors
cargo run --example rule_validator -- --rule-dirs ./rules -vv

# Output results as JSON
cargo run --example rule_validator -- --rule-dirs ./rules --json

# Use parallel processing (default)
cargo run --example rule_validator -- --rule-dirs ./rules --parallel --threads 8
```

### 3. Event Detector
Applies Sigma rules to events stored in a JSON file and outputs detection results.

```bash
# Process events from a file
cargo run --example event_detector -- --rule-dirs ./rules --events events.json

# Output results to a file
cargo run --example event_detector -- --rule-dirs ./rules -i events.json -o results.json

# Pretty print output
cargo run --example event_detector -- --rule-dirs ./rules -i events.json --pretty

# Send output to stdout
cargo run --example event_detector -- --rule-dirs ./rules -i events.json --stdout
```

### 4. Stream Detector
Processes streaming JSON events from stdin and outputs matches in real-time.

```bash
# Process events from a file
cat events.ndjson | cargo run --example stream_detector -- --rule-dirs ./rules

# Tail log files
tail -f /var/log/security.json | cargo run --example stream_detector -- --rule-dirs ./rules

# Output as CSV
cat events.json | cargo run --example stream_detector -- --rule-dirs ./rules --format csv

# Enable metrics reporting
cat events.json | cargo run --example stream_detector -- --rule-dirs ./rules --metrics-interval 5
```

### 5. Parallel Stream Detector
High-performance multi-threaded event processing for maximum throughput.

```bash
# Process with 8 workers
cat large_dataset.ndjson | cargo run --example parallel_stream_detector -- --rule-dirs ./rules --workers 8

# Adjust batch size for better performance
cat events.json | cargo run --example parallel_stream_detector -- --rule-dirs ./rules --batch-size 50

# Enable adaptive scaling
cat events.json | cargo run --example parallel_stream_detector -- --rule-dirs ./rules --adaptive-scaling

# Set backpressure limits
cat events.json | cargo run --example parallel_stream_detector -- --rule-dirs ./rules --max-pending-writes 200
```

## Event Format

Events should be in JSON format. For batch processing (event_detector), events can be either:
- JSON array: `[{"field": "value"}, {"field": "value"}]`
- Newline-delimited JSON: One JSON object per line

For streaming (stream_detector, parallel_stream_detector), use newline-delimited JSON.

Example event:
```json
{
    "CommandLine": "powershell.exe -ExecutionPolicy Bypass -C \"wmic.exe group get name\"",
    "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    "ProcessId": "1264",
    "User": "DESKTOP-IDQQB81\\victim1",
    "UtcTime": "2023-03-03 01:38:13.179"
}
```

## Performance Tips

1. **Rule Validator**: Use `--parallel` with appropriate `--threads` for large rule sets
2. **Event Detector**: Enable progress reporting with `--progress` for large files
3. **Stream Detector**: Adjust `--buffer-size` based on event rate
4. **Parallel Stream Detector**: 
   - Set `--workers` to match CPU cores
   - Tune `--batch-size` for your event size
   - Use `--max-pending-writes` to control memory usage

## Example Workflow

1. Validate your Sigma rules:
   ```bash
   cargo run --example rule_validator -- --rule-dirs ./sigma-rules -v
   ```

2. Test detection on sample events:
   ```bash
   cargo run --example event_detector -- --rule-dirs ./sigma-rules -i test_events.json -o detections.json
   ```

3. Deploy for real-time detection:
   ```bash
   tail -f /var/log/events.json | cargo run --example parallel_stream_detector -- --rule-dirs ./sigma-rules --workers 4
   ```

## Common Issues

1. **Invalid YAML in rules**: Use rule_validator to identify problematic rules
2. **Memory usage**: Adjust batch sizes and queue depths
3. **Performance**: Use parallel_stream_detector for high-volume streams
4. **Backpressure**: Monitor queue depth metrics and adjust `--max-pending-writes`