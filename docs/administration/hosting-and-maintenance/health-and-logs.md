---
title: Health and logs
description: Check whether Ruphish is ready, inspect service output, and manage log retention.
sidebar:
  order: 33
---

Use health probes to check whether Ruphish is responding and ready to serve requests. Use logs and the relevant campaign or integration page to find out why something failed.

## Check readiness

For the local HTTP Docker setup:

```sh
curl --fail --silent --show-error --max-time 3 http://127.0.0.1:3333/health/ready
```

In Windows PowerShell, use `curl.exe` for these curl options. For a hosted installation, use its actual HTTPS address. If it uses a private CA, pass its PEM file with `--cacert` rather than disabling certificate validation.

Both the browser/API listener and the campaign listener provide these unauthenticated endpoints:

| Endpoint | Successful response | Meaning |
| --- | --- | --- |
| `GET /health/live` | `200` and `{"status":"live"}` | The HTTP server can answer. |
| `GET /health/ready` | `200` and `{"status":"ready"}` | Initialization and required tasks have started, shutdown is not underway, and a bounded database read succeeds. |

Readiness returns `503` with `{"status":"not_ready"}` when those conditions are not met. During early startup, the server may not yet accept connections. Configure a timeout and allow a startup grace period, especially during upgrades.

Require exactly HTTP `200` in a monitor. A redirect or sign-in page is not readiness. `HEAD` returns the same status without a body. The paths have no trailing slash; `/health`, `/api/health`, and `/health/ready/` are not aliases.

A healthy instance can still have failed SMTP delivery, a paused reporting mailbox, or an unreachable webhook. Probes do not contact these services, prove database writes, or verify public DNS and HTTPS. Check the relevant [campaign delivery](../../user-guide/campaigns/monitor-sending.md) or resource page separately.

Probe at normal monitoring intervals instead of issuing simultaneous checks to both listeners. A readiness query has a one-second limit, and overlapping queries can return `503`. Monitor the external HTTPS address separately when you also need to verify proxy and network routing.

## Docker health status

The release image includes a readiness check against `http://127.0.0.1:3333/health/ready`. It runs every 30 seconds, allows a 60-second startup period, and marks the container unhealthy after three consecutive failures. Inspect status and recent application logs with:

```sh
docker compose ps
docker compose logs --tail=100 --follow ruphish
```

Press **Ctrl+C** to stop following output. An unhealthy container is not automatically restarted by the restart policy; investigate the service and database first.

If you change the listener used by the probe, also change its URL. For example, a campaign-only container using HTTP port 8081 needs:

```yaml
environment:
  RUPHISH_HEALTH_URL: "http://127.0.0.1:8081/health/ready"
```

For HTTPS with a private CA, set `RUPHISH_HEALTH_CA` to a mounted PEM CA file and use a URL whose hostname matches the certificate. These two variables configure the Docker probe only, not the Ruphish listener. Recreate the container after changing Compose settings.

## Read service logs

For the Linux systemd service:

```sh
sudo journalctl -u ruphish -n 100 --no-pager
sudo journalctl -u ruphish --follow
```

For a foreground native installation, read terminal output or the log file shown at startup. The generated configuration places it in the platform's [log directory](configuration-reference.md#default-locations-and-explicit-configuration).

`RUPHISH_READY` appears in startup output independently of the selected log level. For ordinary operation, use `info`. Temporarily use `debug` or `trace` when you need more application detail, then return to `info` after diagnosing the issue. These levels do not enable dependency logging.

## Set log retention

The Docker examples use the rotating `local` logging driver. Its settings live under the service in Compose:

```yaml
logging:
  driver: local
  options:
    max-size: "10m"
    max-file: "5"
```

Recreate the container to apply changed logging-driver settings. Docker manages this output separately from any application log file.

For native file logging, use a writable filename in `config.json`:

```json
{
  "logging": {
    "filename": "/var/log/ruphish/ruphish.log",
    "level": "info",
    "max_size_bytes": 10485760,
    "max_files": 5
  }
}
```

Create the parent directory and grant the service account write access before restarting. An empty filename selects stderr only. With a file configured, Ruphish writes to both the file and stderr.

The file rotates at approximately 10 MiB and retains five archives in addition to the active file. An individual oversized record stays intact. Archives use increasing numeric suffixes, with higher numbers being newer. Oldest archives are deleted to meet the configured count; this is a size policy, not a retention period in days.

Use ordinary files, not symlinks or device files. Do not remove the log lock or run a second file rotator against a file Ruphish is writing. Processes sharing a log file must use the same retention settings.

An initial file-open failure stops startup. A later write or rotation failure is reported to stderr; stderr logging continues, and file output is retried on later records. Check directory permissions, free space, and archive paths. Records missed by the file are not replayed into it afterward.

## Share useful diagnostics

Record the Ruphish version, operation, time, and relevant status or error code. Application request logs avoid raw request bodies and query values, but startup output can contain the generated temporary administrator password. Remove it before sharing Docker or journal output.

Check proxy, database, and mail-server logs separately; Ruphish cannot redact those systems' output. Operational logs help diagnose failures but are not a complete audit history of every user action.
