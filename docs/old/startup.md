# Native application startup

The executable runs on Windows and Unix systems. Tokio supplies the shared runtime and network lifecycle; small OS-specific modules handle private-file permissions and termination signals. Windows and Linux execution are verified separately. macOS/BSD execution requires a matching host and is not inferred from Linux Docker results.

## Local use

Build the actual admin distribution before starting an admin listener or running the complete native suite:

```sh
npm ci --prefix crates/ruphish-ui/web
trunk build --locked
cargo build --workspace --exclude ruphish-ui --locked
```

Run from this product directory; startup reads or creates the per-user OS configuration:

```sh
cargo run -p ruphish --locked
```

The example uses local admin HTTPS on 3333 and public HTTP on 8081. The decoder itself has Go's zero defaults, not these sample values. Each selected listener requires an explicitly configured, nonempty `listen_url`; a missing address fails startup. Existing Go JSON fields and GOPHISH_INITIAL_ADMIN_PASSWORD / GOPHISH_INITIAL_ADMIN_API_TOKEN environment names are preserved. Generated defaults use absolute OS data/log paths. Relative paths in an automatically discovered configuration resolve against its directory. Explicit --config requires an existing file and preserves working-directory-relative paths. See [configuration locations](../administration/configuration.md#default-locations-and-explicit-configuration). Unknown fields and invalid known-field types fail, so spelling mistakes cannot silently disable listener protections. test_flag cannot enable test mode. SQLite is the default local configuration; MySQL and PostgreSQL are explicitly selectable through [typed database settings](database.md). Invalid/conflicting provider settings fail before readiness, with no fallback.

--mode admin and --mode phish start only the selected listener. Inactive listeners do not need assets, usable addresses or certificates. --disable-mailer stops campaign queue execution in this process; --no-disable-mailer enables it. Only modes containing the admin listener run the campaign worker. Resource management, campaign delivery, public interactions and reporting mailboxes are implemented.

Readiness is emitted to stderr as RUPHISH_READY followed by a JSON object containing the selected bound addresses, independently of logging level. Port 0 is supported for isolated testing. Readiness follows successful asset validation, both selected socket reservations, certificate preparation, database preparation, atomic bootstrap and listener startup. No fake health/shutdown endpoints are added. A fatal initialization/runtime failure exits with status 1; normal Ctrl+C, Windows Ctrl+Break and Unix SIGTERM shutdown exit successfully.

## Existing databases and recovery

Stop all old application instances before a schema upgrade. For SQLite, the short .ruphish-startup.lock serializes cooperating Rust startup operations; it does not establish that a Go process has stopped. Separate admin/public processes can share a current database after startup preparation.

A recognized Go SQLite database is upgraded at its configured path, including a latest-version Go database that still needs the independently versioned Ruphish authentication tables. Startup reserves the writer transaction, takes a separate SQLite VACUUM INTO snapshot including committed WAL contents, validates the snapshot's integrity and history, syncs it, and publishes a private ruphish-pre-upgrade-*.sqlite3 file before applying pending migrations in the original transaction. Temporary snapshots use a different hidden name and are not advertised as recoverable backups. Failed migrations roll back; backups remain available. Unknown/future/inconsistent schemas and corrupt files fail without being initialized as empty. A current schema is not repeatedly backed up or migrated. Arbitrary SQL supplied through migrations_prefix is refused; an external directory must match the full verified migration set.

To restore a snapshot, stop every application process and restore the verified SQLite snapshot to a separate path, then configure db_path to that path. Do not copy a lone live database file over a running database or splice its WAL files. The snapshot contains pre-upgrade state; later application writes are outside that recovery point. Power-loss durability and hostile concurrent filesystem replacement are not established by process-kill tests.

Bootstrap creates the first admin, admin role link, Argon2id hash, API token and password-change flag in one transaction. A still-temporary default admin password is refreshed on restart as in Go. Established password hashes, tokens, renamed accounts and existing records are retained. Generated temporary passwords are printed to startup stderr, including noninteractive container output, but not the application log file. Supplied passwords and API tokens are never echoed. Set GOPHISH_INITIAL_ADMIN_PASSWORD before startup to supply a temporary password instead. Complete the required password change, then remove the variable from your shell. Initial passwords accept up to 1024 UTF-8 bytes; generated temporary passwords contain 16 random bytes encoded as hex. Existing bcrypt hashes remain usable and unchanged on login; changed/new passwords use Argon2id and validate the entire input. The first login enforces changing a temporary password. See [authentication](authentication.md) for session, password and rollback behavior.

