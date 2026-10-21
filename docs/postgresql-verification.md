# PostgreSQL provider verification

Use the `postgres` service in the canonical `tests/acceptance/compose.yaml` fixture. Its storage is disposable tmpfs; it does not create another application lab or operator login. A synthetic administrator connection creates random test databases and restricted service roles. All application operations under test use the public facade or real executable; independent SQL is limited to setup, inspection and fault injection.

Prepare certificates and start the pinned PostgreSQL 18.6 service:

```powershell
docker compose -f tests/acceptance/compose.yaml run --rm prepare --postgres
docker compose -f tests/acceptance/compose.yaml --profile database up -d --wait postgres
$endpoint = docker compose -f tests/acceptance/compose.yaml port postgres 5432
$env:RUPHISH_POSTGRES_TEST_PORT = ($endpoint.Trim() -split ':')[-1]
cargo test -p ruphish-adapters --test database_providers postgres_ --locked -- --ignored --nocapture
cargo test -p ruphish --test startup_process postgres_process --locked -- --ignored --nocapture
```

Build the real admin assets before executable tests, following AGENTS.md. The default image is `postgres@sha256:afc7e2d441324c0388fa80c3d24f733b4194a4eb7f47dd8ee2b08eb1a24a647c`. Test the PostgreSQL 17.11 floor by replacing only that same disposable service:

```powershell
$env:RUPHISH_POSTGRES_IMAGE = 'postgres@sha256:3645570cccdfa447589da9f57dd740faa29b30938e861289a5574b6ca6b03826'
docker compose -f tests/acceptance/compose.yaml --profile database up -d --wait --force-recreate postgres
$endpoint = docker compose -f tests/acceptance/compose.yaml port postgres 5432
$env:RUPHISH_POSTGRES_TEST_PORT = ($endpoint.Trim() -split ':')[-1]
cargo test -p ruphish-adapters --test database_providers postgres_ --locked -- --ignored --nocapture
cargo test -p ruphish --test startup_process postgres_process --locked -- --ignored --nocapture
```

The common provider workflow checks resources, authorization, session revocation, campaign review/create/idempotency, durable delivery claims, domain eligibility, public submissions, analysis, overview and integration state. PostgreSQL-specific tests challenge concurrent startup, partial writes, pool recovery, exact nanosecond revisions, NUL rejection, TLS/authentication failures, schema/history drift, transactional DDL failure, migration-lock timeout/cancellation and lost commit acknowledgement. The acknowledgement test forwards PostgreSQL wire frames through a loopback plaintext fault proxy and drops a confirmed COMMIT response; normal fixtures use verified TLS. It proves persisted state survives and a stale retry cannot duplicate the edit.

The executable workflow is shared with SQLite and MySQL. It creates resources and a campaign through HTTP, verifies domains against controlled DNS/TLS, races two application processes on an idempotent launch, receives personalized SMTP messages, and checks presence-only interactions, raw/unique/cumulative statistics, filtering, pagination and decoded Excel exports. It injects failures at eight campaign/outbox write boundaries and during submission publication, verifying rollback and retry with the same identity. Signed webhook retries and IMAP acknowledgement failures survive application restart. A sustained database transport outage must return bounded server errors, retain the application process, and recover HTTP, domain verification and SMTP work without a restart. An SMTP acceptance whose persistence repeatedly fails becomes uncertain after crash recovery, requires explicit reconciliation, and must never cause an automatic resend. The fixture checks cross-process session revocation and secret canaries in logs, exports and webhook payloads.

Separate facade tests drop real COMMIT acknowledgements after campaign creation and delivery acceptance. They verify typed uncertainty, idempotent launch reconciliation and absence of duplicate delivery/events. Ordinary Cargo tests report server-dependent cases as ignored; ignored cases are not evidence of server verification. Invalid provider configuration cases run in the ordinary executable preflight suite.

Run Linux Rust against the same database service with disposable containers and the existing cache volumes:

```powershell
$fixtureContainer = (docker compose -f tests/acceptance/compose.yaml ps -q postgres).Trim()
$workspacePath = (Get-Location).Path
docker run --rm --network "container:$fixtureContainer" --mount "type=bind,source=$workspacePath,target=/workspace,readonly" --mount type=volume,source=ruphish-acceptance-target,target=/target --mount type=volume,source=ruphish-acceptance-registry,target=/usr/local/cargo/registry --workdir /workspace -e CARGO_TARGET_DIR=/target -e RUPHISH_POSTGRES_TEST_PORT=5432 -e NO_COLOR=true rust:1.99.1-bookworm cargo test -p ruphish-adapters --test database_providers postgres_ --locked -- --ignored --nocapture
docker run --rm --network "container:$fixtureContainer" --mount "type=bind,source=$workspacePath,target=/workspace,readonly" --mount type=volume,source=ruphish-acceptance-target,target=/target --mount type=volume,source=ruphish-acceptance-registry,target=/usr/local/cargo/registry --workdir /workspace -e CARGO_TARGET_DIR=/target -e RUPHISH_POSTGRES_TEST_PORT=5432 -e NO_COLOR=true rust:1.99.1-bookworm cargo test -p ruphish --test startup_process postgres_process --locked -- --ignored --nocapture
```

Stop/remove only the service created for verification, preserving application-lab state:

```powershell
docker compose -f tests/acceptance/compose.yaml stop postgres
docker compose -f tests/acceptance/compose.yaml rm -f postgres
Remove-Item Env:RUPHISH_POSTGRES_IMAGE -ErrorAction SilentlyContinue
Remove-Item Env:RUPHISH_POSTGRES_TEST_PORT -ErrorAction SilentlyContinue
```

The fixture commands also work in isolated CI jobs; no CI workflow or deployment is introduced. PostgreSQL server-major upgrades, operator backup restoration, cross-engine transfer and deployed acceptance are separate from application migration tests.

Additional provider regressions cover exact 255-character ASCII, accented and four-byte Unicode text, trailing spaces within the bound, and atomic rejection of overflows including whitespace-only excess. They assert save/read equality and preservation of attachments and revisions after rejection. Coordinated account-deletion/verification tests assert owner-lock ordering, absence of orphan claims and hostname reuse. Lock-timeout tests assert typed facade failures through accounts, resources, domains and previews, followed by successful operations after release.
