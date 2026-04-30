FROM rust:bookworm AS server-builder
WORKDIR /app
RUN apt-get update \
  && apt-get install -y --no-install-recommends cmake pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY server ./server
RUN cargo build --release -p pkv-sync-server

FROM node:24-bookworm AS plugin-builder
WORKDIR /app/plugin
COPY plugin/package*.json ./
RUN npm ci
COPY plugin ./
RUN npm run build

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server-builder /app/target/release/pkvsyncd /usr/local/bin/pkvsyncd
COPY --from=plugin-builder /app/plugin/main.js /plugin/main.js
COPY --from=plugin-builder /app/plugin/manifest.json /plugin/manifest.json
COPY --from=plugin-builder /app/plugin/styles.css /plugin/styles.css
EXPOSE 6710
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/pkvsyncd"]
CMD ["-c", "/etc/pkv-sync/config.toml", "serve"]
