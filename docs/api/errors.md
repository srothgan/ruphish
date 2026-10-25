---
title: API errors
description: Handle rejected requests, conflicts, and temporary failures without hiding their cause.
sidebar:
  order: 4
---

Read both the HTTP status and response body. Application failures include a machine-readable `code` alongside the compatibility message fields; use the code for branching and the message for diagnosis. Proxy failures and malformed requests rejected before application handling may have a different body, so tolerate non-JSON errors.

Legacy IMAP validation can return HTTP `200` with `success: false`; inspect that envelope as described in [reporting mailboxes](reporting-mailboxes.md). A 2xx status is not universally sufficient to declare every compatibility operation successful.

| Status | Meaning | Client action |
| --- | --- | --- |
| `400` | Invalid input, unsupported field, or invalid workflow state | Correct the request; do not retry unchanged |
| `401` | Authentication is missing or invalid | Check the key and account state |
| `403` | The account cannot perform the operation | Check permissions and browser CSRF requirements where applicable |
| `404` | The resource is missing or not visible to this account | Check its ID and ownership |
| `409` | A name, dependency, or state conflicts with the requested change | Inspect the current resource and resolve the conflict |
| `412` | The supplied revision is stale | Reload and reconcile the edit |
| `413` | The request exceeds the endpoint's body limit | Reduce or split the input where the operation permits it |
| `428` | A required precondition is missing | Supply the required revision or `If-Match` value |
| `429` | A bounded endpoint cannot accept more work | Back off; avoid concurrent retry loops |
| `500` | The operation failed internally or through a dependency | Record the code and consult server logs before retrying writes |
| `503` | The service or a bounded read/work queue is unavailable | Retry reads after a delay; establish the outcome before repeating writes |

Common resource codes include `RESOURCE_INVALID`, `RESOURCE_NOT_FOUND`, `RESOURCE_NAME_TAKEN`, `RESOURCE_CHANGED`, and `RESOURCE_IN_USE`. Codes use uppercase snake case; tolerate new codes without failing to display the error. A failed resource edit leaves the previous saved resource in place. A rejected campaign preparation does not launch a campaign.

One legacy inconsistency is retained: a group PUT whose body ID differs from the path ID returns `500`, while the other reusable resource families return `400`. Always send a matching ID rather than treating that response as a retryable server outage.

For an SMTP timeout after message submission, a failure response does not establish that no message was sent. Read [delivery outcomes](../user-guide/campaigns/delivery.md) before retrying.

## Field validation reports

Input and name-conflict failures can additionally include `validation`, independently of the HTTP status and top-level `code`. Existing compatibility message fields remain present. Each report contains at most 100 issues, with a schema field path and a stable lowercase reason code; it never includes submitted values. Paths such as `targets.1.email` use zero-based collection indexes. Local validation and API failures remain separate concepts: a client can reuse the same issue representation for local rules without inventing an HTTP error.

For example, an empty group name can return HTTP `400` with:

```json
{"success":false,"message":"Group name not specified","data":null,"code":"RESOURCE_INVALID","validation":{"issues":[{"field":"name","code":"required"}]}}
```

Translate known issue codes to field messages and provide a generic fallback for unknown codes. A missing report does not imply valid input: malformed JSON, authorization failures, stale revisions and some workflow failures use the existing operation-level response. Preserve their distinct recovery behavior. Do not apply a delayed report to a form the user has since changed.
