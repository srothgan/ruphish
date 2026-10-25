#!/bin/sh
set -eu

# This URL is a local operational setting, not an application configuration override.
url=${RUPHISH_HEALTH_URL:-http://127.0.0.1:3333/health/ready}
set -- --noproxy '*' --silent --output /dev/null --write-out '%{http_code}' \
    --connect-timeout 1 --max-time 2 --proto '=http,https'
if [ -n "${RUPHISH_HEALTH_CA:-}" ]; then
    set -- "$@" --cacert "$RUPHISH_HEALTH_CA"
fi
# Do not follow redirects or print URLs, bodies, credentials or TLS diagnostics.
if status=$(curl "$@" "$url"); then
    [ "$status" = 200 ] && exit 0
fi
echo 'Ruphish readiness probe failed' >&2
exit 1
