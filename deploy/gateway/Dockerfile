# syntax=docker/dockerfile:1.7.0

ARG CADDY_BASE_IMAGE=docker.io/library/caddy:2.10.2-alpine@sha256:4c6e91c6ed0e2fa03efd5b44747b625fec79bc9cd06ac5235a779726618e530d
FROM ${CADDY_BASE_IMAGE}

ARG JIMIN_BUILD_SHA=dev
LABEL org.opencontainers.image.title="Jimin OS Gateway" \
      org.opencontainers.image.source="https://github.com/Yaklede/jimin-os" \
      org.opencontainers.image.revision="${JIMIN_BUILD_SHA}"

RUN mkdir -p /data/caddy /config/caddy /etc/caddy/tls /tmp \
    && cp /usr/bin/caddy /tmp/caddy \
    && mv /tmp/caddy /usr/bin/caddy \
    && chmod 0755 /usr/bin/caddy \
    && chown -R 65532:65532 /data /config /etc/caddy/tls /tmp

COPY --chown=65532:65532 Caddyfile /etc/caddy/Caddyfile
COPY --chown=65532:65532 tls /etc/caddy/tls

USER 65532:65532
EXPOSE 8443
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=4 CMD ["wget", "-q", "--spider", "http://127.0.0.1:2020/healthz"]
ENTRYPOINT ["caddy"]
CMD ["run", "--config", "/etc/caddy/Caddyfile", "--adapter", "caddyfile"]
