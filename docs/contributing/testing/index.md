---
title: Testing Ruphish
description: Choose verification by changed guarantees and distinguish functional, protocol, and browser evidence.
sidebar:
  order: 1
---

Start with the [validation catalog](validation.md). It owns direct commands, prerequisites, and CI selection. Choose checks by the changed behavior and its consumers, not by running every available suite after every edit.

## What each layer establishes

- Domain and contract tests cover rules, transitions, serialization, and boundary cases without a server.
- Adapter tests cover persistence and external protocols with independent fixtures and controlled services.
- HTTP and executable tests cover authentication, configuration, assets, persistence, workers, restart, and real requests.
- [Database-provider checks](database-providers.md) exercise actual MySQL/PostgreSQL behavior; ignored opt-in cases do not establish provider support for a build.
- The [canonical fixture](local-fixture.md) exercises controlled DNS, HTTPS, SMTP, and public interactions.
- [Manual browser checks](manual-ui.md) cover layout, keyboard interaction, editor behavior, and browser collection paths that direct HTTP tests cannot prove.

Tests should assert observable guarantees: responses, stored state, received messages, and external effects. Use exact bytes where they are the contract, not to freeze incidental markup or wording. Expected results should come from requirements or independent fixtures, not the production helper under test.

## Keep work isolated and bounded

Use synthetic recipients and controlled loopback services. Never contact real recipients or production integration endpoints from tests. Extend `tests/acceptance/` instead of creating another persistent environment. Preserve existing local application data when changing fixture tooling.

Run one heavy workload at a time locally, initially with one build job and test thread. Select focused regressions while editing, then affected formatting, linting, and broader checks after changes settle. Repeating unchanged successful checks does not add useful evidence.

## Report results honestly

Record command, revision, toolchain, provider/version where relevant, exit status, and limitations. Keep passed, failed, ignored, and unexecuted checks separate. A compiler/environment failure is not a failed executed test. Check that a filtered test command executed the intended cases; zero matching tests can still return success.

The legacy test map connects source test identities to replacement strategies and status. It is an obligation inventory, not a substitute for execution. Hosted workflows and release artifacts are also separate evidence from local source checks.
