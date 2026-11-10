---
title: Local acceptance fixture
description: Start and inspect the controlled Docker environment for DNS, HTTPS, SMTP, and participant tests.
sidebar:
  order: 320
---

This is the single Docker test environment for domain verification, public interactions and SMTP acceptance. Keep its admin address on port 3334 and public HTTPS on 8443. Extend this fixture when scenarios change; use fresh data in CI instead of maintaining another lab definition.

This fixture runs a separate Ruphish installation, authoritative test DNS, trusted test HTTPS and a Mailpit inbox. It uses only `*.test` recipients. The seeded SMTP profile points only at the local inbox, SMTP has no host port or relay configuration, and web ports bind to host loopback. The bridge network is not an outbound firewall; keep these fixture resources pointed at the supplied test services. No host DNS, certificate store or production database is changed.

The inbox requires STARTTLS and uses the generated lab server certificate. Only the synthetic **Local inbox only** sending profile enables **Ignore SMTP certificate errors**, because this isolated inbox uses a private lab certificate. SMTP protocol regressions separately verify certificate trust, hostname and expiry checks. For an existing lab created before STARTTLS support, recreate the inbox container with Compose and enable that exception on this named fixture profile before running `seed`; the seeder preserves existing resources and reports incompatible settings instead of changing them.

## Start the fixture

Requirements: Docker with Linux containers and Compose v2, the product Node version from development setup, and the [development toolchain](../development-setup/index.md). Run commands from the product root. Build the admin distribution first if `dist/admin/` is absent or stale: `npm ci --prefix crates/ruphish-ui/web`, then `trunk build --locked` (PowerShell: set `$env:NO_COLOR = 'true'` first).

```sh
node tests/acceptance/lab.mjs init
docker compose -f tests/acceptance/compose.yaml --profile setup run --rm prepare
docker compose -f tests/acceptance/compose.yaml build app
node --test tests/acceptance/image.test.mjs
docker compose -f tests/acceptance/compose.yaml up -d --wait dns inbox app
node tests/acceptance/lab.mjs demo
node tests/acceptance/lab.mjs seed
node --test tests/acceptance/public.test.mjs
```

`prepare` compiles the Linux executable with `embedded-assets` in the Rust builder container using Cargo.lock, exports it to `target/docker/ruphish`, and generates or reuses the local CA/server certificate. The Dockerfile packages that executable as `ruphish:local`; the running app needs neither a source mount nor loose UI files. Building needs network access for toolchains, crates and runtime packages; acceptance scenarios use local services only. `demo` uses normal login/password-change APIs and actual DNS/TLS verification: `https://good.ruphish.test:8443` must pass and `https://bad.ruphish.test:8443` must fail with a TXT mismatch. `seed` preserves existing named resources and creates a new browser campaign each time. The protocol tests check unauthenticated health on both listeners, rejected probes, received SMTP content, public events, duplicate submissions, malformed bodies, completion, absence of canary values from API responses/logs/live SQLite files, and persistence across container replacement. They do not execute the browser collector.

For optimized artifact verification, build the UI with `trunk build --release --locked` and invoke `prepare --release` instead of `prepare`. Both use the same artifact path and runtime image. If a verified executable or image is already supplied, `prepare --certificates-only` prepares the fixture without rebuilding or replacing that artifact.

`image.test.mjs` checks the built image's default configuration, exact executable hash, version, notices, embedded assets and compression, non-root operation on a read-only root filesystem, and persistence across replacement. It uses a temporary named volume and no network access or published ports, then removes its own containers and volume.

To verify database selection through environment variables, prepare and start one canonical provider at a time. These tests create disposable databases/accounts, connect with hostname-verified TLS, check persisted accounts after container replacement, and clean up their resources. They do not alter the application's default SQLite installation:

