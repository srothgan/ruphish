---
title: Participant data
description: Understand what Ruphish records about recipients and their interactions.
sidebar:
  order: 35
---

Ruphish records recipient details that you provide and activity observed during a campaign. Its built-in submission tracking does not collect values entered into participant forms.

## What is recorded

| Information | How it is used |
| --- | --- |
| Recipient email, name, and position | Supplied through recipient groups; used for personalization and campaign results. |
| Sending, open, click, submission, and report events | Used for delivery history and campaign analysis. |
| Password presence | Records whether an eligible password field was nonempty at submission. |
| Browser, operating-system, and device categories | Derived from click and submission request headers. |

The built-in collector does not accept entered usernames, passwords, one-time codes, arbitrary form values, or hashes, lengths, or samples of those values as tracking data.

SMTP and IMAP account passwords serve a different purpose: they let Ruphish connect to the services you configure.

## What password presence means

Presence is recorded when the submitted form contains a named, enabled password field with a value. It can include a value filled by a password manager.

An empty field is not positive presence. A disabled or unnamed password control does not qualify.

This observation describes attempted submission. It does not establish whether the password is real, valid, or accepted by another service. The form's username value is not collected.

## Browser and device observations

Click and submission events may show a browser family, operating-system family, and device category such as desktop, mobile, tablet, or unknown.

These categories come from request headers, which can be incomplete or spoofed. They are not a verified device identity. A recipient's events can show different categories.

This feature does not add the request's IP address, location, raw User-Agent, exact software versions, or device model to campaign event records. Email-open requests and reports do not collect these client categories.

Events without collected client information display an unavailable value; an unrecognized collected category can display **Unknown**.

## Imported pages and custom code

The built-in collector's limits do not prevent your own JavaScript or an external service from reading or transmitting input.

Review scripts and external dependencies in [landing pages](../resources/landing-pages.md), including imported pages. Their isolation from the Ruphish application protects that application boundary; it does not make arbitrary authored code harmless.

Use synthetic values when checking a page's forms and network requests.

## Who can access the records

Campaign resources and results are shared according to account permissions. Configured webhooks can receive events from campaigns across the installation. Downloads and external services create copies outside Ruphish's own records.

Completing a campaign preserves history. Deleting it removes associated application records, but not exports, backups, external webhook copies, or separately retained logs.

An installation's web server, proxy, or other infrastructure may keep its own logs. The campaign-data limits above do not describe every system around Ruphish.
