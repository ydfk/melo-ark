# syntax=docker/dockerfile:1.7

FROM --platform=$BUILDPLATFORM node:26.5.0-bookworm-slim AS web-builder
WORKDIR /build/apps/web
RUN npm install --global pnpm@11.17.0
COPY apps/web/package.json apps/web/pnpm-lock.yaml apps/web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY apps/web/ ./
RUN pnpm build

FROM rust:1.97.1-alpine3.23 AS server-builder
WORKDIR /build/apps/server
COPY apps/server/Cargo.toml apps/server/Cargo.lock ./
COPY apps/server/migrations ./migrations
COPY apps/server/src ./src
RUN --mount=type=cache,id=meloark-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=meloark-server-target-alpine-amd64,target=/build/apps/server/target \
    cargo build --release --locked \
    && cp target/release/meloark-server /tmp/meloark-server

FROM alpine:3.23 AS runtime
RUN apk add --no-cache ca-certificates chromaprint curl ffmpeg \
    && addgroup -g 10001 -S meloark \
    && adduser -u 10001 -S -D -H -G meloark meloark

WORKDIR /opt/meloark
COPY --from=server-builder /tmp/meloark-server /usr/local/bin/meloark-server
COPY --from=web-builder /build/apps/web/dist ./web
COPY apps/server/config ./config
COPY LICENSE THIRD_PARTY_NOTICES.md ./

RUN mkdir -p /data/cache \
    && chown -R meloark:meloark /data /opt/meloark

ENV MELOARK__APP__HOST=0.0.0.0 \
    MELOARK__APP__PORT=31000 \
    MELOARK__APP__ENVIRONMENT=production \
    MELOARK__APP__WEB_DIST=/opt/meloark/web \
    MELOARK__DATABASE__PATH=/data/meloark.db

LABEL org.opencontainers.image.title="MeloArk" \
      org.opencontainers.image.description="Self-hosted local music library manager and OpenSubsonic server" \
      org.opencontainers.image.licenses="Apache-2.0"

USER meloark
EXPOSE 31000
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl --fail --silent http://127.0.0.1:31000/api/health >/dev/null || exit 1
ENTRYPOINT ["meloark-server"]
