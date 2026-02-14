![hero](hero.png)

# Saytype

macOS menu bar app for push-to-talk speech-to-text. Hold the Right Command key to record, release to transcribe and insert text at cursor. Runs entirely offline using Apple Silicon ML.

> **Note:** This app was vibecoded with [Claude](https://claude.ai).

## Installation

### Homebrew

```bash
brew install raphaelmitas/tap/saytype
```

### Manual Download

1. Download the latest release from [GitHub Releases](https://github.com/RaphaelMitas/saytype/releases)
2. Unzip and drag `Saytype.app` to Applications
3. Launch Saytype

### Requirements

- macOS 11.0+ (Big Sur or later)
- Apple Silicon (M1/M2/M3/M4)

## Usage

1. **Launch Saytype** — appears as an icon in your menu bar
2. **Grant permissions** — Microphone, Accessibility, and Input Monitoring (first launch only)
3. **Hold Right Command** — starts recording
4. **Release** — transcribes and pastes text at your cursor

The app runs entirely offline. On first launch, wait for the ML model to load (indicated by a ready sound and status in Settings). Recording won't work until the model is ready.

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/) 20+
- [pnpm](https://pnpm.io/)
- Python 3.10+

### Build

```bash
# Build the ML sidecar (required before first run)
cd sidecar && ./build.sh && cd ..

# Install dependencies
pnpm install

# Development (hot reload)
pnpm tauri dev

# Production build
pnpm tauri build
```

The built app will be at `src-tauri/target/release/bundle/macos/Saytype.app`.

## Architecture

- **Tauri/Rust** — System integration (hotkeys, audio capture, text insertion)
- **Python Sidecar** — ML inference with Parakeet MLX
- **React Frontend** — Settings UI only

## Acknowledgements

Speech recognition powered by [NVIDIA Parakeet TDT](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), licensed under CC-BY-4.0.
