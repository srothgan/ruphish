# Pre-release cleanup plan

Status: implementation and review evidence is tracked in [cleanup-ledger.md](cleanup-ledger.md). Originally created on 2026-10-28; Phase 5 was added on 2026-10-29 for the subsequent performance/query review findings and remains pending until verified. Phase 6 records five further functional and quality findings from the 2026-10-29 review, also pending implementation and verification. The user requested functional fixes first, followed by substantive quality cleanup, with diligent sequential work and a persistent ledger. The original checkout must remain untouched.

## 1. Objective and scope

Prepare the codebase for its first release by fixing confirmed functional defects, controlling unnecessary resource work, consolidating duplicated responsibilities and removing verified obsolete code or state. Start with the 13 findings from the review, then perform an explicit quality pass through every crate. The original findings are a starting backlog, not a claim that all cleanup opportunities have already been discovered.

Keep changes focused on demonstrated behavior or maintenance problems. Preserve supported authorization, privacy, persistence, recovery, wire and migration contracts. The compatibility question was not explicitly resolved during discussion; therefore preserve currently documented compatibility by default. A proposal to drop a supported contract must identify its consumers, migration consequences and tests, and receive a separate scope decision before implementation. Unsupported, unreferenced leftovers may be removed after verification.

Do not introduce new features, a generic form framework, a generic storage framework, speculative caches or broad cosmetic rewrites. Improve readability where it supports the changed responsibility. Complete replacements by removing superseded callers, flags, dependencies, comments and obsolete implementation-only tests where appropriate.

## 2. Workspace, baseline and authorization

- Allowed implementation workspace: `C:/Users/Simon Peter Rothgang/Desktop/project-ruphish/ruphish-review-6838be3`.
- Protected original checkout: `C:/Users/Simon Peter Rothgang/Desktop/project-ruphish/ruphish`. Do not edit, stage, restore, clean, switch, reset, rebase or otherwise mutate this checkout. Do not run builds or generators there.
- At the original handoff, this worktree was detached at the reviewed commit `6838be3e4034aad75393b4d6f24561f7cb77d154`, with this plan as its only intended new file. That is historical setup information; subsequent phases continue from the existing implementation branch and ledger, preserving completed work rather than repeating branch setup.
- At plan creation, the original checkout's committed HEAD was `303549b86a820d74c26d2c73a1cbf8440e7a1a5a`. This is the recommended implementation starting point because it includes subsequent committed demo fixes. Do not silently import uncommitted work from another checkout.
- Before code edits, establish a named cleanup branch in the allowed worktree, preferably `cleanup/pre-release`, from the baseline explicitly authorized in the user's launch instruction. If that branch already exists, inspect it; never overwrite it automatically. If the launch instruction does not authorize branch setup, resolve that setup before implementation.
- Record the actual implementation base SHA and branch in the ledger. Revalidate findings against that base. Do not reset the original branch to the older review baseline. Reassess the PR target before publication: the correct target depends on where the underlying feature work has landed.
- The agent's actual application workspace must be bound to the allowed worktree, not merely a shell command's working directory. Verify the selected folder/worktree and any available thread workspace metadata. A shell-created worktree alone does not bind an existing T3 thread; a thread attached to the original checkout can display other sessions' changes in its automatic checkpoint. If necessary, open this worktree as its own workspace/project before starting the implementation agent.
- Use explicit working directories or absolute paths for commands and edits. Avoid writes to shared `.git` configuration or ignore files to hide planning artifacts. Keep build output in the allowed worktree's normal ignored output locations.
- This plan authorizes no commit, push, PR creation, merge, rebase, deployment, release or tag by itself. Follow the user's current-turn instructions and AGENTS.md for those actions. Do not spawn other agents unless the user explicitly requests delegation.

## 3. Evidence and instructions

Read [AGENTS.md](AGENTS.md), optional PERSONAL.md if present, [architecture](docs/contributing/architecture.md), [setup](docs/contributing/setup.md) and [validation guidance](docs/contributing/testing/validation.md). For frontend changes also read [frontend development](docs/contributing/frontend.md) and [design system](docs/contributing/design-system.md). Respect the pinned toolchain and existing fixtures. Keep Markdown prose paragraphs and individual list items on single source lines.

Review evidence is outside the implementation checkout:

- [Overall review](../ruphish-review-report-6838be3/OVERALL.md).
- [Quality rules](../ruphish-review-report-6838be3/quality-rules.md).
- [Original review ledger](../ruphish-review-report-6838be3/LEDGER.md).
- [Domain](../ruphish-review-report-6838be3/ruphish-domain.md), [contracts](../ruphish-review-report-6838be3/ruphish-contracts.md), [application](../ruphish-review-report-6838be3/ruphish-application.md), [adapters](../ruphish-review-report-6838be3/ruphish-adapters.md), [HTTP](../ruphish-review-report-6838be3/ruphish-http.md), [UI](../ruphish-review-report-6838be3/ruphish-ui.md) and [executable](../ruphish-review-report-6838be3/ruphish.md).

