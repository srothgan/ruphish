---
title: Understand participant data
description: Understand interaction records, presence-only submissions, and the limits of landing-page content.
sidebar:
  order: 8
---

Ruphish's supported tracking contract is event-based. You supply recipient details when creating a group; participant-entered form values are not part of campaign results.

| Data | Treatment |
| --- | --- |
| Operator-supplied recipient email/name/position | Used for personalization and recipient records |
| Delivery, open, click, submission, and report observations | Used for event history and reporting |
| Password presence | Boolean indicating that an eligible password control was nonempty when submitted |
| Entered usernames, passwords, OTPs, or arbitrary field values | Not accepted as tracking data |
| Hashes, lengths, or samples of entered values | Not supported substitutes for value capture |
| Participant IP address, location, or user-agent | Not added to campaign event records |

## What the served page permits

Forms retain labels, names, IDs, autocomplete attributes and validation. Authored scripts, inline handlers, images, stylesheets and fonts are supported. Live pages and functional previews run in an opaque-origin sandbox isolated from the administrative application. Native form destinations, child frames, objects, workers and HTTP-equivalent refresh metadata remain restricted. Templates run before document preparation; they retain text-template escaping behavior. See [landing-page authoring](landing-pages.md).

Custom code and external dependencies can read or transmit participant input independently. Ruphish's guarantee covers its built-in collector and backend data paths, not arbitrary authored execution or author-selected external services. Sandboxing protects the application boundary; it does not make custom code unable to read its own page's controls or interfere with tracking.

The built-in collector sends only `event_id` and `password_present`. Each supported final submission creates a new observation; failed validation and cancelled intermediate submit events do not. The separate retry button retries an unconfirmed delivery with its original identity and presence boolean; that retry does not add another event. The page policy blocks native submission even without the collector. The server rejects extra fields, form-encoded bodies and multipart submissions instead of interpreting them as captured content. Only acknowledged events are presented as confirmed; arbitrary scripted requests are not automatically counted as submissions.

These are implementation and protocol boundaries, not a claim that every browser, password manager, reverse proxy, or imported page has completed end-to-end acceptance. The project's [manual UI/privacy checks](../contributing/testing/manual-ui.md) remain necessary for browser behavior. Check the page and network activity with synthetic values before relying on an installation's complete path.

Password presence is true when the submitted form has at least one named, enabled password control with a nonempty value. Prefilled, password-manager-filled, read-only, revealed-password, and externally form-associated controls count when otherwise eligible. Disabled controls (including disabled fieldsets outside their first legend), unnamed controls, and controls belonging to another form do not count. This observes attempted submission, not whether a password was valid.

## Legacy names and historical data

The API has no capture settings. Supported submissions and password presence are always tracked; entered values are never collected by the built-in event collector.

Supported upgrades remove legacy opaque captured details and location data from active application records. Recovery copies deliberately preserve the original and can still contain data captured by Gophish. Old backups, old logs, reverse proxies, and external copies are not sanitized by upgrading Ruphish. Handle them through your own retention and access controls.

Administrator passwords and configured SMTP/IMAP service credentials have a different purpose and are handled separately. This page describes participant tracking, not the elimination of all secrets from an installation.

## Access, exports, and deletion

Campaign resources and reports are scoped to their application owner and permissions. An authorized administrator can explicitly impersonate an account; configured webhooks can receive events across owners. Exports contain the operator-supplied recipient identities and the documented event fields, so they still require appropriate handling even though submitted values are excluded.

Completing a campaign preserves its history. Deleting it removes the campaign and associated application records; it does not remove downloaded exports, external webhook copies, database backups, or old log files. See [completion and deletion](campaigns/completion-and-deletion.md). No configurable automatic campaign-retention policy is documented here, and application deletion is not a claim of forensic erasure from storage media.
