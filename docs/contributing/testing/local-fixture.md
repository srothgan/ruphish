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
docker compose -f tests/acceptance/compose.yaml up -d dns inbox app
node tests/acceptance/lab.mjs demo
node tests/acceptance/lab.mjs seed
node --test tests/acceptance/public.test.mjs
```

`prepare` builds the real Linux executable using Cargo.lock and generates a local CA/server certificate. It needs network access to obtain the toolchain and crates; the acceptance scenarios use local services only. `demo` uses normal login/password-change APIs and actual DNS/TLS verification: `https://good.ruphish.test:8443` must pass and `https://bad.ruphish.test:8443` must fail with a TXT mismatch. `seed` preserves existing named resources and creates a new browser campaign each time. The protocol test creates its own campaign and synthetic recipient, checks received SMTP content, public events, duplicate submissions, malformed bodies, completion and absence of canary values from API responses, logs and live SQLite files. It does not execute the browser collector.

| Surface | Address |
| --- | --- |
| Admin | `http://127.0.0.1:3334` |
| Local inbox | `http://127.0.0.1:8026` |
| Public passing origin | `https://good.ruphish.test:8443` |
| Public failing origin | `https://bad.ruphish.test:8443` |

Log in as `admin`; retrieve the fixture password with `node tests/acceptance/lab.mjs password`. This is a separate account/database. Its generated secrets, certificate keys, binary and database are ignored under `tests/acceptance/data/`. Do not publish that directory or raw Mailpit messages as CI artifacts. If you change the admin password/API key manually, update the local bootstrap file before using fixture commands again.

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
docker compose -f tests/acceptance/compose.yaml up -d app
node --test tests/acceptance/public.test.mjs
docker compose -f tests/acceptance/compose.yaml down
```

Stop `app` before rebuilding its binary or promoting new UI assets. `down` preserves the database and keys. Mailpit's inbox is ephemeral; each protocol run sends its own uniquely correlated email. Re-running `demo` after repairing the deliberately bad domain refuses to overwrite that successful configuration. Use fresh fixture data for a fresh failing example; never delete data belonging to another installation.

The fixture is self-contained and designed for a Linux GitHub-hosted runner with Docker: build the WASM distribution, run the same setup/protocol commands, run the native Cargo process suite directly, and always run Compose cleanup. The checked-in workflows reuse this fixture; their presence does not establish an executed hosted result. It reserves host ports 3334, 8026 and 8443 and subnet `172.30.53.0/24`; run one fixture per host. Hosted jobs on separate VMs are isolated, while concurrent jobs sharing a self-hosted Docker daemon need separate port/subnet configuration. Cache Cargo outputs separately from fixture secrets/data. Do not upload the whole `data/` directory.

For a containerized Node test client on the `ruphish-acceptance_lab` network, set `RUPHISH_ACCEPTANCE_ADMIN=http://app:3333`, `RUPHISH_ACCEPTANCE_INBOX=http://inbox:8025` and `RUPHISH_ACCEPTANCE_PUBLIC_IP=172.30.53.10`, mount this product directory at `/workspace` and run the same test from there. These overrides change connection addresses only; the public origin and TLS hostname remain `good.ruphish.test`. Host-run Node needs no overrides.