```sh
docker compose -f tests/acceptance/compose.yaml run --rm prepare --mysql
docker compose -f tests/acceptance/compose.yaml --profile database up -d --wait mysql
node --test --test-name-pattern='^mysql ' tests/acceptance/database-image.test.mjs
docker compose -f tests/acceptance/compose.yaml stop mysql
docker compose -f tests/acceptance/compose.yaml rm -f mysql
docker compose -f tests/acceptance/compose.yaml run --rm prepare --postgres
docker compose -f tests/acceptance/compose.yaml --profile database up -d --wait postgres
node --test --test-name-pattern='^postgres ' tests/acceptance/database-image.test.mjs
docker compose -f tests/acceptance/compose.yaml stop postgres
docker compose -f tests/acceptance/compose.yaml rm -f postgres
```

Provider certificates for these tests must include the Compose service hostname (`mysql` or `postgres`) in addition to `localhost`. Use fresh isolated fixture state for older local certificates that lack these names; preparation preserves existing certificates.

To verify with disposable fixture state while preserving the existing lab, set `RUPHISH_ACCEPTANCE_DATA_DIRECTORY` to an absolute directory before running `init` and keep it set for all Node and Compose commands. This changes only the state mount; the canonical services, network and ports remain the same. Stop any existing fixture first and run only one instance at a time. Omit the variable to continue using `tests/acceptance/data/`.

`init` creates `target/docker/` as the host user before any Docker command. This keeps the parent Cargo output directory writable by native tests on Linux; Compose refuses to create a missing artifact bind source as root. If the directory was removed after initialization, recreate it as the host user with `mkdir -p target/docker` (PowerShell: `New-Item -ItemType Directory -Force target/docker`) before running `prepare` again.

| Surface | Address |
| --- | --- |
| Admin | `http://127.0.0.1:3334` |
| Local inbox | `http://127.0.0.1:8026` |
| Public passing origin | `https://good.ruphish.test:8443` |
| Public failing origin | `https://bad.ruphish.test:8443` |

Log in as `admin`; retrieve the fixture password with `node tests/acceptance/lab.mjs password`. This is a separate account/database. Its generated secrets, certificate keys and database are ignored under `tests/acceptance/data/`; the exported binary is ignored under `target/docker/`. Do not publish the data directory or raw Mailpit messages as CI artifacts. If you change the admin password/API key manually, update the local bootstrap file before using fixture commands again.

## What the fixture builds

The root Dockerfile packages a prebuilt executable with embedded UI. `prepare` builds a development executable, and `prepare --release` selects optimized code. `prepare --certificates-only` leaves a supplied binary/image intact when verifying release artifacts.

The fixture chooses its own configuration and persistent state mount. It is separate from your ordinary development database. See [Docker hosting](../../administration/hosting-and-maintenance/run-with-docker-compose.md) for production image configuration and [monitoring](../../administration/hosting-and-maintenance/health-and-logs.md) for probes.

Optional Compose build inputs are `RUPHISH_APP_IMAGE` (default `ruphish:local`), `RUPHISH_IMAGE_VERSION` (default `local-development`), and `RUPHISH_IMAGE_REVISION` (default `unknown`). The latter two set image labels, not the compiled application version. These local commands do not publish an image.

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

Follow the separate [manual UI checklist](manual-browser-checks.md) for participant-browser behavior. Protocol acceptance does not execute the browser collector.

## Repeat and stop

Local manual testing retains the fixture database between runs. The [validation workflow](../../../.github/workflows/validation.yml) uses this same definition with a fresh checkout and disposable runner state. The [validation policy](validation-commands.md) places this suite on fixture changes and full nightly/release source acceptance. Production uses the built application with deployment-specific configuration, real services and separately managed secrets/storage. Test accounts, local certificates, CoreDNS and Mailpit are test dependencies and must not be promoted into production. Deployment is separate from this test environment.

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

Fresh fixture configurations use stderr-only application logging and the Compose app service uses Docker's `local` driver with `max-size: "10m"` and `max-file: "5"`. Existing running containers and their configuration files are not changed by source edits. See [logging and retention](../../administration/hosting-and-maintenance/health-and-logs.md) before recreating a fixture or opting into application file logs.
