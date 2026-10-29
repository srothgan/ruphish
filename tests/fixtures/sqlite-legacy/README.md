# Gophish SQLite migration evidence

These unmodified upstream SQL files and `provenance.json` document the source histories supported by direct Gophish conversion. They are test material, not runtime migrations. The populated snapshots under `../compatibility/` and `../gophish-v0.12.1/` provide independent conversion inputs. Original license terms are retained in the repository's third-party notices.

`crates/ruphish-adapters/src/sqlite/legacy-shapes.json` records each source version's table columns (`name`, declared type, nullability, default expression and primary-key position). It was captured from these upstream schemas; validate it against the populated snapshots when changing recognition. Current Ruphish schemas are defined exclusively by the provider release baselines.
