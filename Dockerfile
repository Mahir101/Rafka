# Multi-stage build for Rafka
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build dependencies first (for better caching)
RUN cargo build --release --bin start_broker --bin start_producer --bin start_consumer

# Copy source code
COPY src/ ./src/

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false appuser

# Set working directory
WORKDIR /app

# Copy binaries from builder stage
COPY --from=builder /app/target/release/start_broker /app/
COPY --from=builder /app/target/release/start_producer /app/
COPY --from=builder /app/target/release/start_consumer /app/

# Copy config
COPY config/ ./config/

# Change ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose ports
EXPOSE 50051 9092

# Default command
CMD ["./start_broker"]