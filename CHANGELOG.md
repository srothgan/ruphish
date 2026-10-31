# Changelog

Development previews have short notes on GitHub Releases. This changelog records user-facing changes for final releases.

## Unreleased

Initial release of a Gophish successor with a native Rust server, a Rust/WASM administration interface, and independent API contracts. Feature coverage follows the documented differences and privacy boundaries.

- Let regular users manage their own webhooks and reporting mailboxes under administrator feature switches; reject configuration saves while a feature is disabled.
- Initialize reporting mailboxes from active campaigns' actual sends, match reports across campaign creators, and ignore new reports after campaign completion while preserving polling progress.
- Record coarse browser, OS, and device categories on clicks and submissions, with per-event recipient details and consistent exports/webhooks; exclude raw headers and network identities.
- Manage recipient groups, email templates, landing pages, reusable static learning pages, attachments, and sending profiles.
- Select hosted learning pages as campaign redirect destinations without adding training-completion tracking.
- Choose campaign email-open tracking before launch, with disabled metrics shown as not tracked and enabled defaults for migrated campaigns.
- Save campaign drafts, review campaigns before launch, and schedule delivery with explicit recovery for uncertain sends.
- Review dashboards and campaign/recipient timelines, with CSV and XLSX exports.
- Use fixed DNS, IPv4, or IPv6 campaign origins over HTTP or HTTPS; required verification remains restricted to DNS hostnames over HTTPS.
- Verify campaign domains, process reports through multiple IMAP mailboxes, and deliver signed webhooks.
- Use English or German interface text with independent date-format and time-zone preferences.
- Run with SQLite, MySQL, or PostgreSQL, with migration support for recognized Gophish SQLite and MySQL histories.
- Deploy with Docker Compose and environment-based configuration, or use a standalone native executable with the administration UI embedded.
- Automate campaign workflows through documented Ruphish contracts, with separate `/api/`, `/auth/`, and `/internal/` interfaces.
- Track participant interactions and password presence without collecting entered credentials or other form values through the built-in collector.

See [differences from Gophish](https://ruphish.com/docs/migration/differences-from-gophish/) for intentional changes and migration considerations.
