---
title: Introducing Ruphish, an open-source successor to Gophish
description: Why Ruphish exists, what its first release includes, and how it differs from Gophish.
date: 2026-11-13
tags: [Announcement, Gophish, Migration]
---

Ruphish is an open-source successor to Gophish. It follows the idea of Gophish, self-hosted phishing simulations, and gives it a new foundation. Its first release, `0.1.0`, covers the campaign workflows people use Gophish for, and the [roadmap](https://github.com/srothgan/ruphish/blob/main/ROADMAP.md) describes what comes next.

## Why a successor

I was studying Information Systems, and in my master's thesis I researched phishing and spear-phishing in an organisational context. During my thesis, I relied on Gophish and appreciated its functionality and privacy. Because of this, I was disappointed to see that the project no longer seems to be maintained and that no clear successor has been defined. Therefore, I started Ruphish: a successor to Gophish with the same base capabilities, but with a complete UI redesign, a new API and new features coming. These changes are inspired by my own experience, by what I could find in Gophish issues and discussions, and by whatever issues will be raised in Ruphish.

## What is in 0.1.0

Just like in Gophish, you can create phishing campaigns, send them and analyse their results. The first release includes:

- **Campaign preparation:** Build personalised emails from reusable recipient groups, email templates, attachments and landing pages. Save a campaign as a draft, preview and review it before launch, and send immediately or schedule delivery over a chosen time window.
- **Delivery you can follow:** Send through your own SMTP service, test sending profiles before use, and inspect every delivery attempt, with recovery options when it is unclear whether a message was sent.
- **Campaign domains:** Manage the domains your campaigns use and verify them before use. Administrators can require verification for every domain.
- **Learning pages:** Show recipients a page after a form submission that explains the exercise and helps them recognise phishing.
- **Starter templates:** Add ready-made email templates, landing pages and learning pages from the Starter Template Repository and customise them for your organisation.
- **Results:** Explore dashboards and campaign and recipient timelines, with optional email-open tracking, click and submission events, password-presence indicators, and browser and device categories. Export results as CSV or Excel (XLSX).
- **Reports and integrations:** Process reported phishing emails through multiple IMAP mailboxes, forward campaign events to signed webhooks, and automate resource management, campaigns and reporting through a documented HTTP API.
- **Teamwork:** Work together on shared resources, drafts, campaigns and results, with account permissions, creator and editor attribution, and protection against conflicting edits.
- **Your language and formats:** Use the interface in English or German, with separate date-format and time-zone preferences.
- **Deployment:** Run Ruphish as a standalone native executable with the administration interface included, or deploy it with Docker Compose. Store data in SQLite, MySQL or PostgreSQL.

The [changelog](https://github.com/srothgan/ruphish/blob/main/CHANGELOG.md) lists every user-facing change in the release.

## What is different

However, Ruphish is not a drop-in replacement for Gophish. I originally aimed to create a drop-in replacement, but this was not feasible. The main differences are:

- **Careful and safe tracking:** Unlike Gophish, Ruphish does not collect recipients' credentials. The built-in submission collector records interactions and whether a password was entered, but never the entered usernames, passwords, one-time codes or other form values. This prevents harm by design. The [participant-data guide](/docs/user-guide/results/participant-data/) describes the model and its limits.
- **Improved campaign results:** Ruphish restructured how interaction events are tracked and stored. This allows for an improved results analysis with a dashboard for each campaign, per-recipient timelines and an export for your own analysis.
- **Shared installations:** Resources, drafts, campaigns and results are shared across a Ruphish installation instead of belonging to individual users. Account permissions decide what each person can do.
- **A new interface and API:** Familiar API paths can have different inputs, responses and permissions, so existing integrations need review. The [API migration guide](/docs/migration/api/) compares the endpoints.

If you run Gophish today, the [migration guide](/docs/migration/) explains how to bring a supported SQLite or MySQL database into Ruphish and what to review before you switch.

## Try it

The [demo](https://demo.ruphish.com/) shows sample campaigns and results without sending email. When you are ready, the [installation guide](/docs/getting-started/install-ruphish/) sets up Ruphish and [your first campaign](/docs/getting-started/run-your-first-campaign/) walks you through a complete exercise. Issues and feature ideas are welcome on [GitHub](https://github.com/srothgan/ruphish/issues).
