# Contributing to Ruphish

Start with the behavior you want to improve and the people it affects. A useful contribution explains the problem, changes its authoritative implementation, and provides evidence that the intended workflow works.

Participation follows the [Code of Conduct](CODE_OF_CONDUCT.md), which also explains how to report conduct concerns privately.

- For code changes, follow [development setup](docs/contributing/setup.md) and the [architecture boundaries](docs/contributing/architecture.md).
- For documentation, follow the [documentation guide](docs/contributing/documentation.md). Small corrections are welcome; larger changes should keep user tasks, API contracts, and contributor instructions distinct.
- For translations, use the [localization guide](docs/contributing/localization.md).
- For a defect report, use the [bug report form](https://github.com/srothgan/ruphish/issues/new?template=bug_report.yml). Describe what happened, how to reproduce it, and the version/environment you know. Relevant redacted errors are helpful; never include API keys, service credentials, participant input, or a live database.

## Propose a change

Check existing issues and work before starting. Use the [feature request form](https://github.com/srothgan/ruphish/issues/new?template=feature_request.yml) to describe the user's problem and the outcome you need. Add a workaround or alternative if useful; an implementation plan is not required. Blank issues remain available for documentation corrections, questions, and reports that do not fit either form.

Discuss substantial features, architecture changes, and compatibility changes with a maintainer before implementation. Small fixes and documentation corrections can proceed directly to review. Keep each contribution focused on the agreed problem, and discuss substantial scope changes before expanding it.

## Prepare and submit a change

Understand the changes you submit, explain their behavior, and verify the affected workflow. These responsibilities apply equally to AI-assisted contributions. Passing checks alone does not establish that a change works.

1. Establish the scope. Read the relevant source, tests, and [AGENTS.md](AGENTS.md), and follow the workflow through authorization, persistence, external effects, failure, and restart.
2. Make a focused change with tests appropriate to the affected behavior. Update affected user documentation or API references with the implementation, and preserve attribution for reused material.
3. Use [validation](docs/contributing/testing/validation.md) to choose and run checks for the changed guarantees. Report exact commands and outcomes, including ignored or unavailable checks. A documentation-only change needs relevant link/example checks, not a product build. Browser appearance and interaction review is manual.
4. Review your diff for unrelated changes, generated output, credentials, and unintended behavior. Check that the implementation, tests, and documentation describe the same result.
5. Submit a pull request explaining the concrete problem and resulting behavior. Link related issues and scope discussions where available, and include validation results and remaining limitations.

Use commit messages in the form `type(scope): imperative summary`, such as `docs(api): explain campaign launch retries`. Keep the scope meaningful and the description about the final change.

Record user-visible changes in the **Unreleased** section of [CHANGELOG.md](CHANGELOG.md). Describe the outcome and any action users need to take; small editorial corrections do not need an entry.

## Work through review

Address outstanding questions and requested changes in the pull request. Explain disagreements with evidence, such as a reproducible scenario, a documented contract, or the relevant implementation. Summarize what changed after feedback and report any additional validation. If feedback expands the work substantially, agree on the revised scope before continuing.

## Compatibility and data handling

Do not weaken ownership, authorization, transaction, recovery, or participant-data boundaries to make a scenario pass. Participant tracking may record explicit event metadata and optional presence booleans; it must not collect entered usernames, passwords, OTPs, arbitrary form values, or their derivatives. Administrator authentication and configured service credentials are separate concerns.

Preserve the behavioral obligations in [the legacy test map](tests/legacy-test-map.toml). Add tests for observable guarantees and plausible defects, using independent expectations and controlled services. Extend the existing [acceptance fixture](tests/acceptance/compose.yaml) instead of creating another persistent lab.

## Project policies

The project license is [MIT](LICENSE); reused material retains its [third-party notices](THIRD-PARTY-NOTICES.md).

Follow the [security policy](SECURITY.md) for suspected vulnerabilities. Do not include secrets or sensitive exploit details in public issues or pull requests.
