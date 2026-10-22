# MySQL provider verification

This is the full provider acceptance catalog. Use the [validation policy](validation.md) to select focused checks after ordinary changes; full historical/version/executable runs belong to migration changes, nightly or explicit acceptance. Run heavy workloads sequentially. On first use, initialize the canonical fixture with `node tests/acceptance/lab.mjs init` before the setup commands below.

These checks exercise fresh initialization, legacy upgrades, runtime facade operations and the actual executable. The same `tests/acceptance/compose.yaml` fixture owns the MySQL service; it does not introduce another application lab or administrator account. Its MySQL root password is a fixed synthetic fixture credential and its database storage is disposable tmpfs. Independent connections only create, corrupt, inspect and drop randomly named test databases. Production and functional operations enter through the facade or HTTP.

Prepare fixture certificates, then start only its database service:

```powershell
docker compose -f tests/acceptance/compose.yaml run --rm prepare --mysql
docker compose -f tests/acceptance/compose.yaml --profile database up -d --wait mysql
$endpoint = docker compose -f tests/acceptance/compose.yaml port mysql 3306
$env:RUPHISH_MYSQL_TEST_PORT = ($endpoint.Trim() -split ':')[-1]
cargo test -p ruphish-adapters --test mysql_preflight --locked -- --ignored --nocapture --test-threads=1
cargo test -p ruphish-adapters --test database_providers mysql_ --locked -- --ignored --nocapture --test-threads=1
cargo test -p ruphish --test startup_process mysql_process --locked -- --ignored --nocapture --test-threads=1
```

The default pinned image is MySQL 9.7.2. To test the 8.4 compatibility floor, replace only that same disposable service:

```powershell
$env:RUPHISH_MYSQL_IMAGE = 'mysql@sha256:0f51e63e537a1f176423fa03ef559aabe6fa4372a53160a44bc7e75da7d15364'
docker compose -f tests/acceptance/compose.yaml --profile database up -d --wait --force-recreate mysql
$endpoint = docker compose -f tests/acceptance/compose.yaml port mysql 3306
$env:RUPHISH_MYSQL_TEST_PORT = ($endpoint.Trim() -split ':')[-1]
cargo test -p ruphish-adapters --test mysql_preflight --locked -- --ignored --nocapture --test-threads=1
cargo test -p ruphish-adapters --test database_providers mysql_ --locked -- --ignored --nocapture --test-threads=1
cargo test -p ruphish --test startup_process mysql_process --locked -- --ignored --nocapture --test-threads=1
```

The 8.4 image resolves to MySQL 8.4.11. The 9.7 default digest is `sha256:e2fa358a74ac81148e7e27d9433e4c0ab62133de8ea4428b4f39a51a4ba7d07f`. Build the real admin assets before executable tests, following AGENTS.md. Explicit server tests fail when their environment is unavailable; ordinary Cargo tests report the opt-in cases as ignored. Ignored results are not MySQL verification.

The transport preflight independently verifies CA rejection, incorrect credentials, missing databases and bounded migration ownership. The full preflight target also verifies CA/hostname checks, incorrect credentials, missing databases, bounded migration ownership, cancellation, all 28 original legacy histories and malformed schemas. The provider suite exercises concurrent startup, independent provider handles, sessions, authorization, stale edits, duplicate creation, atomic child replacement, administrator races, campaign idempotency, delivery leases, uncertain outcomes, public submission deduplication, analysis, overview and integration state. It upgrades and reopens all 28 legacy versions, verifies preserved hashes/identities and private active data, challenges recovery after failed publication, and rejects dirty/modified history, altered collations, indexes and table engines. Legacy queue cases cover ambiguous associations and in-flight work; reporting service credentials remain usable.

The executable workflow is shared with SQLite and PostgreSQL. It creates resources and a campaign through HTTP, verifies domains against controlled DNS/TLS, races two application processes on an idempotent launch, receives personalized SMTP messages, and checks presence-only interactions, raw/unique/cumulative statistics, filtering, pagination and decoded Excel exports. It injects failures at eight campaign/outbox write boundaries and during submission publication, verifying rollback and retry with the same identity. Signed webhook retries and IMAP acknowledgement failures survive application restart. A sustained database transport outage must return bounded server errors, retain the application process, and recover HTTP, domain verification and SMTP work without a restart. An SMTP acceptance whose persistence repeatedly fails becomes uncertain after crash recovery, requires explicit reconciliation, and must never cause an automatic resend. The fixture checks cross-process session revocation and secret canaries in logs, exports and webhook payloads.

