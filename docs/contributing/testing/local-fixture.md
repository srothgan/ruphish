---
title: Local acceptance fixture
description: Run the canonical controlled DNS, HTTPS, SMTP, and participant protocol environment.
sidebar:
  order: 3
---

This is the single Docker test environment for domain verification, public interactions and SMTP acceptance. Keep its admin address on port 3334 and public HTTPS on 8443. Extend this fixture when scenarios change; use fresh data in CI instead of maintaining another lab definition.

This fixture runs a separate Ruphish installation, authoritative test DNS, trusted test HTTPS and a Mailpit inbox. It uses only `*.test` recipients. The seeded SMTP profile points only at the local inbox, SMTP has no host port or relay configuration, and web ports bind to host loopback. The bridge network is not an outbound firewall; keep these fixture resources pointed at the supplied test services. No host DNS, certificate store or production database is changed.

## Start the fixture

Requirements: Docker with Linux containers and Compose v2, Node with native Fetch and `Headers.getSetCookie()`, and the toolchain documented in AGENTS.md. Run commands from the product root. Build the admin distribution first if `dist/admin/` is absent or stale: `npm ci --prefix crates/ruphish-ui/web`, then `trunk build --locked` (PowerShell: set `$env:NO_COLOR = 'true'` first).

```sh
node tests/acceptance/lab.mjs init
docker compose -f tests/acceptance/compose.yaml --profile setup run --rm prepare
docker compose -f tests/acceptance/compose.yaml build app
docker compose -f tests/acceptance/compose.yaml up -d --wait dns inbox app
node tests/acceptance/lab.mjs demo
node tests/acceptance/lab.mjs seed
node --test tests/acceptance/public.test.mjs
```

`prepare` compiles the Linux executable in the Rust builder container using Cargo.lock, exports it to `target/docker/ruphish`, and generates or reuses the local CA/server certificate. The Dockerfile packages that executable and `dist/admin/` as `ruphish:local`; the running app does not mount the source checkout. Building needs network access for toolchains, crates and runtime packages; acceptance scenarios use local services only. `demo` uses normal login/password-change APIs and actual DNS/TLS verification: `https://good.ruphish.test:8443` must pass and `https://bad.ruphish.test:8443` must fail with a TXT mismatch. `seed` preserves existing named resources and creates a new browser campaign each time. The protocol tests check unauthenticated health on both listeners, rejected probes, received SMTP content, public events, duplicate submissions, malformed bodies, completion, absence of canary values from API responses/logs/live SQLite files, and persistence across container replacement. They do not execute the browser collector.

| Surface | Address |
| --- | --- |
| Admin | `http://127.0.0.1:3334` |
| Local inbox | `http://127.0.0.1:8026` |
| Public passing origin | `https://good.ruphish.test:8443` |
| Public failing origin | `https://bad.ruphish.test:8443` |

Log in as `admin`; retrieve the fixture password with `node tests/acceptance/lab.mjs password`. This is a separate account/database. Its generated secrets, certificate keys and database are ignored under `tests/acceptance/data/`; the exported binary is ignored under `target/docker/`. Do not publish the data directory or raw Mailpit messages as CI artifacts. If you change the admin password/API key manually, update the local bootstrap file before using fixture commands again.

## Local image definition

The root [Dockerfile](../../../Dockerfile) packages prebuilt artifacts on a pinned Debian Bookworm slim base compatible with the Linux builder. The runtime includes CA certificates, curl and libgcc, runs as UID/GID `10001:10001`, and starts Ruphish directly as PID 1. It contains no Rust or Node toolchain. The [.dockerignore](../../../.dockerignore) allows only the executable, completed UI, license and probe script into the build context. These are development artifacts: the large unoptimized executable/WASM and separate UI files are not the self-contained production distribution planned for release.

The image expects `--config /etc/ruphish/config.json` by default; the fixture overrides this to `/lab/data/config.json`. Database selection still comes from the ordinary [database configuration](../../administration/databases/index.md). The fixture supplies SQLite and its existing certificates, credentials and database through the data mount. Preparation preserves host ownership of fixture inputs, grants the app group write access to the data directory, and assigns the server key and existing database/log files to the app user. A general deployment must mount its own readable configuration/certificates and writable state paths; fixture settings are not production defaults.

The image's [health probe](../../administration/health.md) requires exactly HTTP `200` from `http://127.0.0.1:3333/health/ready`. It checks every 30 seconds, allows a 60-second startup grace period and becomes unhealthy after three consecutive failures. Curl has a two-second deadline inside Docker's three-second command timeout. Set `RUPHISH_HEALTH_URL` to match an alternative listener/port/scheme and `RUPHISH_HEALTH_CA` to a mounted CA file for private HTTPS; the URL's hostname must match the certificate. These variables configure the probe only. The script bypasses proxy environment variables, rejects redirects and validates TLS. Docker health status alone does not restart a process.

