#!/usr/bin/env python3
"""
Saytype Transcription Server

A sidecar process that uses parakeet-mlx for offline speech-to-text transcription.
Communicates with the Tauri app via command-line arguments and stdout JSON.

Usage:
    # Server mode (waits for stdin commands)
    ./transcribe_server.py

    # Direct transcription mode
    ./transcribe_server.py --transcribe /path/to/audio.wav
"""

import argparse
import json
import sys
from pathlib import Path

import soundfile as sf
import mlx.core as mx
from parakeet_mlx.audio import get_logmel

# Global model cache
_model = None
_model_name = None


def load_model(model_name: str):
    """Load the parakeet-mlx model."""
    global _model, _model_name

    if _model is not None and _model_name == model_name:
        return _model

    try:
        from parakeet_mlx import from_pretrained
        _model = from_pretrained(model_name)
        _model_name = model_name
        return _model
    except ImportError as e:
        print(json.dumps({
            "success": False,
            "error": f"Failed to import parakeet_mlx: {e}"
        }), flush=True)
        sys.exit(1)


def warmup_model(model_name: str):
    """
    Warm up the model by running inference on representative-length audio.

    MLX compiles Metal kernels lazily per tensor shape. We use 3 seconds
    of audio to match typical recording lengths and trigger compilation
    for realistic mel-spectrogram dimensions.
    """
    import numpy as np

    model = load_model(model_name)

    # Create 3 seconds of silence at 16kHz (realistic length)
    # This creates ~188 mel frames, similar to real recordings
    dummy_audio = np.zeros(16000 * 3, dtype=np.float32)
    audio_mx = mx.array(dummy_audio)

    # Run full inference pipeline to compile Metal kernels
    mel = get_logmel(audio_mx, model.preprocessor_config)
    result = model.generate(mel)

    # Force MLX to complete all lazy evaluation by accessing the result
    # This ensures Metal kernels are actually compiled, not just scheduled
    if result:
        _ = result[0].text


def transcribe_audio(audio_path: str, model_name: str) -> dict:
    """
    Transcribe an audio file using parakeet-mlx.

    Uses soundfile to load audio directly, bypassing parakeet-mlx's
    load_audio() function which requires FFmpeg.

    Args:
        audio_path: Path to the WAV audio file
        model_name: Name of the Parakeet model to use

    Returns:
        Dictionary with 'success', 'text' or 'error' keys
    """
    try:
        # Verify file exists
        if not Path(audio_path).exists():
            return {
                "success": False,
                "error": f"Audio file not found: {audio_path}"
            }

        # Load model
        model = load_model(model_name)

        # Load audio with soundfile (no FFmpeg required)
        audio_data, sr = sf.read(audio_path)

        # Convert to mono if stereo
        if len(audio_data.shape) > 1:
            audio_data = audio_data.mean(axis=1)

        # Convert to mlx array (matching load_audio output format)
        audio_mx = mx.array(audio_data.astype('float32'))

        # Get mel spectrogram
        mel = get_logmel(audio_mx, model.preprocessor_config)

        # Generate transcription
        result = model.generate(mel)[0]
        text = result.text.strip()

        return {
            "success": True,
            "text": text
        }

    except Exception as e:
        return {
            "success": False,
            "error": str(e)
        }


def server_mode(model_name: str):
    """
    Run in server mode, reading JSON commands from stdin.

    Commands:
        {"command": "ping"} -> {"success": true, "message": "pong"}
        {"command": "transcribe", "audio_path": "/path/to/file.wav"} -> {"success": true, "text": "..."}
    """
    # Load model and warm up BEFORE reporting ready
    print(json.dumps({"status": "loading"}), flush=True)
    load_model(model_name)
    warmup_model(model_name)
    print(json.dumps({"status": "ready"}), flush=True)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as e:
            print(json.dumps({
                "success": False,
                "error": f"Invalid JSON: {e}"
            }), flush=True)
            continue

        command = request.get("command", "")

        if command == "ping":
            print(json.dumps({
                "success": True,
                "message": "pong"
            }), flush=True)

        elif command == "transcribe":
            audio_path = request.get("audio_path", "")
            if not audio_path:
                print(json.dumps({
                    "success": False,
                    "error": "Missing audio_path"
                }), flush=True)
            else:
                result = transcribe_audio(audio_path, model_name)
                print(json.dumps(result), flush=True)

        elif command == "quit":
            break

        else:
            print(json.dumps({
                "success": False,
                "error": f"Unknown command: {command}"
            }), flush=True)


def http_server_mode(model_name: str, host: str, port: int):
    """
    Run as an HTTP server for remote transcription.

    Endpoints:
        GET  /health     -> {"status": "ready"} or {"status": "loading"}
        POST /transcribe -> {"success": true, "text": "..."} (body: raw WAV bytes)
    """
    import tempfile
    from http.server import HTTPServer, BaseHTTPRequestHandler

    model_ready = False

    class TranscribeHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == "/health":
                status = "ready" if model_ready else "loading"
                self._send_json(200, {"status": status})
            else:
                self._send_json(404, {"error": "Not found"})

        def do_POST(self):
            if self.path == "/transcribe":
                if not model_ready:
                    self._send_json(503, {"success": False, "error": "Model still loading"})
                    return

                content_length = int(self.headers.get("Content-Length", 0))
                if content_length == 0:
                    self._send_json(400, {"success": False, "error": "Empty request body"})
                    return

                audio_bytes = self.rfile.read(content_length)

                # Write to temp file, transcribe, clean up
                with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
                    f.write(audio_bytes)
                    tmp_path = f.name

                try:
                    result = transcribe_audio(tmp_path, model_name)
                    code = 200 if result.get("success") else 500
                    self._send_json(code, result)
                finally:
                    Path(tmp_path).unlink(missing_ok=True)
            else:
                self._send_json(404, {"error": "Not found"})

        def _send_json(self, code, data):
            body = json.dumps(data).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format, *args):
            # Use print for consistent logging
            print(f"[HTTP] {args[0]}", flush=True)

    print(f"[HTTP] Loading model...", flush=True)
    load_model(model_name)
    warmup_model(model_name)
    model_ready = True
    print(f"[HTTP] Model ready", flush=True)

    server = HTTPServer((host, port), TranscribeHandler)
    print(f"[HTTP] Listening on {host}:{port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print(f"\n[HTTP] Shutting down", flush=True)
        server.shutdown()


def main():
    parser = argparse.ArgumentParser(description="Saytype Transcription Server")
    parser.add_argument(
        "--transcribe",
        type=str,
        help="Transcribe a single audio file and exit"
    )
    parser.add_argument(
        "--model",
        type=str,
        default="mlx-community/parakeet-tdt-0.6b-v3",
        help="Parakeet model to use (default: parakeet-tdt-0.6b-v3)"
    )
    parser.add_argument(
        "--http",
        action="store_true",
        help="Run as HTTP server instead of stdin/stdout IPC"
    )
    parser.add_argument(
        "--host",
        type=str,
        default="0.0.0.0",
        help="HTTP server bind address (default: 0.0.0.0)"
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8765,
        help="HTTP server port (default: 8765)"
    )
    args = parser.parse_args()

    model_name = args.model

    if args.transcribe:
        # Direct transcription mode
        result = transcribe_audio(args.transcribe, model_name)
        print(json.dumps(result), flush=True)
    elif args.http:
        # HTTP server mode
        http_server_mode(model_name, args.host, args.port)
    else:
        # Stdin/stdout IPC mode
        server_mode(model_name)


if __name__ == "__main__":
    main()
