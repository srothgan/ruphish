# Local public-interaction acceptance

This is the single Docker test environment for domain verification, public interactions and SMTP acceptance. Keep its admin address on port 3334 and public HTTPS on 8443. Extend this fixture when scenarios change; use fresh data in CI instead of maintaining another lab definition.

This fixture runs a separate Ruphish installation, authoritative test DNS, trusted test HTTPS and a Mailpit inbox. It uses only `*.test` recipients. The seeded SMTP profile points only at the local inbox, SMTP has no host port or relay configuration, and web ports bind to host loopback. The bridge network is not an outbound firewall; keep these fixture resources pointed at the supplied test services. No host DNS, certificate store or production database is changed.

## Start the fixture

Requirements: Docker with Linux containers and Compose v2, Node with native Fetch and `Headers.getSetCookie()`, and the toolchain documented in AGENTS.md. Run commands from the product root. Build the admin distribution first if `dist/admin/` is absent or stale: `npm ci --prefix crates/ruphish-ui/web`, then `trunk build --release --locked` (PowerShell: set `$env:NO_COLOR = 'true'` first).

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

## Manual browser acceptance

Keep the admin campaign view open alongside DevTools Network on the public page. Use invented canaries such as `LOCAL-USERNAME-CANARY` and `LOCAL-PASSWORD-CANARY`; never use real login information. Create a fresh campaign with `seed` when a case requires a clean recipient/session.

1. Open the browser campaign email in Mailpit. Allow its external image if prompted. Confirm one `Email Opened` event only when the tracking image is fetched; opening Mailpit's message view alone is not evidence of a fetch. Follow the practice link and confirm the greeting is personalized and `Clicked Link` appears for the same recipient.
2. Inspect the public document's CSP and DOM. Native form submissions must be blocked by `form-action 'none'`; the authored script must be absent, and `document.body.dataset.authoredScriptExecuted` must be undefined. Ordinary names, IDs and autocomplete attributes should remain usable.
3. Submit with the password empty, then use a fresh page load and submit with a synthetic password. The sole submission request body must contain `event_id` and boolean `password_present`, with no usernames, password values, field maps, lengths or hashes. The corresponding history must show only submission/presence metadata.
4. Test a password manager with a synthetic saved entry and the external password control below the form. Confirm filling only that external control still reports presence. Check the network body again; do not infer privacy from the UI text.
5. Block the public submission request in DevTools, submit, then unblock and retry. The page must show an unconfirmed/error state, recover, and reuse the original event ID. For a true timeout, use a request-stalling proxy; DevTools Offline usually produces immediate failure, which does not prove the 15-second deadline. Timeout/retry logic also has isolated functional tests.
6. Disable JavaScript and reload the public page. Attempt submission: no form data may leave through native navigation. Re-enable JavaScript afterward. Check console CSP failures as well as Network; an error alone does not prove that no data was sent.
7. Disable the domain in admin, then try another page load/submission. It must fail without new events. Re-enable and verify as needed; complete the campaign and confirm later submissions likewise fail. Use a fresh campaign for subsequent cases.
8. Inspect campaign responses/history and `tests/acceptance/data/ruphish.log` for your synthetic canaries. The automated test additionally scans database/WAL bytes for its own rejected-payload canary. Record browser/version, case, expected/actual result and any failure; do not attach real credentials or unredacted admin tokens.

## Repeat, stop and future CI

Local manual testing retains the fixture database between runs. Future GitHub Actions jobs should use this same definition with a fresh checkout and disposable data, run the tests, then remove their containers and test state. Production uses the built application with deployment-specific configuration, real services and separately managed secrets/storage. Test accounts, local certificates, CoreDNS and Mailpit are test dependencies and must not be promoted into production. GitHub environments can later gate deployment credentials and approvals; they do not require a second copy of this test suite.

```sh
docker compose -f tests/acceptance/compose.yaml logs --tail 60 app dns inbox
docker compose -f tests/acceptance/compose.yaml stop app
docker compose -f tests/acceptance/compose.yaml --profile setup run --rm prepare
docker compose -f tests/acceptance/compose.yaml up -d app
node --test tests/acceptance/public.test.mjs
docker compose -f tests/acceptance/compose.yaml down
```

Stop `app` before rebuilding its binary or promoting new UI assets. `down` preserves the database and keys. Mailpit's inbox is ephemeral; each protocol run sends its own uniquely correlated email. Re-running `demo` after repairing the deliberately bad domain refuses to overwrite that successful configuration. Use fresh fixture data for a fresh failing example; never delete data belonging to another installation.

The fixture is self-contained and designed for a Linux GitHub-hosted runner with Docker: build the WASM distribution, run the same setup/protocol commands, run the native Cargo process suite directly, and always run Compose cleanup. No workflow has been added or executed on GitHub. It reserves host ports 3334, 8026 and 8443 and subnet `172.30.53.0/24`; run one fixture per host. Hosted jobs on separate VMs are isolated, while concurrent jobs sharing a self-hosted Docker daemon need separate port/subnet configuration. Cache Cargo outputs separately from fixture secrets/data. Do not upload the whole `data/` directory.

For a containerized Node test client on the `ruphish-acceptance_lab` network, set `RUPHISH_ACCEPTANCE_ADMIN=http://app:3333`, `RUPHISH_ACCEPTANCE_INBOX=http://inbox:8025` and `RUPHISH_ACCEPTANCE_PUBLIC_IP=172.30.53.10`, mount this product directory at `/workspace` and run the same test from there. These overrides change connection addresses only; the public origin and TLS hostname remain `good.ruphish.test`. Host-run Node needs no overrides.
