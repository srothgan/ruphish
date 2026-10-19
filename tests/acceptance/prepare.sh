#!/bin/sh
set -eu
umask 077
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
