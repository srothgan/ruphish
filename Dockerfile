# syntax=docker/dockerfile:1
# Packages an already built Linux executable; compilation is a separate step.
# Local development currently carries the validated UI alongside the executable.
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ruphish \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin ruphish \
    && install -d -o 10001 -g 10001 -m 0700 /var/lib/ruphish

WORKDIR /opt/ruphish
COPY dist/admin/ ./dist/admin/
COPY LICENSE /usr/share/doc/ruphish/LICENSE
COPY --chmod=0755 docker/healthcheck.sh /usr/local/bin/ruphish-healthcheck
COPY --chmod=0755 target/docker/ruphish ./ruphish

ARG VERSION=local-development
ARG REVISION=unknown
LABEL org.opencontainers.image.title="Ruphish" \
      org.opencontainers.image.source="https://github.com/srothgan/ruphish" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}"

USER 10001:10001
EXPOSE 3333 8081
HEALTHCHECK --interval=30s --timeout=3s --start-period=60s --retries=3 \
    CMD ["/usr/local/bin/ruphish-healthcheck"]
ENTRYPOINT ["/opt/ruphish/ruphish"]
CMD ["--config", "/etc/ruphish/config.json"]
