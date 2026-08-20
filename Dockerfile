FROM node:22-bookworm-slim AS web-dependencies

WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev

FROM rust:1.89-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        clang \
        cmake \
        curl \
        libclang-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
    && cargo install dioxus-cli --version 0.7.9 --locked

WORKDIR /app
COPY . .
COPY --from=web-dependencies /app/node_modules /app/node_modules
# Local development patches the three shared crates from sibling repositories.
# The image uses their committed snapshots so a deployment clone is complete.
COPY .cargo/config.docker.toml .cargo/config.toml

RUN dx build --web --fullstack --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/dx/tawny/release/web/ /app/

ENV IP=0.0.0.0 \
    PORT=8080 \
    TAWNY_DATA_DIR=/data

EXPOSE 8080
ENTRYPOINT ["/app/server"]
