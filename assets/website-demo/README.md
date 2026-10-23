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

Each run creates fresh temporary state inside the container. The job checks three campaigns, twelve delivery failures, and 937 emails sent, waits for the dashboard and fonts to load, then replaces the image at 3024 × 1964 pixels. It does not assert exact labels, resource inventory, or previous-period values, and image builds do not run dependency audits or CI checks. A failed run preserves the previous image. The administrator listener is internal, mail delivery is disabled, the sample domain is disabled and unverified, and the recording container has no external network. `--rm` removes its database and credentials when it exits. No acceptance-lab data is used.

Edit sample values in [`seed.mjs`](seed.mjs), update the expected totals in [`capture.mjs`](capture.mjs) when changing the dataset, then rebuild the image. Campaign dates are relative to recording time. This is screenshot authoring, not a product acceptance test; review the resulting image yourself.

The sample's **937 emails sent** in the last 30 days are **332 recipients − 12 failures + 617 recipients**. The older campaign's 400 recipients remain in the recent-campaign table but fall outside that period; they supply the previous-period comparison.

**Manual second-place replacement:** after reviewing `assets/dashboard.png`, copy it over `website/public/dashboard.png`. That second file is served at `/dashboard.png` by both the website landing page and documentation. The recorder deliberately leaves it unchanged. In PowerShell, from the repository root:

```powershell
Copy-Item -LiteralPath assets/dashboard.png -Destination website/public/dashboard.png
```

For a POSIX shell, use `cp assets/dashboard.png website/public/dashboard.png`. If publishing a new screenshot, also update its capture date in [`docs/user-guide/results/dashboard.md`](../../docs/user-guide/results/dashboard.md).
