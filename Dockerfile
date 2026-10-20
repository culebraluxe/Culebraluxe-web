# The CulebraLuxe application: one Rust server that serves the website, the portal and the API.
#
# Build context is the repository root (see .dockerignore). Everything is compiled HERE, from source: the server, and
# the Yew UI to WebAssembly (scripts/rust-ui-build.sh), so a deploy uploads a few MB of source instead of the 7 MB wasm
# (uploading the wasm is what failed with "fetch failed"). The image ships the server, public/ (images, the CSS built
# by scripts/site-build.sh before the image, and the freshly built wasm + glue) and the form templates.
FROM rust:1-bookworm AS builder
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.128 --locked
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
# The image is the app, not the test workspace: `tests/` depends on the private Forge git repo, and
# Cargo resolves every workspace member before building. Drop that member and never COPY tests/.
RUN sed -i '/^    "tests",$/d' Cargo.toml \
    && ! grep -q '"tests"' Cargo.toml
COPY web ./web
COPY middle ./middle
COPY db ./db
COPY cli ./cli
COPY re-host ./re-host
COPY observability ./observability
COPY scripts/rust-ui-build.sh ./scripts/rust-ui-build.sh
RUN mkdir -p public/rust-ui && RUST_UI_PROFILE=release bash scripts/rust-ui-build.sh
# No --locked: dropping the test member rewrites the app lock and must not require Forge access.
RUN cargo build --release --manifest-path Cargo.toml -p web --bin web

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 culebra
WORKDIR /app
COPY --from=builder /build/target/release/web /usr/local/bin/culebraluxe
COPY public ./public
COPY --from=builder /build/public/rust-ui/ui.js /build/public/rust-ui/ui_bg.wasm ./public/rust-ui/
COPY middle/model/forms/templates ./templates
ENV CULEBRA_SITE_DIR=/app/public \
    FORMS_TEMPLATES_DIR=/app/templates \
    PORT=8080
EXPOSE 8080
USER culebra
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
  CMD curl --fail --silent http://127.0.0.1:${PORT}/healthz >/dev/null || exit 1
ENTRYPOINT ["/usr/local/bin/culebraluxe"]
