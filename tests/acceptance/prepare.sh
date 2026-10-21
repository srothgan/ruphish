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
    printf '%s\n' 'subjectAltName=DNS:localhost,IP:127.0.0.1' \
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
cargo build -p ruphish --locked
cp /target/debug/ruphish /lab/data/ruphish
echo 'Local Linux executable and certificates prepared. No system CA store was changed.'
