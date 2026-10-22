# Database provider boundary

Use `ruphish_adapters::database::open(Config, Policy)` once during startup. It recognizes and migrates the configured database before returning an owned `Database`. SQLite (`db_name=sqlite3`), MySQL (`db_name=mysql`) and PostgreSQL (`db_name=postgres`) supply the complete capability bundle. SQLite remains the default local example. Unsupported engines are rejected without fallback.

The handle provides feature-specific application interfaces: accounts, resources, campaigns, domains, interactions, previews, reporting, overview and integrations. Obtain only the capabilities a service or worker needs and inject them. Feature operations retain their typed inputs, results and errors. Atomic workflows belong to the repository operation; callers must not reconstruct them from separately committed writes. The facade is not a generic SQL executor.

Concrete backend modules, constructors and pools are private. Application, HTTP and executable production code must not depend directly on database drivers. Compile-fail documentation examples enforce inaccessible provider internals and the absence of a public pool accessor; workspace dependency tests enforce the crate boundary.

`Database::bootstrap` and `Database::close` own initialization and shutdown. `backup_path` identifies a recoverable SQLite file copy; `has_recovery_backup` also includes retained MySQL legacy tables. Closing a provider drains its connections and closes its rendering admission limit; previously obtained capabilities cannot reopen it. Startup storage errors expose safe categories rather than SQL, bound values or connection details. Feature errors retain their existing external semantics. Storage errors distinguish unavailable, busy, transaction conflict, integrity, unsupported schema and uncertain commit. Failed transactions roll back; external delivery is never automatically replayed after an uncertain commit.

Sessions use the application-owned `storage::Sessions` contract with opaque serialized data and Unix-second expiry. One shared tower-sessions bridge owns serialization and identifier collision retries. Providers implement insert-if-absent, live-only update, expiry-filtered load, idempotent delete and atomic cookie-key creation. A late save after logout or expiry must fail, never recreate a session.

`database::migrations` exposes explicit legacy file-baseline and snapshot-upgrade operations for compatibility tooling. Ordinary installations use `database::open`, which also applies the Ruphish schema. Historical migration SQL and independent fixtures remain unchanged. No cross-engine transfer or live database switching is provided.

MySQL 9.7 LTS is the primary server line and 8.4 LTS is the compatibility floor. Other versions, including MariaDB and unverified Innovation releases, are rejected. `database::migrations::inspect_mysql(&MySqlConfig)` remains a read-only tool for empty or legacy Gophish databases; normal startup uses `database::open` to initialize or upgrade and then create the runtime pool. See [MySQL verification](mysql-verification.md) for the real-server commands.

## MySQL configuration

Operators create the database and service account. Grant database-scoped `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`, `DROP` and `INDEX` permissions for automatic migrations. The application does not require server-wide privileges, triggers or permission to create databases or accounts. Use InnoDB and `utf8mb4`; the application baseline uses exact binary comparison for keys and identities.

Replace the SQLite database fields in your configuration with:

```json
{
  "db_name": "mysql",
  "db_path": "",
  "mysql": {
    "host": "mysql.example.internal",
    "port": 3306,
    "database": "ruphish",
    "username": "ruphish",
    "password_env": "RUPHISH_MYSQL_PASSWORD",
    "tls": "verify_identity",
    "ca_path": "mysql-ca.pem",
    "max_connections": 10,
    "connect_timeout_seconds": 10,
    "acquire_timeout_seconds": 10,
    "lock_timeout_seconds": 10
  }
}
```

Keep the listener and other application settings alongside these fields. Omit `ca_path` for a publicly trusted server certificate. Supply exactly one of `password` and `password_env`; credentials cannot be debug-printed or serialized by the typed configuration. Verified TLS is the default. `tls: "disabled"` explicitly selects plaintext for a trusted local test connection; there is no opportunistic fallback. Legacy Go TCP DSNs in `db_path` are accepted for the documented safe subset; typed `mysql` settings and legacy DSN/CA settings cannot be combined. Custom SQL migration directories are unsupported for MySQL.

