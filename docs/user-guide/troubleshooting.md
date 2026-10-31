---
title: Troubleshoot campaign work
description: Find the next useful check for common editing, delivery, and reporting problems.
sidebar:
  order: 12
---

| Symptom | Diagnose, fix, and verify |
| --- | --- |
| A resource is missing | Confirm the signed-in account and list filter. Resources are owner-scoped; check another tab before recreating anything. |
| Save reports a stale edit | Another write changed the resource. Preserve your unsaved values, inspect the current version, reconcile, and save again. |
| An imported draft disappeared | Import state is held in the tab until creation. Reload/closing/logging out can clear it; import again and finish saving. |
| CSV rows are omitted | Review mapping, encoding, address validity, and duplicate notices in [group import](groups.md). Verify the saved recipient count. |
| A template preview fails | Check field case, syntax, function arguments, and content limits in the [template reference](template-reference.md). Preview again with representative recipient values. |
| Imported landing-page content looks different | Check the [served-content restrictions](landing-pages.md). Replace scripts/remote assets with supported content and inspect the public representation. |
| A domain cannot be selected | Check its enablement and latest verification result. Follow [domain diagnostics](domains.md), then refresh campaign review. |
| Delivery is waiting | Check launch time, domain readiness, and whether this instance runs the mailer. Inspect the next due attempt instead of relaunching. |
| A send outcome is unknown | Inspect receiving-server evidence and use [reconciliation](campaigns/delivery.md); do not assume a retry is duplicate-free. |
| Counts differ across views | Compare cohort, event/time/recipient filters, raw events, unique counts, and [progression rules](results/metrics.md). |
| A forwarded report is not counted | Check the [initial scan boundary](reporting-mailboxes.md#send-a-report), campaign completion state, and whether the message contains one matching recipient identity/origin; inspect mailbox history. |

For listener, certificate, startup, and database problems, use [administration troubleshooting](../administration/troubleshooting.md). Include the visible diagnostic code and redacted context when asking an installation operator for help; avoid sharing keys or participant values.

## Recognize a diagnostic code

The UI translates messages, but public API error `code` values remain stable uppercase identifiers. Domain and delivery history also carry lower-case diagnostic strings; these are separate fields, not inconsistent capitalization of one API code.

| Code | Next step |
| --- | --- |
| `RESOURCE_CHANGED` | Reload the saved resource, reconcile your draft, then retry the edit |
| `RESOURCE_IN_USE` | Inspect the active campaign using the resource before changing/deleting it |
| `CSV_ROW_EMAIL` | Correct the address on the indicated import row and review again |
| `DRAFT_SCHEDULE_PAST` | Choose a future schedule or clear it for immediate launch |
| `DELIVERY_FAILED` | Inspect the SMTP outcome before repeating a test send |
| `domain_not_verified` / `domain_check_expired` | Read domain history and restore valid verification evidence |
| `legacy_delivery_outcome_unknown` | Reconcile the migrated in-flight message with receiving-server evidence |

See [API errors](../api/errors.md) for HTTP statuses and client recovery behavior. Keep the original code when reporting a problem even if you translate the accompanying message.
