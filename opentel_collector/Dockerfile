FROM rust:1.89-slim-bullseye as builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

COPY src ./src

# Build the actual application
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -r -u 1000 -g users telemetry

WORKDIR /app
COPY --from=builder /app/target/release/opentel_collector /app/opentel_collector

# Create log directory and set permissions
RUN mkdir -p /var/log/app /tmp/telemetry && \
    chown -R telemetry:users /var/log/app /tmp/telemetry

USER telemetry

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9090/health || exit 1

CMD ["./opentel_collector"]
