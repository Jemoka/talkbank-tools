# talkbank-cache

SQLite-backed validation / roundtrip cache used by chatter. Lives in its
own crate so that products that don't need it (e.g. batchalign, which
uses an embedded `redb` cache) don't transitively pull `sqlx` and
`libsqlite3-sys` into their wheel build.

Implements the `talkbank_transform::ValidationCache` trait. Schema lives
under `migrations/`; sqlx `migrate!` consumes it at runtime.
