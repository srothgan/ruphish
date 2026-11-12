---
title: Errors
description: HTTP failures, structured error codes, and field-level diagnostics.
sidebar:
  order: 3
---

Use the HTTP status and stable `code` for control flow. `message` is diagnostic text and can change. Clients must tolerate new error codes.

## Error object

Example `412 Precondition Failed` response:

```json
{"success":false,"message":"This resource changed. Your request was not applied. Review the current version before trying again.","data":null,"code":"RESOURCE_CHANGED"}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| success | boolean | False for application error responses. |
| message | string | Human-readable diagnostic; do not parse it. |
| data | null | Retained envelope member. |
| code | string | Stable uppercase error category. |
| validation | object, optional | Field-level failures, when available. |

A validation response can include:

```json
{
  "success": false,
  "message": "No username provided",
  "data": null,
  "code": "USERNAME_REQUIRED",
  "validation": {
    "issues": [
      {
        "field": "username",
        "code": "required"
      }
    ]
  }
}
```

`validation.issues` is a bounded array of objects with `field` (dot-separated path with zero-based array positions) and `code` (validation reason). Submitted field values are not echoed.

For mail connection changes, `credential_reentry_required` on `password` means the saved credential cannot be reused with the requested connection settings. Supply a password explicitly or restore the saved settings. The response uses HTTP `400` and top-level code `RESOURCE_INVALID`.

## Status handling

| HTTP status | Client action |
| --- | --- |
| 400 | Malformed or invalid input; correct the request. |
| 401 | Missing or invalid authentication; check the API key and account state. |
| 403 | Insufficient permission or account restrictions. |
| 404 | The requested object is not available. |
| 409 | Name conflict, resource in use, feature disabled, or incompatible operation identity. |
| 412 | The reviewed resource, draft, or event changed; reload before retrying. |
| 428 | A required revision precondition is missing. |
| 413 | Request exceeds the endpoint body limit. |
| 429 | Capacity or rate limit; application authentication rejections include `Retry-After`. |
| 500 / 503 | Internal failure or unavailable service; reconcile uncertain writes before retrying. |

Malformed query/body extraction and unmatched routes can return plain text instead of this envelope. Check Content-Type before decoding JSON.

## Error codes

| Code | Meaning |
| --- | --- |
| `FEATURE_DISABLED` | This feature is disabled by the administrator. |
| `DRAFT_INVALID` | The saved draft contains invalid or unsupported input. |
| `DRAFT_SCHEDULE_PAST` | The resolved draft launch time is in the past. |
| `CSV_ROW_ENCODING` | Invalid CSV encoding or record. |
| `CSV_ROW_COLUMNS` | Column count does not match the header. |
| `CSV_ROW_EMAIL` | Invalid email address. |
| `INTERNAL_ERROR` | An internal operation failed. |
| `AUTH_INVALID_CREDENTIALS` | Invalid username or password. |
| `AUTH_REQUIRED` | API authentication is required. |
| `AUTH_FORBIDDEN` | You do not have permission for this action. |
| `AUTH_API_KEY_INVALID` | The API key is missing or invalid. |
| `RATE_LIMITED` | Too many requests. Try again shortly. |
| `REQUEST_TIMEOUT` | The request timed out. Reconcile the operation before retrying. |
| `REQUEST_TOO_LARGE` | The request is too large. Reduce its size. |
| `REQUEST_INVALID` | The request is invalid. Check the supplied values. |
| `METHOD_NOT_ALLOWED` | This request method is not supported. |
| `PASSWORD_INVALID` | The current password is incorrect. |
| `PASSWORD_REQUIRED` | Enter a password. |
| `PASSWORD_TOO_SHORT` | Use a password with at least 8 UTF-8 bytes. |
| `PASSWORD_TOO_LONG` | The password exceeds the supported length. |
| `PASSWORD_CHANGE_REQUIRED` | Set your password before continuing. |
| `USERNAME_REQUIRED` | Enter a username. |
| `ROLE_REQUIRED` | Choose a role. |
| `ROLE_NOT_FOUND` | The selected role is unavailable. |
| `ACCOUNT_NOT_FOUND` | The account could not be found. |
| `USERNAME_TAKEN` | The username is already in use. |
| `LAST_ADMIN` | The last administrator must be retained. |
| `SELF_DELETION` | You cannot delete your own account. |
| `SERVICE_BUSY` | The service is busy. Try again shortly. |
| `RESOURCE_NOT_FOUND` | The requested resource could not be found. |
| `RESOURCE_NAME_TAKEN` | This resource name is already in use. |
| `RESOURCE_CHANGED` | The submitted revision does not match the current resource revision. |
| `RESOURCE_IN_USE` | A campaign that is not completed still uses this resource. |
| `RESOURCE_INVALID` | The resource is invalid. Review its content and settings. |
| `DELIVERY_FAILED` | Delivery could not be confirmed. Check the mail server before retrying. |
| `REVISION_REQUIRED` | The operation requires the current saved revision. |
| `CSV_INVALID` | Check the CSV file and its column mapping. |
| `CSV_ROWS_INVALID` | The CSV contains invalid rows. Review the import report. |
| `CAMPAIGN_INVALID` | Review the campaign settings before continuing. |
| `IDEMPOTENCY_KEY_INVALID` | The request identifier is invalid. Retry with a valid idempotency key. |
| `DELIVERY_ACTION_INVALID` | The action is invalid for the reviewed delivery state. |
| `MAILBOX_TLS_REQUIRED` | The mail server must support TLS. |
| `MAILBOX_PORT_INVALID` | Choose a valid mail server port. |
| `ANALYSIS_FILTER_INVALID` | Check the selected analysis filters. |
| `OVERVIEW_WINDOW_INVALID` | Choose an available reporting period. |
| `RESOURCE_ID_MISMATCH` | The resource identifier does not match the request. |
