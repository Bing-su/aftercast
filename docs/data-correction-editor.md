# Data correction rules

ACT applies name substitutions before ability redirection and invokes
`BeforeCombatAction` after both. Both lists use sorted ACT display keys, exact
case-sensitive matching and a single forward pass. Chains and cycles are finite;
their result depends on key order. Redirect types 1, 2 and 3 match registered
damage, healing and other swing types; other integer values match all types.
The pinned `DataCorrectionProbe` executes these policies on original objects.

The managed editor replaces both lists as one revisioned operation. It validates
names and duplicate display keys, waits for admitted action and log processing,
writes a flushed temporary XML file and atomically replaces the settings before
publishing rules to the worker. A mismatched revision, write failure or detected
external file edit leaves the previous in-memory rules intact. Existing XML with
unsupported nodes, duplicate keys or attributes remains readable but is marked
read-only to avoid silently discarding data. An external noncooperating writer can
still race between the final hash check and replacement; the hash is not a lock.

Rules apply to newly submitted actions only; already aggregated encounters and
private-history restores retain their recorded actions. Reparse an available
original log to apply changed rules to past data. Do not confuse a fresh reparse
with restoring the existing checkpoint. Native ACT UI behavior and actual Windows
plugin interaction remain unverified. The session-owned Bridge and Rust API and Rust/UI editor are connected. The editor loads explicitly in a running session, preserves drafts on failure, confirms discarding edits, and disables editing for unsupported XML. Changes affect future parsed actions; previously captured encounters require reparsing. Native interaction remains unverified.
