---
title: Starter catalog implementation
description: Content ownership, API boundaries and functional verification for the offline starter catalog.
---

`crates/ruphish-http/starter-templates/` owns the approved HTML, email alternatives, subjects and logo provenance/license notices. `src/starter_templates.rs` in the same crate defines immutable catalog descriptors and embeds these files with `include_str!`. Read-only HTTP handlers expose the browser-safe contracts; there is no storage adapter, database table, resource relationship or runtime file lookup for the catalog.

The implementation embeds content in the executable instead of copying it into the publicly served UI asset directory. This keeps catalog access behind the admin API and avoids two independently versioned copies. The existing hashed UI asset manifest continues to cover the settings UI and generated transformation module. Future binary and Docker builds automatically contain the catalog through compilation; distribution packaging and release verification remain separate work.

The shell-owned query cache owns listing/detail reads. The Leptos page owns temporary selections, per-page options and the shared logo. Browser transformations preserve Go expressions using the existing `TemplateVault`, operate on a copy and produce ordinary HTML. Uploads are decoded, dimension-checked and rasterized to PNG; resource creation still passes through existing server validation. No marker or catalog identifier is persisted. The UI reads existing resource names, chooses a free suffix, and submits separate requests through existing APIs. Name races can fail normally; ambiguous writes are not retried automatically, and resource inventory queries are invalidated after attempts.

Custom-logo capabilities have one catalog owner and are returned with entry metadata. The five enabled contexts are listed in the [user guide](../user-guide/starter-templates.md#company-logo-availability). Adding a future entry requires individual content review, applicable attribution, metadata, and coverage of the actual form and option transformations. Do not derive email/landing pairings or add installation state.

## Functional verification

`tests/starter_templates_process.rs` exercises the real executable, all catalog entries, authentication and authorization, read-only methods, CSRF, existing create/preview flows, repeated copies, edits/deletions, owner isolation, impersonation rejection and restart persistence. Its Node bridge applies the production browser transformation to real API metadata, then saves and renders all 19 prefill variants and five company-logo variants through the ordinary APIs. Create responses must also pass the UI's shared resource-detail contract. `web/tests/starter-templates.test.mjs` checks offline assets, tracker omission, all prefill/logo transformations, changed DOM structures and upload rejection/cleanup. The existing `landing-collector.test.mjs` also submits every shipped landing layout and its options using synthetic values and asserts that only event identity and password presence leave the form. The existing executable public workflow covers server rejection of value-capture bypasses and persistence.

Manual layout, keyboard interaction and email-client rendering remain the user's review. See [validation](testing/validation.md) for the authoritative check catalog.

## Verification on 2026-10-24

Windows, Rust/Cargo 1.99.1, Node 26.8.1, development builds, one build job/test thread and bounded Tokio/Rayon workers. The following commands exited 0:

- `node --test --test-concurrency=1 crates/ruphish-ui/web/tests/starter-templates.test.mjs`: 5 passed.
- `node --test --test-concurrency=1 crates/ruphish-ui/web/tests/landing-collector.test.mjs`: 10 passed, including all shipped login layouts and supported options.
- `cargo test -p ruphish-ui --lib --locked`: 47 passed.
- `cargo test -p ruphish-ui --test browser_requests --target wasm32-unknown-unknown --locked`: 25 passed, including query lifetime, cancelled requests and uncertain writes.
- `cargo test -p ruphish --test startup_process --locked -- --exact starter_templates_process::catalog_is_admin_only_read_only_and_copies_are_normal_independent_resources public_process::verified_domain_to_campaign_to_private_events_survives_restart_and_rejects_bypasses --nocapture --test-threads=1`: 2 passed. The catalog case validates all 39 normal resources and 24 transformed landing variants.
- `cargo test -p ruphish --test startup_process --locked -- --exact actual_http_servers_serve_built_assets_isolate_public_routes_and_shutdown_on_real_ctrl_c --nocapture --test-threads=1`: 1 passed against the final development distribution, including every manifest asset and browser module.
- `cargo clippy -p ruphish-http --lib --locked -- -D warnings`, `cargo clippy -p ruphish --bin ruphish --test startup_process --locked -- -D warnings`, `cargo clippy -p ruphish-ui --lib --locked -- -D warnings`, and `cargo clippy -p ruphish-ui --target wasm32-unknown-unknown --locked -- -D warnings`: passed.
- `trunk build --locked`: passed for the final UI source and assets.
- `cargo fmt --all -- --check`, `leptosfmt --check crates/ruphish-ui/src crates/ruphish-ui/tests`, and `git diff --check`: passed.
- `node --test website/tools/docs.test.mjs`: 3 passed. A focused check using the existing documentation link resolver resolved all 43 links in the changed guides/reference pages.

Process tests used the same process-local Windows console correction recorded in [email tracking verification](email-tracking.md#verification-on-2026-10-24), without changing fixture shutdown behavior. An initial JavaScript assertion expected only `{{.URL}}`; it was corrected to accept the approved templates' escaped `{{.URL | html}}` form and rerun successfully. Initial compiler feedback about contract documentation and comparison traits was resolved before the passing lint/build runs.

Root `py -3 tools/verify_specification.py` could not execute because Python is not installed (exit 1). No manual browser/inbox review, complete website build, provider matrix, Docker packaging or release validation is claimed.
