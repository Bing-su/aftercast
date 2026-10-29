# Local mini parse preview

The pinned ACT 3.8.5.288 IL (`test:original`, report-export-policy) has a
one-second `tmrTick` and refreshes the visible mini parse at a configured
1–60 tick interval, default 5. `UpdateMiniEnc` reads the active encounter and
uses either the registered text format or ACT's default; the optional aligned
font overload and native graph path are distinct. The IL probe is static: it
does not run a native ACT window.

The analysis screen offers an explicitly started local preview of the active
live encounter. It uses the same registered formatter/sort callbacks and
revision-checked text report as ordinary reports, choosing ACT's default or an
explicitly saved local text profile. The 1–60 second interval and profile name
persist in app settings; starting or stopping is not persisted. Missing profiles
or their missing sort registration are shown as errors. Unregistered formatter
variables keep the existing ACT-compatible token behavior. Plugin-registered
presets can be copied into a local profile in the text-report editor.

Only one request is admitted at a time in a session, including after Stop or
collapse until its accepted request finishes. Responses from older sessions,
settings, hidden screens or encounters are discarded. Native plugin callbacks
remain subject to the existing worker isolation limitations. The display is
escaped text with an explicit 64 KiB preview limit; empty text and no active
live encounter are distinct. No clipboard, network, audio or game input is used
by this preview. It stops on collapse, navigation or host shutdown.

ACT uses a separate native mini window, reads the active zone’s encounter, and
supports column-aligned native font output and native graph. This bounded first slice uses
an embedded panel and returns no encounter outside live combat; it does not claim
native visuals, click-through, opacity, graph rendering or Windows execution.
