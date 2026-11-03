---
title: Run a Linux binary service
description: Install the released binary and run it under systemd with a dedicated account and stable paths.
sidebar:
  order: 22
---

This guide runs the released Linux binary as a systemd service. You need a supported Linux host, `sudo` access, and systemd. For supported platforms and a foreground installation on Windows or macOS, see [Install Ruphish](../../getting-started/install-ruphish.mdx). The native installer does not register an operating-system service; [Docker Compose](run-with-docker-compose.md) is another way to manage a background installation.

These commands create a new installation. If you already have data, stop the existing process and follow [Back up and restore](back-up-and-restore.md) before moving it. A service account uses different default paths from your personal account.

## Install the binary and service account

Run the installer as your normal user, then open a new terminal if needed:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh
ruphish --version
```

On a Linux host with the standard `useradd` utility, create the service account and copy the installed executable to a stable system location:

```sh
sudo useradd --system --user-group --home-dir /var/lib/ruphish --no-create-home --shell /usr/sbin/nologin ruphish
sudo install -m 0755 "$HOME/.local/bin/ruphish" /usr/local/bin/ruphish
sudo install -d -o root -g ruphish -m 0750 /etc/ruphish
sudo install -d -o ruphish -g ruphish -m 0700 /var/lib/ruphish
```

If the `ruphish` account already exists, use it rather than creating another one. The service needs write access to its data directory and read access to its configuration.

## Save the configuration

Save the following as `/etc/ruphish/config.json`, using an editor with elevated permissions, such as `sudoedit /etc/ruphish/config.json`:

```json
{
  "admin_server": {
    "listen_url": "127.0.0.1:3333",
    "use_tls": true,
    "cert_path": "/var/lib/ruphish/admin.crt",
    "key_path": "/var/lib/ruphish/admin.key"
  },
  "phish_server": {
    "listen_url": "127.0.0.1:8081",
    "use_tls": false,
    "cert_path": "/var/lib/ruphish/public.crt",
    "key_path": "/var/lib/ruphish/public.key"
  },
  "db_name": "sqlite3",
  "db_path": "/var/lib/ruphish/ruphish.db",
  "logging": { "filename": "", "level": "info" }
}
```

Set its permissions:

```sh
sudo chown root:ruphish /etc/ruphish/config.json
sudo chmod 0640 /etc/ruphish/config.json
```

This configuration keeps both listeners on the host. Ruphish creates a local certificate for the browser interface at first startup. Logs go to systemd's journal. Use [Network access and HTTPS](network-access-and-https.md) to make the installation available to other devices.

## Create and start the service

Save this as `/etc/systemd/system/ruphish.service`:

```ini
[Unit]
Description=Ruphish
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=ruphish
Group=ruphish
WorkingDirectory=/var/lib/ruphish
ExecStart=/usr/local/bin/ruphish --config /etc/ruphish/config.json
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s
UMask=0077

[Install]
WantedBy=multi-user.target
```

Load the unit, enable startup at boot, and start Ruphish:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now ruphish
sudo systemctl status ruphish --no-pager
sudo journalctl -u ruphish -n 100 --no-pager
```

Look for `RUPHISH_READY` in the journal. A fresh installation also prints the temporary password for `admin`. Open **https://127.0.0.1:3333** on the host and complete the password change. The local certificate causes a browser warning; verify that you are visiting this local installation before continuing.

If you manage a remote Linux host, forward the browser port from your own computer with SSH, replacing the account and hostname:

```sh
ssh -L 3333:127.0.0.1:3333 operator@ruphish.example.com
```

Keep that SSH session open and visit the same local HTTPS address in your browser. This forwards the browser interface only; it does not make campaign pages reachable by recipients.

## Operate the service

```sh
sudo systemctl stop ruphish
sudo systemctl start ruphish
sudo systemctl restart ruphish
```

Run the command for the action you need. Restart after changing `config.json`. If you change the unit file itself, run `sudo systemctl daemon-reload` before restarting.

Stopping the service preserves data and allows a graceful shutdown. Campaign links are unavailable while it is stopped, and unfinished delivery work can resume after startup. Use [health monitoring](health-and-logs.md) to check readiness rather than relying only on systemd's **active** status.

For systemd-specific options, see the [systemd service documentation](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html). Follow [Upgrade Ruphish](upgrade-ruphish.md) when replacing the executable.
