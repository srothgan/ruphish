# Working instructions

These rules guide implementation, review, validation, and reporting in this repository. Apply them to the affected work; use the linked documentation for product behavior and detailed procedures.

## Core rules

- **Understand before changing.** Inspect the affected implementation, consumers, tests, and documentation through success, failure, and recovery. Verify actual support and installed dependency APIs rather than assuming them. Read optional `PERSONAL.md` when present.
- **Reuse and centralize.** Extend existing implementations, helpers, components, and constants. Keep shared rules and transformations at one authoritative owner, fix them there, and update callers instead of adding local copies or compensating checks. Combine code by shared responsibility, not superficial similarity.
- **Keep state coherent.** Give mutable data one owner, derive dependent values, and represent mutually exclusive states explicitly. Keep drafts distinct from confirmed data; define invalidation for necessary caches instead of maintaining manually synchronized copies.
- **Keep solutions focused.** Implement the simplest correct behavior within scope. Add abstractions, configuration, caching, or concurrency only for a concrete current benefit; reuse valid results or batch work before adding machinery. Do not introduce placeholders or success-returning no-ops.
- **Respect boundaries.** Follow the [architecture](docs/contributing/architecture.md) and established conventions. Keep dependencies inward, handlers/adapters thin, and browser contracts free of backend secrets. Preserve authorization and boundary validation; avoid rechecking invariants already enforced within a trusted path.
- **Keep changes coherent.** When changing behavior or contracts, update affected types, callers, persistence, documentation, and tests together. Identify breaking changes explicitly; do not retain compatibility layers by default.
- **Use explicit, typed outcomes.** Express success and failure through clear types, handle expected cases explicitly, and propagate errors with useful context. Avoid silent fallbacks that make failed operations appear successful.
- **Control asynchronous work.** Prevent stale results and duplicate effects; account for retries, cancellation, and uncertain outcomes. Release tasks, listeners, subscriptions, and other resources when their owner ends.
- **Finish replacements.** Remove superseded paths, unused dependencies, obsolete flags, and stale comments within the changed scope. Verify remaining consumers and compatibility before removing legacy behavior; keep names and documentation accurate.
- **Preserve Rust safety.** Keep production code free of unsafe code and accidental `unwrap`, `expect`, `panic`, `exit`, `todo`, `unimplemented`, and `dbg` usage. Limit test-only Clippy exemptions to assertions and setup.

## Frontend

- **Use the shared UI.** Follow [frontend development](docs/contributing/frontend.md) and the [design system](docs/contributing/design-system.md). Extend `crates/ruphish-ui/src/shared/components/`, `ruphish.css`, and `theme.css` instead of recreating generic controls or styles in features. Features own their fields, permissions, API calls, and state; preserve specialized accessibility behavior.
- **Fix layout at its owner.** Align buttons with the actual input/select row and match control heights. Choose alignment deliberately for headings and wrapped rows; avoid pixel nudges and indiscriminate centering. The user owns manual visual/interaction review; do not add browser or screenshot automation by default.

## Tests and validation

- **Test real guarantees.** Assert observable responses, persisted state, protocols, and effects with independent expectations and realistic failure cases. Do not mirror implementation details, weaken assertions, or mock away the boundary under test. Keep affected entries in `tests/legacy-test-map.toml` accurate.
- **Keep fixtures isolated.** Reuse existing setup and the canonical `tests/acceptance/` fixture. Use disposable server databases, copy fixtures before mutation, preserve application-lab data, and never contact real recipients or external integration endpoints.
- **Validate by impact.** Follow [validation](docs/contributing/testing/validation.md) for change-specific checks and commands. Cover affected consumers and real failure boundaries, including provider and recovery scenarios when relevant. Run focused regressions during implementation and applicable completion checks after edits settle; ordinary changes or commit requests do not require full acceptance.
- **Check the actual target.** Run rustfmt, affected Clippy/tests, and Leptos formatting for changed markup. Include WASM checks for UI/browser-shared Rust and a development UI build for changed output/assets. Run relevant JavaScript tests; avoid redundant syntax checks. Prose-only changes require link/example/consistency checks, not product builds.
- **Control validation cost.** Diagnose failures before retrying and rerun affected checks only. Do not duplicate Clippy with `cargo check` for identical targets or build without an executable/artifact need.

## Tools and formatting

- **Use the pinned toolchain.** Follow [development setup](docs/contributing/setup.md). Use Cargo/rustup commands for crate, dependency, and tool changes; let the CLI write dependency versions and lockfiles. Configuration remains directly editable. Keep dependency exceptions narrow and preserve source/license notices.
- **Keep commands direct.** Do not add test-runner wrappers, Cargo aliases, or npm wrappers. Keep CI path groups in `.github/paths.yml` and fixed commands in workflow jobs; never construct commands from changed paths.
- **Respect generated assets.** Run `npm ci` for fresh installs or changed dependency inputs; reuse assets only with unchanged inputs. Let Trunk generate its inputs and prepare them separately when standalone WASM checks need them. Use development builds unless a release task is requested; retain `dist/.stage` staging and `dist/admin` output without hand-editing generated files or weakening manifest/path checks.

## Completion

- **Review the finished change.** Check the diff and affected consumers against these rules, correct introduced defects, and update existing documentation. Report unrelated confirmed issues with evidence and a focused fix without expanding scope; distinguish uncertainty and avoid invented findings or routine checklist reports.
- **Report actual verification.** State exact commands and results, separating setup failures, failing tests, and skipped or unexecuted checks. Never count unrun checks as passing or source validation as proof of untested release artifacts.

## Git and external actions

- **Respect authorization.** Read-only Git inspection and local validation workflow changes/runs are allowed. Commit or push only when requested in that turn; do not carry authorization forward. Branch changes, merges, rebases, and history rewrites require an explicit request.
- **Keep external actions explicit.** Do not operate on GitHub, change remote settings, create releases/tags, publish, package for distribution, or deploy unless explicitly requested.
- **Write precise commit messages.** Describe net changes against `HEAD` in imperative form: `<type>(<scope>): <message>`, with optional scope and type `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, or `revert`. Include 2–5 body bullets; omit temporary planning files unless part of the change and never add agent attribution or a `Co-Authored-By` trailer.
