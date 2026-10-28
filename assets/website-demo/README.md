# Record the dashboard

This isolated asset-authoring job builds Ruphish and its administration UI from the checkout, initializes a new SQLite database, seeds the complete synthetic dataset once, and captures the real dashboard. It replaces only [`assets/dashboard.png`](../dashboard.png). Docker with Linux containers is the only host dependency; no existing executable, UI build, database, credentials, or browser installation is needed.

From the repository root, build the recorder (the first build downloads dependencies and compiles Rust):

```sh
docker build -f assets/website-demo/Dockerfile -t ruphish-dashboard-recorder .
```

Builds use two Cargo workers by default to keep memory use bounded. For three workers, add `--build-arg BUILD_JOBS=3` to the build command. Reuse the built image for further recordings; rebuild when application source or the recording scripts change.

Run it in PowerShell:

```powershell
docker run --rm --init --network none --shm-size=1g --mount "type=bind,source=$((Get-Location).Path)/assets,target=/output" ruphish-dashboard-recorder
```

Or in a POSIX shell:

```sh
docker run --rm --init --network none --shm-size=1g --mount "type=bind,source=$(pwd)/assets,target=/output" ruphish-dashboard-recorder
```

Each run creates fresh temporary state inside the container. The job waits for the dashboard, its content and fonts to load, then replaces the image at 3024 × 1964 pixels. Image builds do not run dependency audits or CI checks. A failed run preserves the previous image. Installation features come from the same `data.json` configuration as the browser demo, so Webhooks and Reporting mailboxes appear in both sidebars. The administrator listener is internal, mail delivery is disabled, individual domains and outbound integrations are inactive, and the recording container has no external network. `--rm` removes its database and credentials when it exits. No acceptance-lab data is used.

Edit sample values in [`data.json`](data.json) and their materialization in [`dataset.mjs`](dataset.mjs), then rebuild the image. The recorder does not assert particular metrics, counts or dashboard layouts. This is the single dataset shared with the public browser demo; it fully replaces the previous screenshot seed. There is no alternate baseline or upgrade mode. [`seed.mjs`](seed.mjs) inserts it into a fresh database using the current schema, preserves a random recorder password, and disables integrations that could contact external services. The static demo simulates enabled integrations without workers. Campaign dates are relative to recording time. This is screenshot authoring, not a product acceptance test; review the resulting image yourself.

The sample's **937 emails sent** in the last 30 days are **332 recipients − 12 failures + 617 recipients**. The older campaign's 400 recipients remain in the recent-campaign table but fall outside that period; they supply the previous-period comparison.

Content examples reference the existing [starter collection](../../crates/ruphish-http/starter-templates/): Microsoft shared document, Google account security, Docusign signature request, and Workday employee document emails; Microsoft side-panel and Google Workspace landing pages; and all three learning pages (SLAM; Stop, Challenge, Protect; Recognize, Resist and Report, Delete). `starter-content.mjs` reads those authored files directly. The browser bundler embeds their content and license notices; no second content snapshot is maintained.

**Manual second-place replacement:** after reviewing `assets/dashboard.png`, copy it over `website/public/dashboard.png`. That second file is served at `/dashboard.png` by both the website landing page and documentation. The recorder deliberately leaves it unchanged. In PowerShell, from the repository root:

```powershell
Copy-Item -LiteralPath assets/dashboard.png -Destination website/public/dashboard.png
```

For a POSIX shell, use `cp assets/dashboard.png website/public/dashboard.png`. If publishing a new screenshot, also update its capture date in [`docs/user-guide/results/dashboard.md`](../../docs/user-guide/results/dashboard.md).
