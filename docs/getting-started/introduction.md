---
title: What is Ruphish?
description: Understand what Ruphish does, who uses it, and how it relates to Gophish.
sidebar:
  order: 2
---

Ruphish is a phishing-simulation application for running security awareness exercises. You prepare a recipient group, write an email, choose a landing page and sending profile, and launch a campaign. The administration interface brings delivery status and recorded interactions together so you can review the exercise.

You can use the browser interface for day-to-day work or the [HTTP API](../api/index.md) to integrate campaign management and reporting with your own tools.

## What you can do

- Organize recipients into groups and import them from CSV.
- Write HTML and plain-text email, personalize content, and attach files.
- Prepare landing pages and verify the domains used by campaign links.
- Save campaign drafts, review recipients, launch immediately, or schedule delivery.
- Review delivery problems, recipient timelines, campaign metrics, and CSV/XLSX exports.
- Receive reported messages through IMAP and send campaign events to webhooks.
- Use English or German interface text with independent date-format and time-zone preferences that follow your account.

Ruphish separates application accounts from campaign recipients. A recipient does not need an application account to receive a campaign message.

## Participant data

The tracking model is designed around interaction events and password-presence booleans. Participant-entered usernames, passwords, and arbitrary form values are outside the supported collection model. This also changes which authored landing-page content can run. Read [participant data](../user-guide/participant-data.md) before importing a page or launching a campaign.

An open or click is a recorded request. Mail scanners, image proxies, and automated clients can produce those requests too. Use the [metric definitions](../user-guide/results/metrics.md) when interpreting results.

## Relationship to Gophish

Ruphish replaces the backend with Rust and the administration interface with Rust/WASM. It preserves familiar concepts and many HTTP and template contracts, while making intentional changes to participant data handling, domain verification, authentication, and operations.

Ruphish is a successor to Gophish with its own API. Supported database migration and authored Go-template behavior remain requirements; automation must use the documented Ruphish contracts. The [compatibility guide](../migration/compatibility.md) explains what carries forward and which integrations need adaptation.

## Installation options

Ruphish's intended distribution options are a release binary and a Docker image. SQLite is the default database; MySQL and PostgreSQL are also configurable. See [installation](installation.md) for setup.

> TODO before publication: Publish and verify the release binaries and Docker image. Both distribution options are still pending.

Next, learn [how a campaign fits together](concepts.md).