Keep these reports unchanged as historical evidence. Their source line references describe `6838be3`; after changing this worktree's baseline or implementation, resolve current locations again. Use Git to inspect the historical commit when needed. The review's 344 passing tests, 41 ignored tests and successful selected Clippy checks are baseline evidence, not verification of future edits.

## 4. Execution discipline and ledger

Create `cleanup-ledger.md` in the allowed worktree before implementation. Keep this plan as the agreed scope; record execution evidence in the ledger. Work sequentially on one owning crate or coherent cross-crate issue at a time. Changes to direct consumers are part of the same issue. Finish its implementation, focused validation and ledger checkpoint before starting an unrelated issue. Do not leave partially replaced paths behind.

For every item, record its ID, review/checklist references, owner, current evidence with file/line references, affected consumers, observable guarantee, proposed scope, actual changes, removed paths, exact verification commands/results, remaining gaps and disposition. Use these statuses: pending, investigating, confirmed, implementing, verified, already resolved, rejected with evidence, or deferred with reason. A failed prerequisite or unavailable test is not a pass. Deferred release-critical work prevents declaring the cleanup release-ready.

Start each item by following success, failure and recovery through the current implementation and callers. Establish a meaningful regression for an actual bug before changing behavior where feasible. Use independent expectations and real boundaries; do not reproduce the implementation in a test or replace important provider behavior with mocks. Pure deletion of unused code normally needs consumer searches and existing checks rather than a new test of absence.

When investigating quality debt, record the candidate before changing it. New functional or safety defects take priority over lower-priority cleanup. Give additional confirmed findings stable IDs such as `CLEAN-001` and map them to quality-rules.md. Record rejected suspicions and deliberate state distinctions so later passes do not rediscover them. Do not use a finding quota.

At each phase boundary, record a concise review checkpoint and continue through the plan unless the user requests a pause or a real scope/setup decision blocks dependent work. The final handoff goes to the independent reviewer. No automatic approval request is needed for each routine, authorized local change.

## 5. Phase 0 — Establish the implementation baseline

- [ ] Verify actual workspace binding, canonical repository root, branch, base SHA and local status. Account for this plan file and preserve any unexpected existing work.
- [ ] Establish the user-authorized cleanup branch in this worktree. Record its base. Recheck that the protected checkout has not been changed by this setup.
- [ ] Inventory the workspace members at the selected base. The review covered seven crates; include any subsequently added crate if it exists at the implementation base.
- [ ] Revalidate the 13 original findings and label each as still present, already resolved or superseded with evidence. Inspect changes since the review only as needed to establish the correct owners and contracts.
- [ ] Record existing setup or test failures separately from introduced regressions. Choose focused initial checks; do not launch full acceptance merely to establish a baseline.

Completion: the branch and baseline are explicit, the working location is isolated, and the backlog describes the code that will actually be edited.

## 6. Phase 1 — Functional correctness and bounded execution

### F1 — Reject obsolete history failures before mutating UI state

Reference: UI-03; S05/C04/S07. Owner: ruphish-ui. Entry points: `campaign_delivery.rs`, `recipient_history.rs`, `event_table.rs`.

The terminal-error callback clears history before the state owner checks the request ticket. Put all success/failure mutations behind the same acceptance check. Preserve clearing on a current terminal failure and retention on a current retryable failure. Cover a read invalidated by a successful edit while the history view remains mounted; component teardown cancellation does not cover that scenario.

Acceptance: a delayed obsolete 403/404 cannot clear newer events, attempts or cursors; a current terminal error still clears inaccessible data; current retryable failure behavior remains correct. Exercise the actual callback/state boundary, not only the existing guard inside `History::finish`. Compile and lint actual WASM consumers as well as running focused state/component tests available in the repository.

### F2 — Bound expensive content work outside async request workers

Reference: HTTP-02; C08 and asynchronous resource ownership. Owners: HTTP with the application/content adapters it invokes. Inspect bundle import, content preview, campaign review and test-delivery composition callers.

Use an owned bounded blocking path for expensive synchronous content processing. Reuse the existing rendering admission pattern where its responsibility matches. Hold capacity until the blocking computation actually ends, including after request cancellation. Preserve authorization before processing and transaction/revision checks before effects. Do not replace the problem with unlimited blocking jobs or a generic job system.

Acceptance: controlled slow content work does not monopolize the async worker needed by an unrelated lightweight operation; admitted work remains bounded when requests are cancelled; actual valid/invalid bundle and rendering fixtures retain their behavior; errors remain explicit. Verify all affected consumers, including standalone preview and test delivery. Timeouts must not be described as preempting a non-yielding synchronous computation.

