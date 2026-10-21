#!/bin/sh
set -eu
"${NODE_BINARY:-node}" --test "$(dirname "$0")/cactbot-audio.test.cjs"
# Isolate browser speech from the user's desktop reader; only this server is cancelled at exit.
audio_state=$(mktemp -d /tmp/aftercast-browser-audio.XXXXXX)
speech-dispatcher -s -S "$audio_state/speechd.sock" -P "$audio_state/speechd.pid" -L "$audio_state" -t 0 -l 3 > "$audio_state/server.log" 2>&1 &
audio_server=$!
trap 'kill "$audio_server" 2>/dev/null || true; wait "$audio_server" 2>/dev/null || true' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
audio_wait=0
while [ ! -S "$audio_state/speechd.sock" ]; do
  audio_wait=$((audio_wait + 1))
  if [ "$audio_wait" -gt 100 ]; then cat "$audio_state/server.log"; exit 1; fi
  sleep 0.1
done
export SPEECHD_ADDRESS="unix_socket:$audio_state/speechd.sock"
"${NODE_BINARY:-node}" "$(dirname "$0")/cactbot-audio.cjs" "$@"
