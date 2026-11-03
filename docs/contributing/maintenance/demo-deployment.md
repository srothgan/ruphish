---
title: Demo deployment
description: Configure and recover optional publication of the prebuilt demo to its separate Vercel project.
sidebar:
  order: 430
next: false
---

The public demo uses its own Vercel project. The website is a separate application and deployment. Start with [local demo development](../development-guides/demo.md) when changing its UI or sample data.

## Understand the publication path

The [demo workflow](../../../.github/workflows/demo.yml) selects demo inputs, runs contracts and browser checks, builds the optimized UI, and packages `demo-vercel-output`. The deploy job lives in [pr.yml](../../../.github/workflows/pr.yml). It downloads that run's artifact and deploys it without checking out or rebuilding application code.

The package is produced by these commands after installing product build dependencies:

```sh
node crates/ruphish-ui/web/build-demo.mjs
trunk build --locked --release --features demo crates/ruphish-ui/web/generated/index.html
node demo/vercel-output.mjs
```

`vercel-output.mjs` verifies the manifest, copies the static distribution to `.vercel/output/static`, and writes routing configuration. Extensionless application routes use `index.html`; backend namespaces and missing assets are rejected. The demo is marked `noindex`. Packaging rejects stale output, unmanifested files, and altered assets.

The artifact is retained for two days. The deployment job invokes the pinned Vercel CLI with `deploy --prebuilt --yes`, adding `--prod` for main. Passing packaging tests does not establish that the deployed routes work; review the resulting deployment separately.

## Activation and credentials

Configure the following in GitHub, not in source files:

| Location | Name | Purpose |
| --- | --- | --- |
| Repository variable | `VERCEL_ORG_ID` | Team owning the demo project |
| Repository variable | `VERCEL_DEMO_PROJECT_ID` | Demo project ID, separate from the website project |
| Repository variable | `DEMO_DEPLOY_ENABLED` | Exactly `true` to enable publication |
| `demo-production` environment secret | `VERCEL_TOKEN` | Token authorized for production demo deployment |
| `demo-preview` environment secret | `VERCEL_TOKEN` | Token authorized for preview deployment |

Create the named protected environments with a required maintainer reviewer. Restrict production deployment to `main`. If a single maintainer must approve their own deployment, configure self-review accordingly. Review the exact PR commit and workflow before releasing preview credentials. These external settings are prerequisites to verify; the YAML cannot establish that they already exist.

If Vercel Git integration currently deploys the demo, retain that working path until a prebuilt preview is verified. During cutover, disconnect only the demo project's Git integration to avoid a second source build. Keep the project/domain and the separate website integration.

## Know which runs can deploy

| Run | Result when demo deployment is enabled and selected |
| --- | --- |
| Push to main | Production deployment after environment approval |
| Manual CI run on main | Full validation and eligible production deployment |
| Same-repository PR, excluding Dependabot | Preview deployment after approval |
| Fork or Dependabot PR | Build/test coverage without deployment credentials |
| Manual CI run on another branch | Validation without deployment |
| Nightly or release | No demo deployment or preview comments |

Deployment waits for the selected demo artifact, not for unrelated source checks or the final PR gate. It can therefore succeed while another check fails. The PR gate still requires source and demo validation.

Eligible PR previews use one bot comment showing build, approval, deployment, or failure status with the source commit and workflow link. Stale runs and closed/superseded PRs are skipped. Comment failures do not block publication, so consult the workflow itself when comment status is incomplete.

## Verify and recover

Open the deployed URL and run the relevant [manual demo review](../development-guides/demo.md#verify-your-change). Include nested-route refresh, asset loading, login, simulation feedback, and exports. Confirm that simulated operations do not contact a backend or send messages.

If publication fails, inspect credentials, selected project IDs, environment approval, and the retained artifact before retrying. A manual CI run on main can rebuild and publish the current main demo when a previous run was cancelled or its artifact expired.

Set `DEMO_DEPLOY_ENABLED` to a value other than `true` to stop new workflow publications. If restoring the previous Vercel Git deployment path, reconnect the demo project's repository integration. These configuration changes do not remove the existing deployment or alter the website project.

Use the script/workflow checks in [CI workflows](ci.md) for implementation changes. Build, packaging, approval, and deployed behavior are separate verification stages.
