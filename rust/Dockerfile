FROM rust:1.90-bookworm AS builder

WORKDIR /build
COPY rust ./rust
WORKDIR /build/rust

RUN cargo build --release -p server --bin http

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 culebra

COPY --from=builder /build/rust/target/release/http /usr/local/bin/culebraluxe-rust-api

ENV RUST_API_BIND=0.0.0.0:8080
EXPOSE 8080

USER culebra

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl --fail --silent --show-error http://127.0.0.1:8080/healthz >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/culebraluxe-rust-api"]
