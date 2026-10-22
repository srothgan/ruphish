# Product architecture

The product consists of seven Cargo packages with private publishing disabled. Workspace tests enforce allowed direct package edges, including dependencies hidden in dev/build/target sections, and require every source/local dependency to remain inside this product directory. This is a build boundary; it is not a claim that empty packages already implement their responsibilities.

| Package | Responsibility | Allowed direct workspace dependencies |
| --- | --- | --- |
| ruphish-domain | Domain types, invariants, validation, permissions, transitions and scheduling rules | None |
| ruphish-contracts | Browser-safe API requests/responses and wire serialization | domain |
| ruphish-application | Use cases, transaction boundaries and storage/integration interfaces | domain |
| ruphish-adapters | Concrete persistence, SMTP/IMAP/webhook/import/rendering/outbound adapters | domain, application |
| ruphish-http | HTTP routes, authentication/session/CSRF middleware and contract mapping | application, contracts |
| ruphish-ui | Leptos client-side admin UI compiled to WASM | contracts, domain |
| ruphish | Native CLI, configuration, dependency wiring, listeners and task lifecycle | Any backend workspace package; never UI |

The executable obtains persistence capabilities from the [database provider factory](database.md) and supplies those application interfaces to services and workers. Concrete database modules and pools remain private to adapters. Domain invariants have one authoritative owner. Wire types are distinct boundary representations, not duplicate domain rule implementations or serialized database records. Integration and campaign-review responses are constructed from shared contracts in HTTP and retained as typed confirmed records in the UI; editable incomplete drafts remain separate. Normal UI requests will use the preserved administrative HTTP API rather than framework-specific server-function routes. Public landing pages remain server-rendered HTML independent of the admin WASM bundle.

The source of truth for permitted package edges is tests/workspace.rs. The manifests supply actual edges. Structural tests compare them and check inherited package/lint settings. They also reject direct HTTP/storage/delivery framework dependencies in domain/contracts/application/UI. This policy does not pretend to inspect third-party implementation internals; browser dependency resolution and actual WASM compilation provide separate evidence, and new third-party dependencies require review.

## Native and browser builds

The virtual workspace defaults to its six native members. ruphish-ui is deliberately excluded, and explicitly compiling it for a native target fails rather than providing a misleading no-op main. Run the separate WASM check/lint and Trunk build. UI-only Leptos dependencies are attached to wasm32-unknown-unknown; the native dependency graph does not pull in the browser framework.

## Shared management UI

`crates/ruphish-ui/src/shared/components/` owns reusable management presentation: `layout.rs` provides page headings, footers and empty surfaces; `lists.rs` provides tables, search/count toolbars, empty-result handling and pagination; `forms.rs` provides fields and fieldsets; `buttons.rs` provides action buttons/links, save and copy controls; `dialogs.rs` and `info.rs` provide confirmations and explanatory popovers. Their shared stylesheet is `crates/ruphish-ui/ruphish.css`; `crates/ruphish-ui/theme.css` owns the palette shared with the isolated visual editor.

Resources, users, domains, webhooks and reporting mailboxes use the shared list toolbar and empty-result components. Campaign and analysis tables use the shared table and action components while retaining their own server pagination and filters. Feature modules supply their data, permission checks, draft handling and API operations. Specialized editor tabs and disclosure controls keep their own accessibility semantics. Sharing the presentation does not imply that every feature uses the same draft state model.

The adapters implement SQLite upgrades/bootstrap and persistence for authentication, resources, campaigns, domain verification, interaction observations, reporting and integrations. Domain/application own policies, transitions and use cases. One shared tower-sessions bridge delegates persistence through an application-owned contract. HTTP owns routes, authorization boundaries, session/CSRF integration and transport mapping. The executable owns CLI/configuration, provider selection, listeners and supervised task lifecycle. Leptos consumes the administrative HTTP API. SQLite is the default provider; MySQL and PostgreSQL are selected explicitly at startup. See the [database boundary](database.md) for provider and transaction responsibilities.

Shared database workflows use application-owned snapshots and claim/checkpoint contracts. Rendering, export generation and integration transport orchestration live outside concrete provider modules; providers retain authorization and atomic persistence. Campaign preparation rules live in the application layer. Domain owns manual delivery reconciliation, account-edit privilege rules, integration configuration and retry policy, and mailbox checkpoint-reset decisions. The browser-safe `domain::template::Field` vocabulary drives editor suggestions and the renderer's exhaustive value mapping. The [database boundary](database.md) defines transaction ownership, cancellation and failure contracts for these workflows.

## Date and time

`ruphish-domain::datetime` owns Jiff-based instant parsing, canonical UTC serialization, IANA-zone resolution, local schedule conversion and monotonic modification timestamps. Contracts and UI depend on the domain crate for these browser-safe rules; application consumers use its `datetime` re-export. Do not add feature-specific timestamp parsers. Existing provider SQL expressions still perform database-side filtering and aggregation; session and certificate adapters retain the external libraries' required `time` types.

Admin display converts instants with Jiff and passes the resulting civil fields to one browser locale formatter. Browser locale controls date order, translated month names and hour cycle; the browser's IANA zone controls conversion and DST. No geolocation is used. My account provides an optional IANA-zone override, persisted with the account and applied after a confirmed server read or save; clearing it restores automatic detection. Missing/invalid zones fall back to detection and then UTC. Missing dates display as unset, invalid dates as unrecorded. Labels omit seconds without rounding and include UTC offsets to distinguish repeated local times. Storage/API timestamps retain fractional precision and UTC, and exports retain their existing machine-readable timestamps.

The WASM binary enables Jiff's `js` feature. The shared dependency retains bundled IANA data for Windows, WASM and minimal server installations. Display preferences never alter existing campaign schedules. Analysis controls use the display zone and preserve an unchanged filter's exact UTC instant; newly entered ambiguous times use the same first-occurrence/forward-gap rule as campaign scheduling.
