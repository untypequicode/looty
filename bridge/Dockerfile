# looty-bridge container image. Build context: this directory.
#   docker build -t looty-bridge bridge/
FROM rust:slim-trixie AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:trixie-slim
# Roots of trust are built in (webpki-roots): no ca-certificates needed.
RUN useradd --system --no-create-home --uid 10001 bridge
COPY --from=builder /app/target/release/looty-bridge /usr/local/bin/looty-bridge
USER bridge
# LOOTY_INSTANCE, LOOTY_TOKEN and LOOTY_SERVICES (comma separated name=url) are read from the environment.
ENTRYPOINT ["/usr/local/bin/looty-bridge"]
