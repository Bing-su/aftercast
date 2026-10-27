# Startup fixture ETXTBSY observation

During the first post-`ef76527` whole check, the existing Unix
`startup_deadline_covers_a_worker_that_never_authenticates` failed to spawn
`/tmp/.tmpREnQly/stalled-worker` with `Text file busy (os error 26)` instead of
reaching its authentication deadline. The isolated rerun passed in 0.16 seconds;
the next whole check passed. This is an unresolved observation, not a fixed race.

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
