# Startup fixture ETXTBSY observation

During the first post-`ef76527` whole check, the existing Unix
`startup_deadline_covers_a_worker_that_never_authenticates` failed to spawn
`/tmp/.tmpREnQly/stalled-worker` with `Text file busy (os error 26)` instead of
reaching its authentication deadline. The isolated rerun passed in 0.16 seconds;
the next whole check passed. The same synthetic test failed again during the post-`12a8a8f` whole check with `ETXTBSY`; its focused rerun and the next whole check passed. This is an unresolved observation, not a fixed race.

| Evidence                                                                                          | Conclusion                                                                                                                               |
| ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `deployment()` creates and owns a unique `TempDir` per test                                       | Tests do not intentionally share this generated executable path.                                                                         |
| Only this test writes `stalled-worker`; `fs::write` returns before chmod/spawn                    | No explicit writable handle is retained by the test's own execution thread.                                                              |
| The agent ran one check command at a time; no separate worker-fixture build overlapped that check | There is no evidence of overlapping agent commands writing this path.                                                                    |
| `test:plugins` runs multiple process-isolation tests concurrently within its Rust test binary     | Other process spawns can overlap; this does not prove which process held a writer.                                                       |
| ETXTBSY occurs during exec                                                                        | A writable open existed at that instant; its owner was not captured. Temporary FD inheritance is a hypothesis, not an established cause. |
| The fixture and process supervisor were unchanged by preset/report work                           | No causal connection to the report changes has been found.                                                                               |

No startup retry, relaxed assertion or unrelated process change was added merely
to obtain a passing run. If it recurs, capture the writable descriptor owner during
the failing spawn before choosing a limited fix. Tests use only generated synthetic
executables and temporary deployment data; no user executable or system settings
were modified.

## Bounded follow-up — 2026-10-30

A controlled Linux-only synthetic experiment opened and flushed a temporary
shell executable with `FD_CLOEXEC` set, forked a child while the writer was open,
and held that child at a pipe barrier. After the parent closed its writer, an
attempted exec failed with `ETXTBSY`; after the child was released and reaped,
the same exec succeeded. This demonstrates that close-on-exec does not close an
inherited writer until the child actually executes. It does **not** establish
that either historical failure had this cause: the experiment injected a holder
and did not run the original test's naturally occurring failure path.

One isolated run of `startup_deadline_covers_a_worker_that_never_authenticates`
and one eight-thread run of the existing ignored `process_isolation` test binary
(14 tests) were traced only through their own spawned descendants. Both passed;
no `ETXTBSY` occurred in either trace. In the observed runs, `stalled-worker`
was opened with `O_CLOEXEC`, the writing thread's descriptor closed, and the
script subsequently executed. Other processes did spawn during the parallel run,
but the trace did not capture a process inheriting that particular open writer.
One successful run cannot exclude a rare interleaving or identify the historical
writer. No user data, external process attachment, environment setting changes,
retries, added sleeps or weakened deadline assertions were used.

**Status:** no root cause is established and no code fix or deterministic
regression for the actual intermittent path is claimed. On a future occurrence,
capture the open writer/descriptor owner for that synthetic inode during the
failed exec. Do not attribute the original failures to inherited descriptors
without such evidence.
