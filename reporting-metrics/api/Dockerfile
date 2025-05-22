# ---- Build Stage ----
FROM rustlang/rust:nightly-slim as builder

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

RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/citz-imb-sre-reporting .
USER appuser

EXPOSE 8080
CMD ["./citz-imb-sre-reporting"]