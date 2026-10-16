#!/usr/bin/env python3
"""
End-to-end test for the frozen transcription sidecar binary.

Exercises the binary in all the modes the Tauri app relies on:

    1. --transcribe        one-shot CLI transcription
    2. stdin/stdout IPC    the protocol sidecar.rs speaks
    3. --http              the protocol remote.rs / transcriber.rs speak
    4. --train             custom-word training, as vocab.rs runs it, then
                           IPC with --vocab-dir picking up the trained head

The point of running the *frozen* binary rather than transcribe_server.py is
that PyInstaller bundling breaks silently (missing hidden imports, missing MLX
data files) in ways the source tree never reveals.

Usage:
    ./sidecar_e2e.py <path-to-sidecar-binary> [--fixture FILE] [--expect TEXT]
"""

import argparse
import http.client
import json
import queue
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

# Guard before any `X | Y` annotation in a def is evaluated — on 3.9 (stock
# macOS python3) those raise an opaque TypeError at import.
if sys.version_info < (3, 10):
    sys.exit(f"sidecar_e2e.py needs Python >= 3.10, found {sys.version.split()[0]}")

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = REPO_ROOT / "tests" / "fixtures" / "hello.wav"
DEFAULT_EXPECT = "The quick brown fox jumps over the lazy dog."

# Cold start downloads ~1.2GB of model weights from HuggingFace, then compiles
# Metal kernels during warmup. Generous, because a timeout here is a CI failure
# with a very unhelpful message.
READY_TIMEOUT = 900.0
TRANSCRIBE_TIMEOUT = 180.0
TRAIN_TIMEOUT = 900.0


class TestFailure(Exception):
    """A test assertion failed."""


def normalize(text: str) -> str:
    """Casing and punctuation are not part of the contract; words are.

    Keep in sync with `normalize` in src-tauri/src/e2e.rs: punctuation maps to
    a space and runs collapse, so "twenty-one" == "twenty one" in both suites.
    """
    return " ".join(re.sub(r"[^a-z0-9]", " ", text.lower()).split())


def assert_transcript(actual: str, expected: str, mode: str) -> None:
    if normalize(actual) != normalize(expected):
        raise TestFailure(f"{mode}: expected transcript {expected!r}, got {actual!r}")
    print(f"  transcript OK: {actual!r}")


class LineReader:
    """Reads lines off a pipe in a background thread so we can apply timeouts."""

    def __init__(self, stream, label: str):
        self._queue: queue.Queue[str | None] = queue.Queue()
        self._label = label
        self._thread = threading.Thread(target=self._pump, args=(stream,), daemon=True)
        self._thread.start()

    def _pump(self, stream) -> None:
        for line in stream:
            self._queue.put(line.rstrip("\n"))
        self._queue.put(None)

    def next_line(self, timeout: float) -> str:
        try:
            line = self._queue.get(timeout=timeout)
        except queue.Empty:
            raise TestFailure(
                f"timed out after {timeout:.0f}s waiting for a line on {self._label}"
            ) from None
        if line is None:
            raise TestFailure(f"{self._label} closed before the expected line arrived")
        print(f"  <{self._label}> {line}")
        return line

    def wait_for(self, needle: str, timeout: float) -> str:
        """Consume lines until one contains `needle`."""
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TestFailure(f"timed out waiting for {needle!r} on {self._label}")
            line = self.next_line(remaining)
            if needle in line:
                return line


def next_json(reader: LineReader, timeout: float) -> dict:
    line = reader.next_line(timeout)
    try:
        return json.loads(line)
    except json.JSONDecodeError as e:
        raise TestFailure(f"expected JSON on stdout, got {line!r} ({e})") from None


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def terminate(proc: subprocess.Popen) -> None:
    if proc.poll() is not None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=15)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=15)


def test_cli_mode(binary: Path, fixture: Path, expected: str) -> None:
    """Mode 1: one-shot `--transcribe`, the simplest smoke test of the freeze."""
    print("\n=== Mode 1: --transcribe ===")
    proc = subprocess.run(
        [str(binary), "--transcribe", str(fixture)],
        capture_output=True,
        text=True,
        timeout=READY_TIMEOUT,
        check=False,
    )
    if proc.returncode != 0:
        raise TestFailure(f"--transcribe exited {proc.returncode}\nstderr:\n{proc.stderr}")

    # stdout must be pure JSON — the HuggingFace download banner goes to stderr,
    # and anything leaking onto stdout would break the Rust IPC parser.
    try:
        result = json.loads(proc.stdout.strip())
    except json.JSONDecodeError as e:
        raise TestFailure(f"--transcribe stdout is not JSON: {proc.stdout!r} ({e})") from None

    if not result.get("success"):
        raise TestFailure(f"--transcribe failed: {result.get('error')}")
    assert_transcript(result.get("text", ""), expected, "--transcribe")


