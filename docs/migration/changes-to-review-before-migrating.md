---
title: Changes to review before migrating
description: Identify changes to access, stored data, campaign execution, and integrations before conversion.
sidebar:
  order: 10
---

Review these changes before starting Ruphish against a Gophish database. They apply whether you use the interface or an API client.

| Area | Change | Required action |
| --- | --- | --- |
| Resource and campaign visibility | Resources, drafts, campaigns, and results are shared across the Ruphish installation. Creator attribution does not make an object private. | Review who has access to this installation and what their roles permit before admitting users or clients. |
| Captured participant data | Conversion clears opaque event details and IP/location values from active data. New submissions record presence observations, not submitted usernames, passwords, or arbitrary form values. | Decide how to retain the sensitive pre-migration backup. Remove dependencies on captured values and geolocation. |
| Pending delivery | In-flight Gophish queue entries can become `unknown`; malformed or ambiguous queue entries are retained as migration issues instead of becoming runnable work. | Review [delivery conversion](what-happens-to-your-data.md#pending-messages) before enabling sending. |
| Campaign content | Imported campaigns preserve the content available at conversion. New campaigns snapshot selected content and recipients at launch. | Preview imported content. Do not assume a later resource edit changes an existing campaign. |
| Campaign URLs | Ruphish uses registered domains and an installation-level verification policy. Existing URLs still need working routing and HTTPS where applicable. | Verify the original links from already-sent messages before cutover. |
| Sending and reporting connections | Network and TLS checks can reject a connection that previously worked with insecure settings. | Verify certificates, authentication, and the permitted destinations with controlled accounts. |
| Reporting mailboxes | Per-user IMAP settings become named shared mailbox resources. Some imported configurations are disabled for review. | Inspect every imported mailbox and its processing boundary before enabling access to the mail server. |
| Webhooks | Configuration is shared; the administrator controls feature availability. Delivery has durable attempts and stable event identity. | Review receivers, signing secrets, deduplication, and retries. |
| API clients | Familiar endpoint paths can have different inputs, responses, permissions, and effects. | Follow the [endpoint comparison](api/index.md); database conversion alone does not make a client compatible. |
| Application configuration | A bare `ruphish` command uses the operating-system configuration location rather than assuming `./config.json`. | Supply an explicit configuration and database path during migration. |

See [What happens to your data](what-happens-to-your-data.md) for exact conversion treatment, [Using the new interface](using-the-new-interface.md) for task locations, and [API migration](api/index.md) for request and response changes.

## Content and tracking

Go-template authoring remains supported. Preview your actual subjects, text, HTML, attachments, and campaign URLs rather than treating successful database conversion as a rendering test. Landing pages use Ruphish's managed submission behavior and sandbox; review [landing-page authoring](../user-guide/resources/landing-pages.md) if your pages contain custom scripts or forms.

Open tracking is a campaign setting. Migrated campaigns start with `track_opens: true`; that does not prove that historical Gophish messages contained a tracking pixel. Imported events do not gain reconstructed browser or device information.

## Configuration and files

Use the released binary or Docker image. Database conversion does not install services or copy certificates, private keys, external files, or application configuration. Use the current [installation](../getting-started/install-ruphish.mdx) and [hosting instructions](../administration/hosting-and-maintenance/index.md).

Remove `migrations_prefix`: Ruphish embeds its migrations. Translate legacy MySQL connection strings into the typed configuration shown in the [MySQL migration procedure](database/migrate-mysql.md). Keep public hostnames and certificates available for URLs already delivered to recipients.
