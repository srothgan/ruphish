# syntax=docker/dockerfile:1
# Packages an already built Linux executable; compilation is a separate step.
# Build with embedded-assets on a Bookworm-compatible Linux builder.
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ruphish \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin ruphish \
    && install -d -o 10001 -g 10001 -m 0700 /var/lib/ruphish \
    && install -d -m 0755 /etc/ruphish

WORKDIR /opt/ruphish
COPY LICENSE /usr/share/doc/ruphish/LICENSE
COPY THIRD-PARTY-NOTICES.md /usr/share/doc/ruphish/THIRD-PARTY-NOTICES.md
COPY dist/admin/THIRD-PARTY-NOTICES.txt /usr/share/doc/ruphish/THIRD-PARTY-NOTICES.txt
COPY --chmod=0444 docker/config.json /etc/ruphish/config.json
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
# Use native configuration discovery, so additional CLI flags keep these defaults.
ENV XDG_CONFIG_HOME=/etc XDG_DATA_HOME=/var/lib XDG_STATE_HOME=/var/lib
EXPOSE 3333 8081
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=30s --timeout=3s --start-period=60s --retries=3 \
    CMD ["/usr/local/bin/ruphish-healthcheck"]
ENTRYPOINT ["/opt/ruphish/ruphish"]
