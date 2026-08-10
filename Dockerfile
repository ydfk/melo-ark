# syntax=docker/dockerfile:1.7

FROM --platform=$BUILDPLATFORM node:26.5.0-bookworm-slim AS web-builder
WORKDIR /build/apps/web
RUN npm install --global pnpm@11.17.0
COPY apps/web/package.json apps/web/pnpm-lock.yaml apps/web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY apps/web/ ./
RUN pnpm build

FROM --platform=linux/amd64 rust:1.97.1-bookworm AS server-builder
WORKDIR /build/apps/server
COPY apps/server/Cargo.toml apps/server/Cargo.lock ./
COPY apps/server/migrations ./migrations
COPY apps/server/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/apps/server/target \
    cargo build --release --locked \
    && cp target/release/meloark-server /tmp/meloark-server

FROM --platform=linux/amd64 debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl ffmpeg libchromaprint-tools \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 meloark \
    && useradd --uid 10001 --gid meloark --create-home --shell /usr/sbin/nologin meloark

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
