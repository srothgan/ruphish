---
title: Webhooks
description: Send campaign events to another system and inspect delivery results.
sidebar:
  order: 18
---

A webhook sends campaign events to another service as they happen. Use it to connect Ruphish to a system that can receive and process its event format.

Enabled webhooks receive new events across the installation, including campaigns created by colleagues. They are not limited to one campaign or the account that created the webhook.

## Before you begin

An administrator must enable webhooks in company settings. Your account then needs the relevant resource permissions.

You need a receiving endpoint and a signing secret agreed with the receiver. The receiver must support the [Ruphish webhook contract](../../api/webhooks.md), including signature verification and handling repeated deliveries.

## Add an endpoint

1. Open **Resources → Webhooks** and choose **Create**.
2. Enter a name and the **Endpoint URL**. Use HTTPS for the receiving service.
3. Enter the **Signing secret**.
4. Clear **Enabled** while you check the setup, save, then choose **Validate connection**.
5. Confirm that the receiver accepts the signed validation message.
6. Edit the endpoint, select **Enabled**, and save.
7. Generate a controlled campaign event and check the webhook history and receiver.

Validation sends a test object containing `{"success":true}`. It does not create a campaign event or prove that the receiver can handle every event type.

## Inspect delivery

Open the endpoint to review delivery history. It shows the event, attempt number, outcome, and HTTP status where available.

| Outcome | What it means |
| --- | --- |
| Accepted | The receiving service accepted the request. Check that service for its subsequent processing. |
| Retry scheduled | A temporary network or HTTP failure will be tried again. |
| Failed | Inspect the outcome and receiver configuration before deciding what to change. |

Ruphish retries network failures and selected temporary HTTP responses. It does not follow redirects. Configure the final endpoint URL rather than an address that redirects elsewhere.

Repeated deliveries can happen, so the receiver should use the event's identity to avoid processing it twice. For exact event fields and retry rules, use the [webhook API reference](../../api/webhooks.md).

## Change the endpoint or secret

Leave the signing-secret field blank to retain the saved secret. Enter a value to replace it; saved secrets are not displayed again.

Coordinate changes with the receiver. Saving a new configuration cancels queued work for the previous configuration, while a request already in flight may still arrive using the old settings.

Webhooks deliver newly committed events; enabling an endpoint does not replay all existing campaign history.

## Disable or delete an endpoint

Disable the endpoint to stop event delivery. An administrator can also disable webhooks for the installation.

Deleting an endpoint cancels its retained delivery jobs. Neither disabling nor deleting can recall requests already authorized or sent, or remove copies stored by the receiver.
