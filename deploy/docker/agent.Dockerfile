# syntax=docker/dockerfile:1.7.0

ARG RUST_BUILDER_IMAGE=docker.io/library/rust:1.95.0-slim-bookworm@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082
ARG NODE_RUNTIME_IMAGE=docker.io/library/node:22.14.0-bookworm-slim@sha256:1c18d9ab3af4585870b92e4dbc5cac5a0dc77dd13df1a5905cea89fc720eb05b

FROM ${RUST_BUILDER_IMAGE} AS builder
WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml ./
COPY .cargo ./.cargo
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
COPY schemas ./schemas

RUN cargo build --locked --release --package jimin-agent --bin jimin-agent

FROM ${NODE_RUNTIME_IMAGE} AS codex-installer
ARG CODEX_VERSION=0.144.1
ARG CODEX_NPM_INTEGRITY=sha512-Xir1zqPfpenhdoAoshN53uonzbBXj18COyzRkFlVZpSNyEl5XtkuYu9oddELePFN7K/0sXUcSO34Ad5IeCXPbw==

RUN set -eu; \
    actual_integrity="$(npm view "@openai/codex@${CODEX_VERSION}" dist.integrity)"; \
    test "${actual_integrity}" = "${CODEX_NPM_INTEGRITY}"; \
    npm install --global --omit=dev --no-audit --no-fund "@openai/codex@${CODEX_VERSION}"; \
    codex --version | grep -F "${CODEX_VERSION}"

FROM ${NODE_RUNTIME_IMAGE} AS runtime
ARG JIMIN_BUILD_SHA=dev
ARG CODEX_VERSION=0.144.1
LABEL org.opencontainers.image.title="Jimin OS Agent" \
      org.opencontainers.image.source="https://github.com/Yaklede/jimin-os" \
      org.opencontainers.image.revision="${JIMIN_BUILD_SHA}" \
      io.jimin-os.codex.version="${CODEX_VERSION}"

COPY --from=codex-installer /usr/local/lib/node_modules/@openai /usr/local/lib/node_modules/@openai
COPY --from=builder --chown=node:node /workspace/target/release/jimin-agent /usr/local/bin/jimin-agent
COPY --chown=node:node apps/agent/tests/fixtures/generic-prompt.txt /opt/jimin-agent/fixtures/generic-prompt.txt

RUN rm -rf /usr/local/lib/node_modules/npm \
    && rm -f /usr/local/bin/npm /usr/local/bin/npx /usr/local/bin/corepack /usr/local/bin/yarn /usr/local/bin/pnpm \
    && ln -s ../lib/node_modules/@openai/codex/bin/codex.js /usr/local/bin/codex \
    && mkdir -p /home/node/.codex /workspace/data /workspace/.git/objects /workspace/.git/refs/heads /workspace/.git/refs/tags \
    && printf 'ref: refs/heads/main\n' > /workspace/.git/HEAD \
    && printf '[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n' > /workspace/.git/config \
    && chown -R node:node /home/node/.codex /workspace

ENV CODEX_HOME=/home/node/.codex \
    JIMIN_AGENT_CODEX_BIN=/usr/local/bin/codex \
    JIMIN_AGENT_WORKSPACE=/workspace \
    HOME=/home/node

USER node:node
WORKDIR /workspace
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 CMD ["jimin-agent", "health"]
ENTRYPOINT ["/usr/local/bin/jimin-agent"]
