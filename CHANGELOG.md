# Changelog

Development previews have short notes on GitHub Releases. This changelog records user-facing changes for final releases.

## 0.1.0 - 2026-11-13

Initial release of Ruphish, a self-hosted phishing simulation platform and successor to Gophish.

### Added

- Create personalized phishing emails with reusable recipient groups, email templates, attachments, and landing pages.
- Save campaign drafts, preview and review content before launch, and send immediately or schedule delivery over a chosen time window.
- Send through your own SMTP service, test sending profiles, and inspect delivery attempts with recovery options for uncertain sends.
- Manage campaign domains and verify them before use, with an optional administrator policy requiring verification.
- Show reusable learning pages after form submissions to explain the exercise and help recipients recognize phishing.
- Explore dashboards and campaign and recipient timelines, with optional email-open tracking, click and submission events, password-presence indicators, and browser and device categories. Export results as CSV or Excel (XLSX).
- Process reported phishing emails through multiple IMAP mailboxes and forward campaign events to signed webhooks.
- Work together on shared resources, drafts, campaigns, and results, with account permissions, creator and editor attribution, and protection against conflicting edits.
- Use the interface in English or German, with separate date-format and time-zone preferences.
- Automate resource management, campaigns, and reporting through a documented HTTP API.
- Run as a standalone native executable with the administration interface included, or deploy with Docker Compose. Choose SQLite, MySQL, or PostgreSQL for storage.
- Migrate supported Gophish SQLite and MySQL databases.

The built-in submission collector records interactions and password presence without collecting entered usernames, passwords, one-time codes, or other form values. Learning pages provide educational content without tracking training completion. See [participant data](https://ruphish.com/docs/user-guide/results/participant-data/) for collection details and limits.

Start with the [installation guide](https://ruphish.com/docs/getting-started/install-ruphish/) or [your first campaign](https://ruphish.com/docs/getting-started/run-your-first-campaign/). If you are moving from Gophish, review the [migration guide](https://ruphish.com/docs/migration/): Ruphish has its own interface and API contracts, so existing integrations need review.
