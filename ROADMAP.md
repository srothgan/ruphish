# Roadmap

Bugs, errors, security issues, and reliability problems are addressed continuously across all releases.

Ruphish targets Gophish compatibility and a drop-in replacement, subject to [documented differences](https://ruphish.com/docs/migration/differences-from-gophish/).

Milestones describe priorities, not release dates; scope may change with feedback.

## 0.1.0 - First release

**Status: In progress until shipped.** Target feature parity with Gophish.

See the [changelog](CHANGELOG.md) for existing capabilities.

Complete the first-release scope:

- **Gophish feature coverage:** Cover all Gophish features, with intentional differences documented.
- **Distribution:** Publish verified release binaries and a Docker image.
- **Health endpoints:** Add HTTP liveness and readiness endpoints for deployment and service monitoring.
- **Starter templates:** Bundle an offline catalog of editable email templates and landing pages.
- **Learning pages:** Add simple static educational landing pages as post-submission redirect destinations.
- **Admin settings:** Add initial configuration controls, starting with domain-verification policy.
- **Release verification:** Complete documentation, in-app guidance, security checks, and workflow, migration, upgrade, and recovery acceptance.

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
