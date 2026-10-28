# Sound and speech preferences

Pinned ACT `FormActMain.TTS` walks enabled `clbSpeechCorrections` entries in
list order, splits each on `▶`, applies .NET `Regex.Replace` with `IgnoreCase`,
then passes the result to the replaceable `PlayTtsMethod` delegate. Exceptions
are logged and suppress that call. `Options_Sound.InitializeComponent` sets both
WAV and TTS volume controls to 100. `test:speech-policy` verifies these facts from
the pinned ACT IL; it does not start a native ACT window or audio device.

Aftercast stores ordered rules in the worker's `SpeechCorrections.json`. The
settings editor loads them explicitly, supports ordering and activation, and saves
only on an explicit action with a session-owned revision. A failed save retains
both the draft and prior runtime rules. A changed external file is rejected before
replacement; a noncooperating external writer can still race the last hash check
and replacement. As in the existing correction editor, this is not a cross-process
transaction or a power-loss durability guarantee.

Enabled rules use .NET case-insensitive regex and replacement syntax with the
calling thread's culture. Both the queued default delegate and plugin-replaced
`PlayTtsMethod` receive the corrected text. The rule set is snapshotted for each
call. To bound work, there are at most 64 rules, 1,024 UTF-16 characters per
pattern/replacement, 100 ms per expression, two seconds cooperatively per call,
and 4,096 UTF-8 bytes of input/output when any rule is enabled. `▶` and NUL are
rejected in rule fields; invalid expressions are rejected when saving. ACT lacks
these limits and ignores unchecked malformed rules, so this is an explicit
compatibility difference. A rule error is logged using the existing TTS boundary
and the delegate is not called. No rules preserves the prior delegate path.

Application preferences persist 0–100 sound and speech volumes and apply them
before plugin initialization on the next Host start. Editing or saving does not
play audio. The desktop child currently installs no built-in audio backend, so
sound, speech and voice selection are unavailable there unless a plugin replaces
the corresponding ACT delegate. Other Rust applications may inject a provider;
voice/rate selection is supported by standalone `aftercast-tts` requests but is
not a desktop setting. Timer cue defaults, a test-play button, ACT's
`PlayMiscSound`, duplicate suppression and native voice/device handling remain
outside this implementation. No live audio or external service was used for these
tests. Native GTK/Windows input, playback and voice behavior remain unverified.