### F3 — Reject oversized audience aggregation promptly

Reference: APP-01; C06. Owner: application campaign preparation, with provider callers.

Stop accumulating new unique recipients as soon as the campaign audience ceiling is exceeded. Preserve first-occurrence precedence and conflict behavior for valid audiences. The group and campaign ceilings are distinct business limits even if their numbers match. An early aggregation check does not bound provider loading that happened earlier; inspect and report that separately if it presents another confirmed problem.

Acceptance: exact-cap audience succeeds, one additional unique recipient rejects, duplicates do not consume extra capacity, valid conflict ordering is stable, and a large invalid tail does not keep expanding aggregation containers. Retain actual provider preparation coverage; avoid brittle timing assertions.

### F4 — Preserve typed authorization/storage failures

Reference: APP-02; D05/O05/C03. Owner: application delivery service.

Replace its manual authentication-error mapping with the existing canonical conversion. Keep the subsequent permission requirement. Do not introduce another wrapper or change external status codes incidentally.

Acceptance: busy/unavailable storage errors retain their typed classification; missing identity is unauthorized; missing permission is forbidden. A focused fake can verify this conversion, while real provider authorization tests remain in place.

Phase completion: these findings are fixed and verified or explicitly disproven on the selected base, with no unresolved introduced functional regressions. Newly discovered functional blockers are recorded and addressed before continuing to discretionary cleanup.

## 7. Phase 2 — Remove unnecessary database work

### P1 — Batch resource-summary counts

Reference: ADP-01; C06. Owner: adapter resource repositories for SQLite, MySQL and PostgreSQL. Inspect the HTTP single-group lookup that currently obtains all summaries as a related consumer, without counting the same query problem twice.

Replace per-resource aggregate queries with set-oriented queries or a fixed number of grouped queries. Preserve owner filtering, zero counts, the exact active/completed campaign predicate and snapshot consistency. Keep provider-specific SQL and locking where required. Do not introduce a mutable cache of counts.

Acceptance: independently seeded empty/single/multiple-resource cases and another owner's data return correct counts; completed/active distinctions remain correct; query work no longer grows by one round trip per resource. Validate all modified providers with their real contract suites using disposable databases. If a provider environment is unavailable, report that gap and do not claim provider parity is verified.

### P2 — Calculate export size only for export requests

Reference: ADP-02; C06. Owner: adapter reporting snapshots for all providers.

Run the byte-size aggregate and its rejection only for actual exports. Preserve filtering, row limits, the 32 MiB threshold and transaction boundaries. Required ordinary analytics aggregates remain in place.

Acceptance: analysis values are unchanged, oversized exports still reject, and ordinary reads do not execute the export-size calculation. Keep existing independent event-count and parsed CSV/XLSX checks. Coordinate real-provider verification with P1 where practical without conflating the changes.

## 8. Phase 3 — Concrete quality improvements and removal

### Q1 — Retain typed delivery reads and remove repeated page copying

References: UI-01 and UI-02; L04/O07/C06. Owner: UI campaign query and delivery presentation.

Keep the complete validated delivery DTO through query state and derive typed presentation from it. Remove the JSON wrapper/conversion and string-status conventions where superseded. Give each keyed recipient row access to its current item without cloning/searching the whole page independently. Use derived data or borrowed access rather than another mutable cache. Editable incomplete drafts have a different responsibility and are outside this conversion unless separately justified.

Acceptance: empty pages, selection, cursors, polling, retained presentation during refresh and missing-recipient errors remain correct. Refresh, reorder and removal preserve the right recipient's expanded history identity. F1 remains protected. Verify actual WASM callers and relevant mounted workflow coverage. Demonstrate the reduction in repeated work without fragile performance thresholds.

### Q2 — Serialize typed HTTP responses directly

Reference: HTTP-01; C06/O08. Owner: HTTP reporting/overview/settings/domain/integration response callers.

Use the existing generic serializer directly for complete DTOs and vectors. Remove redundant intermediate JSON trees and convenience layers with no remaining purpose. Retain dynamic JSON where it genuinely constructs the required heterogeneous object.

Acceptance: response shapes, explicit nulls, escaping, numeric/time precision, error mapping and headers remain correct at affected endpoints. Do not add an object-member ordering guarantee that the API does not promise.

### Q3 — Give reporting filter validation one owner

Reference: ADP-04; D01/D04. Owners: application filter contract and adapter boundary callers.

Centralize the identical window/offset/search/selection predicates. Keep validation calls at each external provider boundary and keep dialect-specific query code separate. Choose the existing authoritative type/helper that represents this responsibility.

