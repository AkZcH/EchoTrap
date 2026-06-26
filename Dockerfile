# ── Builder stage ─────────────────────────────────────────────────────────────
FROM rust:1.85-slim AS builder

WORKDIR /app

# Cache dependencies separately from source so a source change doesn't
# re-download the entire dependency tree.
COPY Cargo.toml Cargo.lock ./

# Build a dummy main to cache compiled deps.
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Now copy real source and build.
COPY src ./src
# Touch main.rs so cargo knows the source changed.
RUN touch src/main.rs
RUN cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# ca-certificates needed if EchoTrap ever makes outbound TLS calls.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root user — honeypots shouldn't run as root.
RUN useradd -ms /bin/bash echotrap
USER echotrap
WORKDIR /home/echotrap

COPY --from=builder /app/target/release/echotrap /usr/local/bin/echotrap

# Honeypot port and dashboard port.
EXPOSE 9000 8081

# Sensible defaults — all overridable via docker run or compose env.
CMD ["echotrap", \
     "--port", "9000", \
     "--threshold", "3", \
     "--window", "10", \
     "--persona", "ssh", \
     "--max-connections", "10000", \
     "--dashboard-port", "8081", \
     "--log", "/home/echotrap/echotrap.log"]