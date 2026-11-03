# syntax=docker/dockerfile:1
# The server and the SPA it serves, in one image (KOK-51). Rust builds the server and the engine's
# WASM, Node builds the SPA, and the image keeps only the binary and the build. It runs with only
# DATABASE_URL set, and ORIGIN wherever it isn't served from http://localhost:8080.

FROM rust:1-bookworm AS chef
# Node 26 and pnpm for the SPA, whose build runs scripts/bindings.mjs, which needs Rust too.
COPY --from=node:26-bookworm-slim /usr/local/ /usr/local/
RUN npm install --global pnpm@11.21.0 && cargo install cargo-chef --locked
WORKDIR /src
COPY rust-toolchain.toml .
RUN rustup toolchain install
# The wasm-bindgen CLI must match the crate Cargo.lock pins, or scripts/bindings.mjs installs it.
COPY Cargo.lock .
RUN cargo install wasm-bindgen-cli --locked --version \
    "$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p}' Cargo.lock)"

# Only the manifests, so the dependencies below rebuild when they change, not with every commit.
FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=plan /src/recipe.json .
RUN cargo chef cook --release --locked -p kokonoe-server --recipe-path recipe.json \
    && cargo chef cook --profile wasm --target wasm32-unknown-unknown --locked -p kokonoe-wasm \
        --recipe-path recipe.json
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml web/.npmrc web/
RUN pnpm -C web install --frozen-lockfile
COPY . .
RUN cargo build --release --locked -p kokonoe-server && pnpm -C web build

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 kokonoe
COPY --from=build /src/target/release/kokonoe-server /usr/local/bin/kokonoe-server
COPY --from=build /src/web/build /srv/kokonoe
ENV WEB_ROOT=/srv/kokonoe PORT=8080
USER kokonoe
EXPOSE 8080
CMD ["kokonoe-server"]
