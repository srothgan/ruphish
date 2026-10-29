# Independent Gophish MySQL schema fixtures

The SQL files are byte-preserved upstream migrations from the commit recorded in `provenance.json`; `LICENSE` retains the upstream MIT notice. They are executed only in disposable test databases. Production does not replay them: an early migration destroys the old SMTP table, and a later migration interprets timestamps using the session timezone.

`mysql_preflight` executes each original Up section and records its Goose version before asking the production facade to assess the resulting schema. This independently checks the version/column/type manifest in the private MySQL adapter. The `database_providers` tests convert populated instances of all 28 histories directly to the release baseline and cover interruption recovery, index/constraint validation and privacy transformations.
