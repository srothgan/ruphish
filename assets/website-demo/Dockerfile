# Local asset authoring only; this is not a product distribution image.
FROM node:26.8.1-bookworm-slim AS node

FROM rust:1.99.1-bookworm AS build
COPY --from=node /usr/local/ /usr/local/
ARG BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=${BUILD_JOBS} TOKIO_WORKER_THREADS=2 RAYON_NUM_THREADS=2 NO_COLOR=true
WORKDIR /source
COPY rust-toolchain.toml Trunk.toml ./
RUN rustup show
RUN cargo install trunk --version "$(node --input-type=module -e 'import fs from "node:fs"; console.log(fs.readFileSync("Trunk.toml", "utf8").match(/trunk-version\s*=\s*"=([^"]+)"/)[1])')" --locked --profile dev
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY crates/ crates/
RUN npm ci --no-audit --no-fund --prefix crates/ruphish-ui/web
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/source/target \
    cargo build -p ruphish --locked && mkdir /built && cp target/debug/ruphish /built/ruphish
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/source/target \
    trunk build --locked

FROM node AS recorder
ENV TOKIO_WORKER_THREADS=2 RAYON_NUM_THREADS=2 NO_COLOR=true
WORKDIR /recorder
COPY assets/website-demo/package.json assets/website-demo/package-lock.json ./
RUN npm ci --no-audit --no-fund && npx playwright install --with-deps chromium
COPY --from=build /built/ruphish /app/ruphish
COPY --from=build /source/dist/admin/ /app/dist/admin/
COPY assets/website-demo/*.mjs ./
COPY assets/website-demo/data.json ./
ENTRYPOINT ["node", "/recorder/record.mjs"]
