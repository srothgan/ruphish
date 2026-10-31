# Roadmap

Bugs, errors, security issues, and reliability problems are addressed continuously across all releases.

Ruphish is a successor to Gophish, covering its campaign workflows with supported data/template migration and independent API contracts. The [documented differences](docs/migration/differences-from-gophish.md) define intentional changes and deferred features.

Milestones describe priorities, not release dates; scope may change with feedback.

## 0.1.0 - First release

**Status: In progress until shipped.** Cover Gophish campaign workflows within the documented feature and privacy boundaries.

See the [changelog](CHANGELOG.md) for existing capabilities.

Implemented in the development version:

- **Email-open tracking:** Choose tracking per campaign before launch; new and migrated campaigns default to enabled. Disabled metrics show **Not tracked** or **-**.
- **Campaign URLs:** Support fixed DNS, IPv4, and IPv6 origins over HTTP or HTTPS. Required verification limits eligibility to DNS hostnames over HTTPS.
- **Client observations:** Show read-only browser, OS, and device categories per click/submission event, associated with the existing recipient. Exclude entered values, raw headers, IP/geolocation, and fingerprinting.
- **Reporting mailboxes:** Initialize new mailbox scans from the earliest actual Ruphish send in any active campaign, or new arrivals if none exists. Match across campaign creators, keep polling between campaigns, and ignore reports after completion without blocking mailbox progress.
- **Integration access:** Let regular users manage their own mailbox and webhook configurations when an administrator enables the feature. Webhook delivery spans campaign creators; campaign management remains creator-scoped.
- **Starter and learning pages:** Bundle editable offline email/landing templates and static educational pages for post-submission redirects.
- **Administration and monitoring:** Provide domain-verification policy, integration feature switches, and HTTP liveness/readiness probes.

Remaining release work:

- **Distribution:** Publish verified release binaries and a Docker image.
- **Release verification:** Complete security checks and workflow, migration, upgrade, recovery, and distribution acceptance. Feature-level tests and documentation checks do not establish full release acceptance.

## 0.2.0 - Administration and campaign operations

**Status: Proposed.** Simplify daily administration and repeated campaigns.

- **Logs and auditing:** Add filtered admin diagnostics, redacted downloads, bounded history, and audit records for sensitive actions.
- **Admin settings:** Extend configuration controls based on operational needs.
- **Recurring campaigns:** Add recurrence, rotate paired email templates and landing pages between runs, and support recipient exclusions with launch authorization.
- **Working-hour schedules:** Limit sending to configured weekdays and hours, with explicit time-zone and daylight-saving behavior.
- **Pause and resume:** Pause campaign delivery and resume remaining work while preserving observations and delivery history.
- **Scheduled completion:** Automatically complete campaigns at a chosen date and time while retaining results.
- **Sending checks:** Add configurable limits and improve content, SMTP, and domain preflight checks.
- **Mail OAuth:** Support OAuth authentication for sending profiles and reporting mailboxes.
- **Bounce processing:** Correlate structured delivery-status notifications with sent messages and record confirmed delivery failures, leaving ambiguous notifications unresolved.
- **QR codes:** Embed recipient-specific campaign QR codes in email templates.
- **Group CSV export:** Export recipient groups as editable, reusable CSV files.

## 0.3.0 - Deeper reporting and integrations

**Status: Proposed.** Make changes over time easier to understand and share.

- **Comparisons:** Compare campaigns and periods using consistent metrics and audiences.
- **Positive behavior:** Highlight reporting rates and time-to-report.
- **Trends:** Analyze audiences, campaigns, and templates while preserving historical meaning.
- **Scanner activity:** Investigate better automation and false-positive classification, including operator review, while retaining observations and showing classification uncertainty.
- **Link and attachment interactions:** Distinguish individual link and landing-page interactions and investigate separate tracking for existing email attachments.
- **Reporting integrations:** Add shareable summaries and improve API and webhook reporting workflows.
- **Custom recipient fields:** Import additional recipient fields and use them in templates with validation.
- **Directory imports:** Import recipients and groups from LDAP/Active Directory with a review step.

## Ideas for discussion

These ideas have no committed release target:

- **Batch reports:** Consider multiple distinct recipient-ID/origin pairs in one reported message; the first release rejects ambiguous attribution.
- **Embedded pages:** Consider authored iframe/frame support; the first release keeps embedded-document restrictions and provides ordinary landing pages and starter templates.
- **Recipients and groups:** Discuss independent recipient management, memberships, and compatibility implications.
- **SSO, MFA, and team access:** Assess single sign-on, administrator MFA, shared resources, and organization boundaries.
- **Active campaign audiences:** Discuss adding or removing recipients during active runs, with explicit authorization and preserved audience and reporting history.
- **Out-of-office detection:** Investigate whether absence replies can be classified reliably without depending on language-specific subject matching.
- **Multiple-domain certificates:** Assess per-domain certificate management and automatic renewal.
- **Learning outcomes:** Explore acknowledgement events, completion tracking, and external training integrations after the initial static learning pages.
- **Aggregate-only reporting:** Explore results that cannot be linked back to individual recipients.
- **Multi-step scenarios:** Explore sign-in, MFA, and consent decisions without collecting authentication secrets.
- **AI assistance:** Explore optional content and report drafting with explicit data-sharing controls.
- **MCP:** Explore scoped read access and draft preparation independently of in-app AI.
