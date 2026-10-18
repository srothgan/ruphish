# Gophish compatibility corpus

This corpus stores synthetic inputs and independently captured reference outputs for regression tests. Reference: Gophish Community Edition 0.12.1, commit `95618469799295e2c0fec980805a2dfbb818816b`. capture.json describes capture boundaries and limitations; manifest.json records the exact captured bytes.

| File | Meaning and comparison policy |
| --- | --- |
| `http.json` | Ordered API-router scenarios; begin with a fresh legacy schema/bootstrap admin and its synthetic API key. Execute in array order: group-create supplies group-read state. Compare status, recorded headers, JSON structure, null/missing fields, arrays and IDs exactly. Replace actual generated timestamp values only at the listed JSON pointers with `<generated-time>`, verifying they are valid timestamps first. Text bodies and redirect Location are exact. |
| `public-http.json` | Public direct-handler cases for robots and invalid/missing rid. These do not prove public router precedence or middleware behavior. |
| `templates.json` | Typed campaign-template context, input template, exact rendered output, and semantic error classification. observed_go_error is diagnostic evidence, not a required Rust engine message. Error cases can have partial output. |
| `template-context.json` | Exact URL/query/tracking/From context construction for synthetic recipients. |
| `pages.json` | Input landing-page HTML/capture flags and expected transformed fields/HTML for all flag combinations. |
| `csv.json` | Uploaded CSV text, multipart filename/field, and normalized recipients. |
| `password-policy.json` | Legacy policy cases, including UTF-8 byte-length semantics and a long password whose hashing limit is a separate concern. |
| `password.json` | Synthetic plaintext and stored legacy bcrypt hash; verify compatibility without regenerating the salt/hash. |
| `webhook.json` | Secret, structured payload, exact compact JSON bytes and HMAC signature captured at a loopback HTTP receiver. Sign bytes exactly; pretty-printed JSON is not equivalent for signing. |
| `sqlite-runtime-schema.json` | Actual schema read from the reference-created database, including Goose version metadata. |
| `legacy-latest.sqlite3` | Latest legacy SQLite schema populated with bootstrap admin, roles/permissions, one group/recipient, synthetic bcrypt/API key, and stable seed timestamps. It is an upgrade input fixture, not the initial seed for replaying the ordered API cases. |

All credentials and addresses are synthetic test values. Never use them in a real installation. Copy the database to a per-test temporary location before any upgrade or mutation. Preserve the original fixture. Intermediate populated database versions, complete campaign lifecycles, and additional protocol cases will extend this initial corpus.

No Rust implementation has been validated by capturing these fixtures. The new product must execute the corresponding functional paths and compare their observable outcomes against these independent expectations.