Optional Compose build inputs are `RUPHISH_APP_IMAGE` (default `ruphish:local`), `RUPHISH_IMAGE_VERSION` (default `local-development`) and `RUPHISH_IMAGE_REVISION` (default `unknown`). The latter two label the image and do not change the compiled application version. No image is published by these commands.

## Open a browser without changing system DNS or trust

Use an isolated Chrome/Chromium profile with explicit test-host mappings and a certificate exception scoped to this fixture's public key. Do not use a general certificate-error bypass. Server-side domain verification and the Node protocol test still perform normal CA/hostname validation. These Chromium flags are intended for this local fixture; they do not establish that normal end-user certificate provisioning works.

Windows PowerShell, from the product root (adjust the Chrome executable path if installed elsewhere):

```powershell
$pin = node tests/acceptance/lab.mjs browser-pin
$profile = Join-Path (Get-Location) 'tests/acceptance/data/browser-profile'
& 'C:\Program Files\Google\Chrome\Application\chrome.exe' "--user-data-dir=$profile" '--host-resolver-rules=MAP good.ruphish.test 127.0.0.1, MAP bad.ruphish.test 127.0.0.1' "--ignore-certificate-errors-spki-list=$pin" 'http://127.0.0.1:8026'
```

Linux, with Chromium installed:

```sh
pin=$(node tests/acceptance/lab.mjs browser-pin)
chromium --user-data-dir="$PWD/tests/acceptance/data/browser-profile" --host-resolver-rules='MAP good.ruphish.test 127.0.0.1, MAP bad.ruphish.test 127.0.0.1' --ignore-certificate-errors-spki-list="$pin" http://127.0.0.1:8026
```

Close this dedicated browser completely before changing its flags. The fixture never launches or automates a browser itself. Chromium documents the [SPKI certificate exception](https://chromium.googlesource.com/chromium/src/+/HEAD/services/network/ignore_errors_cert_verifier.h) and [local host-mapping examples](https://www.chromium.org/quic/playing-with-quic/). Mailpit's [Docker image](https://mailpit.axllent.org/docs/install/docker/) and [HTTP API](https://mailpit.axllent.org/docs/api-v1/) provide the local inbox.

## Browser checks

Follow the separate [manual UI checklist](manual-ui.md) for participant-browser behavior. Protocol acceptance does not execute the browser collector.

## Repeat and stop

Local manual testing retains the fixture database between runs. The [validation workflow](../../../.github/workflows/validation.yml) uses this same definition with a fresh checkout and disposable runner state. The [validation policy](validation.md) places this suite on fixture changes and full nightly/release source acceptance. Production uses the built application with deployment-specific configuration, real services and separately managed secrets/storage. Test accounts, local certificates, CoreDNS and Mailpit are test dependencies and must not be promoted into production. Deployment is separate from this test environment.

```sh
docker compose -f tests/acceptance/compose.yaml logs --tail 60 app dns inbox
docker compose -f tests/acceptance/compose.yaml stop app
docker compose -f tests/acceptance/compose.yaml --profile setup run --rm prepare
docker compose -f tests/acceptance/compose.yaml build app
docker compose -f tests/acceptance/compose.yaml up -d --wait app
node --test tests/acceptance/public.test.mjs
docker compose -f tests/acceptance/compose.yaml down
```

Stop `app` before rebuilding its binary or promoting new UI assets. `down` preserves the database and keys. Mailpit's inbox is ephemeral; each protocol run sends its own uniquely correlated email. The protocol test uses a unique unregistered hostname for rejected-host coverage, so a manually repaired bad example remains intact. Re-running `demo` after repairing the deliberately bad domain refuses to overwrite that successful configuration; `demo` owns the explicit wrong-TXT verification check on fresh fixture data. Never delete data belonging to another installation.

The fixture is self-contained and designed for a Linux GitHub-hosted runner with Docker: build the WASM distribution, run the same setup/protocol commands, run the native Cargo process suite directly, and always run Compose cleanup. The checked-in workflows reuse this fixture; their presence does not establish an executed hosted result. It reserves host ports 3334, 8026 and 8443 and subnet `172.30.53.0/24`; run one fixture per host. Hosted jobs on separate VMs are isolated, while concurrent jobs sharing a self-hosted Docker daemon need separate port/subnet configuration. Cache Cargo outputs separately from fixture secrets/data. Do not upload the whole `data/` directory.

For a containerized Node test client on the `ruphish-acceptance_lab` network, set `RUPHISH_ACCEPTANCE_ADMIN=http://app:3333`, `RUPHISH_ACCEPTANCE_INBOX=http://inbox:8025` and `RUPHISH_ACCEPTANCE_PUBLIC_IP=172.30.53.10`, mount this product directory at `/workspace` and run the same test from there. These overrides change connection addresses only; the public origin and TLS hostname remain `good.ruphish.test`. Host-run Node needs no overrides.
