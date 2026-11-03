---
title: Choose and write tests
description: Select checks for the behavior you change and place regressions at the boundary they verify.
sidebar:
  order: 300
---

Start with the guarantee your change affects. A domain rule, SQL transaction, browser form, and release archive need different evidence. Use focused regressions while editing, then run the applicable [validation commands](validation-commands.md) once the change settles.

## Choose the test boundary

| Change | Test location and evidence |
| --- | --- |
| Business rule or state transition | Domain tests: accepted/rejected inputs and independently expected outcomes |
| Request or response shape | Contract serialization and HTTP tests, plus affected browser consumers |
| Persistence or concurrency | Adapter/provider tests: stored values, atomicity, competing writers, rollback, and uncertain outcomes |
| Server startup or background workflow | `tests/startup_process.rs` and its modules: actual process, HTTP/protocol effects, restart, and shutdown |
| Browser transformation or request | Native UI tests, functional JavaScript tests, and WASM request tests |
| Mounted forms and navigation | Built-UI artifact tests with synthetic API/demo transports |
| Layout, keyboard, sandbox, or mail-client rendering | [Manual browser checks](manual-browser-checks.md), including actual browsers or mail clients where required |
| MySQL/PostgreSQL behavior | [Real database-provider tests](database-provider-tests.md) |
| Public DNS/HTTPS/SMTP and packaged image behavior | [Local acceptance fixture](local-acceptance-fixture.md) |
| Documentation | Links, example syntax and source correctness, site build, and the affected reader journey |

Native workspace defaults exclude the UI. Native success alone does not establish WASM compatibility. A built-UI test in JSDOM exercises real application code but cannot prove layout or browser sandbox enforcement.

## Write a meaningful regression

Choose an observable failure: a wrong response, partial write, duplicated external effect, lost draft, or invalid state transition. Derive the expected result from the intended contract, not from the production helper being tested. Include realistic failure or recovery cases when the change crosses those boundaries.

For example, a resource-update regression can submit a stale revision and assert both the conflict response and unchanged saved content. A provider test can force failure while replacing children and verify the original record remains intact. Neither needs to assert private helper call order.

Do not delete a regression, weaken its assertions, or mock away the affected boundary merely to make a failing implementation pass. If an intentional contract change makes an expectation obsolete, explain that change and replace the assertion with one that verifies the new guarantee. Use mocks for dependencies outside the behavior under test; keep the boundary whose correctness you are claiming in the test.

Reuse existing test helpers and the shared provider scenarios. Keep affected entries in [the legacy test map](../../../tests/legacy-test-map.toml) accurate; it records inherited obligations and intentional replacements, not executed results.

## Keep fixtures isolated

Use synthetic recipients and controlled loopback services. Do not contact real recipients, production mailboxes, or external integration endpoints. Copy fixtures before mutation and use disposable server databases.

Extend `tests/acceptance/` for the persistent local application lab. Preserve its existing data when changing tooling; use a separately selected disposable state directory for destructive verification. Provider-only services use disposable databases and do not need another application installation.

## Report what ran

Record commands, results, and relevant environment/provider versions. Separate setup failures, failed assertions, ignored cases, and unexecuted checks. Confirm filtered commands executed the intended tests: Rust can exit successfully with zero matches.

A site build proves rendering completed, not that a command example works. A protocol test proves the tested exchange, not a human browser interaction. Full source acceptance, a verified release artifact, and a deployed installation are separate results.