Acceptance: all providers reject the same invalid cases with the established outcome; supported boundary values remain accepted; ownership and query filtering remain enforced. Remove the duplicated predicates after updating every caller.

### Q4 — Reuse one SQL-placeholder conversion

Reference: ADP-05; D05/D01. Owner: shared adapter helper used by event edits and campaign ordering.

Consolidate the identical internal positional-placeholder transformation after confirming both callers' generated SQL assumptions. Keep provider parameter binding intact. A small existing/shared utility is sufficient.

Acceptance: relevant SQLite/MySQL/PostgreSQL edit and cursor queries bind the same values in the correct positions. Both consumers use the single owner; the superseded implementation is removed.

### Q5 — Express the learning sanitizer's final policy directly

Reference: ADP-03; L04/O08. Owner: the content-processing module that owns learning sanitization at the chosen base.

Remove the obsolete configure-then-revoke control allowlists. Preserve the distinctions between stripping wrappers and removing contents, useful semantic containers, title/root attributes, literal template text and removed-control diagnostics. Authored landing capability policy remains distinct from static learning content.

Acceptance: learning fixtures preserve explanations and layout semantics while removing active controls and collectors; authored landing behavior stays supported; obsolete configuration paths and their misleading comments are removed.

### Q6 — Remove unused dashboard helpers

Reference: UI-04; L01. Owner: UI `overview_state.rs`.

Repeat the workspace consumer search for `ChangeGoal`, its `tone` method and `count_change`. If they remain unused by production consumers, remove them and their helper-only tests. Keep active dashboard helpers and supported backend posture contracts. Do not add a new UI feature to justify otherwise dead code.

Acceptance: no remaining consumers or obsolete exports; native UI tests and WASM checks pass. No new behavior test is required solely to assert that unused code was deleted.

## 9. Phase 4 — Dedicated quality audit, crate by crate

Review in dependency order: domain, contracts, application, adapters, HTTP, UI, executable. Insert any additional crate present at the implementation baseline into the appropriate position. Complete the inventory, candidate disposition, focused cleanup, checks and ledger entry for one crate before opening the next crate's audit. Cross-crate consumer tracing is allowed for the current issue.

For each crate, explicitly inspect these responsibilities rather than relying only on lint warnings:

- Dead and obsolete code: production references, exports, feature/target-gated consumers, dependencies, superseded APIs, stale flags/TODOs, compatibility branches and tests that only keep unused helpers alive.
- Parallel state: who writes each value, which owner is authoritative, whether the value is derived, how it is invalidated and whether stale success/error/cancellation paths can mutate it. Remove confirmed redundant storage and synchronization together.
- Duplicated logic: business decisions, transformations, error conversions, constants and repeated conditionals that have the same responsibility and must change together.
- Unnecessary complexity: wrappers without a contract, redundant validation inside a verified trusted path, silent failure defaults, speculative abstractions and unnecessary caching or concurrency.
- Boundaries and readability: dependency direction, coherent names, secret-safe contracts, thin handlers, shared UI usage and local extraction around a real responsibility.
- Test quality: independent behavioral expectations, realistic failure/recovery, missing boundary coverage, copied fixtures and setup, and assertions weakened or bypassed by mocks. Never count an unrun or zero-test target as a behavioral pass.

Do not collapse states merely because their types or values resemble each other. Draft versus confirmed data, presentation snapshots versus cached results, operation identity versus resource revision, and accepted versus unknown delivery each encode distinct requirements. Record the rationale when retaining such a distinction. Remove truly redundant parallel state and the effects/listeners/caches that existed to synchronize it.

A crate may finish with no additional finding. Conversely, a newly confirmed functional defect returns to the functional-priority backlog before further discretionary cleanup. Avoid endless speculative polishing: the audit closes when the scoped candidates are resolved or explicitly dispositioned with evidence.

## 10. Phase 5 — Performance and query execution

Added after the second review on 2026-10-29. Owner: ruphish-adapters, with executable worker and HTTP/UI consumer checks where behavior is affected. References: C06/T01/T02 from quality-rules.md. Work in the existing review worktree and implementation branch. Revalidate current code before each item because earlier follow-up fixes may already have changed it. This phase extends P1 beyond query round trips to actual database work; do not reopen verified behavior unnecessarily.

### Evidence and execution order

Complete PERF-01 through PERF-05 sequentially, with a ledger checkpoint after each item. Record separate SQLite, MySQL and PostgreSQL dispositions, including exact versions, schema/migration state, representative data, query plans, measured work, behavioral results and remaining gaps. Distinguish confirmed runtime behavior from source-level concerns. If a provider already has an efficient plan, record the evidence and leave it unchanged. Do not copy SQLite-specific SQL, locking changes or performance conclusions blindly into the server providers.

