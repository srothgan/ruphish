#!/bin/sh
set -eu
# Docker Desktop bind mounts do not expose Unix key ownership/permissions.
# Copy the disposable fixture key into the container and restrict it there.
cp /tls/server.key /tmp/ruphish-postgres.key
chown postgres:postgres /tmp/ruphish-postgres.key
chmod 600 /tmp/ruphish-postgres.key
exec /usr/local/bin/docker-entrypoint.sh postgres -c ssl=on -c ssl_ca_file=/tls/ca.pem -c ssl_cert_file=/tls/server.pem -c ssl_key_file=/tmp/ruphish-postgres.key -c log_parameter_max_length=0 -c log_parameter_max_length_on_error=0 -c log_statement=none
