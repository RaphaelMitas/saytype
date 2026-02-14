# Saytype

macOS menu bar app for push-to-talk speech-to-text. Hold the Right Command key to record, release to transcribe and insert text at cursor. Runs entirely offline using Apple Silicon ML.

## Usage
1. Launch Saytype (appears in menu bar)
2. Grant Microphone, Accessibility, and Input Monitoring permissions
3. Hold Right Command key to record
4. Release to transcribe and paste

## Building from Source

### Build sidecar (first time)
cd sidecar && ./build.sh

### Development
pnpm install
pnpm tauri dev

### Production build
pnpm tauri build

## Architecture
- Tauri/Rust: System integration (hotkeys, audio, text insertion)
- Python Sidecar: ML inference with Parakeet MLX
- React Frontend: Settings UI

## Acknowledgements
Speech recognition powered by [NVIDIA Parakeet TDT](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), licensed under CC-BY-4.0.