## PostgreSQL configuration

PostgreSQL 18 is the primary line and 17 is the compatibility floor. Other major versions are rejected until verified. Use a writable UTF8 database with SCRAM authentication and verified TLS. Operators create the database and a dedicated login role; the role needs `CONNECT` and `CREATE` on that database to create and own the `ruphish` schema. It owns application tables for automatic migrations. It does not need superuser, role creation or database creation privileges. Keep other applications out of this database and do not grant unrelated roles access to the application schema.

Replace the SQLite database fields with:

```json
{
  "db_name": "postgres",
  "db_path": "",
  "postgres": {
    "host": "postgres.example.internal",
    "port": 5432,
    "database": "ruphish",
    "username": "ruphish",
    "password_env": "RUPHISH_POSTGRES_PASSWORD",
    "tls": "verify_identity",
    "ca_path": "postgres-ca.pem",
    "max_connections": 10,
    "connect_timeout_seconds": 10,
    "acquire_timeout_seconds": 10,
    "lock_timeout_seconds": 10,
    "statement_timeout_seconds": 30
  }
}
```

Keep the listener and other application settings. Omit `ca_path` for a publicly trusted certificate. Exactly one password source is required. Pool size is 1-128; deadlines are 1-300 seconds. Verified TLS is the default; plaintext requires explicit `tls: "disabled"` and no CA. PostgreSQL uses typed configuration, not a DSN. Remove legacy `db_path`, `db_sslca_path`, custom migration paths and MySQL settings. Ruphish controls the search path, UTC session timezone, deadlines and connection policy; unset `PGOPTIONS`, `PGSSLCERT`, `PGSSLKEY` and `PGSSLROOTCERT` rather than relying on ambient session/TLS overrides. Password files are not read.

PostgreSQL begins with one fresh application baseline. It has no historical Gophish PostgreSQL migration or cross-engine importer. A dedicated connection obtains bounded database-scoped migration ownership; SQLx applies transactional DDL and version/checksum history before the runtime pool opens. Unknown populated databases, schema drift and dirty/modified/unknown history are rejected. Failed baseline DDL rolls back; cancellation and failures release ownership when the dedicated connection closes. Application schema changes require an operator backup; transactional DDL does not replace backups. PostgreSQL returns no generated recovery-copy path.

Native booleans, identity columns with `RETURNING`, typed recipient-ID arrays, exact `C` collations and a partial unique active-verification index implement the existing contracts. Revision timestamps retain RFC3339 nanoseconds, including year-one compatibility sentinels; queue deadlines use epoch seconds. Multi-query reads use read-only repeatable snapshots, while writes lock ownership and parent rows before modifying children. PostgreSQL 18's extra catalog representation of NOT NULL constraints is normalized; nullability is checked on both supported versions. See [PostgreSQL verification](postgresql-verification.md) for actual server and process checks.

## MySQL schema upgrades and recovery

Stop old application writers and arrange an operator-managed backup before upgrading. Startup acquires bounded migration ownership on a dedicated connection, recognizes the complete legacy version history and schema, and transforms supported Gophish tables into staging tables. It verifies ownership and copied row counts, then publishes all tables with one atomic rename. Original tables remain as `rp_original_*`; `_ruphish_upgrade` records the source version, transformation checksum and stage. A failed copy rolls back its data transaction; a failed publication can resume without recopying committed staging data. Original tables may contain legacy captured values and location data, so protect them under your retention policy. The active tables remove those values. Do not remove or rename recovery tables independently of their journal.