The review measured the current SQL using bundled SQLite 3.51.3 in isolated in-memory databases with fixture-derived relevant schema/indexes and current settings/learning additions. Adding 100,000 unrelated records left selected results unchanged but increased VM instructions for nine query variants: campaign recovery, mailbox recovery, webhook recovery, campaign fairness, template summaries, learning-page summaries, mailbox history, template inventory and domain usage. Indexed campaign audience and delivery counts remained essentially constant as controls. These are diagnostic observations, not production latency measurements or permanent acceptance thresholds. Reproduce with the real repository/startup schema in regression coverage. MySQL/PostgreSQL query and index inspection identified shared concerns, but equivalent execution-plan/scaling measurements remain pending.

### PERF-01 — Bound worker recovery to relevant active work

Priority: medium, first because recovery is repeated inside write transactions. Inspect `crates/ruphish-adapters/src/{sqlite,mysql,postgres}/campaigns/queue.rs`, `mailbox_worker.rs`, `webhook_worker.rs` and their schema/index owners. SQLite's campaign attempt, mailbox run and webhook attempt recovery queries scanned completed history even when nothing needed recovery; each grew from approximately 13–14 to 300,013–300,014 VM instructions in its isolated diagnostic. MySQL/PostgreSQL select campaigns, jobs or mailboxes before the corresponding recovery updates, so their invocation scope differs; inspect their lookup indexes and actual plans separately.

Drive recovery from expired active work and locate the corresponding attempts/runs through suitable indexes. Preserve generation/revision fencing, transaction boundaries, lock order, lease semantics and uncertain-delivery classification. Never rerun SMTP or other external effects merely to recover an uncertain outcome. Avoid adding a second mutable representation of active work.

Acceptance: completed unrelated history does not cause proportional work in idle recovery or recovery for one selected item. Cover no expired work, a genuinely expired lease, a current lease, a stale generation/revision and competing workers. Verify the intended durable transitions and absence of duplicate external effects through existing provider/recovery fixtures. Measure the real recovery statements, not a simplified SELECT substitute.

### PERF-02 — Restrict campaign fairness work to candidate campaigns

Priority: medium. Inspect `crates/ruphish-adapters/src/{sqlite,mysql,postgres}/campaigns/queue.rs` and delivery indexes. The fairness CTE aggregates delivery history before choosing a runnable item. In SQLite, adding 100,000 accepted deliveries for a completed campaign increased one claim query from 252 to 1,000,276 VM instructions. The server providers contain the same broad aggregate shape; verify their actual plans before deciding the change.

Identify eligible candidate campaigns before calculating their fairness values, using indexed lookups where appropriate. Preserve the existing meaning of `max(last_claim)` for each candidate campaign; simply removing accepted or other historical rows from that maximum can change scheduling. Preserve profile exclusivity, cooldowns, authorization, domain eligibility and claim fencing. Do not introduce a cached fairness counter or scheduler state alongside durable delivery state.

Acceptance: unrelated completed campaign history does not cause proportional work in selecting one runnable item. Cover competing eligible campaigns, due versus future work, empty queues, profile contention and history that affects the correct fairness order. Verify selection order and concurrency behavior as well as query work on each affected provider.

### PERF-03 — Finish bounded resource-summary lookups

Priority: medium; follow-up to P1. Inspect `crates/ruphish-adapters/src/resource_summaries.rs`, each provider's `resources.rs` and campaign-reference indexes. The newer correlated group-count fix is present; revalidate and retain its regression rather than treating it as missing. Template and learning-page summaries still scanned unrelated campaigns in the measured code, growing from 113 to 800,113 and from 129 to 1,000,129 VM instructions respectively. Page and SMTP-profile summaries share the template implementation and require their own coverage.

Make selected resource IDs drive supported indexed reference lookups, or use an equivalently bounded provider plan. Query restructuring and indexes must be evaluated together. Preserve owner selection, zero counts, single/list lookup behavior, snapshot consistency and the exact active/completed predicate, including existing NULL handling. Reference ownership must not silently change established count semantics. Keep the fixed-round-trip guarantee from P1; do not reintroduce one database request per resource or mutable count caches.

Acceptance: exercise all five resource families, list and single lookups, empty/used/foreign/missing selections, active/completed campaigns and large unrelated data. Assert independent expected counts and physical work. An aggregate returning only selected IDs is not sufficient proof that unrelated rows were not scanned.

### PERF-04 — Index mailbox history by mailbox and ordering

Priority: medium. Inspect `crates/ruphish-adapters/src/{sqlite,mysql,postgres}/integrations.rs` and mailbox-run schemas. The latest-30-runs query has no supporting mailbox/history index in the inspected migrations. In SQLite, a mailbox with no history scanned 100,000 runs belonging to another mailbox, increasing work from 10 to 300,010 VM instructions. LIMIT bounds output, not rows examined.

