---
title: Configure a sending profile
description: Connect to SMTP, choose sender identities, and test delivery.
sidebar:
  order: 9
---

A sending profile tells Ruphish how to connect to your SMTP service. You need its hostname, port, permitted sender address, and any service credentials.

1. Open **Resources → Sending profiles → Create**.
2. Give the profile a recognizable name and enter `host:port`.
3. Set the From address and any required username/password.
4. Keep certificate verification enabled unless you are diagnosing a controlled local test service.
5. Save, open **Send test**, then choose **Test connection only**.

The default port is 25. Port 465 uses implicit TLS; other SMTP connections use STARTTLS when advertised. Authentication requires encryption except for loopback test services. Private/internal destinations also need the installation's [outbound allowance](../administration/outbound-connections.md); changing the hostname alone does not bypass it.

Ruphish automatically chooses CRAM-MD5, PLAIN or LOGIN from the server's advertised authentication mechanisms, in that preference order. Connection checks, test emails and campaign delivery use the same negotiation. A rejected credential or malformed challenge fails the attempt without falling back to another mechanism. CRAM-MD5 is supported for relay compatibility; it does not replace TLS.

## Understand the sender fields

The profile supplies the SMTP envelope sender. A template can override the visible From header through its legacy `envelope_sender` field. These are distinct identities; verify both when diagnosing a rejected sender or unexpected message appearance.

Custom profile headers use key/value pairs. Keep their values appropriate for the receiving mail system and check a captured message after changing them.

## Test connection versus send test

**Test connection only** checks connectivity and authentication without submitting a message. **Send test email** sends to one selected recipient using the chosen content. Use a local capture inbox while experimenting.

If you choose **Save and send test**, the save completes first. A later send failure does not undo that save. SMTP acceptance confirms submission to the server, not inbox placement.

A connection loss after submitting message content may leave an unknown outcome. Check the receiving system before repeating the test. Test sends do not automatically retry an uncertain submission; campaign delivery has a separate [durable retry and reconciliation workflow](campaigns/delivery.md).

## Diagnose failures

| Problem | Next check |
| --- | --- |
| Destination denied | Ask the installation operator to review the exact host/IP against outbound policy |
| TLS failure | Check hostname, port, certificate validity, and trust; do not disable verification to conceal a production configuration error |
| Authentication failure | Verify service credentials and the server's supported encrypted authentication path |
| Sender/recipient rejection | Check the SMTP service's allowed envelope sender and recipient policy |
| Accepted but absent from inbox | Inspect the receiving SMTP service and mail filtering; Ruphish cannot infer final inbox placement |

Resource bodies have a 32 MiB limit including JSON and base64 content. The SMTP adapter bounds concurrent connection/submission operations to four. Profile configuration errors can block campaign deliveries until corrected.

Connection verification has a 30-second deadline. Sending separately bounds preparation/connection and final submission to 30 seconds each, so do not interpret a submission timeout as proof of non-delivery. These are adapter deadlines; an HTTP caller can disconnect before the external outcome is known.
