# Dockerfile for rJMX-Exporter
# Multi-stage build for minimal production image
#
# Build: docker build -t rjmx-exporter .
# Run:   docker run -v ./config.yaml:/config.yaml rjmx-exporter

# =============================================================================
# Stage 1: Build
# =============================================================================
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

# Create app directory
WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies
RUN mkdir -p src && \
    echo 'fn main() { println!("Dummy"); }' > src/main.rs && \
    echo '' > src/lib.rs

# Build dependencies (this layer is cached)
RUN cargo build --release --target x86_64-unknown-linux-musl && \
    rm -rf src target/x86_64-unknown-linux-musl/release/deps/rjmx*

# Copy actual source code
COPY src ./src

# Build the actual binary
RUN cargo build --release --target x86_64-unknown-linux-musl && \
    strip target/x86_64-unknown-linux-musl/release/rjmx-exporter

# =============================================================================
# Stage 2: Runtime
# =============================================================================
FROM alpine:3.19

# Labels
LABEL org.opencontainers.image.title="rJMX-Exporter"
LABEL org.opencontainers.image.description="High-performance JMX metrics exporter written in Rust"
LABEL org.opencontainers.image.source="https://github.com/jsoonworld/rJMX-Exporter"
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

# Install runtime dependencies (CA certificates for HTTPS)
RUN apk add --no-cache ca-certificates tzdata && \
    adduser -D -H -s /sbin/nologin rjmx

# Copy binary from builder
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rjmx-exporter /usr/local/bin/

# Use non-root user
USER rjmx

# Expose metrics port
EXPOSE 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:9090/health || exit 1

# Default entrypoint
ENTRYPOINT ["/usr/local/bin/rjmx-exporter"]
CMD ["-c", "/config.yaml"]
