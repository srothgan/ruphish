<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/ruphish-wordmark-white.svg">
  <img alt="Ruphish" src="assets/ruphish-wordmark.svg" width="320">
</picture>

[![Version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fsrothgan%2Fruphish%2Fmain%2FCargo.toml&query=%24.workspace.package.version&label=version&color=blue)](https://github.com/srothgan/ruphish/blob/main/Cargo.toml)
[![CI](https://github.com/srothgan/ruphish/actions/workflows/pr.yml/badge.svg?branch=main&event=push)](https://github.com/srothgan/ruphish/actions/workflows/pr.yml?query=branch%3Amain+event%3Apush)
[![Docs](https://img.shields.io/badge/docs-website-blueviolet)](https://ruphish.com/docs/)
[![License](https://img.shields.io/badge/license-MIT-green)](https://github.com/srothgan/ruphish/blob/main/LICENSE)

Ruphish is a phishing-simulation application with a native Rust server and a Rust/WASM admin interface. Prepare personalized campaigns, deliver them through your SMTP service, and review delivery, participant interactions, and reported messages in one place.

**[Try the demo](https://demo.ruphish.com/)** with username `demo` and password `demodemo`.

Ruphish is a successor to Gophish with its own API, a new interface, persistent campaign drafts, verified domains, richer analysis, and explicit delivery recovery. Supported Gophish SQLite and MySQL databases migrate directly to Ruphish, preserving supported data and authored Go-template behavior. Adapt automation to the documented Ruphish API and review the [documented differences](https://ruphish.com/docs/migration/differences-from-gophish/) before migrating.

![Ruphish dashboard showing campaign outcomes, recent campaigns, and deliveries needing review.](assets/dashboard.png)

## What you can do

- Manage recipient groups, email templates, landing pages, attachments, and sending profiles.
- Save campaign drafts, review the audience and content, and launch immediately or schedule delivery.
- Inspect delivery attempts and resolve uncertain outcomes without assuming a failed connection means a message was never sent.
- Explore campaign and recipient timelines, distinguish observed events from summary metrics, and export CSV or XLSX reports.
- Process reported messages through multiple IMAP mailboxes and deliver signed events to webhooks.
- Use English or German interface text with independent date-format and time-zone preferences.
- Automate workflows through Ruphish's HTTP API.

Participant tracking is designed around interaction events and optional presence booleans. Entered usernames, passwords, OTPs, and arbitrary form values are outside the supported collection model. Read [participant data](https://ruphish.com/docs/user-guide/participant-data/) for behavior and verification limits.

## Start here

| Your goal | Guide |
| --- | --- |
| Try a contained exercise with a local inbox | [Your first campaign](https://ruphish.com/docs/getting-started/first-campaign/) |
| Install a release binary or run the Docker image | [Installation](https://ruphish.com/docs/getting-started/installation/) |
| Run campaigns and understand results | [User guide](https://ruphish.com/docs/user-guide/) |
| Configure databases, TLS, accounts, or recovery | [Administration](https://ruphish.com/docs/administration/) |
| Integrate with Ruphish | [HTTP API](https://ruphish.com/docs/api/) |
| Bring an existing Gophish installation across | [Migration and compatibility](https://ruphish.com/docs/migration/) |
| Change code, translations, or documentation | [Contributing](CONTRIBUTING.md) |
| Report a suspected security vulnerability | [Security policy](SECURITY.md) |

The HTTP interface separates `/api/` for public automation, `/auth/` for browser authentication/session data, and `/internal/` for browser-specific operations. See the [API overview](https://ruphish.com/docs/api/) before choosing an endpoint for an integration.

## Installation

Native installers download a release binary with the administration interface included. By default, they select the latest full release.

> The website installer URLs below are pending deployment and verification. Published archives are available from [GitHub Releases](https://github.com/srothgan/ruphish/releases).

Linux or macOS:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://www.ruphish.com/install.ps1 | iex
```

To include development previews alongside full releases, use:

```sh
curl -fsSL https://www.ruphish.com/install.sh | RUPHISH_INCLUDE_PREVIEWS=1 sh
```

```powershell
$env:RUPHISH_INCLUDE_PREVIEWS = '1'
irm https://www.ruphish.com/install.ps1 | iex
```

Until a full release is published, use the preview option; the default reports that no full release is available. Previews are intended for disposable installations, and upgrades are not guaranteed.

The PowerShell setting lasts for the current session; clear it with `Remove-Item Env:RUPHISH_INCLUDE_PREVIEWS` to return to full-release selection. See the [installer options](https://www.ruphish.com/docs/getting-started/installation/#installer-options-and-environment) for environment settings and explicit overrides.

The installer prints the installed executable's path and does not start the server automatically. See the [installation guide](https://ruphish.com/docs/getting-started/installation/) for supported platforms, first startup, Docker, updates, and removal. SQLite is the default database; see [database administration](https://ruphish.com/docs/administration/databases/) for MySQL and PostgreSQL requirements.

For development, follow [contributing](CONTRIBUTING.md) for setup, architecture, and validation.

Read the [changelog](CHANGELOG.md) for user-visible changes and the [Code of Conduct](CODE_OF_CONDUCT.md) for community expectations and private conduct reporting.

## License and attribution

Ruphish is licensed under [MIT](LICENSE), copyright (c) 2026 Simon Peter Rothgang. Reused Gophish material and dependency notices are preserved in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). Fixture credentials and recipient data are synthetic and must not be reused for an actual installation.
