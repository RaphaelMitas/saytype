#!/bin/bash
# Build the transcription server sidecar

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/src-tauri/binaries"

echo "Building transcription server sidecar..."

# Create virtual environment if it doesn't exist
if [ ! -d "$SCRIPT_DIR/venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv "$SCRIPT_DIR/venv"
fi

# Activate virtual environment
source "$SCRIPT_DIR/venv/bin/activate"

# Install dependencies
echo "Installing dependencies..."
pip install --upgrade pip
pip install -r "$SCRIPT_DIR/requirements.txt"

# Build with PyInstaller
echo "Building executable with PyInstaller..."
cd "$SCRIPT_DIR"
pyinstaller \
    --onefile \
    --name transcribe-server \
    --distpath "$OUTPUT_DIR" \
    --clean \
    --noconfirm \
    --hidden-import=mlx._reprlib_fix \
    --hidden-import=mlx.core \
    --hidden-import=soundfile \
    --hidden-import=parakeet_mlx.audio \
    --collect-all=mlx \
    --collect-all=parakeet_mlx \
    transcribe_server.py

# Tauri expects binaries with target triple suffix
# Get the current target triple
TARGET_TRIPLE=$(rustc -vV | grep host | awk '{print $2}')
BINARY_PATH="$OUTPUT_DIR/transcribe-server"
FINAL_PATH="$OUTPUT_DIR/transcribe-server-$TARGET_TRIPLE"

if [ -f "$BINARY_PATH" ]; then
    mv "$BINARY_PATH" "$FINAL_PATH"
    chmod +x "$FINAL_PATH"
    echo "Built: $FINAL_PATH"
else
    echo "Error: Binary not found at $BINARY_PATH"
    exit 1
fi

echo "Sidecar build complete!"
