---
title: Imports, previews, and test sends
description: CSV and content imports, HTML previews, SMTP connection checks, and test messages.
sidebar:
  order: 18
---

All operations require a Bearer API key with `modify_objects`. Imports return editable content; they do not create saved resources. Save returned content through the corresponding resource endpoint. Requests on this page have a 32 MiB HTTP body limit; individual import formats impose the narrower limits below.

## POST `/api/import/group/review`

Parse CSV and return valid recipients alongside rejected rows.

**Request:** resources::request::CsvRequest. **Response:** `200 OK`, `resources::CsvReview` as `application/json`.

Request body (`request.json`):

```json
{
  "content": "Email,First Name,Last Name,Position\nada@example.com,Ada,Lovelace,Engineer\n",
  "columns": [
    "Email",
    "First Name",
    "Last Name",
    "Position"
  ]
}
```

```sh
curl --request POST "$RUPHISH_URL/api/import/group/review" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "targets": [
    {
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer"
    }
  ],
  "columns": [
    "Email",
    "First Name",
    "Last Name",
    "Position"
  ],
  "errors": []
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| content | string | CSV source, at most 8 MiB and 100,000 data rows. |
| columns | string[4] or null | Source-column mapping in the order email, first name, last name, position. Null or omission uses the default names. Header matching ignores case, surrounding whitespace, spaces, and underscores. |
| targets | Recipient[] | Valid rows, with `email`, `first_name`, `last_name`, and `position`. |
| columns (response) | string[] | Available header names in the uploaded source. |
| errors | CsvError[] | Rejected rows; an empty array means none were rejected. |

### CSV error fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `row` | `integer` | Source row number. |
| `code` | `errors::ErrorCode` | Stable rejection code. |
| `message` | `string` | Human-readable diagnostic. |
| `fields` | `string[]` | Source values supplied by the administrator. |

`code` is `CSV_ROW_ENCODING`, `CSV_ROW_COLUMNS`, or `CSV_ROW_EMAIL`. Data rows start at row 2 because row 1 contains headers.

## POST `/api/import/group`

Parse CSV file parts and return recipients. **Request:** `multipart/form-data`. **Response:** `200 OK`, `Vec<resources::details::Recipient>` as JSON. Any invalid row fails the import with `400 CSV_ROWS_INVALID`; use review to inspect rejected rows.

Save this as `recipients.csv`:

```csv
Email,First Name,Last Name,Position
ada@example.com,Ada,Lovelace,Engineer
```

```sh
curl --request POST "$RUPHISH_URL/api/import/group" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --form "file=@recipients.csv;type=text/csv"
```

The multipart field name is not significant; file parts must have filenames. Example response:

```json
[
  {
    "email": "ada@example.com",
    "first_name": "Ada",
    "last_name": "Lovelace",
    "position": "Engineer"
  }
]
```

## POST `/api/import/email`

Extract subject, text, and HTML from an RFC 822 message.

**Request:** resources::request::EmailRequest. **Response:** `200 OK`, `resources::EmailImport` as `application/json`.

Request body (`request.json`):

```json
{
  "content": "Subject: Training notification\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=example\r\n\r\n--example\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello Ada\r\n--example\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello Ada</p>\r\n--example--\r\n",
  "convert_links": false
}
```

```sh
curl --request POST "$RUPHISH_URL/api/import/email" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "subject": "Training notification",
  "text": "Hello Ada",
  "html": "<p>Hello Ada</p>"
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| content | string | Raw MIME email, at most 8 MiB. |
| convert_links | boolean | Defaults to false. When true, rewrites HTML anchor href attributes to `{{.URL}}`. |
| subject / text / html | string | Imported subject and body content; no resource ID is assigned. |

## POST `/api/import/site`

Fetch and transform an HTTP(S) page into editable source.

**Request:** resources::request::SiteRequest. **Response:** `200 OK`, `resources::HtmlImport` as `application/json`.

The response example assumes that URL returns `<html><head></head><body><h1>Training</h1></body></html>`. `url` is subject to the installation’s outbound-connection policy. `html` contains transformed source with resolved base and form-origin information.

Request body (`request.json`):

```json
{
  "url": "https://pages.example.com/training"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/import/site" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "html": "<html><head><base href=\"https://pages.example.com/training\"></head><body><h1>Training</h1></body></html>"
}
```

## POST `/api/import/page-bundle`

Import a base64-encoded ZIP containing index.html and its local assets.

**Request:** resources::request::PageBundleRequest. **Response:** `200 OK`, `resources::HtmlImport` as `application/json`.