## TLS, assets and shutdown

Supplied EC SEC1 and RSA PKCS#1 PEM pairs are loaded and validated without rewriting their bytes. When both files are absent, startup generates a P-384 certificate/key pair with localhost and configured-host SANs. One missing file or a mismatched pair fails explicitly. Because two files cannot be published atomically, a hard interruption can leave an incomplete pair; repair it explicitly before restart. Generated keys, new databases and recoverable backups use Unix mode 0600 or a protected Windows DACL granting the current identity access. Existing operator-supplied files retain their permissions.

TLS uses Rustls with ring and TLS 1.2/1.3. Obsolete protocol/key-exchange support is not reproduced. Self-signed generated certificates require explicit trust; the synthetic legacy no-SAN compatibility fixture is the sole test that bypasses identity validation.

Trunk.toml explicitly runs web/build.mjs to generate editor modules before compilation, then generates asset-manifest.json over the staged output. The native admin router validates and retains the same asset bytes, serving that snapshot for its lifetime. Replacing a distribution file or introducing a symlink after startup cannot change served content; restart after rebuilding to load new assets. Missing JavaScript/WASM, unknown API routes and traversal requests return 404 rather than a SPA shell. The public listener never exposes admin assets. Distribution symlinks are rejected during startup. The manifest establishes build coherence, not publisher authenticity.

Incomplete HTTP headers and TLS handshakes have ten-second deadlines. Authentication forms and account JSON have 16 KiB body limits and ten-second read deadlines. Administrative requests also have a thirty-second deadline covering body extraction and handler execution, with at most 64 admitted requests per process. Expiration returns 408; admission saturation returns 503 with Retry-After. These deadlines do not recall external effects already performed or bound response-body transmission. HTTP serving tasks are supervised and required-listener failure triggers application shutdown. Termination stops accepting and drains active requests for up to ten seconds, closes the SQLite pool and flushes the owned log worker. The Tokio runtime has a bounded shutdown wait. Access logs contain a standard HTTP method (or OTHER), the router-owned route pattern (or <unmatched>) and the numeric response status. They omit raw paths, queries, headers, bodies and peer diagnostics. Logging appends to an existing file, accepts legacy level names, and represents fatal failures using tracing's ERROR severity; logging text layout is modernized.

Application debug/trace levels never enable dependency logging: dependency targets are disabled even at warning/error levels because their diagnostics may include protocol or SQL input. Application boundaries emit static messages and safe typed fields. Raw fatal errors and CLI argument values are suppressed in console output as well. Temporary snapshot files can remain after a hard process termination; their hidden .ruphish-backup-tmp- names distinguish them from verified published backups.

## Verification

AGENTS.md lists direct checks. tests/startup_cli.rs runs the real executable with isolated working directories, separate stdout/stderr capture and deadlines. tests/startup_process.rs uses native PTYs, real listeners, actual built WASM/JavaScript, synthetic environments, bounded output and observable readiness. It covers all 25 populated legacy schemas, migration rollback and hard termination during a live migration journal, bootstrap, restart, shared modes, preflight failure, HTTP/TLS, traversal, stale assets, logs and native signals. Adapter integration tests additionally check committed WAL data, untouched queue ownership, migration-directory validation and platform file permissions. Config observations come from unchanged Go code. No production failure/termination endpoint or browser automation is used.


## Participant privacy during upgrades

Campaign tracking settings authorize event/presence metadata only, never entered field values. Migration removes opaque legacy event details from the working database, and campaign readers rebuild permitted details from a strict allowlist. Verified pre-upgrade backups deliberately retain the recoverable original and may contain credentials captured by Go. Protect or retire those backups under your retention policy; neither this migration nor new logging safeguards sanitize old backups, old logs, reverse proxies or externally retained copies. Public participant event collection is not implemented in this campaign-delivery package.
