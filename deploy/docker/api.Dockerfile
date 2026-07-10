# syntax=docker/dockerfile:1.7.0

ARG RUST_BUILDER_IMAGE=docker.io/library/rust:1.95.0-slim-bookworm@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082
ARG DEBIAN_RUNTIME_IMAGE=docker.io/library/debian:bookworm-slim@sha256:60eac759739651111db372c07be67863818726f754804b8707c90979bda511df

FROM ${RUST_BUILDER_IMAGE} AS builder
WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml ./
COPY .cargo ./.cargo
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
COPY schemas ./schemas

RUN cargo build --locked --release --package jimin-api --bin jimin-api

FROM ${DEBIAN_RUNTIME_IMAGE} AS runtime
ARG JIMIN_BUILD_SHA=dev
LABEL org.opencontainers.image.title="Jimin OS API" \
      org.opencontainers.image.source="https://github.com/Yaklede/jimin-os" \
      org.opencontainers.image.revision="${JIMIN_BUILD_SHA}"

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder --chown=10001:10001 /workspace/target/release/jimin-api /usr/local/bin/jimin-api

ENV HOME=/nonexistent \
    JIMIN_API_BIND_ADDR=0.0.0.0:8080 \
    JIMIN_API_PROBE_ADDR=127.0.0.1:8080 \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=4 CMD ["jimin-api", "probe", "ready"]
ENTRYPOINT ["/usr/local/bin/jimin-api"]
