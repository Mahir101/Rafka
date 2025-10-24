# Use the official Rust image with Alpine (use rust:alpine or a specific version like rust:1.82-alpine)
FROM rust:alpine as builder

# Install necessary dependencies for building the Rust project
RUN apk add --no-cache musl-dev

# Set the working directory inside the container
WORKDIR /usr/src/rafka

# Copy the source code into the container
COPY . .

# Build the application
RUN cargo build --release --target x86_64-unknown-linux-musl


# Create a smaller final image based on Alpine
FROM alpine:latest

# Install necessary runtime dependencies (using libssl3 instead of libssl1.1)
RUN apk add --no-cache \
    libssl3 \
    ca-certificates


# Set the working directory
WORKDIR /usr/local/bin

# Copy the built executable from the builder image
COPY --from=builder /usr/src/rafka/target/x86_64-unknown-linux-musl/release/rafka /usr/local/bin/rafka

EXPOSE 8080

# Run the application
CMD ["/usr/local/bin/rafka"]
