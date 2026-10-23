---
title: Configure listeners and TLS
description: Separate administration from public campaign traffic and configure certificates and proxy trust.
sidebar:
  order: 4
---

The administration listener serves accounts, the UI, and management APIs. The public listener serves campaign links and tracking. Give them deliberate binding addresses; the public listener does not expose administration assets.

## Direct TLS

Set `use_tls: true` and provide `cert_path` and `key_path` for the selected listener. The certificate must cover the hostname clients use. For public campaigns, it must also satisfy the server's domain-verification trust checks.

If both configured files are absent, startup can generate a development certificate/key pair valid for one year. This replaces the previous ten-year lifetime for newly generated certificates; existing files are preserved. If only one is absent or the pair does not match, startup fails. Development self-signed certificates need explicit local trust; they do not establish public trust for arbitrary campaign domains. Supplied certificate/key bytes are not rewritten.

TLS uses modern TLS 1.2/1.3. Built-in ACME, automatic per-host certificates, and DNS-provider provisioning are not implemented. Use a suitable certificate covering your hostnames or a reverse proxy that manages them.

## Public reverse proxy

A trusted public proxy must terminate HTTPS and overwrite exactly one `X-Forwarded-Host` and `X-Forwarded-Proto: https`. Configure its peer network in `domain_verification.trusted_proxies`. Headers from untrusted peers are ignored. Preserve routing to the correct Ruphish instance so its short-lived verification proof is reachable.

This is public-listener trust. Administration requests do not use forwarded headers to infer peer identity or scheme. Browser origin/CSRF behavior therefore needs attention if you place the admin listener behind a proxy; do not assume that public proxy settings configure admin trust.

## Optional public files

Files under `static/endpoint/` are snapshotted at startup and served under `/static/` for verified, enabled hosts. Hidden files, symlinks, and directory listings are not exposed. Limits are 1,000 files, 8 MiB per file, and 32 MiB total, with bounded nesting.

Restart after changing files. Static documents retain restrictive sandbox/content policy, and landing-page resource restrictions still apply. Public files are not a separate form-submission or credential-capture mechanism.

Finish by [verifying the domain](../user-guide/domains.md) from the running installation, not only by opening it in your own browser.
