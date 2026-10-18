# Authentication reference fixtures

Captured from unchanged Gophish commit 95618469799295e2c0fec980805a2dfbb818816b by tools/auth_capture/main.go on 2026-10-18. http.json records 18 ordered account API observations. Only newly generated API keys are normalized; later requests obtain the real key from their own create response. Statuses, error messages, IDs, roles and timestamps remain Go observations.

legacy-accounts.sqlite3 is the pristine database before those requests. Its synthetic admin password is FixtureOnlyPassword42! and its API key is 64 repetitions of 1. Never use these credentials in a real installation. Tests copy the database before mutation and verify the original remains unchanged. The manifest hashes every fixture file; the root evidence verifier checks hashes and SQLite integrity.
