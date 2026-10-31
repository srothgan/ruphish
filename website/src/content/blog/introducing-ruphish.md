---
title: Introducing Ruphish, an open-source successor to Gophish
description: Why Ruphish exists, what it carries forward from Gophish, and where the project is heading.
date: 2026-10-31
tags: [Announcement, Gophish, Migration]
---

Ruphish is an open-source successor to Gophish. It follows the idea of Gophish, self-hostable phishing simulations, and gives it a new foundation. Ruphish matches Gophish in capabilities and has more features planned on the [roadmap](https://github.com/srothgan/ruphish/blob/main/ROADMAP.md).

## Why a successor

I was studying Information Systems, and in my master's thesis I researched phishing and spear-phishing in an organisational context. During my thesis, I relied on Gophish and appreciated its functionality and privacy. Because of this, I was disappointed to see that the project no longer seems to be maintained and that no clear successor has been defined. Therefore, I started Ruphish: a successor to Gophish with the same base capabilities, but with a complete UI redesign, a new API and new features coming. These changes are inspired by my own experience, by what I could find in Gophish issues and discussions, and by whatever issues will be raised in Ruphish.

## What carries over

Ruphish is a successor, so the capabilities for which someone might use Gophish are carried over into Ruphish. Just like in Gophish, you can create phishing campaigns, track them and analyse their results. It also includes the familiar resources: recipient groups, email templates, landing pages, sending profiles, webhooks and reporting mail accounts. If you run Gophish today, the [migration guide](/docs/migration/) explains how to bring an existing installation with its persisted data into Ruphish and where the differences are.

## What is different

However, Ruphish is not a drop-in replacement for Gophish. I originally aimed to create a drop-in replacement for Gophish, but this was not feasible. This mainly concerns the differences between the API endpoints that Gophish and Ruphish offer, while new capabilities were also introduced with `0.1.0`. These changes are:

- **Improved campaign results:** Ruphish restructured how interaction events are tracked and stored. This allows for an improved results analysis with a dedicated dashboard for each campaign, per-recipient views and an export for your own analysis.
- **Careful and safe tracking:** Unlike Gophish, Ruphish does not collect recipients' credentials. Instead, Ruphish only records whether a password was entered, preventing any harm by design. The [participant-data guide](/docs/user-guide/participant-data/) describes the model.
- **Starter templates:** To shorten the time from setup to the first campaign, Ruphish offers a starter template repository in the company settings. There, admins can directly import email templates as well as landing and learning pages.
- **Draft campaigns before launch:** Users can now create, save and continue editing a campaign while keeping it as a draft.

## Try it

The [demo](https://demo.ruphish.com/) shows sample campaigns and results without sending email. When you are ready, [getting started](/docs/getting-started/) walks through a first campaign. Issues and feature ideas are welcome on [GitHub](https://github.com/srothgan/ruphish/issues).
