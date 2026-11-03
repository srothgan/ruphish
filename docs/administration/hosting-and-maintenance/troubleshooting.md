---
title: Troubleshoot the installation
description: Diagnose startup, access, storage, and integration problems from their symptoms.
sidebar:
  order: 34
---

Start with the service's recent output and readiness status. For Docker, run these from the installation folder:

```sh
docker compose ps
docker compose logs --tail=100 ruphish
```

For the Linux service:

```sh
sudo systemctl status ruphish --no-pager
sudo journalctl -u ruphish -n 100 --no-pager
```

Find the first startup error before repeatedly restarting. Remove any temporary administrator password from output you share.

## Ruphish will not start

| Symptom | Check and fix |
| --- | --- |
| Configuration file cannot be found | An explicit `--config` file must already exist and be readable. Check the path in the service command. Without `--config`, check the operating-system account and [default location](configuration.md#default-locations-and-explicit-configuration). |
| Configuration is rejected | Check JSON syntax, field names, environment values, and conflicting database settings. A malformed file is not repaired by environment overrides. Use the [configuration reference](configuration.md). |
| Port is already in use | Stop the other process or select a free port. In Docker, changing the left side of a port mapping changes the host port without changing the container listener. |
| Database cannot be opened | Check the selected path and directory permissions for SQLite, or credentials, host, port, TLS, and privileges for a server database. See [Databases](databases/index.md). |
| Schema or migration history is rejected | Preserve the database and backup. Confirm that the target release supports its history. Do not delete migration records or initialize over it. See [Upgrade Ruphish](upgrades.md). |
| TLS certificate fails to load | Check both certificate and key paths, permissions, and whether the pair matches. In Docker, paths refer to mounted files inside the container. |
| Log file cannot be opened | Check that the directory exists and is writable by the service account. Use `logging.filename: ""` for stderr-only output if file logging is not needed. |
| Browser assets are missing or invalid | Reinstall the matching released binary or image, which includes its interface. Check that the service is running that executable rather than an older development binary. |

If the native process works in your terminal but fails as a service, compare its operating-system user, working directory, environment, and filesystem access. Prefer absolute paths in a service configuration.

## The website does not open

Check readiness directly on the configured listener before diagnosing the proxy. Use HTTP for the documented local Docker setup and HTTPS for the default native browser listener. See [Health and logs](monitoring.md) for the probe command.

If direct access works but the public hostname does not, check DNS, firewall rules, proxy upstreams, and certificate issuance. The sign-in website must route to the administration listener; campaign pages must route to the public listener.

If sign-in or saves fail behind an HTTPS proxy, verify the browser origin in `admin_server.trusted_origins` and preserve the public `Host` header. Forwarded campaign headers do not configure browser-origin permission. Follow [Network access and HTTPS](network-and-https.md).

## Docker is running but unhealthy

Check whether the health probe still uses the correct address and scheme. A changed port, campaign-only mode, or private HTTPS certificate may need `RUPHISH_HEALTH_URL` or `RUPHISH_HEALTH_CA`.

If the probe address is correct, inspect startup and database errors. Readiness needs a successful database read; a running process alone is insufficient. A database upgrade may also need a longer startup grace period. Do not expect the restart policy to restart a container merely because it is unhealthy.

## My accounts or campaigns have disappeared

Stop and check which database Ruphish selected before creating new accounts or importing data. Common causes are a different native user account, a changed `db_path`, or a different Compose project name that selected a new volume.

Inspect the startup paths and deployment configuration. Return to the original storage or follow [Back up and restore](backup-and-restore.md). Do not delete the old volume while investigating.

## Campaign links do not work

Test from the recipient's network. A `127.0.0.1` link only works on the device running Ruphish. For hosted campaigns, compare the registered origin with the URL's scheme, hostname, and port.

Check that the domain is enabled and meets the company's verification policy, the campaign is still active, and the proxy forwards the campaign host and protocol from a trusted source. Adding a domain in Ruphish does not configure DNS or HTTPS. Follow [Domain setup](../../user-guide/resources/domains.md).

## Mail or integrations fail while readiness is healthy

Readiness does not test external services. Open the relevant resource or campaign delivery page and use its reported outcome:

| Problem | Next check |
| --- | --- |
| Sending fails or is uncertain | Check the sending profile and provider credentials, then [campaign delivery](../../user-guide/campaigns/delivery.md). Do not resend an uncertain submission without reviewing whether the provider accepted it. |
| An internal service is blocked | Allow its intended IP or CIDR under the appropriate [internal-service policy](outbound-connections.md), restart, and repeat the connection test. |
| Reporting mailboxes or webhooks are missing from navigation | An administrator must enable the feature in [Company settings](../team-administration/settings.md). |
| A reporting mailbox has stopped | Check whether it is enabled or paused, its credentials and folder, and [mailbox processing status](../../user-guide/resources/reporting-mailboxes.md). |
| A webhook is failing | Check the endpoint's delivery history, receiver status, and signing secret. Follow [Webhooks](../../user-guide/resources/webhooks.md). |

For campaign-building and resource-form problems, use [User guide troubleshooting](../../user-guide/troubleshooting.md). For account access, ask another administrator to review the account's lock and password-setup state using [Manage accounts](../team-administration/accounts.md).
