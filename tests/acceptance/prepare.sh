#!/bin/sh
set -eu
umask 077
if [ "${1:-}" = '--mysql' ] || [ "${1:-}" = '--postgres' ]; then
    provider=${1#--}
    mkdir -p "/lab/data/$provider"
    cd "/lab/data/$provider"
    if [ -f ca.pem ] && [ -f server.pem ] && [ -f server.key ]; then
        openssl verify -CAfile ca.pem server.pem
        chmod 755 .
        chmod 644 ca.pem server.pem server.key
        exit 0
    fi
    for file in ca.pem ca.key server.pem server.key; do
        if [ -e "$file" ]; then
            echo 'Incomplete database fixture certificate set; existing files kept.' >&2
            exit 1
        fi
    done
    openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 365 \
        -keyout ca.key -out ca.pem -subj '/CN=Ruphish database fixture CA' \
        -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign'
    openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -subj '/CN=localhost'
    printf '%s\n' "subjectAltName=DNS:localhost,DNS:$provider,IP:127.0.0.1" \
        'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' \
        'extendedKeyUsage=serverAuth' > server.ext
    openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial \
        -out server.pem -days 365 -sha256 -extfile server.ext
    # Disposable fixture material is read by the database image's startup process.
    chmod 755 .
    chmod 644 ca.pem server.pem server.key
    exit 0
fi
cd /lab/data
if [ -f ca.pem ] && [ -f server.pem ] && [ -f server.key ]; then
    openssl verify -CAfile ca.pem server.pem
else
    for file in ca.pem ca.key server.pem server.key; do
        if [ -e "$file" ]; then
            echo 'Incomplete certificate set; keep the files and inspect data/ before retrying.' >&2
            exit 1
        fi
    done
    openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 365 \
        -keyout ca.key -out ca.pem -subj '/CN=Ruphish local domain lab CA' \
        -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign'
    openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr \
        -subj '/CN=good.ruphish.test'
    printf '%s\n' 'subjectAltName=DNS:good.ruphish.test,DNS:bad.ruphish.test' \
        'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' \
        'extendedKeyUsage=serverAuth' > server.ext
    openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial \
        -out server.pem -days 365 -sha256 -extfile server.ext
fi
cd /workspace
# Host-side HTTPS clients need the public certificates, never the private keys.
chmod 644 /lab/data/ca.pem /lab/data/server.pem
# Preserve host ownership of fixture inputs while allowing the image's group to
# create SQLite/log files. Only application-owned private files change owner.
chgrp 10001 /lab/data
chmod g+wx /lab/data
for file in server.key ruphish.db ruphish.db-wal ruphish.db-shm ruphish.db-journal ruphish.db.ruphish-startup.lock ruphish.log; do
    if [ -f "/lab/data/$file" ]; then
        chown 10001:10001 "/lab/data/$file"
    fi
done
if [ "${1:-}" = '--certificates-only' ]; then
    echo 'Fixture certificates prepared; supplied image and executable kept.'
    exit 0
fi
if [ "${1:-}" = '--release' ]; then
    cargo build -p ruphish --release --features embedded-assets --locked
    cp /target/release/ruphish /artifact/ruphish
elif [ "$#" = 0 ]; then
    cargo build -p ruphish --features embedded-assets --locked
    cp /target/debug/ruphish /artifact/ruphish
else
    echo 'Expected --release, --certificates-only, --mysql, --postgres or no argument.' >&2
    exit 1
fi
chmod 755 /artifact/ruphish
echo 'Linux executable prepared in target/docker for the runtime image. No system CA store was changed.'
