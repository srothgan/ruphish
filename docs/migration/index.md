---
title: Migration from Gophish
description: Migrate a Gophish database, then move to the Ruphish interface or adapt your API client.
sidebar:
  order: 0
prev: false
---

Database migration converts stored Gophish data into Ruphish's schema. It does not translate an API client or reproduce the Gophish interface. Choose your path below; both start with the same database procedure.

| Your destination | Reading order |
| --- | --- |
| Use the Ruphish interface | [Check the changes](changes-to-review-before-migrating.md) → [migrate the database](migrate-the-database.md) → [find your tasks in the interface](using-the-new-interface.md) → [verify and cut over](verification-and-rollback.md). |
| Use Ruphish through your own UI, scripts, or service | [Check the changes](changes-to-review-before-migrating.md) → [map your API calls](api/index.md) → [migrate a database copy](migrate-the-database.md) → adapt and test your client → [verify and cut over](verification-and-rollback.md). |

If someone else hosts your installation, they perform the database migration. You can use the interface or API migration instructions without becoming a system administrator.

## Supported starting points

| Source | Destination |
| --- | --- |
| Recognized Gophish SQLite schema and migration history | Ruphish SQLite, converted automatically at startup. |
| Recognized Gophish MySQL schema and migration history | Ruphish MySQL on a supported server, converted automatically at startup. |
| Gophish SQLite or MySQL → PostgreSQL | No automatic conversion or cross-engine transfer. |
| An existing released Ruphish installation | Follow [Upgrade Ruphish](../administration/hosting-and-maintenance/upgrade-ruphish.md). |

The API comparison uses Gophish Community Edition **0.12.1**, commit `95618469799295e2c0fec980805a2dfbb818816b`, and the current Ruphish contracts introduced with 0.1.0. Database recognition covers 25 recorded SQLite schema versions and 28 recorded MySQL schema versions. Recognition checks the actual schema and history; a version label alone does not establish compatibility. Customized schemas and older Ruphish development histories are not accepted automatically.

Rehearse with a copy before changing the production database. Keep Gophish stopped during the final conversion, and do not run both applications against the same database.
