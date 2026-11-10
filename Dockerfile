# One image, one container: the Rust gateway (/ws, /health, /metrics) also serves the website.
#   docker build -t pairspan:latest .
#   docker compose up -d

# 1. Website: Next.js static export → apps/web/out
FROM node:22-alpine AS web
WORKDIR /src
COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
COPY apps/desktop/package.json apps/desktop/package.json
RUN npm ci --workspace apps/web --no-audit --no-fund
COPY install.sh ./
COPY apps/web apps/web
RUN cp install.sh apps/web/public/install.sh && npm run build --workspace apps/web

# 2. Gateway: static musl binary
FROM rust:1-alpine AS gateway
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.lock ./Cargo.lock
COPY crates/gateway/Cargo.toml ./Cargo.toml
RUN mkdir src && echo 'fn main(){}' > src/main.rs && touch src/lib.rs \
 && cargo build --release --bin pairspan-gateway && rm -rf src
COPY crates/gateway/src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release --bin pairspan-gateway \
 && strip target/release/pairspan-gateway

# 3. Runtime: no shell, no package manager, non-root
FROM scratch
ARG APP_REVISION=dev
ENV APP_REVISION=$APP_REVISION PORT=3000 DOCS_ROOT=/public
COPY --from=gateway /src/target/release/pairspan-gateway /pairspan-gateway
COPY --from=web /src/apps/web/out /public
USER 65534:65534
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s CMD ["/pairspan-gateway", "healthcheck"]
ENTRYPOINT ["/pairspan-gateway"]
