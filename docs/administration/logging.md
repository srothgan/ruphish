---
title: Read application logs
description: Configure log destinations and collect useful diagnostics without exposing request values.
sidebar:
  order: 8
---

Set `logging.filename` to append to a file, or leave it empty for stderr only. With a file configured, logs also go to stderr. Generated configurations select `ruphish.log` in the OS log directory. The process must be able to write the selected path; relative paths follow the configuration directory for automatic discovery and the working directory with explicit `--config`. See [configuration locations](configuration.md#default-locations-and-explicit-configuration).

Use `info` for ordinary operation. `debug` and `trace` increase application diagnostics, but do not enable dependency logging. `warn`/`warning`, `error`, `fatal`, and `panic` are accepted compatibility names; fatal/panic map to error severity.

Startup readiness is a separate stderr line beginning `RUPHISH_READY` with bound-address JSON. It is emitted independently of logging level after initialization succeeds. An open process alone is not proof of readiness.

Request logs record the method, router-owned route pattern, and response status. They omit raw request paths/queries, headers, bodies, and peer diagnostics. Operational boundaries use safe categories instead of raw SMTP/SQL responses. Generated temporary administrator passwords are deliberately delivered through startup stderr, separately from the application log file, and can therefore appear in container logs or redirected output. Supplied passwords and API tokens are never printed. Startup failures identify the operation and safe failure category; raw database, protocol and input text remains excluded.

When collecting diagnostics, include the operation, time, safe code/status, and affected version/configuration category. Review external proxy/mail/database logs separately: application redaction does not sanitize those systems or historical copies.

## File rotation and retention

File logging rotates automatically, including for existing configurations that omit the new settings:

```json
{
  "logging": {
    "filename": "ruphish.log",
    "level": "info",
    "max_size_bytes": 10485760,
    "max_files": 5
  }
}
```

`max_size_bytes` is a positive integer byte threshold, defaulting to 10 MiB. `max_files` is a positive integer archive count, defaulting to five; the active file is additional. Ruphish rotates before the next record would exceed the threshold. A single oversized record remains intact and may exceed it, then rotates before the following record. This bounds ordinary local history to approximately 60 MiB with the defaults, rather than promising a strict byte quota or a retention period in days.

The active filename stays unchanged. Archives have increasing numeric suffixes (`ruphish.log.1`, `ruphish.log.2`, and so on); higher numbers are newer. Oldest archives are deleted to enforce retention, including when a lower limit is selected at restart. Those numeric names belong to Ruphish; do not store unrelated files under them. Existing active content is preserved until rotation. New log and lock files use private permissions. Rotation preserves the existing file's permissions, including for operator-supplied files.

File logging requires ordinary files; symbolic links, directories and device files are rejected. Use an empty filename for stderr output. A persistent `ruphish.log.lock` coordinates cooperating processes. Processes sharing a file must use the same retention settings. Do not delete that lock or apply an external file rotator while Ruphish is writing. File logging uses a bounded 4,096-record worker queue and retains its existing backpressure behavior when the queue fills. Normal shutdown asks the worker to drain; forced termination or an exhausted shutdown timeout can lose queued records. These files are operational diagnostics, not a durable audit archive.

Initial file-open failures stop startup with a diagnostic. Later file-write/rotation failures report a safe error category to stderr, leave stderr logging active, and retry file output on later records. Recovery is reported once file writing resumes. Records that could not reach the file are not replayed from stderr. Check directory access, free space and obstructed archive paths if this occurs. See [troubleshooting](troubleshooting.md).

## Docker output

For Docker, use `"filename": ""` to send application logs only to stderr, and configure retention on Docker's logging driver. The local Compose fixture generates that setting and uses:

```yaml
logging:
  driver: local
  options:
    max-size: "10m"
    max-file: "5"
```

Docker's `max-file` counts its total retained files; Ruphish's `max_files` counts archives in addition to the active file. The Docker driver manages its own storage and compression. Application file rotation never limits Docker's captured output. If you explicitly enable a file in Docker, both copies exist and need their own retention policy. A Dockerfile cannot select the host logging driver; configure it in Compose or your deployment environment. Existing containers keep their logging-driver settings until recreated, and existing application configurations are not silently rewritten by this change.

## Basis for the defaults

[NIST SP 800-92](https://csrc.nist.gov/pubs/sp/800/92/final) distinguishes manageable local log rotation from longer-term retention and preservation. [OWASP's Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html) calls for protecting logs, preventing disk exhaustion and testing logging failures. [Apache Log4j's rolling-file guidance](https://logging.apache.org/log4j/2.x/manual/appenders/rolling-file.html) describes size-triggered rotation and the hazards of independently coordinated writers. [Docker recommends the rotating local driver](https://docs.docker.com/engine/logging/configure/) to avoid unbounded container logs. The 10 MiB/five-archive native defaults are Ruphish's operational choice, not a universal standard. Configure suitable retention or forward logs to your collection system when you need a longer history.
