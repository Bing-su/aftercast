# Offline ODBC table preview

The pinned ACT 3.8.5.288 IL (`test:original`, report-export-policy) builds five
ODBC tables: `encounter_table`, `combatant_table`, `damagetype_table`,
`attacktype_table`, and `swing_table`. It intersects the destination schema's
column names with registered `SqlDataName` values using current-culture
case-insensitive comparison, calls `GetSqlData` on the original object, and
formats every value as a quoted SQL literal. `SqlSafe` replaces the current
culture's float NaN and infinity strings with `0`, then doubles apostrophes.
`CreateOdbcTable` lowercases column names with the current culture and uses
registered SQL type strings. Native SQL hacks, database schema discovery and
batch execution are not exercised by this static IL evidence.

`HostEnvironment.PreviewOdbcTable` is an offline managed contract, not an ODBC
driver. The caller supplies synthetic destination column names in destination
order. It captures the existing report selection, validates its revision and
owner thread, and returns a complete single-table plan only after all selected
SQL callbacks succeed. For an encounter row, the caller selects `encounter=true`
with a combatant report capture belonging to that encounter. Each of the other
four tables uses a matching existing report capture. The plan contains a quoted
schema, an INSERT with positional `?` placeholders and independent string
parameters. This parameterized representation intentionally differs from ACT's
literal SQL: apostrophes remain part of the string parameter, never SQL syntax.
No database connection, driver, credential, file or network access exists here.

Identifier quotes are doubled; types are restricted to a small explicit safe
syntax and type allowlist, and missing or ambiguous columns fail instead of
being guessed. This syntax is an offline preview and is not a promise that a
particular ODBC driver supports double-quoted identifiers or every listed type.
Null, invalid UTF-8, NUL and excessive values fail explicitly. ACT's cultural
NaN/infinity replacement is retained on string parameters; date/time strings
and other culture-specific values are not reinterpreted. Guarded callbacks are
bounded by the existing 16,384-row, 256-column, 4 KiB per cell, 8 MiB total and
cooperative two-second limits; a native callback that never returns cannot be
preempted by this budget. No partial plan is returned on a failure. Synthetic
managed tests use a local in-memory sink and synthetic data only.

The managed `ReadOdbcHierarchy` takes an existing all-tables hierarchy capture,
returns all five plans together, and validates that capture and every column and
category registration around callbacks. It includes all category rows, attack
rows only for ACT's last registered outgoing/incoming category keys, and swing
rows only for the last outgoing key's localized All attack. The caller supplies
the All term explicitly; the current built-in English ACT term is `All`.
The existing hierarchy snapshot limits and XML-valid name constraints also apply.
A cooperative four-second whole-plan budget and an 8 MiB aggregate text limit
apply; no partial plan is published if any table fails. Synthetic checks include
registration mutations, stale revisions and failing callbacks.

The managed paths are not yet connected to Bridge/Rust or a GUI;

the offline hierarchy still does not reproduce ACT's original multi-encounter
selection, native batch policy, SQL hacks or driver behavior. Native ACT ODBC execution, external database access and independent
review remain unverified. The managed `current_table` preview returns an inert
reset statement and inserts an empty EncId without invoking its callback, as
verified against original IL. This does not delete any table or data.