Schemas predating Gophish's UTC conversion require `mysql.legacy_timezone` with the original fixed UTC offset, such as `"+02:00"`; startup never guesses the source timezone. Already-UTC schemas are not converted again. This offset is appropriate only when it accurately represents the legacy stored dates; installations with mixed historical offsets need operator assessment before migration. Missing dates use the compatibility year-one sentinel. UTC timestamps are stored as bounded RFC3339 text to retain nanosecond revisions and year-one values; numeric queue deadlines use epoch seconds.

Ordinary application migrations use SQLx version/checksum history. Dirty, unknown or modified history and schema drift fail closed before listeners start. MySQL DDL is not generally transactional: an interrupted ordinary DDL migration requires recovery rather than blindly replaying statements. Restore the operator backup into a separate database when a rollback is needed; no cross-engine transfer or runtime switching is provided.

## Transactions and concurrency

Reads spanning several queries use repeatable snapshots. Server-provider writes use read committed transactions with engine row locks at the relevant ownership boundary; SQLite retains its transactional write ownership. Account changes lock affected users in ID order; owned resource writes lock their owner before replacing children. Workers lock parents before jobs/results and recheck revisions and lease generations. Three small invariant rows coordinate administrator membership, cookie-key creation and the global delivery capacity check. They are transaction-scoped database locks, not process mutexes or a global lock on every write. SMTP, IMAP, HTTP delivery, template rendering and export generation run outside write transactions.

Server providers insert event provenance and webhook jobs in the same transaction as the event. Deletion removes the associated outbox and history atomically. There are no server-provider triggers or binary-log administrative prerequisites. A stale lease cannot overwrite a newer generation; expired preparation can be reclaimed, while expired submission becomes uncertain and requires reconciliation. A confirmed acceptance from the same unresolved attempt can settle uncertainty; an outcome from a superseded generation cannot.

Functional tests obtain operations through the same factory as production. Independent test connections are permitted only for fixture creation, fault injection and persisted-state inspection. They are never passed to an operation under test. The rendering isolation test additionally observes private factory-owned capacity and the owned pool to prove that rendering releases the actual connection; it still calls the public interaction capability. There is no public test-only pool accessor.

Adding another provider requires implementing the existing feature contracts, private connection/migration lifecycle and provider selection, then passing the common functional and executable workflows against the real engine. SQL dialects and locking strategies may differ; ownership, revision checks, rollback, durable claims, session revocation and externally visible outcomes must not. Do not copy transport, rendering or export rules into each database implementation.

## Shared workflows and provider responsibilities

The factory composes provider persistence with shared adapters. `application::campaign::preparation` owns audience identity/conflicts, required selections, scheduling validation and tracking-default resolution. Shared adapter modules own URL template validation, participant-metadata allowlisting, public/preview rendering, integration transport orchestration and report-file generation. These modules do not import a database driver. A new provider implements the persistence contracts rather than copying those workflows.

| Operation | Provider transaction | Work outside the transaction |
| --- | --- | --- |
| Public interaction | Load an owned `interactions::Snapshot`; later recheck campaign/domain eligibility and atomically record event, submission deduplication and derived state | Shared template context, rendering, final HTML restrictions and redirect validation; cancellation retains rendering capacity until the blocking job ends |
| Preview | Authorize/store preview identity or load an owned `delivery::PreviewSnapshot`; finish the read before returning | Preview identity generation and blocking template/HTML rendering; previews never create campaign events |
| Analysis/export | Authorize and collect a consistent `reporting::Analysis` snapshot through `reporting::Snapshots` | Format/scope validation, bounded blocking CSV/XLSX generation and spreadsheet formula neutralization |
| Webhook | Atomically claim a durable job and history row; finish only the current attempt and retain configuration cancellation | Signed HTTP request and application of the domain retry decision through `integrations::work::WebhookQueue` |
| Reporting mailbox | Claim lease/history; initialize UID checkpoint; commit correlation, report event and UID progress; persist acknowledgement; finish only the current revision/lease | IMAP connection/validation, bounded message identification, external acknowledgement after commit and application of domain polling backoff through `integrations::work::MailboxQueue` |
| Campaign preparation | Authorize and load selected resources, check current domain eligibility and atomically persist campaign/audience/jobs | Shared preparation rules are reusable functions; resource selection and launch consistency remain inside the provider operation |

