---
title: Releases
description: Publish development previews, prepare final notes, generate drafts, and recover failed publication.
sidebar:
  order: 420
---

Releases are published by pushing an annotated version tag. Development notes are generated automatically; final notes come from `CHANGELOG.md`. Run commands from the repository root using the [pinned tools](../development-setup/index.md).

## Publish a development preview

1. Set `[workspace.package].version` in `Cargo.toml` to the intended preview, for example `0.1.0-dev.4`, and run `cargo update --workspace` to update `Cargo.lock`.
2. Merge the release changes into `main` through the normal contribution process. Development previews do not need a changelog entry.
3. Run `gh workflow run release.yml --ref main` to validate the intended source without publishing. Wait for the **Release** workflow to finish successfully and inspect its results in Actions.
4. From the exact commit that passed validation, create and push the matching annotated tag:

```sh
git tag -a v0.1.0-dev.4 -m "Release 0.1.0-dev.4"
git push origin v0.1.0-dev.4
```

The tag push runs release validation again and publishes after all required jobs pass. Preview releases start with a warning that they are for disposable installations and upgrades are not guaranteed, followed by grouped changes with commit, PR, and contributor links. They never become the latest stable release.

## Prepare final notes

Set the final version in `Cargo.toml`, update `Cargo.lock` with `cargo update --workspace`, and add a matching dated entry to [CHANGELOG.md](../../../CHANGELOG.md), following its existing heading format.

Write the entry yourself or [generate a draft](#optionally-generate-a-draft), paste it into the changelog, and edit it. Include any upgrade instructions and contributor credits you want published. Use explicit links for PRs and usernames so they work in both the changelog and release notes.

The leading version selects the entry; an optional title or link after the date is allowed. The final release publishes only that entry's body, up to the next `##` heading. It adds no warning, footer, or regenerated content. Missing, duplicate, empty, or unfinished entries containing TODOs block the release.

Merge the version update and completed notes into `main`, review [migration requirements](../testing/database-provider-tests.md), and validate with `gh workflow run release.yml --ref main`. From the exact validated commit, publish the matching final tag:

```sh
git tag -a v0.1.0 -m "Release 0.1.0"
git push origin v0.1.0
```

Use your actual release version in the tag commands. Tags must match the workspace version, and the tagged commit must be on `main`'s history. An existing release tag must never be moved or reused.

## Optionally generate a draft

Check out the source you want to describe and ensure its commits are pushed to GitHub. Use an authenticated GitHub CLI, or provide `GH_TOKEN` or `GITHUB_TOKEN` with contents and pull-request read access.

```sh
node .github/scripts/release.mjs notes 0.1.0 srothgan/ruphish > release-notes.md
```

Open `release-notes.md`, copy the content beneath your dated changelog heading, and edit it as needed. The command only prints Markdown; the shell redirects it into the file. It does not edit `CHANGELOG.md`, change the version, create a commit or tag, or publish a release.

By default, the draft ends at the current `HEAD`. Add a commit or ref after the repository to select another ending point. Passing a development version such as `0.1.0-dev.4` previews the development warning and its change list instead.

### What the generated list includes

| Notes | Changes included |
| --- | --- |
| Development preview | Since the preceding reachable preview of the same base version; otherwise since the preceding reachable final release |
| Final draft | Since the preceding reachable final release, including changes already shown in previews |
| No eligible earlier release | All reachable history through the selected commit |

Baselines must have an older version and be reachable from the selected commit. Unrelated preview versions, current/newer tags, and unreachable tags are ignored. Every non-merge commit in the range is included, oldest first in topological order. Direct commits, squashed PRs, and reverts remain; structural merge commits are excluded.

The shared renderer groups known types in this order: **Features** (`feat`), **Fixes** (`fix`), **Performance** (`perf`), **Documentation** (`docs`), **Refactoring** (`refactor`), **Code style** (`style`), **Tests** (`test`), **CI** (`ci`), **Maintenance** (`chore`), and **Reverts** (`revert`). Empty groups disappear; historical or unrecognized subjects appear under **Other changes**. Known type/scope prefixes are hidden, while explicit breaking markers remain visible. Neither generated style includes a comparison link.

Commit links come from Git history. GitHub supplies verified merged PR references and contributor accounts, including bots. Direct commits do not receive invented PR links; unlinked authors retain their recorded name. Git or API failures stop generation. The release workflow uses its built-in read-only token; no additional App or stored secret is needed.

## Recover a failed publication

Open the failed **Release** run in Actions and inspect the failed job before retrying.

| Failure | Next action |
| --- | --- |
| Source checks, build, or artifact verification failed | Fix the source and prepare a new release version; do not move the existing tag. |
| Publication failed after uploads started | Keep the original run and its `verified-release-bundle`. Fix the external issue, then rerun only the failed publication job. |
| Anonymous GHCR pull failed because the package is private | Set the package visibility to Public, then rerun the original publication job. |

Publication reuses the retained artifacts and resumes only a matching draft release. It checks the tag, release notes, uploaded files, and image digests before continuing. Conflicting content stops the retry; do not rebuild or replace artifacts under the same published version. The release stays a draft until uploads and anonymous image verification succeed.

## What the workflow verifies

The [release workflow](../../../.github/workflows/release.yml) checks source identity and notes, runs full source acceptance, root/browser npm audits and Cargo advisory policy, then builds and tests the embedded administration UI, native archives, and Docker images. It assembles checksums and build provenance, and every required stage must pass before publication. The retained `notes.md` body is uploaded unchanged.

Native targets are Linux x86-64/ARM64, Windows x86-64/ARM64, and macOS ARM64; Docker targets are Linux amd64/arm64. Archives are unsigned and macOS executables are not notarized. Final releases can advance GitHub and Docker `latest` without replacing a newer final release; previews cannot. Manual workflow runs validate and retain artifacts without publishing.

Installation instructions belong in [Getting started](../../getting-started/index.md). Installer release selection is documented in the [website implementation](../../../website/README.md#native-installers-and-release-discovery).

## Validate release-tool changes locally

Run the focused regressions:

```sh
node --test .github/scripts/release.test.mjs
```

Also run the applicable [JavaScript checks](../testing/validation-commands.md#javascript) and [workflow checks](ci-workflows.md#dependencies-and-workflow-changes). Local script tests and workflow lint do not prove a successful hosted release or platform executable.
