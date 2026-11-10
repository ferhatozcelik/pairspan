# Build from the repository root:  docker build -f gateway/Dockerfile -t pairspan-gateway .
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.lock ./Cargo.lock
COPY gateway/Cargo.toml ./Cargo.toml
# Cache dependencies separately from sources.
RUN mkdir src && echo 'fn main(){}' > src/main.rs && touch src/lib.rs \
 && cargo build --release --bin pairspan-gateway && rm -rf src
COPY gateway/src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release --bin pairspan-gateway \
 && strip target/release/pairspan-gateway

FROM scratch
COPY --from=build /src/target/release/pairspan-gateway /pairspan-gateway
COPY web/public /public
ENV PORT=8080 DOCS_ROOT=/public
USER 65534:65534
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s CMD ["/pairspan-gateway", "healthcheck"]
ENTRYPOINT ["/pairspan-gateway"]
