# syntax=docker/dockerfile:1
FROM rust:1-trixie AS build
WORKDIR /app
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then export SSL_CERT_FILE=/run/secrets/proxy_ca CARGO_HTTP_CAINFO=/run/secrets/proxy_ca; fi; \
    rustup target add wasm32-unknown-unknown && \
    rustup component add rustfmt clippy && \
    curl ${SSL_CERT_FILE:+--cacert "$SSL_CERT_FILE"} -fsSL \
      https://github.com/DioxusLabs/dioxus/releases/download/v0.7.10/dx-x86_64-unknown-linux-gnu.tar.gz \
      | tar -xz -C /usr/local/bin dx
RUN dx --version
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then export SSL_CERT_FILE=/run/secrets/proxy_ca CARGO_HTTP_CAINFO=/run/secrets/proxy_ca; fi; \
    cargo fmt --all --check && \
    cargo test -p comic-model -p comic-server --locked && \
    cargo clippy -p comic-model -p comic-server --all-targets --locked -- -D warnings && \
    cargo clippy -p comic-web --target wasm32-unknown-unknown --locked -- -D warnings && \
    cargo build -p comic-server --release --locked
WORKDIR /app/crates/web
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then export SSL_CERT_FILE=/run/secrets/proxy_ca CARGO_HTTP_CAINFO=/run/secrets/proxy_ca; fi; \
    dx build --web --release --debug-symbols false --locked -p comic-web && \
    test -f /app/target/dx/comic-web/release/web/public/index.html && \
    cp assets/archive-forest.png /app/target/dx/comic-web/release/web/public/archive-forest.png
WORKDIR /app

FROM debian:trixie-slim AS runtime
WORKDIR /app
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /app/target/release/comic-server /app/comic-server
COPY --from=build /app/target/dx/comic-web/release/web/public /app/public
USER 10001:10001
ENV COMIC_BIND=0.0.0.0:10000 COMIC_PUBLIC_DIR=/app/public
EXPOSE 10000
ENTRYPOINT ["/app/comic-server"]
CMD ["serve"]
