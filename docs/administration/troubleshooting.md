---
title: Troubleshoot an installation
description: Diagnose configuration, startup, trust, database, and worker failures.
sidebar:
  order: 11
---

| Symptom | Check | Correction and verification |
| --- | --- | --- |
| No readiness message | Stderr and safe startup diagnostic | Fix the reported configuration/assets/storage issue, restart, and wait for `RUPHISH_READY` |
| Configuration worked elsewhere | Working directory, environment password source, file permissions | Use the intended directory/environment; confirm the resolved instance and bound addresses |
| Missing/stale asset error | `dist/admin/` and build inputs | Rebuild the development distribution, restart, and sign in |
| TLS identity failure | Hostname, certificate SANs, trust bundle, matching key | Correct the pair/chain/trust for that connection; repeat validation without a general bypass |
| Domain verification fails only on the server | Resolver, destination policy, public routing | Use [domain diagnostics](../user-guide/domains.md); verify all checks from the instance |
| Internal SMTP/IMAP/webhook denied | Exact resolved addresses and outbound policy | Add the narrow intended allowance, restart, and validate the connection |
| Database unavailable | Server version, account permissions, TLS, environment secret, network | Correct the provider setup; verify startup and a read before inspecting queued work |
| Unknown or modified schema | Source database/version and migration history | Stop; preserve the original and follow [recovery](../migration/verification-and-rollback.md) instead of deleting migration metadata |
| Campaign never sends | Selected process mode, mailer flag, due time, domain status | Restore the missing prerequisite and watch delivery status; don't create a duplicate campaign |
| Browser mutations rejected | Session state, CSRF, Origin/Referer, proxy scheme/host | Sign in again and correct the origin configuration; public proxy trust does not configure admin trust |

Share diagnostic codes and redacted configuration structure when requesting help. Do not include API keys, passwords, cookies, raw reported messages, or historical captured data. [Logging](logging.md) explains the application's diagnostic boundaries.
