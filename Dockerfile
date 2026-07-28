# syntax=docker/dockerfile:1.7
FROM node:24-bookworm-slim AS web-builder
WORKDIR /source
COPY web/package.json web/package-lock.json ./web/
RUN npm --prefix web ci --ignore-scripts
COPY web ./web
RUN npm --prefix web run build

FROM rust:1.94-bookworm AS rust-builder
WORKDIR /source
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY src ./src
COPY --from=web-builder /source/web/dist ./web/dist
COPY web/package.json web/package-lock.json web/tsconfig.json web/vite.config.ts web/index.html web/tokens.css ./web/
COPY web/public ./web/public
COPY web/src ./web/src
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 65532 lightbws \
    && useradd --uid 65532 --gid lightbws --no-create-home --home-dir /data lightbws \
    && install -d -o lightbws -g lightbws /data
COPY --from=rust-builder /source/target/release/lightbws /usr/local/bin/lightbws
USER 65532:65532
ENV LIGHTBWS_BIND=0.0.0.0:8080 \
    LIGHTBWS_DATA_DIR=/data \
    LIGHTBWS_COOKIE_SECURE=false
VOLUME ["/data"]
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/lightbws"]
