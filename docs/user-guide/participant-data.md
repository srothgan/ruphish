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
| Password presence | Optional boolean indicating that a designated password control was nonempty |
| Entered usernames, passwords, OTPs, or arbitrary field values | Not accepted as tracking data |
| Hashes, lengths, or samples of entered values | Not supported substitutes for value capture |
| Participant IP address, location, or user-agent | Not added to campaign event records |

## What the served page permits

Forms retain useful labels, names, IDs, autocomplete attributes, and presentation. Authored scripts, event handlers, frames, objects, form destinations, external images/fonts/stylesheets, and automatic refresh are removed or blocked in the public representation. Templates run before those restrictions are applied.

The application-controlled collector sends only `event_id` and `password_present`. It waits for an explicit acknowledgement and reuses the event identity if a retry is needed. Without the collector, the page policy blocks native submission. The server rejects extra fields, form-encoded bodies, and multipart submissions instead of interpreting them as captured content.

These are implementation and protocol boundaries, not a claim that every browser, password manager, reverse proxy, or imported page has completed end-to-end acceptance. The project's [manual UI/privacy checks](../contributing/testing/manual-ui.md) remain necessary for browser behavior. Check the page and network activity with synthetic values before relying on an installation's complete path.

## Legacy names and historical data

The API retains `capture_credentials` and `capture_passwords` for compatibility. Their meanings are submission-event tracking and optional password presence; they cannot enable raw-value collection.

Supported upgrades remove legacy opaque captured details and location data from active application records. Recovery copies deliberately preserve the original and can still contain data captured by Gophish. Old backups, old logs, reverse proxies, and external copies are not sanitized by upgrading Ruphish. Handle them through your own retention and access controls.

Administrator passwords and configured SMTP/IMAP service credentials have a different purpose and are handled separately. This page describes participant tracking, not the elimination of all secrets from an installation.

## Access, exports, and deletion

Campaign resources and reports are scoped to their application owner and permissions. An authorized administrator can explicitly impersonate an account; configured webhooks can receive events across owners. Exports contain the operator-supplied recipient identities and the documented event fields, so they still require appropriate handling even though submitted values are excluded.

Completing a campaign preserves its history. Deleting it removes the campaign and associated application records; it does not remove downloaded exports, external webhook copies, database backups, or old log files. See [completion and deletion](campaigns/completion-and-deletion.md). No configurable automatic campaign-retention policy is documented here, and application deletion is not a claim of forensic erasure from storage media.