Add or reuse an index supporting mailbox selection and descending history order. Coordinate with PERF-01 so recovery and history indexes are not duplicated without a demonstrated need. Keep history retention, ordering, authorization and response contracts unchanged.

Acceptance: no-history, fewer-than-30 and more-than-30 histories return the correct newest records; large newer histories for other mailboxes do not cause proportional lookup work. Verify real provider plans, including the empty-history case that cannot stop early after finding 30 matches.

### PERF-05 — Fill demonstrated inventory and domain-reference index gaps

Priority: low. Inspect provider `overview.rs`, `domains.rs` and relevant migrations. SQLite's owner-filtered template inventory query scanned all templates; its domain campaign-count subquery scanned all campaign-domain associations. The diagnostics grew from 19 to 300,019 and from 119 to 400,119 VM instructions respectively. MySQL/PostgreSQL already have owner indexes on templates, pages, groups and SMTP profiles; do not add duplicates. The domain-reference lookup lacks an index beginning with domain_id in the inspected schemas across providers.

Add the missing owner/reference indexes where execution evidence warrants them. Inspect sibling resource inventory queries for the same verified schema gap. Retain necessary counts and supported ownership semantics; do not replace them with maintained counters or approximate values. Existing indexed audience/delivery aggregates are controls, not automatic refactoring targets.

Acceptance: inventory and domain usage counts remain correct for empty and populated owners/domains, and unrelated owners' records do not produce proportional scans. Verify all affected sibling queries and per-provider index definitions rather than assuming one migration covers every provider.

### Phase validation and completion

- Reuse disposable databases, canonical fixtures and existing provider test infrastructure. Keep production/application-lab data untouched. Run one heavy workload at a time under the existing bounded build/test settings.
- Record before/after plans and work at multiple unrelated-data sizes while keeping relevant data constant. Use SQLite VM instructions and suitable server-provider execution evidence such as rows examined, actual loops or buffer work. Avoid brittle wall-clock assertions and exact whole-plan snapshots; test the access/work guarantee together with correct results.
- Validate both fresh initialization and upgrades from supported existing schema versions through the real migration path. Update schema versions, schema validation and migration expectations coherently where required; do not only edit original CREATE statements or hand-install indexes in tests. Check restart and preservation of existing records. Preserve provider-specific locking and supported database-version behavior.
- Run focused provider tests and affected worker/process regressions during each item, then the applicable formatting, Clippy and completion checks after edits settle. Use the documented supported provider matrix; an unavailable provider remains explicitly unverified, not passing. Linux/macOS acceptance remains assigned to CI/nightly, with evidence tied to the tested revision.
- Update `cleanup-ledger.md` per item and provider using the established statuses: verified, already resolved, rejected with evidence or deferred with reason. Explain when a provider was already efficient. Close the phase only after the confirmed defects are fixed and verified or explicitly dispositioned with evidence. Independent review must inspect query plans, regressions, migration behavior and preserved fairness/recovery contracts before accepting the phase.

## 11. Phase 6 — Remaining functional and quality findings

Added on 2026-10-29 after reviewing what the existing uncommitted cleanup missed. These five findings are source-confirmed: four medium priority and one low priority. Behavioral tests and builds were not run during that review because another Cargo compilation was active. Revalidate the current implementation before each item, preserve concurrent work and record any already-resolved finding with evidence. Complete the functional recovery/navigation items first, then the repeated-work and documentation items; update the ledger after each disposition.

### FOLLOW-01 — Recover settings drafts coherently after refresh and revision conflict

Priority: medium. Reference: S05. Owner: [company settings UI](crates/ruphish-ui/src/company_settings.rs), with the revision-conflict contract in the settings adapters and [settings guide](docs/administration/settings.md).

Evidence: the initialization effect copies the saved revision and policy only when `draft` is absent. Refresh updates the query result but retains the old draft and revision; the dirty comparison then compares that old draft against the newly fetched policy. After another administrator saves a change, a previously clean form can appear dirty and its next save still submits the stale revision, producing another conflict. The existing Discard action can reset the draft, but refresh alone does not provide the documented recovery.

Keep an explicit editing baseline. Let a clean form follow refreshed confirmed data; retain dirty input with an explicit reload/discard or reconciliation action when the server revision changes. Update the revision, policy and numeric text fields coherently. Do not silently advance a dirty draft's revision and overwrite another administrator's changes.

Acceptance: exercise two-session changes followed by refresh for both clean and dirty forms. A clean form receives the current policy/revision without becoming falsely dirty; dirty edits remain available and conflict recovery leads to a save using the reviewed revision. Cover failed refresh, explicit discard/reload and stale reads arriving after a confirmed save.

### FOLLOW-02 — Preserve the applied Results selection when returning from recipient analysis

