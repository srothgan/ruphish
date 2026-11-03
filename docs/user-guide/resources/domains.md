---
title: Domains
description: Choose and register the web address that serves your campaign pages.
sidebar:
  order: 16
---

A domain tells Ruphish which web address to use for campaign pages. It is separate from your sending account and the address you use to sign in to Ruphish.

Adding a domain does not register a hostname, create DNS records, or set up HTTPS. The address must already reach Ruphish's campaign-page service.

## Choose the address

| Where you will open campaign links | Address to use |
| --- | --- |
| On the same computer as a local Ruphish installation | `http://127.0.0.1:8081` with the default getting-started setup. |
| On other devices or a hosted installation | A reachable address such as `https://training.example.com`, configured for your installation. |

The local address points back to the computer opening the link. It will not reach your installation when a recipient opens it on another device. Use `127.0.0.1` for the local example; the domain form does not accept the single-label hostname `localhost`.

For hosted use, arrange DNS and HTTPS with the person hosting Ruphish. The address must serve campaign pages, normally through the public listener on port 8081 or a reverse proxy to that listener. A browser redirect to the Ruphish sign-in page does not provide this routing. See [network access and HTTPS setup](../../administration/listeners-and-tls.md).

## Add the domain

1. Open **Resources → Domains** and choose **Create domain**.
2. Enter a recognizable name.
3. In **Public origin**, enter the base address, including `http://` or `https://` and a port if needed.
4. Leave **Enable this domain** selected.
5. Save the domain.

Enter only the base address, for example `https://training.example.com`. Put a campaign-specific path or query in campaign setup, not in this field.

When domain verification is off, an enabled domain is available for selection. Verification is off by default; your administrator may require it.

## Verify the domain when required

Required verification supports DNS hostnames over HTTPS. A local HTTP address or an IP address cannot satisfy that policy.

1. Open the saved domain and copy the displayed TXT record name and value.
2. Publish that TXT record with your DNS provider.
3. Confirm that the hostname routes to this Ruphish installation and serves a valid, trusted HTTPS certificate.
4. Choose **Verify domain**.
5. Inspect the completed requirements and history. Correct a failed requirement and verify again.

The checks cover DNS ownership, address resolution, HTTPS, and routing to the right Ruphish installation. Keep the TXT record published for later checks. **Refresh status** reads the current result; **Verify domain** starts a check.

## Read the status

| Status or condition | What to do |
| --- | --- |
| Verification not required | Keep the domain enabled and check that the address actually serves your campaign pages. |
| Not verified | Complete the displayed setup requirements and run verification. |
| Check queued or running | Wait for the completed result; requesting a check does not itself verify the domain. |
| Verified | The check succeeded. The domain must also be enabled to use it. |
| Failed | Read the failed requirement, fix the cause, and verify again. |
| Expired | Renew verification before new sending can continue. |

When verification is required, Ruphish rechecks domains and may expire old results according to company settings. Failed rechecks and disabling a domain can make existing campaign links unavailable. Expiry pauses new sending; it does not by itself close existing links.

## Change or delete a domain

Renaming changes how the domain appears in Ruphish. A saved domain's address cannot be changed; create another domain for a different address.

Disabling a domain affects campaigns that use that address. Check those campaigns before disabling it. Deletion is blocked while any retained campaign references the domain, including completed campaigns. Keep the domain if you need to retain those campaigns.
