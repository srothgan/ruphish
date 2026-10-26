---
title: Run and restart the application
description: Understand working directories, process modes, assets, and graceful shutdown.
sidebar:
  order: 10
---

Running Ruphish without `--config` reads or creates the OS user configuration and uses its persistent data/log paths. For a service or Docker deployment, select an explicit configuration and working directory: relative database, certificate and log paths in that file depend on the working directory. Development asset paths also depend on it. A service manager must preserve the same relationship you verified interactively. See [configuration discovery and path rules](configuration.md#default-locations-and-explicit-configuration).

Use [installation](../getting-started/installation.md) for the intended release-binary and Docker-image setup paths.

> TODO before publication: Verify the service and container lifecycle instructions against the published release binary and Docker image, including configuration paths, persistent storage, signals, and asset packaging.

For source builds, the native executable and `dist/admin/` are separate build outputs. The admin listener validates and snapshots the asset manifest at startup. Rebuild assets after UI input changes and restart to serve the new distribution. Replacing files under a running process does not replace its served snapshot.

## Process modes

`--mode all` runs both listeners. `admin` and `phish` run only their respective listener. After schema preparation, cooperating processes can share the current database. Stop all old writers before an upgrade rather than relying on the startup lock to stop them.

The campaign worker runs in modes with the admin listener unless disabled. Domain verification and integration work have their own persisted state and lifecycle. `--disable-mailer` does not mean all external integration work is disabled.

## Shutdown and outages

Use Ctrl+C, Windows Ctrl+Break, or Unix SIGTERM. Shutdown stops accepting new work, drains active requests with a bounded wait, closes database resources, and flushes owned logging. A forced kill can leave delivery outcomes uncertain; inspect the queue after restart.

Storage outages use bounded failures and worker backoff. SMTP outcome persistence is retried separately from submission; a persistence problem must not be interpreted as permission to send again. See [delivery recovery](../user-guide/campaigns/delivery.md).

The repository's Compose acceptance fixture is for local exercises and development checks. Use the distribution Docker image for a deployed installation once it is available.
