# Product development instructions

## Scope and structure

The seven crate boundaries are domain, wire contracts, application use cases, adapters, HTTP, WASM UI, and executable composition. Keep domain rules authoritative, handlers/adapters thin, dependencies directed inward, and browser contracts free of backend secrets or infrastructure. Workspace architecture tests enforce the declared boundary policy; update that policy only for an intentional architectural change.

Native libraries are scaffolds until functionality is implemented with its tests; do not add fake domain models, no-op operations, or success-returning placeholder handlers. Existing compatibility fixtures are independent expectations, not evidence that Rust already supports those features. Preserve the legacy-test map's honest statuses and all original scenario guarantees as they are implemented.

## Working discipline

- Inspect relevant implementation, fixtures, consumers, and error paths before changing behavior. Follow complete workflows through validation, persistence, emitted effects, and recovery.
- Each invariant, transition, relationship, and derived value has one authoritative owner. Avoid duplicated state, redundant validation, and alternative stale implementations.
- Use `cargo new`, `cargo add`, `cargo remove`, `cargo update`, `cargo install`, and `rustup` commands for crate creation, dependency/tool installation and changes; let the CLI write dependency version entries and lockfiles rather than adding dependencies by manually typing versions into manifests. Editing workspace metadata, profiles, feature configuration, and lint policy is normal configuration work.
- Preserve external wire/data/template contracts and meaningful negative guarantees. Record an intentional compatibility difference and its tests explicitly before adopting it.
- Match user-visible capabilities and outcomes; cleaner implementations and UI arrangements are allowed. Legacy components, markup structure, and harmless serialization formatting are not compatibility requirements. Define tests by what users can do and what their content does, preserving exact bytes only where they are part of a real contract.
- Prefer typed errors at library boundaries and contextual startup errors. Production code forbids unsafe code and accidental unwrap/expect/panic/exit/todo/unimplemented/dbg usage; tests have narrow Clippy exemptions for assertions and setup.
- Add tests for actual domain/functionality guarantees, persisted state, protocols, and real failure boundaries. Do not add mirror tests for trivial code or claim scaffold checks establish feature parity.
- The user performs manual visual/UI interaction tests. Compile and build WASM; do not add browser/screenshot automation by default.
- Review frontend alignment deliberately: labeled fields and adjacent buttons align on the actual control row with matching heights; headings, actions and wrapped rows need their own appropriate alignment. Fix the shared layout owner instead of adding pixel nudges. Do not default every flex/grid container to center alignment; keep centering where it serves the content, such as within icon buttons.
- Reuse management UI from `crates/ruphish-ui/src/shared/components/`: page/table/form primitives, list search and empty states, action buttons/links and dialogs. Shared styles live in `crates/ruphish-ui/ruphish.css`; the palette lives in `crates/ruphish-ui/theme.css` and is shared with the isolated visual editor. Feature modules own their fields, permissions, API calls and state; they must not recreate generic controls or inline replacement empty states. Specialized editor tabs, disclosure controls and JavaScript-bound controls retain their specific accessibility behavior.
- Never manually hard-wrap Markdown paragraphs or list items. Use one source line per paragraph or list item.

## Validation after changes

[Validation](docs/contributing/testing/validation.md) is the single command and CI reference. Select checks by changed behavior and affected consumers, including shared contracts, configuration, build inputs and fixtures. During implementation run the focused regression; after edits settle run the applicable checks below. A local commit does not require full acceptance.

CI provides the broader backstop: `pr.yml` selects main-PR checks, `nightly.yml` runs full source acceptance, and `release.yml` adds optimized-profile checks for release-target PRs or explicit manual runs. Native CI covers Linux, Windows and macOS. The shared `validation.yml` owns source-check commands. Once the repository exists, require `PR gate` on main and `Release gate` on release. Do not copy those full matrices into agent completion work; directly affected guarantees still require focused local regressions. Single-binary shipping is a later feature, separate from this validation foundation.

| Change | Agent completion checks |
| --- | --- |
| Prose documentation | Relevant links, examples and consistency; no product builds |
| Native Rust | Rustfmt, affected-target Clippy and relevant unit/integration tests |
| Rust UI or browser-shared domain/contracts | Rustfmt, Leptos formatting where markup changes, affected functional tests and WASM Clippy/tests |
| JavaScript | Relevant functional tests; syntax checks for files not parsed by those tests or the build |
| UI output/assets | Development UI build; user owns visual/manual browser review |
| HTTP/authentication/privacy | Rejection and legitimate-use cases plus the affected executable workflow |
| Provider SQL/shared persistence | Relevant real-provider scenarios; shared storage covers affected engines; version-sensitive SQL/schema/driver changes cover both supported versions |
| Migrations | Affected upgrade paths, preserved data, rollback/recovery and reopen |
| Dependencies/toolchain | Dependency policy and affected native/browser targets |
| Validation workflows/selection | Workflow gate tests, workflow lint and YAML path-group/coverage inspection; no automatic product acceptance run |

