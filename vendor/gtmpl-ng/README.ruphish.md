# Template escape-decoder patch

This directory contains the published `gtmpl-ng` 1.0.0 crate from upstream revision `5c40ce473366434e21203cb8115fda75873c6842`, subdirectory `gtmpl-ng`, with its original MIT license and notices. `Cargo.toml.orig` and `.cargo_vcs_info.json` preserve provenance. The registry lockfile and Cargo cache marker are omitted. The normalized published manifest is unchanged.

The local change in `src/utils.rs` makes hexadecimal and Unicode escape decoding use checked string ranges. Malformed escapes containing multibyte characters return a parse error instead of panicking on a UTF-8 boundary. Repeated escape decoding checks the prefix at the current cursor, and a `\U` escape consumes only its own ten bytes, allowing ordinary text and other escape kinds afterward.

The scheduled [fuzz run 45431742279](https://github.com/srothgan/ruphish/actions/runs/45431742279) captured the original crash. Its exact bytes are retained in `fuzz/seeds/template_render/utf8-escape-crash.txt`. The application regression `malformed_unicode_escapes_return_parse_errors_and_leave_rendering_usable` in `crates/ruphish-adapters/tests/legacy_templates.rs` checks the crash, malformed and truncated escapes, valid trailing text, mixed escapes, and successful rendering after rejection. Fuzz harness seed replay also exercises the captured input.

This is a temporary local patch, not an upstream release. When an upstream release includes the fix, update through Cargo, remove both workspace path patches and the root workspace exclusion, and remove this directory. Rerun the template regression target and fuzz harness tests with both lockfiles synchronized. Retain the captured seed and application regression.
