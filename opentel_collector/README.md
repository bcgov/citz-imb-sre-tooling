# OpenTelemetry Sidecar Collector

An OpenTelemetry-compatible sidecar collector written in Rust for Kubernetes environments

## Architecture Overview

The sidecar collector is designed with a modular architecture for maintainability and extensibility:

```
src/
├── lib.rs              # Library exports and main API
├── main.rs             # Binary entry point
├── config.rs           # Configuration management
├── errors.rs           # Error handling and types
├── telemetry.rs        # Telemetry data structures
├── log_parser.rs       # Log parsing (JSON, regex, composite)
├── buffer.rs           # In-memory buffering with priority support
├── transport.rs        # HTTP transport with retry logic
└── collector.rs        # Main orchestration logic
```

### Deploying as a Sidecar

Simply add as an additional container within a deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: your-app
spec:
  template:
    spec:
      containers:
      # Your existing application container
      - name: app
        image: your-app:latest
        volumeMounts:
        - name: app-logs
          mountPath: /var/log/app

      # Telemetry sidecar
      - name: telemetry-sidecar
        image: <registry>/opentel_collector:latest
        env:
        - name: SERVICE_NAME
          value: "service"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        - name: GATEWAY_URL
          value: "http://telemetry-gateway:8080"
        - name: LOG_PATHS
          value: "/var/log/app/application.log,/var/log/app/error.log"
        volumeMounts:
        - name: app-logs
          mountPath: /var/log/app
          readOnly: true
        resources:
          requests:
            memory: "64Mi"
            cpu: "25m"
          limits:
            memory: "128Mi"
            cpu: "100m"

      volumes:
      - name: app-logs
        emptyDir: {}
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVICE_NAME` | Name of service | `unknown-service` |
| `POD_NAME` | Kubernetes pod name | `unknown-pod` |
| `NAMESPACE` | Kubernetes namespace | `default` |
| `GATEWAY_URL` | Telemetry gateway URL | `http://telemetry-gateway:8080` |
| `LOG_PATHS` | Comma-separated log file paths | `/var/log/app/application.log` |
| `BATCH_SIZE` | Number of entries per batch | `100` |
| `FLUSH_INTERVAL_SECONDS` | Forced flush interval | `30` |
| `MAX_RETRIES` | Maximum retry attempts | `3` |
| `RETRY_BACKOFF_MS` | Initial retry backoff | `1000` |
| `MAX_BUFFER_SIZE` | Maximum buffer entries | `10000` |
| `HTTP_TIMEOUT_SECONDS` | HTTP request timeout | `10` |
| `PARSE_STRUCTURED_LOGS` | Enable JSON parsing | `true` |
| `ENABLE_TRACE_CORRELATION` | Enable trace correlation | `true` |
| `RUST_LOG` | Log level | `info` |

### Log Format Support

#### JSON Logs
```json
{
  "timestamp": 1701234567,
  "level": "ERROR",
  "message": "Database connection failed",
  "trace_id": "abc123def456",
  "span_id": "def456abc123",
  "attributes": {
    "user_id": "12345",
    "request_id": "req-789"
  }
}
```

#### Structured Text Logs
```
[2023-12-01T10:30:45Z] ERROR: Database connection failed
2023/12/01 10:30:45 [error] Connection timeout
2023-12-01 10:30:45.123 ERROR [trace-id,span-id] --- Request failed
ERROR: Something went wrong
```

## Performance Tuning

### Memory Usage
- **Buffer size**: Tune `MAX_BUFFER_SIZE` based on log volume
- **Batch size**: Larger batches = better throughput, higher latency
- **Container limits**: Set appropriate memory limits (64-128Mi typical)

### Network Efficiency
- **Batch size**: Balance between latency and network efficiency
- **Flush interval**: Shorter intervals = lower latency, more requests
- **Retry settings**: Tune for your network reliability

### CPU Optimization
- **Log parsing**: JSON parsing is faster than regex
- **Buffer management**: Priority buffers help with CPU-intensive workloads
- **File monitoring**: 500ms check interval balances responsiveness vs CPU

## Monitoring and Observability

### Health Checks
- **Startup**: Gateway connectivity test
- **Runtime**: Continuous error monitoring with backoff
- **Shutdown**: Graceful cleanup with buffer flushing

### Troubleshooting

#### Log Analysis
```bash
# Enable debug logging
kubectl set env deployment/app RUST_LOG=debug -c telemetry-sidecar

# Watch real-time logs
kubectl logs -f deployment/app -c telemetry-sidecar
```

## Advanced Configuration

### Custom Log Patterns
Extend the regex parser with custom patterns by modifying `log_parser.rs`:

```rust
// Add custom pattern to RegexLogParser::default_patterns()
LogPattern {
    regex: Regex::new(r"^CUSTOM (\d{4}-\d{2}-\d{2}) (\w+): (.+)$").unwrap(),
    level_group: 2,
    message_group: 3,
    timestamp_group: Some(1),
    trace_id_group: None,
    span_id_group: None,
},
```

### Priority Processing
High-priority logs (errors, critical events) are processed first:

```rust
// Automatic priority detection for errors
pub fn is_high_priority_log(log_entry: &LogEntry) -> bool {
    matches!(log_entry.level, LogLevel::Error | LogLevel::Fatal)
        || log_entry.message.to_lowercase().contains("critical")
}
```

### Buffer Tuning
Optimize buffer behavior for workload:

```rust
// Custom buffer configuration
let buffer_config = BufferConfig {
    max_size: 20000,           // Larger buffer for high-volume apps
    batch_size: 250,           // Bigger batches for better throughput
    flush_threshold: 80.0,     // Flush at 80% capacity
};
```

## Development

### Running Tests
```bash
cargo test
cargo test --release  # Test optimized builds
```

### Local Development
```bash
# Run with environment variables
export SERVICE_NAME="test-service"
export GATEWAY_URL="http://localhost:8080"
export LOG_PATHS="/tmp/test.log"
export RUST_LOG="debug"

cargo run
```

### Contributing
1. Follow Rust formatting: `cargo fmt`
2. Check with Clippy: `cargo clippy`
3. Run tests: `cargo test`
4. Update documentation for new features

## Security Considerations

### Container Security
- **Non-root execution**: Runs as UID 1000
- **Minimal attack surface**: Only necessary dependencies
- **Read-only log access**: Logs mounted read-only
- **Resource limits**: CPU and memory constraints

### Network Security
- **TLS support**: Uses rustls for secure connections
- **Certificate validation**: Validates gateway certificates
- **Network policies**: Restrict egress to gateway only

### Data Privacy
- **No log storage**: Logs buffered in memory only
- **Configurable retention**: Buffer limits prevent unbounded growth
- **Secure transmission**: HTTPS transport to gateway
