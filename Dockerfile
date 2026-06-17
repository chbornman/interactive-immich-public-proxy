# ---- frontend build ----
FROM node:20-alpine AS web
WORKDIR /web
COPY web/package.json ./
RUN npm install
COPY web/ ./
RUN npm run build

# ---- backend build ----
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml ./
# Cache dependency compilation: build a dummy bin so deps land in their own layer.
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
COPY migrations ./migrations
# Touch so cargo re-detects the real main.rs over the cached dummy build.
RUN touch src/main.rs && cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/interactive-ipp /usr/local/bin/interactive-ipp
COPY --from=web /web/dist /app/web/dist
ENV WEB_DIR=/app/web/dist \
    DB_PATH=/data/ipp.db \
    BIND=0.0.0.0:3000
EXPOSE 3000
VOLUME ["/data"]
CMD ["interactive-ipp"]
