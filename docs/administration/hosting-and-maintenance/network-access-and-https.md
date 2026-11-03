---
title: Network access and HTTPS
description: Give colleagues and campaign recipients working HTTPS addresses with the correct routing.
sidebar:
  order: 24
---

Ruphish serves two kinds of traffic. Keep their addresses separate so campaign recipients reach campaign pages rather than the sign-in screen.

| Address | Purpose | Default port |
| --- | --- | --- |
| Ruphish website, for example `https://ruphish.example.com` | Sign-in, resources, campaigns, results, and management APIs | `3333` through `admin_server` |
| Campaign website, for example `https://training.example.com` | Recipient links, tracking, landing pages, and learning pages | `8081` through `phish_server` |

For a local trial, keep the getting-started addresses. `127.0.0.1` always means the device opening the URL, so it cannot serve recipients on another computer.

For hosted use, create DNS records for your chosen hostnames and route each hostname to the correct listener. A reverse proxy accepts HTTPS on port 443 and forwards the request to Ruphish. Use proxying, not a redirect to `127.0.0.1:8081`: the recipient must keep using your public campaign address.

## Docker example with HTTPS

This example uses Caddy to obtain and renew certificates. You need two DNS hostnames pointing to the Docker host, inbound TCP ports 80 and 443 available, and outbound access for certificate issuance. Replace both example hostnames in the files below. See [Caddy's automatic HTTPS requirements](https://caddyserver.com/docs/automatic-https).

Use the same installation folder and `ruphish-data` volume as your existing Compose setup. Back up first if you are changing a running installation. Save this complete replacement as `compose.yaml`:

```yaml
services:
  ruphish:
    image: ghcr.io/srothgan/ruphish:latest
    restart: unless-stopped
    stop_grace_period: 30s
    environment:
      ADMIN_SERVER_TRUSTED_ORIGINS: '["https://ruphish.example.com"]'
      DOMAIN_VERIFICATION_TRUSTED_PROXIES: '["172.30.50.2/32"]'
    volumes:
      - ruphish-data:/var/lib/ruphish
    networks:
      - ruphish
    logging:
      driver: local
      options:
        max-size: "10m"
        max-file: "5"
  caddy:
    image: caddy:2
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
      - caddy-config:/config
    networks:
      ruphish:
        ipv4_address: 172.30.50.2
    logging:
      driver: local
      options:
        max-size: "10m"
        max-file: "5"
networks:
  ruphish:
    ipam:
      config:
        - subnet: 172.30.50.0/24
volumes:
  ruphish-data:
  caddy-data:
  caddy-config:
```

The example gives the proxy a fixed private address so Ruphish can trust that proxy alone. If `172.30.50.0/24` overlaps a network you already use, choose a free subnet and update the subnet, proxy address, and trusted-proxy value together.

Create `Caddyfile` alongside `compose.yaml` before starting the containers:

```text
ruphish.example.com {
    reverse_proxy ruphish:3333
}

training.example.com {
    reverse_proxy ruphish:8081 {
        header_up X-Forwarded-Host training.example.com
        header_up X-Forwarded-Proto https
    }
}
```

Start and inspect the services:

```sh
docker compose pull
docker compose up -d --wait
docker compose logs --tail=100 ruphish caddy
```

Open your Ruphish HTTPS address and sign in. Test readiness through both hostnames, replacing these examples:

```sh
curl --fail --show-error --max-time 10 https://ruphish.example.com/health/ready
curl --fail --show-error --max-time 10 https://training.example.com/health/ready
```

Each should return HTTP `200` with `{"status":"ready"}` and a valid certificate. Certificate issuance can take longer than application startup; inspect Caddy's logs if HTTPS is not ready yet. This example exposes the Ruphish sign-in page publicly. Apply your organization's access restrictions at the proxy or network if it should be limited to a VPN or trusted users' networks.

## Register the campaign address

After routing works, open **Resources → Domains** and register your actual campaign origin, such as `https://training.example.com`. Registration tells Ruphish which address to put in campaign links; it does not create DNS records, certificates, or proxy routes.

Follow [Domain setup](../../user-guide/resources/domains.md), including DNS verification if required by company policy. The Ruphish host must also be able to resolve and reach this address for verification. Test a campaign link from a recipient's device before launching to a wider audience.

## Use an existing proxy or a native installation

You can use your existing reverse proxy instead of Caddy. Configure these two routes:

1. Forward the Ruphish website to the administration listener and preserve its public `Host` header. If the proxy accepts HTTPS and forwards HTTP, add the full browser origin to `admin_server.trusted_origins`.
2. Forward the campaign website to the public listener. Overwrite exactly one `X-Forwarded-Host` header with the public hostname and port, and exactly one `X-Forwarded-Proto` header with `https`. Add the proxy's actual source IP or CIDR to `domain_verification.trusted_proxies`.

For a proxy running on the same Linux host as the native service, the upstreams can be `127.0.0.1:3333` and `127.0.0.1:8081`. If you use HTTP upstreams, set `admin_server.use_tls` to `false`, keep both listeners bound to loopback, and trust `127.0.0.1/32` for the public proxy. Restart Ruphish after changing its configuration.

For example, merge these fields into the corresponding objects in the native service configuration:

```json
{
  "admin_server": {
    "use_tls": false,
    "trusted_origins": ["https://ruphish.example.com"]
  },
  "domain_verification": {
    "trusted_proxies": ["127.0.0.1/32"]
  }
}
```

This is a fragment, not a complete configuration. Keep the existing listener addresses, database, and other fields. A proxy in a container generally reaches the application from a container-network address, not `127.0.0.1`; trust the actual peer address for your topology.

Public proxy trust and browser-origin permission solve different problems. `trusted_proxies` lets Ruphish identify the original campaign origin. `trusted_origins` permits browser requests to the application. Neither setting grants users access or creates a campaign domain.

## Serve HTTPS directly

To terminate HTTPS in Ruphish, set `use_tls` to `true` for the listener and supply readable PEM certificate and key files. For example, merge these fields into `phish_server`:

```json
{
  "listen_url": "0.0.0.0:8443",
  "use_tls": true,
  "cert_path": "/etc/ruphish/tls/fullchain.pem",
  "key_path": "/etc/ruphish/tls/privkey.pem"
}
```

Use a certificate covering the campaign hostname, allow the selected port through the firewall, and register the matching origin, such as `https://training.example.com:8443`. In Docker, also mount the certificate files and publish the port.

Ruphish does not obtain or automatically renew public certificates. Renew them through your certificate tooling and restart Ruphish to load the new files. If both configured files are absent, Ruphish can generate a local self-signed pair; that pair is not a trusted certificate for your public hostname. A missing half of a pair or a mismatched certificate and key stops startup.

## Optional static files

If your campaign setup needs additional static files, place them under `static/endpoint/` relative to Ruphish's working directory and restart. They are served under `/static/` for eligible campaign hosts. For the Docker image, mount that directory at `/opt/ruphish/static/endpoint`; the Linux service above uses `/var/lib/ruphish/static/endpoint`.

Ruphish snapshots these files at startup. Hidden files, symlinks, and directory listings are not served. Limits are 1,000 files, 8 MiB per file, and 32 MiB total. For ordinary campaign content, use [landing pages](../../user-guide/resources/landing-pages.md) instead.
