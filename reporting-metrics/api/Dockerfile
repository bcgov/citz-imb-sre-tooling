# ---- Build Stage ----
FROM rust:1.87-bullseye as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates

# Copy just manifests to leverage cache
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Copy the full source code
COPY . .

# Final build using nightly
RUN cargo +nightly build --release

# ---- Runtime Stage ----
FROM debian:bookworm-slim

RUN adduser --disabled-password --gecos "" appuser

RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/reporting-metrics-api .
USER appuser

EXPOSE 8080
CMD ["./reporting-metrics-api"]