Priority: medium. Reference: C01. Owners: [campaign Results](crates/ruphish-ui/src/campaign_results.rs) and [recipient analysis](crates/ruphish-ui/src/recipient_analysis.rs). Contract: [metrics guide](docs/user-guide/results/metrics.md).

Evidence: the recipient-analysis link carries only campaign and recipient IDs. Recipient analysis removes query parameters and builds its Back link from the bare campaign Results path. The built-in Back action therefore loses the originating filters, time window, sorting and pagination, contrary to the guide's promise that the selected filters remain available on return. Browser history Back is a separate path and does not establish correctness of the built-in action.

Preserve the originating Results URL through the existing navigation mechanism and use it for the return action. Keep recipient analysis as the recipient's full history. Validate any return target as an appropriate in-app destination for the same campaign, and provide a sensible Results fallback for direct entry without an originating selection.

Acceptance: filtered Results → recipient analysis → built-in Back restores the exact applied selection, sort and page. Recipient history remains unfiltered. Cover direct/bookmarked entry, the error-state return path and navigation between campaigns without retaining another campaign's return target.

### FOLLOW-03 — Remove repeated full-response cloning and recipient searches from Results rows

Priority: medium. Reference: C06. Owners: [Results row presentation](crates/ruphish-ui/src/campaign_results.rs) and [query presentation](crates/ruphish-ui/src/query.rs). Reuse the established delivery presentation approach where its responsibility matches.

Evidence: each Results row uses a derived signal that reads presentation data, searches the recipients vector and clones the matching recipient. Multiple cells independently read that signal. The presentation data signal itself clones its retained response on each read, so using `data.with` at the row does not avoid copying the owned Results vectors. Repeated full-response copies and linear recipient searches multiply work across the rendered rows.

Share the immutable response and derive indexed or keyed row access once per response, following the existing delivery solution where appropriate. Preserve one authoritative response owner; do not add a manually synchronized mutable row cache. Inspect other consumers if the shared query presentation contract changes.

Acceptance: refreshed, reordered and removed recipients retain the correct row identity and values. Preserve pending-selection presentation, refresh errors and export state. Demonstrate bounded response-copy and lookup work with meaningful instrumentation or independent scaling evidence rather than fragile timing thresholds. Verify actual WASM consumers and affected query-state regressions.

### FOLLOW-04 — Reuse authoritative page-validation diagnostics during save

Priority: medium. Reference: C06. Owners: [HTTP resource save](crates/ruphish-http/src/resources.rs), [application resource save](crates/ruphish-application/src/resources.rs) and [adapter resource validation](crates/ruphish-adapters/src/resources.rs).

Evidence: HTTP save obtains learning-page preview warnings or landing-page warnings before calling application save. Application save invokes the resource validator, which renders the same submitted page again through `page_preview` or `page_warnings`. The second pass discards the diagnostics already computed for the response. This repeats full content processing for potentially large documents and consumes bounded content-processing capacity twice.

Return diagnostics from the authoritative validation operation and carry them through save to the HTTP response. Keep validation at the appropriate boundary and preserve redirect checks, authorization, revision checks and persistence failures. Do not skip validation, introduce a cross-request cache or replace an error-producing validator with a success-returning no-op.

Acceptance: a successful landing-page or learning-page save performs one content-validation pass and returns the expected warnings. Real valid/invalid fixtures retain their diagnostics and rejection behavior, including redirect validation. Storage and revision failures persist no changes; bounded-work ownership remains correct under cancellation. Instrument the real save path rather than testing only a helper or mocking away validation.

### FOLLOW-05 — Correct the metrics guide's evidence-retention claims

Priority: low. Reference: L05. Owner: [metrics guide](docs/user-guide/results/metrics.md), checked against [event correction guidance](docs/user-guide/results/correcting-events.md) and [event-edit persistence](crates/ruphish-adapters/src/event_edit.rs).

Evidence: the metrics guide says the event table retains excluded observations and original evidence and identifies exclusions. The persistence implementation overwrites edited event values and permanently deletes removed events and related records. The correction guide already states that previous values and deleted events cannot be restored; origin and edited markers do not constitute retained original evidence.

Remove the obsolete retention/exclusion claims and describe the supported origin/edit indicators consistently with the correction guide. Keep the correction guide as the canonical explanation of irreversible edits and deletion. Do not build an audit-history feature to satisfy stale prose.

Acceptance: cross-check both guides against the current event DTOs and persistence behavior, verify local links and terminology, and remove unsupported promises about recoverable original/deleted evidence. This item requires documentation checks, not a product build.

### Phase validation and completion

