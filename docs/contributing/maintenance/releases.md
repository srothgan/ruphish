---
title: Releases
description: Prepare release identity and notes, verify artifacts, and recover publication without rebuilding.
sidebar:
  order: 420
---

The [release workflow](../../../.github/workflows/release.yml) validates source, builds optimized assets and native packages, verifies Docker images, and publishes the verified outputs. End-user installation belongs in [Getting started](../../getting-started/index.md).

## Prepare a release

1. Select the release commit on `main`. Update the workspace package version and its lockfile records through the normal Cargo workflow.
2. For a final release, add exactly one dated `CHANGELOG.md` section matching the version, such as `## [0.1.1] - 2026-11-03`. Complete the release notes; the metadata check rejects empty notes and TODOs.
3. Review migrations and compatibility instructions. Released SQL migrations are immutable; schema changes need forward migrations and the relevant [database tests](../testing/database-providers.md).
4. Run the **Release** workflow manually on the intended commit/ref to validate the release process without publication. Inspect its source, dependency-security, native, image, bundle, and gate jobs.
5. After validation and maintainer review, create an annotated version tag on the release commit and push that tag to publish. The tag must equal `v` followed by the workspace version, and the commit must be an ancestor of `origin/main`.

For example, after setting the workspace version to `0.1.1` and checking out the intended release commit:

```sh
git tag -a v0.1.1 -m "Release 0.1.1"
git push origin v0.1.1
```

These commands publish a release through CI. Substitute the prepared version; do not recreate or move an existing release tag. Supported identities are `MAJOR.MINOR.PATCH` and numbered development previews such as `0.2.0-dev.1`. Development previews are for disposable installations and do not promise upgrade compatibility between previews or to the final release.

## What the workflow verifies

| Stage | Result |
| --- | --- |
| Metadata | Matching source commit, version, annotated tag for publication, and complete release notes |
| Source acceptance | Full development-profile checks through the shared validation workflow |
| Dependency security | Root/browser npm audits and Cargo advisory policy before optimized builds |
| UI | One optimized administration distribution and mounted-UI checks |
| Native packages | Release-profile tests, embedded-assets executable, exact archive extraction, and standalone startup/restart checks |
| Docker images | Linux images built from the verified native executables, then image, public-protocol, and database-image tests |
| Bundle | Archives, image manifest, checksums, release manifest, and publication provenance |
| Gate | Every required stage must succeed before publication |

Native packages target Linux x86-64 and ARM64, Windows x86-64 and ARM64, and macOS ARM64. Docker images cover Linux amd64 and arm64. The release matrix is authoritative; native platform execution and exact-archive checks are separate from ordinary source tests.

The workflow builds the server with `embedded-assets`, so the shipped executable contains its verified UI. It does not rebuild artifacts during publication. Native archives are unsigned and macOS executables are not notarized; SHA-256 checksums and GitHub build provenance are separate forms of verification, not platform signing.

## Publication and version selection

Manual workflow runs validate and retain artifacts without publishing. A matching tag push publishes only after the release gate succeeds. The publisher uploads a manifest first, resumes only matching draft releases, checks image digests, and verifies anonymous image access before exposing the release and advancing `latest`.

Final releases can advance GitHub's latest release and the Docker `latest` tag when they are at least as new as all existing final releases. Development previews publish as prereleases and never advance `latest`. Release publication is serialized so competing runs do not race that update.

The public website's release-discovery endpoint reads GitHub release metadata for the installers. Its response selection and caches are independent of the publisher; see [website implementation](../../../website/README.md#native-installers-and-release-discovery).

## Recover a failed publication

Inspect the failed job before retrying. If compilation, tests, or artifact verification failed, fix the underlying source problem and prepare an appropriate new release identity instead of overriding the gate.

If publication failed after uploads began, retain the original workflow run and its `verified-release-bundle`. Correct the external issue and rerun only the failed publication job. The publisher compares existing manifests, assets, tag commit, and image digest with the verified bundle and refuses conflicting content. Do not regenerate artifacts under the same published version or move the tag to bypass that check.

For a new GHCR package, anonymous pull verification can fail because package visibility is private. A maintainer must set the package to Public and rerun the original publication job. The uploaded artifacts remain available for that retry; the release and `latest` updates stop until verification succeeds.

## Validate release-tool changes locally

From the repository root, using the product Node version:

```sh
node --test .github/scripts/release.test.mjs
```

Also run the applicable [JavaScript checks](../testing/validation.md#javascript) and [workflow checks](ci.md). Tests of package metadata and simulated publication failures do not establish that a hosted release or platform executable has passed its actual workflow.