A separate facade test drops MySQL COMMIT OK packets after campaign creation and delivery acceptance. It verifies typed uncertainty, idempotent launch reconciliation and absence of duplicate delivery/events. This protocol fault uses explicitly disabled TLS only within its isolated loopback fixture; ordinary server workflows retain verified TLS. These tests establish concrete covered guarantees; broader deployed acceptance remains separate work.

To exercise Linux Rust as well as a Linux database server, run disposable build/test containers against that same fixture. These commands reuse its existing cache volumes and share the MySQL network namespace so the same verified loopback certificate works:

```powershell
$fixtureContainer = (docker compose -f tests/acceptance/compose.yaml ps -q mysql).Trim()
$workspacePath = (Get-Location).Path
docker run --rm --network "container:$fixtureContainer" --mount "type=bind,source=$workspacePath,target=/workspace,readonly" --mount type=volume,source=ruphish-acceptance-target,target=/target --mount type=volume,source=ruphish-acceptance-registry,target=/usr/local/cargo/registry --workdir /workspace -e CARGO_TARGET_DIR=/target -e RUPHISH_MYSQL_TEST_PORT=3306 -e NO_COLOR=true rust:1.99.1-bookworm cargo test -p ruphish-adapters --test database_providers mysql_ --locked -- --ignored --nocapture --test-threads=1
docker run --rm --network "container:$fixtureContainer" --mount "type=bind,source=$workspacePath,target=/workspace,readonly" --mount type=volume,source=ruphish-acceptance-target,target=/target --mount type=volume,source=ruphish-acceptance-registry,target=/usr/local/cargo/registry --workdir /workspace -e CARGO_TARGET_DIR=/target -e RUPHISH_MYSQL_TEST_PORT=3306 -e NO_COLOR=true rust:1.99.1-bookworm cargo test -p ruphish --test startup_process mysql_process --locked -- --ignored --nocapture --test-threads=1
```

Stop/remove only the database service after verification; do not tear down the application lab:

```powershell
docker compose -f tests/acceptance/compose.yaml stop mysql
docker compose -f tests/acceptance/compose.yaml rm -f mysql
Remove-Item Env:RUPHISH_MYSQL_IMAGE -ErrorAction SilentlyContinue
Remove-Item Env:RUPHISH_MYSQL_TEST_PORT -ErrorAction SilentlyContinue
```

The same direct Docker/Cargo commands work in CI with an isolated fixture workspace; the [validation workflow](../.github/workflows/validation.yml) selects the appropriate scope. On POSIX shells, obtain the published port with `docker compose ... port mysql 3306` and export `RUPHISH_MYSQL_TEST_PORT` accordingly.

## Preservation, races and schema metadata

The provider suite also checks unsafe server SQL-mode defaults, exact Unicode/trailing-space round trips, rejected replacements and creations, and typed lock-timeout errors across four facade capabilities. Coordinated account-deletion tests pause deletion after hostname-claim cleanup and prove domain claiming/completion cannot recreate orphan records; another account must subsequently verify the same hostname successfully. These tests use isolated databases and synthetic data. The SQL-mode test restores the server default immediately after opening the application connections; run separate invocations of that test sequentially against a shared fixture service.

Startup compares live columns, defaults, auto-increment/generated attributes, index definitions, check expressions and enforcement with `crates/ruphish-adapters/src/mysql/schema.json`. This metadata is generated after applying all ordered migrations in `migrations/mysql/`, using `src/mysql/schema_inventory.sql`, rather than a handwritten second schema. The generation connection must use `SET NAMES utf8mb4 COLLATE utf8mb4_0900_bin`, matching application connections. Exclude database names and data-dependent auto-increment counters. Regenerate only after an intentional schema migration, from a fresh disposable schema, and validate on both supported MySQL versions. The regression suite alters defaults, auto-increment, generated expressions, checks, keys, engine and collation; startup must reject each alteration and accept restoration without losing existing data.