Public and preview rendering share an eight-job admission limit. Blocking tasks own their permits, so cancelling a request cannot admit replacement CPU work while its renderer is still executing. Export generation has a separate two-job limit. Snapshots contain owned values, never SQL rows, connections, transactions or lazy database iterators. Secret-bearing integration records deliberately do not derive `Debug` or browser serialization.

## Failure and commit contracts

Existing feature error types remain authoritative: permission and missing-object outcomes, stale revision conflicts, invalid input and safe internal failures retain their existing HTTP mappings. Storage startup/session failures use `storage::Error`; integration workers retain closed diagnostic codes rather than raw protocol or driver errors. A late network result cannot overwrite cancelled work. Mailbox acknowledgement follows the committed report/checkpoint; an acknowledgement failure leaves durable work for a later attempt. Webhooks remain at-least-once deliveries with stable event identity; a timeout does not prove the receiver did nothing.

No generic database retry loop is introduced. Server-provider commits distinguish explicit rollback/constraint rejection from acknowledgement loss. A lost or unrecognized commit acknowledgement returns `UncertainCommit`; callers reconcile persisted state rather than blindly replaying writes or external delivery. PostgreSQL deadlocks/serialization conflicts map to `TransactionConflict`, lock/statement deadlines to `Busy`, constraint failures to `Integrity`, and transport/authentication failures to `Unavailable`. Auth/resource errors retain this category internally while HTTP keeps safe existing failure responses. Invalid PostgreSQL NUL/oversized resource text produces a safe input error and rolls back the edit; text is never silently stripped or truncated.

Campaign and domain workers remain alive when storage is unavailable. Their retry delay grows from one second to a maximum of 30 seconds and resets after successful storage work; waits are cancellable. Campaign outcome persistence retries the durable write, never SMTP submission. Exhausted outcome writes retain the durable lease so recovery can distinguish abandoned preparation from uncertain submission. Explicit operator confirmation records an acceptance event and a reconciliation audit event atomically, keeping delivery state, analysis and dashboard denominators consistent without sending another message.

## Preservation and concurrency guarantees

MySQL configures strict SQL mode on both migration and pooled runtime connections, including replacement connections. MySQL and PostgreSQL resource writes additionally check bounded text by Unicode character count before assignment; overflow, including trailing spaces, is rejected without changing the resource, its children or its revision. SQLite retains its existing unbounded text behavior. Campaign names and preview recipient fields use the same server text-bound checks.

Account deletion and domain workers acquire the owner before the domain. A claim skips a deleting owner; verification completion waits and then rechecks existence, revision and lease. A late check cannot recreate claims or history for a deleted account. SQL statements retain provider-neutral storage categories across account, resource, campaign, domain, preview, reporting and integration capabilities; safe HTTP messages remain separate from those typed errors.

Time-zone resolution and interaction capacity belong to application rules; delivery outcome transitions and retry decisions belong to domain rules. Providers load and fence current state, call those shared rules, and atomically persist the resulting changes. Domain-edit validation and account-to-resource error conversion also have a single shared owner.

Campaign creation authorizes once within its transaction and retains the domain identity resolved during preparation. Standalone review still authorizes independently; launch acquires the provider-specific domain lock before using the resolved identity. Integration authorization checks the complete permission set against one loaded account. Account listings batch account, role and permission reads within one snapshot, without a persistent role cache.

Integration deletion does not load resource content, credentials or history. Expected revisions guard deletion atomically; legacy requests without a revision resolve only the current revision. Provider-specific locking reads remain in place, and a failed conditional delete uses an existence lookup to distinguish a missing resource from a changed revision. Associated jobs, attempts and checkpoints are removed in the same transaction.
