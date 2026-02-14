# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Saytype is a macOS menu bar app for push-to-talk speech-to-text. Hold the Right Command key to record, release to transcribe and insert text at the cursor position. Runs entirely offline using Apple Silicon ML.

## Development Commands

```bash
# Development (hot reload for frontend, requires restart for Rust changes)
pnpm tauri dev

# Production build (creates Saytype.app)
pnpm tauri build

# Build Python sidecar (required before first run)
cd sidecar && ./build.sh
```

Prerequisites: `brew install sox` for audio recording.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Recording Flow                            │
├─────────────────────────────────────────────────────────────────┤
│ Hotkey (Right Cmd)  →  sox recording  →  Sidecar (Parakeet)     │
│      hotkey.rs           audio.rs          sidecar.rs           │
│                                                 ↓                │
│ Frontend event  ←  Tauri emit  ←  Clipboard paste               │
│   useRecording.ts     lib.rs      text_insertion.rs             │
└─────────────────────────────────────────────────────────────────┘
```

### Three-Process Architecture

1. **Tauri/Rust** (`src-tauri/src/`) - System integration layer
   - Hotkey capture via macOS CGEventTap (runs on dedicated CFRunLoop thread)
   - Audio recording via `sox` shell command (16kHz WAV output)
   - Text insertion via clipboard + AppleScript keystroke simulation
   - Sidecar lifecycle management via stdin/stdout JSON IPC

2. **Python Sidecar** (`sidecar/`) - ML inference
   - Parakeet MLX model for offline transcription
   - Keeps model loaded in memory between requests
   - Protocol: `{"command": "transcribe", "audio_path": "..."}` → `{"success": true, "text": "..."}`

3. **React Frontend** (`src/`) - Settings UI only
   - Event-driven state via Tauri emit/listen (`recording-started`, `transcription-complete`)
   - Onboarding flow for permission setup
   - No audio/transcription logic—purely display

### Key Rust Modules

| Module | Responsibility |
|--------|---------------|
| `lib.rs` | AppState, Tauri commands, app setup/builder |
| `hotkey.rs` | Right Cmd key detection (keycode 54), AtomicBool state |
| `audio.rs` | sox process management, sample rate conversion |
| `sidecar.rs` | Sidecar spawn, JSON IPC, cross-platform binary lookup |
| `text_insertion.rs` | Clipboard save/restore, `AXIsProcessTrusted()` check |
| `tray.rs` | Menu bar icon, recording state indicator |

### State Management

Rust side uses `Arc<Mutex<T>>` for thread-safe shared state:
```rust
pub struct AppState {
    pub is_recording: Arc<Mutex<bool>>,
    pub sidecar_ready: Arc<Mutex<bool>>,
}
```

Frontend uses `useRecording` hook subscribing to Tauri events.

## Platform-Specific Code

This app is macOS-only with heavy use of:
- `CGEventTap` for global hotkey capture
- `AXIsProcessTrusted()` for accessibility permission checks
- `afplay` for system sounds
- `osascript` for Cmd+V simulation

Sidecar binary naming follows Tauri convention: `transcribe-server-{arch}-apple-darwin`

## Permissions Required

- **Microphone**: Requested programmatically via `request_microphone_permission()`
- **Accessibility**: Must be manually enabled (opens System Preferences, user toggles)

## Testing

No automated test framework. Manual testing via Settings UI:
- "Test Record (3s)" - Records audio and transcribes
- "Test Sidecar" - Sends silent WAV to verify sidecar works