This ZIP contains only `index.html`, with `<h1>Training</h1>`. The JSON object accepts only `content`. ZIP input and expanded contents are each limited to 8 MiB, with at most 256 entries. `index.html` must be at the root. Local asset references are embedded without fetching external resources.

Request body (`request.json`):

```json
{
  "content": "UEsDBBQAAAAAAABgY13y7KnPEQAAABEAAAAKAAAAaW5kZXguaHRtbDxoMT5UcmFpbmluZzwvaDE+UEsBAhQAFAAAAAAAAGBjXfLsqc8RAAAAEQAAAAoAAAAAAAAAAAAAAIABAAAAAGluZGV4Lmh0bWxQSwUGAAAAAAEAAQA4AAAAOQAAAAAA"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/import/page-bundle" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "html": "<html><head></head><body><h1>Training</h1></body></html>"
}
```

## POST `/api/util/preview`

Render email HTML with sample recipient values without adding an open tracker.

**Request:** resources::request::PreviewRequest. **Response:** `200 OK`, `resources::PagePreview` as `application/json`.

Request body (`request.json`):

```json
{
  "html": "<p>Hello {{.FirstName}}</p>",
  "recipient": {
    "email": "ada@example.com",
    "first_name": "Ada",
    "last_name": "Lovelace",
    "position": "Engineer"
  }
}
```

```sh
curl --request POST "$RUPHISH_URL/api/util/preview" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "html": "<p>Hello Ada</p>",
  "warnings": []
}
```

### Preview request and response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `html` | `string` | Authored HTML source. |
| `recipient` | `Recipient` | Recipient values used for the preview. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `html` | `string` | Prepared HTML. |
| `warnings` | `resources::details::PageWarning[]` | Findings, including a required empty list when no issue is found. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `code` | `string` | Stable diagnostic code translated by the UI. |
| `control` | `integer or null` | One-based input position in the processed document. |

The optional `recipient` defaults to empty strings and has `email`, `first_name`, `last_name`, and `position`. Warnings are advisory; `control` is a one-based input index or null for a document-level finding.

## POST `/api/pages/preview`

Prepare a landing-page preview with the managed collector in nontracking mode.

**Request:** resources::request::PreviewRequest. **Response:** `200 OK`, `resources::PagePreview` as `application/json`.

The complete response below includes the generated collector. Render authored landing HTML in a sandboxed frame with `sandbox="allow-scripts allow-forms"`; do not grant it your integration UI’s origin.

Request body (`request.json`):

```json
{
  "html": "<p>Hello {{.FirstName}}</p>",
  "recipient": {
    "email": "ada@example.com",
    "first_name": "Ada",
    "last_name": "Lovelace",
    "position": "Engineer"
  }
}
```

