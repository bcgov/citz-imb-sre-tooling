# Technical Implementation and Choices in the Reporting Metrics API

## Architecture Overview

This project implements a service monitoring API in Rust, designed to track the health and performance of various web services. It follows a modular architecture with clear separation of concerns.

## Technology Stack and Library Choices

1. **Rust as the Programming Language**
   - Provides memory safety without garbage collection
   - Strong type system helps prevent bugs at compile time
   - High performance comparable to C/C++
   - Growing ecosystem for web services

2. **Actix-Web Framework (v4)**
   - High-performance, concurrent web framework
   - Asynchronous design suitable for service health monitoring
   - Provides routing, middleware, and request handling capabilities
   - Well-documented and actively maintained

3. **Tokio Runtime**
   - Asynchronous runtime enabling non-blocking I/O operations
   - Facilitates concurrent service health checks
   - Used with "full" features for comprehensive async capabilities

4. **Reqwest HTTP Client**
   - Provides an ergonomic HTTP client for making outbound requests
   - Used for checking service health by sending HTTP requests
   - Configured with timeout to prevent long-hanging requests

5. **Serde for Serialization/Deserialization**
   - Handles JSON conversion for API requests and responses
   - Type-safe approach to data handling

6. **Synchronization Primitives**
   - `Mutex` for thread-safe access to shared state
   - Ensures data consistency in a concurrent environment

7. **Environment Management**
   - `dotenv` for configuration via environment variables
   - Flexible deployment across different environments

8. **Logging via env_logger**
   - Structured logging for operational visibility
   - Configurable log levels

## Architecture Decisions

1. **Shared State Pattern**
   - `AppState` struct contains shared application data
   - Thread-safe access via `Mutex`
   - Includes:
     - `metrics_cache`: Cache of service metrics
     - `services`: List of services to monitor
     - `http_client`: Shared HTTP client instance

2. **Background Processing**
   - Separate background task for metrics collection
   - Non-blocking architecture using Tokio's spawning capabilities
   - Allows API to remain responsive while monitoring occurs

3. **RESTful API Design**
   - Clear endpoint structure:
     - `/services` for service management
     - `/metrics` for metrics retrieval
     - `/health` for API health checks
   - Proper HTTP methods (GET, POST, DELETE)

4. **Data Models**
   - Clearly defined types with Serde integration
   - `ServiceConfig` for service registration
   - `ServiceMetrics` for health and performance data

5. **Controller-Service Pattern**
   - Controllers handle HTTP interactions
   - Services contain business logic
   - Clear separation of concerns

## Performance Considerations

1. **HTTP Client Configuration**
   - 10-second timeout to prevent resource exhaustion
   - Single shared client to minimize connection overhead

2. **In-Memory Data Storage**
   - Uses HashMap for O(1) access to metrics by service name
   - Avoids database dependencies for simplicity

3. **Mutex Locking Strategy**
   - Fine-grained locks to minimize contention
   - Locks released quickly to improve concurrency

## Scalability Aspects

1. **Stateful Design**
   - Current implementation uses in-memory state
   - Future enhancement could include persistent storage

2. **Asynchronous Processing**
   - Non-blocking I/O for maximum throughput
   - Efficient handling of concurrent requests

## Error Handling and Resilience

1. **Defensive Programming**
   - Service existence checks before operations
   - Proper error responses with meaningful messages

2. **Graceful Error Handling**
   - Returns appropriate HTTP status codes
   - Includes informative error messages

## GitHub Metrics Integration

The API has been extended to collect and expose metrics from GitHub repositories associated with monitored services:

1. **DevOps Metrics Collection**
   - Added support for tracking key DevOps performance indicators
   - GitHub repository references are stored with service definitions
   - Background collector queries the GitHub API at regular intervals

2. **Captured GitHub Metrics**
   - **Deployment Frequency**: Tracks how often code changes are deployed
     - Count of deployments in the last 30 days
     - Average weekly deployment rate
     - Timestamp of most recent deployment
   - **Lead Time for Changes**: Measures time from code commit to production
     - Average, median, and 90th percentile statistics
   - **Workflow Metrics**: CI/CD performance data
     - Status and conclusion of recent workflow runs
     - Workflow run durations
     - Success/failure ratios
   - **Deployment Tags**: Tracks version information
     - Latest deployed image tags by environment
     - Deployment timestamps

3. **API Endpoints**
   - `/github-metrics`: Retrieve GitHub metrics for all services
   - `/github-metrics/{name}`: Retrieve GitHub metrics for a specific service

## Future Extension Points

The modular architecture allows for:
1. Adding persistent storage backends
2. Implementing authentication/authorization
3. Adding more sophisticated metrics collection
4. Scaling to handle more services

This implementation balances simplicity with robustness, providing a solid foundation for service monitoring with room to grow as requirements evolve.
