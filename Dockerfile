# The CulebraLuxe application: one Rust server that serves the website, the portal and the API.
#
# Build context is the repository root (see .dockerignore): the server is compiled from rust/, and the image ships the
# built site (public/: the Yew wasm, its glue, the CSS, images) and the form templates the PDF renderer reads.
# The wasm and CSS are built BEFORE this (scripts/site-container.sh runs scripts/site-build.sh), so this image needs
# no Node and no wasm toolchain.
FROM rust:1-bookworm AS builder
WORKDIR /build/rust
COPY rust ./
RUN cargo build --locked --release -p server --bin http

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 culebra
WORKDIR /app
COPY --from=builder /build/rust/target/release/http /usr/local/bin/culebraluxe
COPY public ./public
COPY lib/forms/templates ./templates
ENV CULEBRA_SITE_DIR=/app/public \
    FORMS_TEMPLATES_DIR=/app/templates \
    PORT=8080
EXPOSE 8080
USER culebra
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
  CMD curl --fail --silent http://127.0.0.1:${PORT}/healthz >/dev/null || exit 1
ENTRYPOINT ["/usr/local/bin/culebraluxe"]
