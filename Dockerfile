FROM rust:1.87-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    sqlite-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

ENV RUSTFLAGS="-C target-feature=-crt-static"
ENV PKG_CONFIG_ALL_STATIC=1
ENV PKG_CONFIG_ALL_DYNAMIC=0

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx

RUN cargo install sqlx-cli --no-default-features --features sqlite

ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM alpine:3.19

RUN apk add --no-cache \
    ca-certificates \
    sqlite \
    libgcc \
    wget

# Create app user
RUN addgroup -g 1000 appuser && \
    adduser -D -s /bin/sh -u 1000 -G appuser appuser

# Create app directory
WORKDIR /app

# Create data directory for SQLite database
RUN mkdir -p /app/data && \
    chown -R appuser:appuser /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/meet-slack-bot /app/meet-slack-bot

# Copy sqlx-cli from builder stage
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migrations and entrypoint script
COPY --from=builder /app/migrations /app/migrations
COPY docker-entrypoint.sh /app/docker-entrypoint.sh

# Make files executable
RUN chmod +x /app/meet-slack-bot && \
    chmod +x /app/docker-entrypoint.sh

# Switch to non-root user
USER appuser

# Expose port
EXPOSE 9001

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:9001/health || exit 1

# Run the application with entrypoint script
CMD ["./docker-entrypoint.sh"]
