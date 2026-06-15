FROM node:24-bookworm AS plugin-builder
WORKDIR /app/plugin
COPY plugin/package*.json ./
RUN npm ci
COPY plugin ./
RUN npm run build

FROM rust:bookworm AS server-builder
WORKDIR /app
RUN apt-get update \
  && apt-get install -y --no-install-recommends cmake pkg-config \
  && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY server ./server
RUN mkdir -p plugin
COPY --from=plugin-builder /app/plugin/main.js ./plugin/main.js
COPY --from=plugin-builder /app/plugin/manifest.json ./plugin/manifest.json
COPY --from=plugin-builder /app/plugin/styles.css ./plugin/styles.css
RUN CARGO_NET_RETRY=10 CARGO_HTTP_MULTIPLEXING=false cargo build --release -p pkv-sync-server

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server-builder /app/target/release/pkvsyncd /usr/local/bin/pkvsyncd
EXPOSE 6710
USER 65532:65532
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD curl -fsS http://127.0.0.1:6710/api/health || exit 1
ENTRYPOINT ["/usr/local/bin/pkvsyncd"]
CMD ["-c", "/etc/pkv-sync/config.toml", "serve"]