- Record FOLLOW-01 through FOLLOW-05 separately in the ledger, including current evidence, the focused fix or other disposition, exact executed checks and unverified behavior. Source inspection alone does not establish that recovery, navigation or performance regressions pass.
- Reuse existing fixtures and workflow coverage; keep databases disposable and external recipients/integration endpoints untouched. Run focused regressions and applicable native/WASM/formatting checks under the shared validation gate, with one heavy workload at a time. Manual visual/interaction review remains with the user.
- Close the phase when all five findings are fixed and verified, already resolved with evidence, rejected with evidence or explicitly deferred with a reason. Preserve the earlier phases' verified behavior and inspect affected consumers before accepting shared-state or validation-contract changes.

## 12. Validation and release-readiness gate

Use the command catalog in the repository's validation guidance for the chosen baseline. Start with one build job, one test thread and bounded Tokio/Rayon workers; run one heavy workload at a time. Run focused regressions while implementing and applicable completion checks after edits settle. Diagnose failures before retrying.

Each changed Rust crate and its affected consumers require appropriate formatting, Clippy and tests. UI/browser-shared Rust requires actual WASM-target checking; changed markup requires Leptos formatting. Changed browser code requires relevant JavaScript tests and a development UI build when output/assets change. Use the existing artifact/workflow tests where they exercise the changed behavior. Keep `tests/legacy-test-map.toml` accurate. Prepare fresh dependency/generated inputs through the documented tools; do not hand-edit distribution output.

Before declaring the cleanup ready for independent review, provide the exact commands, results and scope for all affected paths. Before declaring first-release readiness, complete the applicable repository acceptance matrix, including real supported providers, executable persistence/restart/shutdown scenarios, actual development distribution, browser/WASM behavior, dependency/license checks and supported-platform checks. Use existing CI evidence where applicable and identify the exact commit it verifies. Full acceptance is a final planned gate, not a per-item repetition. A missing environment remains an explicit unresolved gap rather than a passing check.

Use disposable fixtures and the canonical acceptance setup. Never contact real recipients or production integrations, mutate application-lab data or overwrite existing fixture installations. Manual visual, keyboard and interaction review remains with the user unless explicitly delegated; do not introduce browser/screenshot automation by default. This cleanup does not authorize publishing or creating a release artifact for distribution.

## 13. Independent review and PR handoff

Use a named implementation branch to make the final changes reviewable against a recorded base. Keep commits coherent if the user authorizes commits. Favor separately reviewable functional, performance and quality batches. Decide whether to publish one PR or split it by phase based on the final diff and dependencies; avoid unrelated changes bundled solely because they were found during cleanup. Do not open a PR against an outdated base or inadvertently include unrelated feature work.

The implementation agent should finish with:

- The exact worktree, branch, implementation base SHA and current HEAD, plus the state of uncommitted changes.
- An updated cleanup ledger covering every original finding and every additional candidate.
- A concise net-change summary organized by owning crate and behavior, including removals and any deliberate contract change.
- Exact verification commands and results, with failures, ignored cases, filtered cases and unexecuted checks separated.
- Remaining risks, scope decisions and release-readiness gaps.
- Confirmation that the protected checkout was not modified by this task.

The original review agent can then independently inspect the final diff and affected consumers against this plan, quality-rules.md and AGENTS.md. It should challenge state ownership, removals, error/recovery paths and test validity, and run focused checks where new evidence warrants them. Do not treat implementation self-review as the independent review. Return review findings to the implementation agent for focused corrections; re-review changed areas before a user-authorized commit/push/PR/merge step.

## 14. Completion checklist

- [ ] All 13 original findings have current evidence and a final disposition.
- [ ] Functional defects are fixed with meaningful regression coverage or disproven with evidence.
- [ ] All baseline crates completed the dedicated quality pass; additional confirmed dead code, redundant state and duplicated responsibilities were removed or explicitly dispositioned.
- [ ] PERF-01 through PERF-05 have separate provider dispositions, physical-work evidence, migration verification where applicable and ledger checkpoints; remaining performance gaps are explicit.
- [ ] FOLLOW-01 through FOLLOW-05 have current evidence and ledger dispositions, with settings recovery, Results return navigation, repeated-work guarantees and documentation accuracy verified or remaining gaps explicitly recorded.
- [ ] Replacements are complete: no superseded path, unnecessary compatibility shim, obsolete dependency or stale comment remains within the changed scope.
- [ ] Relevant native, WASM, JavaScript, provider and process guarantees have actual verification evidence; unexecuted release checks remain explicit.
- [ ] The plan, ledger, baseline and final diff agree; no unrelated work was absorbed.
- [ ] Independent review is completed and its findings are resolved or explicitly accepted by the user.
- [ ] The original checkout and the other agent's work remain untouched by this cleanup.

Implementation can be ready for review before every release-platform environment is available. First-release readiness requires the release gate above; distinguish these two outcomes clearly.
