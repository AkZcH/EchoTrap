# ── Builder stage ─────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /app

# Cache dependencies separately from source.
COPY Cargo.toml Cargo.lock ./

# Dummy src and benches so cargo can parse the full manifest.
RUN mkdir src benches \
    && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > benches/throughput.rs
RUN cargo build --release
RUN rm -rf src benches

# Build real source.
COPY src ./src
COPY benches ./benches
RUN touch src/main.rs
RUN cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash echotrap
USER echotrap
WORKDIR /home/echotrap

COPY --from=builder /app/target/release/echotrap /usr/local/bin/echotrap

EXPOSE 9000 8081

CMD ["echotrap", \
     "--port", "9000", \
     "--threshold", "3", \
     "--window", "10", \
     "--persona", "ssh", \
     "--max-connections", "10000", \
     "--dashboard-port", "8081", \
     "--log", "/home/echotrap/echotrap.log"]