def test_ipc_mode(binary: Path, fixture: Path, expected: str) -> None:
    """Mode 2: the stdin/stdout JSON protocol that sidecar.rs implements."""
    print("\n=== Mode 2: stdin/stdout IPC ===")
    proc = subprocess.Popen(
        [str(binary)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=None,
        text=True,
        bufsize=1,
    )
    try:
        stdout = LineReader(proc.stdout, "stdout")
        assert proc.stdin is not None

        # sidecar.rs expects exactly this handshake: loading, then ready.
        # READY_TIMEOUT even for the first line: PyInstaller onefile re-extracts
        # the bundle and imports mlx before the sidecar prints anything.
        loading = next_json(stdout, READY_TIMEOUT)
        if loading.get("status") != "loading":
            raise TestFailure(f"expected status 'loading' first, got {loading}")

        ready = next_json(stdout, READY_TIMEOUT)
        if ready.get("status") != "ready":
            raise TestFailure(f"expected status 'ready', got {ready}")

        proc.stdin.write(json.dumps({"command": "ping"}) + "\n")
        proc.stdin.flush()
        pong = next_json(stdout, 30.0)
        if not pong.get("success") or pong.get("message") != "pong":
            raise TestFailure(f"ping failed: {pong}")
        print("  ping OK")

        proc.stdin.write(json.dumps({"command": "transcribe", "audio_path": str(fixture)}) + "\n")
        proc.stdin.flush()
        result = next_json(stdout, TRANSCRIBE_TIMEOUT)
        if not result.get("success"):
            raise TestFailure(f"IPC transcribe failed: {result.get('error')}")
        assert_transcript(result.get("text", ""), expected, "IPC")

        # An unknown command must be reported, not crash the process.
        proc.stdin.write(json.dumps({"command": "bogus"}) + "\n")
        proc.stdin.flush()
        unknown = next_json(stdout, 30.0)
        if unknown.get("success") is not False:
            raise TestFailure(f"expected failure for unknown command, got {unknown}")
        print("  unknown-command handling OK")

        proc.stdin.write(json.dumps({"command": "quit"}) + "\n")
        proc.stdin.flush()
        try:
            proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            raise TestFailure("sidecar did not exit after 'quit' command") from None
        print("  quit OK")
    finally:
        terminate(proc)


def http_request(
    port: int, method: str, path: str, body: bytes | None = None, host: str = "127.0.0.1"
):
    conn = http.client.HTTPConnection(host, port, timeout=TRANSCRIBE_TIMEOUT)
    try:
        headers = {"Content-Type": "application/octet-stream"} if body else {}
        conn.request(method, path, body=body, headers=headers)
        resp = conn.getresponse()
        return resp.status, json.loads(resp.read())
    finally:
        conn.close()


def test_http_mode(binary: Path, fixture: Path, expected: str) -> None:
    """Mode 3: the HTTP server that remote.rs talks to in client/server mode."""
    print("\n=== Mode 3: --http ===")
    port = free_port()
    proc = subprocess.Popen(
        [str(binary), "--http", "--host", "::", "--port", str(port)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=None,
        text=True,
        bufsize=1,
    )
    try:
        stdout = LineReader(proc.stdout, "stdout")
        # transcriber.rs blocks on this exact substring to decide the server is up.
        stdout.wait_for("[HTTP] Listening on", READY_TIMEOUT)

        # .local names often resolve to IPv6 first, so the server must answer on both
        for host in ("127.0.0.1", "::1"):
            status, health = http_request(port, "GET", "/health", host=host)
            if status != 200 or health.get("status") != "ready":
                raise TestFailure(
                    f"/health on {host} returned {status} {health}, expected 200 ready"
                )
        print("  /health OK on IPv4 and IPv6")

        status, result = http_request(port, "POST", "/transcribe", fixture.read_bytes())
        if status != 200 or not result.get("success"):
            raise TestFailure(f"/transcribe returned {status} {result}")
        assert_transcript(result.get("text", ""), expected, "HTTP")

        status, _ = http_request(port, "GET", "/nope")
        if status != 404:
            raise TestFailure(f"expected 404 for unknown path, got {status}")
        print("  404 handling OK")
    finally:
        terminate(proc)


def test_train_mode(binary: Path, fixture: Path, expected: str) -> None:
    """Mode 4: --train writes a head, and a serving sidecar swaps it in before transcribing."""
    print("\n=== Mode 4: --train, then IPC with --vocab-dir ===")
    with tempfile.TemporaryDirectory() as tmp:
        vocab = Path(tmp)
        job = vocab / "job.json"
        # The fixture stays held out, so the check below catches a head that hurts dictation
        example_text = "I use Saytype every day."
        example = vocab / "example.wav"
        subprocess.run(
            ["say", "-o", str(example), "--data-format=LEI16@16000", "--", example_text],
            check=True,
        )
        spec = {"words": [{"word": "Saytype", "say_as": None}]}
        job.write_text(
            json.dumps(
                {
                    "words": spec["words"],
                    "examples": [{"id": "example", "audio": str(example), "text": example_text}],
                    "cache_dir": str(vocab / "cache"),
                    "head_path": str(vocab / "head.safetensors"),
                    "meta_path": str(vocab / "head.json"),
                    "spec": spec,
                }
            )
        )
        proc = subprocess.run(
            [str(binary), "--train", str(job)],
            capture_output=True,
            text=True,
            timeout=TRAIN_TIMEOUT,
            check=False,
        )
        lines = [json.loads(line) for line in proc.stdout.splitlines() if line.startswith("{")]
        if proc.returncode != 0 or not lines or lines[-1].get("success") is not True:
            raise TestFailure(f"--train failed: {lines[-1:] or proc.stdout!r}\n{proc.stderr}")
        if not any("progress" in line for line in lines):
            raise TestFailure("--train reported no progress")
        meta = json.loads((vocab / "head.json").read_text())
        if meta.get("spec") != spec or not (vocab / "head.safetensors").exists():
            raise TestFailure(f"--train wrote an unexpected head: {meta}")
        print(f"  trained, {len(lines)} progress lines")

        serve = subprocess.Popen(
            [str(binary), "--vocab-dir", str(vocab)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        try:
            stdout = LineReader(serve.stdout, "stdout")
            # A head that can't be loaded falls back to the base model, which transcribes fine
            stderr = LineReader(serve.stderr, "stderr")
            assert serve.stdin is not None
            stdout.wait_for('"ready"', READY_TIMEOUT)
            serve.stdin.write(
                json.dumps({"command": "transcribe", "audio_path": str(fixture)}) + "\n"
            )
            serve.stdin.flush()
            stderr.wait_for("[VOCAB] Applied head", TRANSCRIBE_TIMEOUT)
            result = next_json(stdout, TRANSCRIBE_TIMEOUT)
            if not result.get("success"):
                raise TestFailure(f"transcribe with trained head failed: {result.get('error')}")
            assert_transcript(result.get("text", ""), expected, "IPC with trained head")
        finally:
            terminate(serve)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="Path to the frozen sidecar binary")
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--expect", type=str, default=DEFAULT_EXPECT)
    args = parser.parse_args()

    if not args.binary.exists():
        print(f"Sidecar binary not found: {args.binary}", file=sys.stderr)
        return 1
    if not args.fixture.exists():
        print(f"Fixture not found: {args.fixture}", file=sys.stderr)
        return 1

    print(f"Sidecar: {args.binary}")
    print(f"Fixture: {args.fixture}")
    print(f"Expect:  {args.expect!r}")

    tests = [test_cli_mode, test_ipc_mode, test_http_mode, test_train_mode]
    failures: list[str] = []
    for test in tests:
        started = time.monotonic()
        try:
            test(args.binary, args.fixture, args.expect)
            print(f"  PASS ({time.monotonic() - started:.1f}s)")
        except (TestFailure, subprocess.TimeoutExpired, OSError) as e:
            print(f"  FAIL: {e}")
            failures.append(f"{test.__name__}: {e}")

    print("\n=== Summary ===")
    if failures:
        for failure in failures:
            print(f"FAIL {failure}")
        return 1
    print(f"All {len(tests)} sidecar E2E modes passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
