# citz-imb-sre-reporting

A high-performance Rust-based API service for collecting and reporting SRE (Site Reliability Engineering) metrics. Designed for reliability, observability, and scalability, this service integrates with modern monitoring and tracing tools.

## 🚀 Features

- Built with [Actix Web](https://actix.rs/)

## 🧱 Project Structure

imb-sre-reporting-metrics/
└── citz-imb-sre-reporting/
├── src/
├── Cargo.toml
└── ..

## 🛠️ Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)

### Installation

```bash
cd imb-sre-reporting-metrics/citz-imb-sre-reporting
cargo build
```

### Running Locally
```bash
cargo run
```

## 🐳 Running the API with Docker

To run the `citz-imb-sre-reporting` API using the provided `Dockerfile`, follow these steps:

### 1. **Build the Docker Image**
From the `imb-sre-reporting-metrics/citz-imb-sre-reporting/` directory:

```bash
docker build -t citz-imb-sre-reporting .
```

### 2. ** Run the Container**
```bash
docker run -p 8080:8080 citz-imb-sre-reporting
```

This maps port 8080 from your local machine to the container. Make sure the server in your Rust code binds to 0.0.0.0 (not 127.0.0.1) so that it's accessible from outside the container.

### 3. **Test the API**

You can test the health check endpoint:

```bash
curl http://0.0.0.0:8080/health
```

<<<<<<< HEAD
Or open your browser to [here](http://0.0.0.0:8080/health)
=======
You can test the greet endpoint with the following:

```bash
curl -X POST http://0.0.0.0:8080/greet -H 'Content-type: application/json' -d '{"name":"me"}'   
```

Or open your browser to (here)[http://0.0.0.0:8080/health]
>>>>>>> 59e566f (adding documentation for greet endpoint)
