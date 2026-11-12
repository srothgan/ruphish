# Changelog

Development previews have short notes on GitHub Releases. This changelog records user-facing changes for final releases.

## Unreleased

- Improve SQLite attachment and mail-header lookups, align role and permission uniqueness across database providers, and make PostgreSQL learning-page text comparisons independent of the database's default collation.
- Keep mailbox checkpoint recovery available after later connection failures, restarts, and ordinary configuration edits; expose its persistent state as `reset_required` in the API.
- Protect an untouched campaign draft from silent navigation after a lost creation response, and reconcile retries using the original creation identity and confirmed save checkpoint.
- Dismiss operation feedback with a close control; successful messages disappear after five seconds. Keep company-settings approval separate from save progress and results, and make back navigation more prominent.
- Use consistent English and German connection-test wording for sending profiles and reporting mailboxes, with a clearer **Tests** menu for sending profiles.

- Check saved sending-profile connections and send fixed-content test emails from a grouped testing menu. Keep durable results and history, and optionally require a successful test of current settings before new campaigns launch.

- Require numeric revisions for shared-resource updates and deletions; reject stale writes atomically and keep unsaved editor input with explicit conflict recovery.

- Share application resources, campaign drafts, results and exports across the installation while retaining account and administrator permissions. Record read-only creator and last-editor attribution; preserve shared work when accounts are deleted.

Initial release of a Gophish successor with a native Rust server, a Rust/WASM administration interface, and independent API contracts. Feature coverage follows the documented differences and privacy boundaries.

- Let regular users manage shared webhooks and reporting mailboxes under administrator feature switches; reject configuration saves while a feature is disabled.
- Initialize reporting mailboxes from active campaigns' actual sends, match reports across campaign creators, and ignore new reports after campaign completion while preserving polling progress.
- Record coarse browser, OS, and device categories on clicks and submissions, with per-event recipient details and consistent exports/webhooks; exclude raw headers and network identities.
- Manage recipient groups, email templates, landing pages, reusable static learning pages, attachments, and sending profiles.
- Select hosted learning pages as campaign redirect destinations without adding training-completion tracking.
- Choose campaign email-open tracking before launch, with disabled metrics shown as not tracked and enabled defaults for migrated campaigns.
- Save campaign drafts with numeric IDs and idempotent creation, review campaigns before launch, and schedule delivery with explicit recovery for uncertain sends.
- Freeze campaign email, attachments, sender identity/headers, landing pages and learning content at launch while keeping SMTP transport settings and domain eligibility live.
- Review dashboards and campaign/recipient timelines, with CSV and XLSX exports.
- Use fixed DNS, IPv4, or IPv6 campaign origins over HTTP or HTTPS; required verification remains restricted to DNS hostnames over HTTPS.
- Verify campaign domains, process reports through multiple IMAP mailboxes, and deliver signed webhooks.
- Use English or German interface text with independent date-format and time-zone preferences.
- Run with SQLite, MySQL, or PostgreSQL, with migration support for recognized Gophish SQLite and MySQL histories.
- Deploy with Docker Compose and environment-based configuration, or use a standalone native executable with the administration UI embedded.
- Automate campaign workflows through documented Ruphish contracts, with separate `/api/`, `/auth/`, and `/internal/` interfaces.
- Track participant interactions and password presence without collecting entered credentials or other form values through the built-in collector.

See [differences from Gophish](https://ruphish.com/docs/migration/changes-to-review-before-migrating/) for intentional changes and migration considerations.
