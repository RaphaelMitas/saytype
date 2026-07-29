#!/usr/bin/env bash
# End-to-end test of the built Saytype.app.
#
# Runs the real bundled binary in headless E2E mode (see src-tauri/src/e2e.rs):
# it skips the tray, window and hotkey setup, transcribes a fixture through the
# normal transcriber path, and exits non-zero if the transcript is wrong.
#
# This covers what the sidecar-only tests cannot: sidecar lookup inside the
# .app bundle, process spawn, the ready handshake, and the Rust <-> Python
# round trip through production-built code.
#
# Usage: tests/e2e/app_e2e.sh [path-to-app-binary]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE="${SAYTYPE_E2E_AUDIO:-$REPO_ROOT/tests/fixtures/hello.wav}"
EXPECT="${SAYTYPE_E2E_EXPECT:-The quick brown fox jumps over the lazy dog.}"
DEFAULT_BIN="$REPO_ROOT/src-tauri/target/release/bundle/macos/Saytype.app/Contents/MacOS/Saytype"
APP_BIN="${1:-$DEFAULT_BIN}"

if [ ! -x "$APP_BIN" ]; then
    echo "App binary not found or not executable: $APP_BIN" >&2
    echo "Build it first with: pnpm tauri build" >&2
    exit 1
fi

if [ ! -f "$FIXTURE" ]; then
    echo "Fixture not found: $FIXTURE" >&2
    exit 1
fi

echo "App:     $APP_BIN"
echo "Fixture: $FIXTURE"
echo "Expect:  $EXPECT"

# The app exits itself once the transcript is checked; the watchdog inside
# e2e.rs bounds the run so a wedged sidecar fails instead of hanging CI.
set +e
SAYTYPE_E2E_AUDIO="$FIXTURE" \
SAYTYPE_E2E_EXPECT="$EXPECT" \
SAYTYPE_E2E_MODE="${SAYTYPE_E2E_MODE:-local}" \
SAYTYPE_E2E_TIMEOUT_SECS="${SAYTYPE_E2E_TIMEOUT_SECS:-900}" \
    "$APP_BIN"
STATUS=$?
set -e

# A crashed app can leave the sidecar holding the model in memory.
pkill -f transcribe-server >/dev/null 2>&1 || true

if [ $STATUS -ne 0 ]; then
    echo "App E2E FAILED (exit $STATUS)" >&2
    exit $STATUS
fi

echo "App E2E passed."
