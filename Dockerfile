# Multi-stage Dockerfile for ACPMS

# Stage 1: Build dependencies
FROM rust:alpine AS builder
WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static postgresql-dev curl unzip

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary
RUN cargo build --release --bin acpms-server

# Stage 2: Development
FROM rust:alpine AS development
WORKDIR /app

# Install development dependencies (including git for worktree management, nodejs/npm for Claude Code CLI)
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static postgresql-dev curl unzip git nodejs npm

# This stage is used with volume mounts for hot-reload

# Stage 3: Production runtime
FROM alpine:3.19 AS runtime
WORKDIR /app

# Install runtime dependencies (openssh-client for ssh-keyscan UI only; SSH exec uses russh)
RUN apk add --no-cache libgcc openssl postgresql-libs openssh-client

# Copy binary from builder
COPY --from=builder /app/target/release/acpms-server /usr/local/bin/acpms-server

# Create non-root user
RUN addgroup -g 1000 acpms && \
    adduser -D -u 1000 -G acpms acpms

USER acpms

EXPOSE 3000

CMD ["acpms-server"]
