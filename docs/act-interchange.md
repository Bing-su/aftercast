# ACT XML/gzip interchange

Public ACT interchange is separate from raw-log parsing and the precise private
history database. Existing readers and writers remain the implementation; no new
XML dialect or report exporter is introduced.

| Contract        | Behavior                                                                                                                                                                                                                                                        |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Import          | Auto-detect XML versus gzip bytes, initialize parser categories first, end live combat; decode completely before attaching encounters. Malformed input adds no encounters. File and expanded XML remain bounded to 64 MiB. Parser hooks/audio are not replayed. |
| Export          | One session-owned retained encounter, explicit XML or gzip format, a new destination only. Atomic publication refuses an existing file, including a destination created during the write.                                                                       |
| Log policy      | `ActExportOptions.include_logs` controls retained LogLine output; legacy `export_act` and default options include logs. Omitting logs does not strip actions or mutate the original encounter.                                                                  |
| Private history | Always includes its precise log/identity metadata. Public export options do not alter checkpoint behavior.                                                                                                                                                      |
| Accepted work   | Existing Archive Begin/Advance and Host shutdown drain apply. Dropping a response future or changing screen does not undo an accepted import/export. File selection/confirmation may be canceled before admission.                                              |
| Errors          | Missing parent, invalid XML/gzip, unsupported input and destination conflicts are visible errors. Existing destinations remain unchanged; no replace/overwrite permission is inferred.                                                                          |

Bridge export flags use bit 0 for gzip and bit 1 for **omit** logs. Zero/one retain
the prior include-logs behavior. All flags must be zero for non-export commands;
unknown bits are rejected before admission.

Synthetic isolated-worker regressions exercise XML/gzip × included/omitted logs,
source graph preservation, refusal to overwrite and exact destination bytes,
missing parent failure and private-history restart restoration. Existing ACT
archive/original comparisons remain required. Native file dialogs/clipboard and
Windows input/focus are separate execution checks.