- Keep direct Cargo/Node/Docker commands visible; do not add a local test-runner, Cargo alias or npm wrapper. Keep CI path groups in `.github/paths.yml`; workflow jobs own fixed check commands and must not construct shell commands from changed paths.
- Do not automatically run the full process suite, historical migrations, all database versions or public Docker acceptance after ordinary edits. Run these when their guarantees are affected or the task requests acceptance.
- Never treat ignored, skipped, unavailable or unexecuted checks as passing. Report exact commands and limitations. Preserve the legacy-test map and meaningful negative guarantees.
- Run one heavy workload at a time locally, initially one build job and test thread with bounded Tokio/Rayon workers. Investigate failures before retrying; rerun only affected checks after corrections.
- Run rustfmt for Rust changes and Leptos formatting for markup changes. Preserve the pinned formatters, 100-column Rust/markup style and LF endings. Review formatter edits; do not blanket-skip formatting.
- Use Clippy for static compilation/lint feedback; avoid duplicate cargo check for identical targets. Build when an executable, changed distribution or artifact test requires it. Native success does not prove WASM compatibility.
- Run npm ci on fresh installations or changed dependency inputs. Reuse unchanged valid assets. Trunk runs the JS/CSS generator; do not repeat it immediately before Trunk. Standalone WASM checks may require prior generation.
- Use development builds normally; release builds require an explicit release task. Keep Trunk staging at dist/.stage and the completed distribution at dist/admin. Never hand-edit generated files or weaken manifest/path checks.
- Keep one canonical Docker fixture under tests/acceptance. Use disposable server databases, preserve application-lab data and never contact real recipients/integration endpoints during tests.
- Local PR, nightly and manual validation workflows are authorized. Remote branch protection, publishing and deployments remain separate explicit actions. Source acceptance does not validate untested release artifacts.

## Toolchain

`rust-toolchain.toml` owns Rust/components/targets. `Cargo.toml` workspace metadata owns leptosfmt and cargo-deny versions; `Trunk.toml` owns Trunk. Use Cargo/rustup CLI installation and dependency commands. See [development setup](docs/contributing/setup.md) and [validation prerequisites](docs/contributing/testing/validation.md#prerequisites). Keep dependency exceptions narrow and source/license notices intact.

## Data and side effects

Ruphish's built-in collector and backend must not collect or retain entered login information, arbitrary submitted field values, or derivatives. Allow only explicit event metadata and optional presence booleans through the closed event schema; preserve the boundary in logs, errors, database records, exports, webhooks and legacy-data reads. No built-in capture setting is authorized. The approved DEC-053 direction permits user-authored/imported landing-page scripts as normal functionality, without a curated script allowlist or separate opt-in mode. Authors/operators control their code and external destinations, so do not promise that arbitrary authored pages cannot read or transmit participant input. Keep authored execution isolated from the administrative application and its credentials; retain ownership, authorization and server-side validation. The migration-root participant-privacy contract owns the detailed decision. This is planned work until implemented and verified; existing tests do not establish the changed rendering behavior. Administrator authentication and configured service credentials are separate from participant input.

SQLite remains the default local configuration; MySQL and PostgreSQL are selected explicitly at startup. Database upgrades must preserve a recoverable original and be tested against independent legacy data. Fixtures contain synthetic credentials and must be copied before mutation. Product tests must not contact real recipients or external integration endpoints.

Do not operate on GitHub, create releases/tags, publish, package for distribution, or deploy unless explicitly requested. Report commands actually run and any unresolved checks or reachable defects honestly. Optional PERSONAL.md in this directory adds local preferences if present.

## Git and commit messages

Read-only Git inspection such as `git status`, `git diff`, `git log`, and `git show` is allowed.

Commit and push only when the user asks for it in that turn, never as a side effect of finishing a task. One approval covers that commit or push only; it does not carry over to later ones.

Do not merge, rebase, create/delete/rename branches, switch branches, or rewrite history unless explicitly requested.

When writing or suggesting a commit message:

- Describe the net changes against `HEAD`, not the local implementation path.
- Use imperative mood, present tense, and concise language.
- Use the format `<type>(<scope>): <message>`; scope is optional. Allowed types are `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, and `revert`.
- Include 2–5 precise bullet points in the body.
- Do not mention temporary or uncommitted planning files unless they are part of the intended change.
- Do not add a `Co-Authored-By` trailer, a generated-by footer, or any other agent attribution.
