---
title: HTTP API
description: Automate Ruphish with its public API and understand the separate authentication and internal interfaces.
sidebar:
  order: 1
---

Use the HTTP API to manage campaign resources, launch campaigns, read results, and configure integrations. Requests go to the **administrative listener**, not the participant-facing listener.

## Three interfaces, three purposes

Ruphish splits its HTTP interfaces into **`/api/`, `/auth/`, and `/internal/`**. This separation is part of the application architecture and matters when choosing an endpoint.

| Prefix | Purpose | Intended consumer |
| --- | --- | --- |
| `/api/` | Public resource and workflow API using documented Ruphish contracts | External integrations and the admin UI |
| `/auth/` | Browser authentication and session lifecycle | The admin UI and its sign-in flow |
| `/internal/` | Browser-specific account and application operations | The admin UI; not a supported external automation contract |

Use an API key with `/api/` for scripts. Do not automate the browser login flow or depend on `/internal/` routes just because they appear in browser network requests. Browser sessions have their own cookie and CSRF requirements. See [authentication](authentication.md).

Operational monitoring uses separate unauthenticated [`/health/live` and `/health/ready` probes](../administration/health.md) on every enabled listener. They are outside these administrator API namespaces.

## Make your first request

After creating an account and completing its password setup, obtain its API key from account settings. The examples use curl and POSIX shell syntax. Replace the dummy values below with your administrative HTTPS origin, without a trailing slash, and your API key. Keep real keys out of saved scripts, shared examples, and shell history.

```sh
export RUPHISH_URL='https://ruphish.example.com'
export RUPHISH_API_KEY='replace-with-your-api-key'

curl --header "Authorization: Bearer $RUPHISH_API_KEY" \
  "$RUPHISH_URL/api/groups/"
```

A successful request returns a JSON array of your groups; a new account receives `[]`.

If your installation uses a private CA, supply its certificate file explicitly:

```sh
export RUPHISH_CA_FILE='/path/to/your/ca.pem'

curl --cacert "$RUPHISH_CA_FILE" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  "$RUPHISH_URL/api/groups/"
```

Include `--cacert "$RUPHISH_CA_FILE"` in subsequent curl requests when using that private CA. Publicly trusted certificates normally need no custom CA file. Keep certificate validation enabled.

These variables are conventions for the example commands, not automatic Ruphish configuration overrides. In PowerShell, set variables with `$env:RUPHISH_URL = 'https://ruphish.example.com'` and read them as `$env:RUPHISH_URL`; use `curl.exe` and put the request on one line instead of using POSIX backslash continuations.

## Choose a resource

| Task | Reference |
| --- | --- |
| Understand requests, revisions, and failures | [Conventions](conventions.md), [errors](errors.md) |
| Manage reusable campaign content | [Groups](groups.md), [templates](templates.md), [landing pages](landing-pages.md), [sending profiles](sending-profiles.md) |
| Import or preview content | [Imports and previews](imports-and-previews.md) |
| Manage static explanations and campaign destinations | [Learning pages](learning-pages.md) |
| Select bundled campaign content | [Starter templates](starter-templates.md) |
| Prepare and launch a campaign | [Campaigns](campaigns.md), [campaign drafts](campaign-drafts.md) |
| Control installation policy or close active campaigns | [Company settings](settings.md) |
| Verify a campaign origin | [Domains](domains.md) |
| Read outcomes or download reports | [Analysis and exports](analysis-and-exports.md), [overview](overview.md) |
| Manage application access | [Users and API keys](users-and-api-keys.md) |
| Receive events or process reported mail | [Webhooks](webhooks.md), [reporting mailboxes](reporting-mailboxes.md) |
| Integrate with participant reporting or understand tracking requests | [Participant endpoints](participant-endpoints.md) |

Ruphish adds draft persistence, campaign preparation and delivery controls, domain verification, analysis and exports, dashboard aggregates, and multiple reporting mailboxes. These are distinct from the compatible legacy resource endpoints. The [compatibility guide](../migration/compatibility.md) explains that boundary.

These references describe the current source checkout. They are maintained from handlers, wire types, and tests; they are not a generated OpenAPI specification. Examples are illustrative unless their page explicitly records execution.
