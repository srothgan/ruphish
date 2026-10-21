# Independent Gophish MySQL schema fixtures

The SQL files are byte-preserved upstream migrations from the commit recorded in `provenance.json`; `LICENSE` retains the upstream MIT notice. They are executed only in disposable test databases. Production does not replay them: an early migration destroys the old SMTP table, and a later migration interprets timestamps using the session timezone.

`mysql_preflight` executes each original Up section and records its Goose version before asking the production facade to assess the resulting schema. This independently checks the version/column/type manifest in the private MySQL adapter. Populated migration fidelity, interruption recovery, index/constraint validation and privacy transformation require separate tests when actual upgrade writes are implemented.
