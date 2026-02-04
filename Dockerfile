# =============================================================================
# Build Stage
# =============================================================================
FROM rust:1.93-trixie as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    perl \
    make \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build release binary with reduced optimization for dependencies to avoid OOM
# Note: pq-sys and openssl-sys are bundled, so no system deps needed
# Disable incremental compilation to reduce memory usage
ENV CARGO_INCREMENTAL=0
ENV CARGO_BUILD_JOBS=1

# Create a custom Cargo config to reduce optimization for dependencies
RUN mkdir -p .cargo && \
    echo '[profile.release]' > .cargo/config.toml && \
    echo 'opt-level = 3' >> .cargo/config.toml && \
    echo '[profile.release.package."*"]' >> .cargo/config.toml && \
    echo 'opt-level = 1' >> .cargo/config.toml

RUN cargo build --release -j 1

# =============================================================================
# Runtime Stage
# =============================================================================
FROM rust:1.93-trixie

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 crashcache

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/crash-cache /usr/local/bin/crash-cache

# Copy migrations (needed for runtime)
COPY --from=builder /app/migrations /app/migrations

# Set ownership
RUN chown -R crashcache:crashcache /app

# Switch to non-root user
USER crashcache

# Expose port
EXPOSE 3000

# Run the application
CMD ["/usr/local/bin/crash-cache", "serve"]
