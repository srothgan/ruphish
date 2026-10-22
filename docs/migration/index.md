---
title: Migration and upgrades
description: Plan a Gophish migration or Ruphish upgrade with explicit compatibility and recovery checks.
sidebar:
  order: 1
---

Start with the [compatibility guide](compatibility.md), then choose [migrate from Gophish](from-gophish.md) or [upgrade Ruphish](upgrading-ruphish.md). Both workflows finish with [verification and rollback planning](verification-and-rollback.md).

Migration is more than opening a database. Review configuration, templates, sender trust, public domains, active campaigns, API clients, and integrations. The [differences guide](differences-from-gophish.md) identifies behavior that requires attention even when familiar resource APIs still work.

Use a recoverable copy first. Do not run Gophish and Ruphish against the same live database, and do not let a rehearsal send queued mail to actual recipients.
