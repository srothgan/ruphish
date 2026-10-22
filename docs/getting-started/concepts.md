---
title: Understand a campaign
description: Learn the objects and lifecycle behind a Ruphish exercise.
sidebar:
  order: 3
---

A campaign brings together an audience, a message, a destination, and delivery settings. You can reuse the resources across exercises, but each launched campaign has its own recipient results and history.

| Concept | What it means |
| --- | --- |
| Application account | A person who signs in to manage resources or administer the installation |
| Recipient | An address and optional name/position supplied by the campaign operator |
| Group | A saved collection of recipients |
| Email template | The subject, HTML/plain-text content, visible sender override, and attachments |
| Landing page | The content shown after a recipient follows a campaign link |
| Sending profile | SMTP connection settings and transport sender |
| Domain | A public HTTPS origin whose control, routing, DNS, and TLS have been verified |
| Draft | Editable campaign configuration that has not started delivery |
| Campaign | A launched or scheduled execution with recipients, delivery work, and event history |

## From resources to results

Suppose you're preparing an exercise for a small training group. You import its recipients, write a personalized email, and create a landing page. You then choose a sending profile and verified domain, save a draft, and review the final audience and message.

Launching creates the execution state. A future launch time leaves messages waiting until they are due. Saving a draft does not start a campaign, even when its proposed launch time passes.

The SMTP server accepting a message establishes a delivery observation; it does not prove that the message reached an inbox. Requests for tracking images and landing pages create other observations. Reporting a message is a separate outcome.

## Resource changes and ownership

Resources belong to application accounts. Administrator permissions do not make every campaign appear in your own dashboard. Account administration and campaign ownership are separate boundaries.

Saved campaign resources are not a general versioning system. Editing a referenced resource can affect its consumers. Duplicate content when you need a separate version, and review current resources before launch. Resources used by unfinished campaigns have deletion protection.

## Two different kinds of credentials

Your application password and configured SMTP/IMAP credentials authorize the application to do work. Participant-entered values are different: they are not supported campaign data. A password-presence observation means only that a designated field was nonempty, not that a password was valid or retained.

Continue with [installation](installation.md), or follow the [local tutorial](first-campaign.md) if the build tools are already available.
