# Product architecture

The product consists of seven Cargo packages with private publishing disabled. Workspace tests enforce allowed direct package edges, including dependencies hidden in dev/build/target sections, and require every source/local dependency to remain inside this product directory. This is a build boundary; it is not a claim that empty packages already implement their responsibilities.

| Package | Responsibility | Allowed direct workspace dependencies |
| --- | --- | --- |
| ruphish-domain | Domain types, invariants, validation, permissions, transitions and scheduling rules | None |
| ruphish-contracts | Browser-safe API requests/responses and wire serialization | None |
| ruphish-application | Use cases, transaction boundaries and storage/integration interfaces | domain |
| ruphish-adapters | Concrete persistence, SMTP/IMAP/webhook/import/rendering/outbound adapters | domain, application |
| ruphish-http | HTTP routes, authentication/session/CSRF middleware and contract mapping | application, contracts |
| ruphish-ui | Leptos client-side admin UI compiled to WASM | contracts |
| ruphish | Native CLI, configuration, dependency wiring, listeners and task lifecycle | Any backend workspace package; never UI |

The executable supplies concrete adapters to application operations. Domain invariants have one authoritative owner. Wire types are distinct boundary representations, not duplicate domain rule implementations or serialized database records. Normal UI requests will use the preserved administrative HTTP API rather than framework-specific server-function routes. Public landing pages remain server-rendered HTML independent of the admin WASM bundle.

The source of truth for permitted package edges is tests/workspace.rs. The manifests supply actual edges. Structural tests compare them and check inherited package/lint settings. They also reject direct HTTP/storage/delivery framework dependencies in domain/contracts/application/UI. This policy does not pretend to inspect third-party implementation internals; browser dependency resolution and actual WASM compilation provide separate evidence, and new third-party dependencies require review.

## Native and browser builds

The virtual workspace defaults to its six native members. ruphish-ui is deliberately excluded, and explicitly compiling it for a native target fails rather than providing a misleading no-op main. Run the separate WASM check/lint and Trunk build. UI-only Leptos dependencies are attached to wasm32-unknown-unknown; the native dependency graph does not pull in the browser framework.

## Shared management UI

`crates/ruphish-ui/src/shared/components/` owns reusable management presentation: `layout.rs` provides page headings, footers and empty surfaces; `lists.rs` provides tables, search/count toolbars, empty-result handling and pagination; `forms.rs` provides fields and fieldsets; `buttons.rs` provides action buttons/links, save and copy controls; `dialogs.rs` and `info.rs` provide confirmations and explanatory popovers. Their shared stylesheet is `styles.css` in the same directory.

Resources, users, domains, webhooks and reporting mailboxes use the shared list toolbar and empty-result components. Campaign and analysis tables use the shared table and action components while retaining their own server pagination and filters. Feature modules supply their data, permission checks, draft handling and API operations. Specialized editor tabs and disclosure controls keep their own accessibility semantics. Sharing the presentation does not imply that every feature uses the same draft state model.

The adapters implement SQLite upgrades/bootstrap, transactional account persistence, session storage, bounded bcrypt/Argon2 password work, Go-template compatibility and TLS material handling. Domain/application own account policy, authorization and use cases. HTTP integrates tower-sessions and axum_csrf with account routes, asset routing and transport helpers. The executable owns CLI/configuration, wiring, listeners and lifecycle. Leptos provides account administration and a functional template workbench. Resource APIs and campaign execution remain later work; no success-returning placeholders stand in for them.
