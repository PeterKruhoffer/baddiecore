FROM node:24-bookworm-slim AS web
WORKDIR /build/web
RUN corepack enable
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm run build

FROM rust:1.98-bookworm AS backend
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home cms
COPY --from=backend /build/target/release/baddiecore /usr/local/bin/baddiecore
COPY --from=web /build/web/dist /app/web
ENV PORT=3000 BADDIE_STATIC=/app/web
USER cms
WORKDIR /app
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s CMD curl --fail --silent "http://127.0.0.1:${PORT}/health" || exit 1
CMD ["baddiecore"]
