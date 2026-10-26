---
title: Read application logs
description: Configure log destinations and collect useful diagnostics without exposing request values.
sidebar:
  order: 8
---

Set `logging.filename` to append to a file, or leave it empty for stderr only. With a file configured, logs also go to stderr. The process must be able to write the selected path; relative paths follow its working directory.

Use `info` for ordinary operation. `debug` and `trace` increase application diagnostics, but do not enable dependency logging. `warn`/`warning`, `error`, `fatal`, and `panic` are accepted compatibility names; fatal/panic map to error severity.

Startup readiness is a separate stderr line beginning `RUPHISH_READY` with bound-address JSON. It is emitted independently of logging level after initialization succeeds. An open process alone is not proof of readiness.

Request logs record the method, router-owned route pattern, and response status. They omit raw request paths/queries, headers, bodies, and peer diagnostics. Operational boundaries use safe categories instead of raw SMTP/SQL responses. Generated temporary administrator passwords are deliberately delivered through startup stderr, separately from the application log file, and can therefore appear in container logs or redirected output. Supplied passwords and API tokens are never printed. Startup failures identify the operation and safe failure category; raw database, protocol and input text remains excluded.

When collecting diagnostics, include the operation, time, safe code/status, and affected version/configuration category. Review external proxy/mail/database logs separately: application redaction does not sanitize those systems or historical copies.

Ruphish appends to its log file; these docs do not promise built-in log rotation or retention. Arrange that with your process environment and verify how it handles open files. See [troubleshooting](troubleshooting.md) for symptom-specific checks.
