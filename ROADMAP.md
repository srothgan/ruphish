# Roadmap

Bugs, errors, security issues, and reliability problems are addressed continuously across all releases.

Ruphish is a successor to Gophish, covering its campaign workflows with supported data/template migration and independent API contracts. The [documented differences](docs/migration/changes-to-review-before-migrating.md) define intentional changes and deferred features.

Milestones describe priorities, not release dates; scope may change with feedback.

## 0.1.0 - First release

**Status: Released on 2026-11-13.** Covers Gophish campaign workflows with supported migration, shared resources and campaigns, account permissions, and protection against conflicting edits. Event histories retain repeated observations while summaries distinguish recipient counts from event counts. The built-in submission collector records password presence without collecting entered values. Editable starter templates and learning pages support campaign preparation and education.

See the [changelog](CHANGELOG.md) for the complete release summary, including delivery, integrations, database support, and installation options.

## 0.2.0 - Campaign management

**Status: Proposed.** Improve preparing, scheduling, and running campaigns.

- **Recurring campaigns:** Add recurrence, rotate paired email templates and landing pages between runs, and support recipient exclusions with launch authorization.
- **Working-hour schedules:** Limit sending to configured weekdays and hours, with explicit time-zone and daylight-saving behavior.
- **Pause and resume:** Pause campaign delivery and resume remaining work while preserving observations and delivery history.
- **Scheduled completion:** Automatically complete campaigns at a chosen date and time while retaining results.
- **Viewer role:** Add a built-in read-only role for shared resources, campaigns, and results, with consistent restrictions across the interface and API.
- **Anonymous reporting:** Add a reporting mode that uses random recipient IDs instead of names and email addresses in results and exports. Define how identity mappings are handled; reporting remains pseudonymous where IDs can still be linked back to recipients.
- **Group CSV export:** Export recipient groups as editable, reusable CSV files.

## 0.3.0 - Mail and administration

**Status: Proposed.** Improve mail authentication, delivery controls, and operational diagnostics.

- **OAuth authentication for SMTP and IMAP:** Support provider-issued tokens for sending profiles and reporting mailboxes. Define supported providers and authorization flows before implementation; signing into Ruphish itself is covered separately under future access features.
- **Sending limits and checks:** Add configurable sending limits and extend content, SMTP, and domain preflight checks beyond the existing profile tests and optional successful-test requirement for launch.
- **Bounce processing:** Correlate structured delivery-status notifications with sent messages and record confirmed delivery failures, leaving ambiguous notifications unresolved.
- **Logs and auditing:** Add filtered admin diagnostics, redacted downloads, bounded history, and audit records for sensitive actions.

## 0.4.0 - Reporting and analysis

**Status: Proposed.** Improve analysis within and across campaigns, including changes over time and sharing results.

- **Reporting rates and time-to-report:** Highlight positive reporting behavior and how quickly campaign messages are reported.
- **Comparisons:** Compare campaigns and periods using consistent metrics and audiences.
- **Trends:** Analyze audiences, campaigns, and templates while preserving historical meaning.
- **Reports and integrations:** Add shareable summaries and improve API and webhook reporting workflows.
- **Link and attachment interactions:** Distinguish individual link and landing-page interactions and investigate separate tracking for existing email attachments.
- **Scanner activity (exploratory):** Investigate automation and false-positive classification, including operator review, while retaining observations and showing classification uncertainty.

## Future possibilities

These possibilities have no assigned release or delivery order. Feedback and implementation scope will help determine priorities.

### Access and administration

- **Single sign-on (SSO) and multi-factor authentication (MFA):** Assess signing into Ruphish through an identity provider and additional authentication protection for administrators.
- **Organization boundaries:** Assess separate access boundaries within an installation; resources and campaigns are already shared within the current workspace.
- **Multiple-domain certificates:** Assess per-domain certificate management and automatic renewal.

### Recipients and integrations

- **Recipients and groups:** Discuss independent recipient management, memberships, and compatibility implications.
- **Custom recipient fields:** Import additional recipient fields and use them in templates with validation.
- **Directory imports:** Import recipients and groups from LDAP/Active Directory with a review step.
- **Active campaign audiences:** Discuss adding or removing recipients during active runs, with explicit authorization and preserved audience and reporting history.
- **Batch reports:** Consider multiple distinct recipient-ID/origin pairs in one reported message; the first release rejects ambiguous attribution.
- **Out-of-office detection:** Investigate whether absence replies can be classified reliably without depending on language-specific subject matching.

### Learning and scenarios

- **QR codes:** Embed recipient-specific campaign QR codes in email templates.
- **Learning outcomes:** Explore acknowledgement events, completion tracking, and external training integrations after the initial static learning pages.
- **Multi-step scenarios:** Explore authoring and measuring individual sign-in, MFA, and consent decisions without collecting authentication secrets, beyond the intermediate steps already present in interactive starter pages.
- **Embedded pages:** Consider authored iframe/frame support; the first release keeps embedded-document restrictions and provides ordinary landing pages and starter templates.

### Exploratory ideas

- **AI assistance:** Explore optional content and report drafting with explicit data-sharing controls.
- **MCP:** Explore scoped read access and draft preparation independently of in-app AI.
