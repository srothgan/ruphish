---
title: Native installer reference
description: Select a release, customize the binary installation, control PATH changes, and remove the installed program.
sidebar:
  order: 22
---

Use the native installer to download a released Ruphish binary for your operating system and processor. It installs for your operating-system account, verifies the archive checksum and executable version, and prints the installed executable's path. It does not start Ruphish or create a background service.

For supported platforms, prerequisites, and first startup, follow [Install Ruphish](../../getting-started/install-ruphish.mdx). To run the binary as a service, follow [Run a Linux binary service](run-a-linux-binary-service.md).

## Run the installer with options

On Linux or macOS, pass options after `sh -s --`. For example, display the installer's help:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --help
```

In PowerShell, load the script into a script block so you can pass named parameters:

```powershell
$installer = [scriptblock]::Create((irm https://www.ruphish.com/install.ps1))
& $installer -Help
```

The PowerShell examples below use `$installer` in the same terminal session. The default `irm ... | iex` installation command accepts the [environment settings](#environment-settings), but use the script-block form to supply explicit parameters. Installer options belong to the script; they are separate from the application's [command-line options](command-line-reference.md).

## Options

| Linux/macOS | PowerShell | Effect |
| --- | --- | --- |
| `--version VERSION` | `-Version VERSION` | Install an exact published version, such as `0.1.0` or `0.1.0-dev.2`. A leading `v` is accepted. Skips latest-release discovery. |
| `--include-previews` | `-IncludePreviews` | Select the latest version from full releases and numbered development previews. Without this setting, select full releases only. |
| `--no-include-previews` | `-IncludePreviews:$false` | Select full releases only, overriding the preview environment setting. |
| `--install-dir DIRECTORY` | `-InstallDir DIRECTORY` | Choose the managed program directory. Use the same directory for later updates and removal. |
| `--no-modify-path` | `-NoModifyPath` | Skip shell-profile edits on Linux/macOS or user PATH edits on Windows. Does not undo earlier edits. |
| `--modify-path` | `-NoModifyPath:$false` | Enable the installer's normal PATH setup, overriding the environment setting. |
| `--allow-downgrade` | `-AllowDowngrade` | Accept an older program version without an interactive confirmation. Does not reverse database or configuration changes. |
| `--uninstall` | `-Uninstall` | Remove managed program files and normally clean up installer-owned PATH changes. Retain application configuration, databases, certificates, and logs. |
| `--help` or `-h` | `-Help` | Print help without installing or removing anything. |

Do not combine an exact version with `--include-previews` or `-IncludePreviews`. Do not combine uninstall with an exact version, either of those preview options, or downgrade permission. Custom directories and PATH controls can be used with uninstall.

## Choose a release

With no version options or preview environment setting, the installer selects the latest full release. Preview selection includes development releases alongside full releases; it does not force installation of a preview when a newer full release is available. Use previews with disposable data because upgrades between them are not guaranteed.

Install a specific release:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --version 0.1.0
```

```powershell
& $installer -Version '0.1.0'
```

Include previews when selecting the latest release:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --include-previews
```

```powershell
& $installer -IncludePreviews
```

An exact version must have a published archive for your platform. The installer reports discovery, download, checksum, and version-check failures instead of silently selecting another version. If the requested version is already installed, it checks the existing executable rather than downloading it again.

## Installation directory and PATH

| Platform | Default program directory | Executable |
| --- | --- | --- |
| Linux/macOS | `~/.local/share/ruphish-install` | `~/.local/share/ruphish-install/current/ruphish` |
| Windows | `%LOCALAPPDATA%\Programs\Ruphish` | `%LOCALAPPDATA%\Programs\Ruphish\current\ruphish.exe` |

Choose an absolute path to a dedicated directory that your account can write to. Keep it separate from [application configuration and data](configuration-reference.md#default-locations-and-explicit-configuration). On Windows, use a local path, not a UNC network path. The installer rejects unmanaged files and unsafe directory layouts rather than overwriting them.

For example, install in a custom directory without editing your shell profile or user PATH:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --install-dir "$HOME/Applications/ruphish" --no-modify-path
```

```powershell
& $installer -InstallDir (Join-Path $env:LOCALAPPDATA 'Programs\Ruphish-custom') -NoModifyPath
```

Changing the program directory does not move an existing installation or change where Ruphish stores its database. Record a custom directory and supply it again when updating or removing that installation.

On Linux/macOS, the installer creates `~/.local/bin/ruphish` as a link to the installed executable, even with `--no-modify-path`. When PATH changes are enabled and `~/.local/bin` is missing from PATH, it adds a marked block to `.bashrc` for Bash, `.zshrc` for Zsh, or `.profile` for other shells. For Fish, it asks you to add the directory yourself. An existing command link owned by another installation prevents installation into a different directory.

On Windows, normal PATH setup adds the installation's `current` directory to your user PATH. Open a new terminal after installation, or use the executable path printed by the installer immediately. If another Ruphish executable appears earlier on PATH, the installer warns you; use the printed path to select the intended installation.

## Environment settings

These settings work with the short installation commands from Getting started. Explicit options or PowerShell parameters override the corresponding environment settings.

| Variable | Default | Meaning |
| --- | --- | --- |
| `RUPHISH_INCLUDE_PREVIEWS` | `false` | Include development previews in latest-version selection. Ignored when selecting an exact version or uninstalling. |
| `RUPHISH_INSTALL_DIR` | The platform's default program directory | Set the managed directory for installation, updates, and removal. |
| `RUPHISH_NO_MODIFY_PATH` | `false` | Skip shell-profile or user PATH edits. Also applies during uninstall. |

Boolean settings accept `1` or `true`, and `0` or `false`, without regard to letter case. Unset or empty values use the default. Other boolean values produce an error when that setting is read.

On Linux/macOS, place the environment assignment on the `sh` side of the pipe so the installer receives it:

```sh
curl -fsSL https://www.ruphish.com/install.sh | RUPHISH_INCLUDE_PREVIEWS=1 sh
```

This assignment applies only to that invocation. In PowerShell, an environment assignment lasts for the current session. For example:

```powershell
$env:RUPHISH_INCLUDE_PREVIEWS = '1'
irm https://www.ruphish.com/install.ps1 | iex
Remove-Item Env:RUPHISH_INCLUDE_PREVIEWS
```

To override a preview setting without clearing it, use `--no-include-previews` or `-IncludePreviews:$false`. To enable PATH setup despite `RUPHISH_NO_MODIFY_PATH`, use `--modify-path` or `-NoModifyPath:$false`.

The scripts also respect a nonempty `NO_COLOR` to suppress colored output and a nonempty `CI` to disable interactive downgrade confirmation and animated progress. In unattended runs, an intentional downgrade requires explicit downgrade permission.

## Update or downgrade

Follow [Upgrade Ruphish](upgrade-ruphish.md) to stop the application, back up its data, replace the program, and verify startup. Rerun the installer under the same operating-system account and with the same program directory. It replaces program files; it does not stop or restart an existing process. A separately copied service executable must also be updated as described in that guide.

Selecting a version older than the installed version requires confirmation. Interactive runs default to cancellation; unattended runs fail unless you supply `--allow-downgrade` or `-AllowDowngrade`.

For an intentional program downgrade to `0.1.0`:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --version 0.1.0 --allow-downgrade
```

```powershell
& $installer -Version '0.1.0' -AllowDowngrade
```

An older binary does not undo database migrations. When rolling back an upgrade that changed the database, restore the matching pre-upgrade database and configuration before starting the older release. See [upgrade recovery](upgrade-ruphish.md#if-startup-rejects-the-database).

## Uninstall

Stop Ruphish first. Run the installer under the account that installed it:

```sh
curl -fsSL https://www.ruphish.com/install.sh | sh -s -- --uninstall
```

```powershell
& $installer -Uninstall
```

If you chose a custom program directory, include `--install-dir` or `-InstallDir`, or set `RUPHISH_INSTALL_DIR` to that same directory. Removal does not select or download a release.

The uninstaller removes managed program files and the command link on Linux/macOS. It normally removes the shell-profile block or user PATH entry recorded as installer-owned. If you disable PATH modification, those profile or PATH entries remain for you to clean up. A profile block you edited is preserved for manual review.

Your configuration, databases, certificates, and logs remain in their [application locations](configuration-reference.md#default-locations-and-explicit-configuration). A service definition and any binary copied outside the managed installation are also outside the uninstaller's scope.
