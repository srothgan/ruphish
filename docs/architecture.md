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

The five libraries intentionally contain documented empty boundaries until real behavior and its tests are added. There are no success-returning API stubs, speculative domain types, or unused SQL/SMTP drivers. The executable is a version-printing build entry point. The UI is only a title, not a login screen or completed design. Existing mapped cases remain not_implemented.

