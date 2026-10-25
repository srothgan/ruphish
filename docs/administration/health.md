---
title: Health probes
description: Monitor process liveness and application readiness.
sidebar:
  order: 9
---

Ruphish exposes two unauthenticated operational endpoints on every enabled HTTP or HTTPS listener. They work in `all`, `admin` and `phish` modes and follow each listener's configured address and TLS settings. They require no session, API key, CSRF token or verified campaign domain and never set cookies.

| Request | Success | Unavailable |
| --- | --- | --- |
| `GET /health/live` | `200`, `{"status":"live"}` | An unresponsive or stopped HTTP server cannot answer; the client must impose a timeout. |
| `GET /health/ready` | `200`, `{"status":"ready"}` | `503`, `{"status":"not_ready"}` |

GET responses use `Content-Type: application/json` and `Cache-Control: no-store`. `HEAD` returns the same status and headers without a body. Other methods return an empty `405` response with `Cache-Control: no-store`. There is no `/health`, `/api/health` or trailing-slash alias. Exact `/health/live` and `/health/ready` paths are reserved on the public listener, including requests carrying `rid`; these requests never render a campaign or record an interaction. Other campaign paths retain their existing behavior. Existing campaign URLs that use either reserved path must use a different path.

## What readiness establishes

Readiness requires completed initialization, started required listener/worker tasks, and no shutdown decision. It reads the singleton settings identifier through the application's existing database pool, using the configured SQLite, MySQL or PostgreSQL provider. A missing settings table/row, unavailable connection or query error returns `503`. The one-second deadline covers both connection acquisition and execution. Only one readiness query runs per process at a time; concurrent probes return `503` instead of queuing. Successful results are not cached. Retry probes at normal monitoring intervals rather than issuing simultaneous requests to both listeners.

The runtime supervisor owns lifecycle state. It clears readiness before cancelling workers and draining HTTP connections, including unexpected required-task exit. Liveness remains successful while HTTP can respond. Once a listener stops accepting requests, connection failures replace HTTP responses. A probe completing after the shutdown decision cannot restore readiness.

Migrations, configuration, asset validation, certificates and administrator bootstrap complete before HTTP serving starts. During that early startup period a probe can time out or fail to connect; a bound TCP port alone does not prove readiness. Once HTTP serving begins, readiness remains `503` until the supervisor has started the required tasks. Allow a startup grace period sized for database upgrades. Startup errors retain the normal nonzero process exit.

Disabled workers and empty queues are healthy. Ordinary SMTP, IMAP, webhook and domain-verification failures follow their existing retry/error behavior and do not make the whole installation unready. Probes do not contact these external services or send messages. Worker supervision detects task exits; it does not detect a stuck task that remains running. Database read success does not establish write availability, full schema integrity, delivery success or externally reachable DNS/TLS/load-balancer paths.

The responses contain no configuration, service credentials, participant data or diagnostic errors. Health requests bypass ordinary session and campaign handling and do not generate per-probe access-log entries. Use existing application diagnostics for operational errors. The probes are not authenticated; restrict network access at your existing proxy/firewall if they should not be publicly reachable. They do not expose the administrative application on the public listener.

## Native monitoring and Docker

For a listener configured on plain HTTP loopback, a direct probe is:

```sh
curl --fail --silent --show-error --max-time 3 http://127.0.0.1:3333/health/ready
```

Use the configured port and scheme. For HTTPS, use a hostname covered by the certificate and a trusted CA (`--cacert` when necessary); do not disable verification as a production default. A monitor should require exactly `200`, rather than treat redirects or a login page as success. Local probes check the instance; monitoring the external deployment path is a separate check.

Use `/health/live` for process responsiveness and `/health/ready` for traffic eligibility. The [local Docker image](../contributing/testing/local-fixture.md#local-image-definition) now includes a readiness `HEALTHCHECK`: a 30-second interval, three-second command timeout, 60-second startup grace period and three consecutive failures. Its curl probe requires exactly `200` within two seconds, bypasses proxies, rejects redirects and validates HTTPS certificates. `RUPHISH_HEALTH_URL` overrides the default `http://127.0.0.1:3333/health/ready`; `RUPHISH_HEALTH_CA` supplies a mounted private CA file. These settings affect only the probe; configure the application listener separately. Adjust Docker's startup grace period for expected migration duration. Production release packaging remains separate. Standalone Docker health status does not itself restart a container; restart policies apply when the process exits. See [Docker HEALTHCHECK](https://docs.docker.com/reference/dockerfile/#healthcheck) and [restart policies](https://docs.docker.com/engine/containers/start-containers-automatically/).