```sh
curl --request POST "$RUPHISH_URL/api/pages/preview" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

<details>
<summary>Example JSON response (complete generated content)</summary>

```json
{
  "html": "<html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' https: http: data: blob:; style-src 'unsafe-inline' https: http: data: blob:; img-src https: http: data: blob:; font-src https: http: data: blob:; connect-src https: http:; form-action 'none'; base-uri https: http:; object-src 'none'; frame-src 'none'; worker-src 'none'\"><meta name=\"referrer\" content=\"no-referrer\"><base href=\"https://preview.invalid/\"><script data-ruphish-collector>(()=>{const endpoint=null;const messages={\n  \"en\": {\n    \"preview\": \"Preview submission only. No interaction was recorded.\",\n    \"submitting\": \"Submitting\u00e2\u20ac\u00a6\",\n    \"submitted\": \"Submitted.\",\n    \"unconfirmed\": \"Submission could not be confirmed. Please try again.\",\n    \"retry\": \"Retry this submission\"\n  },\n  \"de\": {\n    \"preview\": \"Nur eine Vorschau. Es wurde keine Interaktion aufgezeichnet.\",\n    \"submitting\": \"Wird \u00c3\u00bcbermittelt\u00e2\u20ac\u00a6\",\n    \"submitted\": \"\u00c3\u0153bermittelt.\",\n    \"unconfirmed\": \"\u00c3\u0153bermittlung nicht best\u00c3\u00a4tigt. Bitte erneut versuchen.\",\n    \"retry\": \"Diese \u00c3\u0153bermittlung erneut versuchen\"\n  }\n}\n;// Installed before authored scripts. Only this built-in collector's requests have\n// the presence-only contract; authored code can make independent requests.\nconst passwordControls = new WeakSet(document.querySelectorAll('input[type=\"password\"]'));\n// Remember password roles across reveal toggles, including newly inserted controls.\nconst remember = (root) => {\n  if (root instanceof HTMLInputElement && root.type === 'password') {\n    passwordControls.add(root);\n  }\n  root\n    .querySelectorAll?.('input[type=\"password\"]')\n    .forEach((control) => passwordControls.add(control));\n};\nconst rememberChanges = (records) =>\n  records.forEach((record) => {\n    if (record.type === 'attributes' && record.oldValue?.toLowerCase() === 'password') {\n      passwordControls.add(record.target);\n    }\n    remember(record.target);\n    record.addedNodes.forEach(remember);\n  });\nconst observer = new MutationObserver(rememberChanges);\nobserver.observe(document, {\n  subtree: true,\n  childList: true,\n  attributes: true,\n  attributeOldValue: true,\n  attributeFilter: ['type'],\n});\nconst pending = new Set();\nlet destination = null;\nconst candidates = [\n  document.documentElement.lang,\n  ...(navigator.languages || [navigator.language]),\n];\nconst language =\n  candidates\n    .map((value) => (value || '').split('-')[0].toLowerCase())\n    .find((value) => Object.hasOwn(messages, value)) || 'en';\nconst text = messages[language];\nwindow.addEventListener('submit', (event) => {\n  const form = event.target;\n  if (!(form instanceof HTMLFormElement) || !form.isConnected || event.defaultPrevented) {\n    return;\n  }\n  event.preventDefault();\n  // Custom flows cancel their submit event and call ruphish.submit(form) only\n  // after their own validation/final step. Native navigation stays blocked by CSP.\n  void submit(form, false);\n});\n\nObject.defineProperty(window, 'ruphish', {\n  value: Object.freeze({ submit: (form) => submit(form, true) }),\n});\n\nasync function submit(form, validate) {\n  if (!(form instanceof HTMLFormElement) || form.ownerDocument !== document || !form.isConnected) {\n    throw new TypeError('Expected a connected form in this page');\n  }\n  if (validate && !form.reportValidity()) {\n    return false;\n  }\n  // Authored code may insert/reveal a password and submit within the same task.\n  rememberChanges(observer.takeRecords());\n  const notice = document.createElement('p');\n  notice.setAttribute('role', 'status');\n  const status = document.createElement('span');\n  const retry = document.createElement('button');\n  retry.type = 'button';\n  retry.textContent = text.retry;\n  retry.hidden = true;\n  notice.append(status, retry);\n  form.append(notice);\n  if (endpoint === null) {\n    status.textContent = text.preview;\n    return false;\n  }\n  try {\n    const bytes = crypto.getRandomValues(new Uint8Array(16));\n    const event_id = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');\n    const password_present = Array.from(form.elements).some(\n      (control) =>\n        control instanceof HTMLInputElement &&\n        (passwordControls.has(control) || control.type === 'password') &&\n        !control.matches(':disabled') &&\n        control.name !== '' &&\n        !control.closest('datalist') &&\n        control.value !== '',\n    );\n    const attempt = {\n      payload: { event_id, password_present },\n      status,\n      retry,\n      busy: false,\n      done: false,\n    };\n    pending.add(attempt);\n    retry.addEventListener('click', () => deliver(attempt));\n    return await deliver(attempt);\n  } catch {\n    status.textContent = text.unconfirmed;\n    return false;\n  }\n}\n\nasync function deliver(attempt) {\n  if (attempt.busy || attempt.done) {\n    return attempt.done;\n  }\n  attempt.busy = true;\n  attempt.retry.hidden = true;\n  attempt.status.textContent = text.submitting;\n  // Cover both response headers and body, allowing time beyond the server's 10s\n  // deadline. A timeout means unconfirmed, not necessarily unrecorded.\n  const controller = new AbortController();\n  const timeout = setTimeout(() => controller.abort(), 15_000);\n  try {\n    const response = await fetch(endpoint, {\n      method: 'POST',\n      credentials: 'omit',\n      redirect: 'error',\n      cache: 'no-store',\n      headers: { 'Content-Type': 'application/json' },\n      body: JSON.stringify(attempt.payload),\n      signal: controller.signal,\n    });\n    if (!response.ok) {\n      throw new Error('submission_failed');\n    }\n    const result = await response.json();\n    if (controller.signal.aborted || result?.success !== true) {\n      throw new Error('submission_unconfirmed');\n    }\n    attempt.done = true;\n    pending.delete(attempt);\n    attempt.status.textContent = text.submitted;\n    if (typeof result.redirect === 'string') {\n      destination = result.redirect;\n    }\n    // Do not abandon another observed submission merely because this one finished first.\n    if (pending.size === 0 && destination !== null) {\n      location.assign(destination);\n    }\n    return true;\n  } catch {\n    attempt.status.textContent = text.unconfirmed;\n    attempt.retry.hidden = false;\n    return false;\n  } finally {\n    clearTimeout(timeout);\n    attempt.busy = false;\n  }\n}\n })();</script></head><body><p>Hello Ada</p></body></html>",
  "warnings": []
}
```

</details>

## POST `/api/pages/validate`

Return landing-page diagnostics without a prepared HTML document.

**Request:** resources::request::PreviewRequest. **Response:** `200 OK`, `resources::PageDiagnostics` as `application/json`.

Request body (`request.json`):

```json
{
  "html": "<p>Hello {{.FirstName}}</p>",
  "recipient": {
    "email": "ada@example.com",
    "first_name": "Ada",
    "last_name": "Lovelace",
    "position": "Engineer"
  }
}
```

```sh
curl --request POST "$RUPHISH_URL/api/pages/validate" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "warnings": []
}
```

## POST `/api/learning-pages/preview`

Prepare static learning HTML and return diagnostics.

**Request:** resources::request::PreviewRequest. **Response:** `200 OK`, `resources::PagePreview` as `application/json`.

Both learning-page endpoints return `html` and `warnings`. Learning content is static and does not expand recipient template expressions.

Request body (`request.json`):

```json
{
  "html": "<h1>Training complete</h1>"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/learning-pages/preview" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "html": "<!doctypehtml><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body><h1>Training complete</h1></body></html>",
  "warnings": []
}
```

## POST `/api/learning-pages/validate`

Prepare static learning HTML and return diagnostics.

**Request:** resources::request::PreviewRequest. **Response:** `200 OK`, `resources::PagePreview` as `application/json`.

Both learning-page endpoints return `html` and `warnings`. Learning content is static and does not expand recipient template expressions.

Request body (`request.json`):

```json
{
  "html": "<h1>Training complete</h1>"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/learning-pages/validate" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "html": "<!doctypehtml><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body><h1>Training complete</h1></body></html>",
  "warnings": []
}
```

## POST `/api/util/test_connection`

Test SMTP connectivity and authentication without sending a message.

**Request:** resources::request::SendingProfile. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

The request is a sending-profile write object. With an existing ID, an omitted password preserves the saved credential only within its saved connection settings. Changing the host, port, username, or disabling certificate verification requires an explicit password; otherwise the request returns `400` with validation issue `password: credential_reentry_required` before any connection. See [sending profile fields](sending-profiles.md#request-fields).

Request body (`request.json`):

```json
{
  "id": 1,
  "name": "Training sender",
  "interface_type": "SMTP",
  "host": "smtp.example.com:587",
  "username": "training@example.com",
  "from_address": "training@example.com",
  "ignore_cert_errors": false,
  "headers": []
}
```

```sh
curl --request POST "$RUPHISH_URL/api/util/test_connection" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "success": true,
  "message": "SMTP connection verified",
  "data": null
}
```

## POST `/api/util/send_test_email`

Send a saved template and page to one explicit test recipient.

**Request:** resources::request::TestRequest. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

Request body (`request.json`):

```json
{
  "template": {
    "name": "Training message"
  },
  "page": {
    "name": "Training page"
  },
  "smtp": {
    "source": "saved",
    "id": 1
  },
  "url": "http://127.0.0.1:8081",
  "email": "ada@example.com",
  "first_name": "Ada",
  "last_name": "Lovelace",
  "position": "Engineer"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/util/send_test_email" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "success": true,
  "message": "Email Sent",
  "data": null
}
```

### Test-send fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `template` | `EmailTemplate` | Selected email template. |
| `page` | `LandingPage` | Selected landing page. |
| `smtp` | `super::TestProfile` | Explicit sending-profile selection. |
| `url` | `string` | HTTP(S) destination URL, up to 1,000 bytes, without credentials or a fragment. |
| `email` | `string` | Recipient email address. |
| `first_name` | `string` | Recipient given name. |
| `last_name` | `string` | Recipient family name. |
| `position` | `string` | Recipient position. |

`template.name` and `page.name` select saved resources; inline body fields in those objects are not used as replacement content. `smtp` is either `{"source":"saved","id":1}` or `{"source":"draft","profile":{...}}`, where `profile` is the [sending-profile write object](sending-profiles.md#request-fields). The draft form can use an existing ID to preserve an omitted password only within its saved connection settings, under the same rule as connection tests. Recipient fields control this test’s address and personalization. Success acknowledges the send operation; a lost response does not make repeating it safe.
