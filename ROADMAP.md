# Roadmap

Bugs, errors, security issues, and reliability problems are addressed continuously across all releases.

Ruphish targets Gophish compatibility and a drop-in replacement, subject to [documented differences](https://ruphish.vercel.app/docs/migration/differences-from-gophish/).

Milestones describe priorities, not release dates; scope may change with feedback.

## 0.1.0 - First release

**Status: In progress until shipped.** Target feature parity with Gophish.

See the [changelog](CHANGELOG.md) for existing capabilities.

Complete the first-release scope:

- **Gophish feature coverage:** Cover all Gophish features, with intentional differences documented.
- **Distribution:** Publish verified release binaries and a Docker image.
- **Starter templates:** Bundle an offline catalog of editable email templates and landing pages.
- **Learning pages:** Add reusable educational pages as a new resource, with campaign follow-up and external redirects.
- **Admin settings:** Add initial configuration controls, starting with domain-verification policy.
- **Release verification:** Complete documentation, in-app guidance, security checks, and workflow, migration, upgrade, and recovery acceptance.

## 0.2.0 - Administration and campaign operations

**Status: Proposed.** Simplify daily administration and repeated campaigns.

- **Logs and auditing:** Add filtered admin diagnostics, redacted downloads, bounded history, and audit records for sensitive actions.
- **Admin settings:** Extend configuration controls based on operational needs.
- **Recurring campaigns:** Add recurrence, template rotation, and recipient exclusions with launch authorization.
- **Scheduled completion:** Automatically complete campaigns at a chosen date and time while retaining results.
- **Sending checks:** Add configurable limits and improve content, SMTP, and domain preflight checks.
- **Mail OAuth:** Support OAuth authentication for sending profiles and reporting mailboxes.
- **QR codes:** Embed recipient-specific campaign QR codes in email templates.
- **Group CSV export:** Export recipient groups as editable, reusable CSV files.

## 0.3.0 - Deeper reporting and integrations

**Status: Proposed.** Make changes over time easier to understand and share.

- **Comparisons:** Compare campaigns and periods using consistent metrics and audiences.
- **Positive behavior:** Highlight reporting rates and time-to-report.
- **Trends:** Analyze audiences, campaigns, and templates while preserving historical meaning.
- **Scanner activity:** Flag likely automation, retain observations, and show classification uncertainty.
- **Reporting integrations:** Add shareable summaries and improve API and webhook reporting workflows.
- **Custom recipient fields:** Import additional recipient fields and use them in templates with validation.
- **Directory imports:** Import recipients and groups from LDAP/Active Directory with a review step.

## Ideas for discussion

These ideas have no committed release target:

- **Recipients and groups:** Discuss independent recipient management, memberships, and compatibility implications.
- **SSO and team access:** Assess single sign-on, shared resources, and organization boundaries.
- **Aggregate-only reporting:** Explore results that cannot be linked back to individual recipients.
- **Multi-step scenarios:** Explore sign-in, MFA, and consent decisions without collecting authentication secrets.
- **AI assistance:** Explore optional content and report drafting with explicit data-sharing controls.
- **MCP:** Explore scoped read access and draft preparation independently of in-app AI.
