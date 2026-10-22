---
title: Configure webhooks
description: Send committed campaign events to an external service and inspect delivery outcomes.
sidebar:
  order: 7
---

Administrators manage **Administration → Webhooks**. Enabled endpoints receive newly committed campaign events across resource owners, so choose a receiver appropriate for that scope.

1. Create a named HTTP(S) endpoint and signing secret. Use HTTPS for an actual receiving service.
2. Save it and choose **Validate connection**.
3. Verify that the receiver handles the signed validation object `{"success":true}`.
4. Enable event delivery and inspect recent history after a controlled campaign event.

The secret is write-only. Leave it empty in the edit form to retain it; enter a new value to replace it. Coordinate rotation with the receiver, because requests already authorized or in flight may still use the prior configuration.

Validation is not a campaign event. Ordinary deliveries contain campaign events with stable event identity and an HMAC signature; the receiver must implement the [webhook contract](../api/webhooks.md), including duplicate handling.

History shows attempt number, event identity, HTTP status if available, and a fixed outcome code. Response bodies are discarded. Network failures and selected transient statuses retry; other statuses fail. Redirects are not followed.

Saving a configuration cancels queued work for its previous version. Deleting an endpoint cancels retained jobs, but cannot recall a request already authorized or sent. Use history to distinguish configuration changes from receiver failures before replaying anything outside Ruphish.
