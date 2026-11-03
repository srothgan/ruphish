---
title: Get started with Ruphish
description: Install Ruphish, connect a sending account, and run your first phishing simulation.
sidebar:
  order: 1
---

Ruphish helps you run phishing simulations for security awareness training. You choose who receives the campaign, prepare an email and the page its link opens, and send it through your mail provider. In Ruphish, you can then see delivery status, link clicks, form submissions, and reported messages.

Your first campaign sends a message through your mail provider to an inbox you control. You'll follow its campaign link and find the interaction in your results. The [Starter Template Repository](../user-guide/add-starter-templates.md) includes ready-made emails and pages, so you can try the workflow before writing your own content.

## Start your first campaign

1. **[Install Ruphish](install-ruphish.mdx).** Choose Docker or a native binary, start Ruphish, and sign in. Both include everything you need to use Ruphish in your browser, with a built-in SQLite database for your data.
2. **[Run your first campaign](run-your-first-campaign.md).** Connect a dedicated sending account, add starter content, and follow a campaign from preparation to results.
3. **[Understand your campaign](understand-your-campaign.md).** Revisit how the pieces fit together and what the results tell you.

You can start installing now. The tutorial explains the concepts as you use them.

## Already have access to Ruphish?

If your organization already runs Ruphish, sign in with your own user account. If you haven't received access yet, ask the person managing Ruphish for the sign-in URL and a user account. You can skip installation and start with [your first campaign](run-your-first-campaign.md). Check which shared sending profiles, templates, and campaign domains are already available before creating new ones.

Recipients don't need Ruphish accounts to receive campaign emails or open their links. Your user account lets you prepare campaigns and view results.

## Coming from Gophish?

You can bring data from supported Gophish SQLite and MySQL databases into Ruphish. Follow [migration from Gophish](../migration/migrate-the-database.md) to check compatibility and prepare your data, and review [what changes](../migration/changes-to-review-before-migrating.md) before reusing settings or automation.

## Where to go after getting started

- [User guide](../user-guide/index.md): create your own content, schedule campaigns, and explore results.
- [Administration](../administration/index.md): configure hosting, trusted HTTPS, accounts, databases, and backups.
- [HTTP API](../api/index.md): automate campaign management and reporting.
- [Contributing](../contributing/index.md): build from source and work on Ruphish.
