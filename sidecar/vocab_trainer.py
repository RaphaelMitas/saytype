"""Teach the model the user's word list by fine-tuning its decoder and joint network.

Run as `transcribe-server --train job.json`. Each word is spoken in carrier sentences by the
macOS `say` voices, and the user's own recordings of it are mixed in. The encoder stays frozen,
so every clip goes through it once and is cached per word: adding a word only synthesizes that
word.

Job file, written by the app:
    {"words": [{"word": "Flatsby", "say_as": null}],
     "examples": [{"id": "…", "audio": "/…/x.wav", "text": "The Flatsby app…"}],
     "cache_dir": "…", "head_path": "…/head.safetensors", "meta_path": "…/head.json",
     "spec": {…}}

Progress goes to stdout as JSON lines: {"progress": 0.4}, then {"success": true} or
{"success": false, "error": "…"}.
"""

import hashlib
import json
import random
import re
import subprocess
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
import numpy as np
import sentencepiece as spm
import soundfile as sf
from huggingface_hub import hf_hub_download
from mlx.utils import tree_flatten, tree_map
from parakeet_mlx.audio import get_logmel
from tdt_loss import tdt_log_likelihood
from vocab_templates import DE, EN, FILLERS_DE, FILLERS_EN

# Bump when anything below changes what a cached clip contains
RECIPE = 1
CLIPS_PER_WORD = 16
FILLER_FRACTION = 0.25
GERMAN_FRACTION = 0.2
EPOCHS = 3
BATCH = 4
LEARNING_RATE = 1e-4
EXAMPLE_WEIGHT = 4
MAX_EXAMPLE_SECONDS = 15
RATES = [150, 170, 185, 200, 220]
SAMPLE_RATE = 16000

# Preinstalled voices that sound like people; the novelty voices (Bells, Zarvox…) are left out
VOICES = {
    "en": [
        "Samantha",
        "Daniel",
        "Karen",
        "Moira",
        "Tessa",
        "Rishi",
        "Aman",
        "Tara",
        "Eddy (English (US))",
        "Eddy (English (UK))",
        "Flo (English (US))",
        "Flo (English (UK))",
        "Grandma (English (US))",
        "Grandpa (English (UK))",
        "Reed (English (US))",
        "Reed (English (UK))",
        "Rocko (English (US))",
        "Sandy (English (UK))",
        "Shelley (English (US))",
        "Shelley (English (UK))",
    ],
    "de": [
        "Anna (German (Germany))",
        "Eddy (German (Germany))",
        "Flo (German (Germany))",
        "Grandpa (German (Germany))",
        "Reed (German (Germany))",
        "Rocko (German (Germany))",
        "Sandy (German (Germany))",
        "Shelley (German (Germany))",
    ],
}
TEMPLATES = {"en": EN, "de": DE}
FILLERS = {"en": FILLERS_EN, "de": FILLERS_DE}


def emit(**message) -> None:
    print(json.dumps(message), flush=True)


def installed_voices() -> dict[str, list[str]]:
    listing = subprocess.run(["say", "-v", "?"], capture_output=True, text=True, check=True)
    names = {
        match.group(1)
        for line in listing.stdout.splitlines()
        if (match := re.match(r"^(.+?)\s{2,}[a-z]{2}_[A-Z]{2}\b", line))
    }
    voices = {lang: [v for v in wanted if v in names] for lang, wanted in VOICES.items()}
    if not voices["en"]:
        raise RuntimeError("No English system voices installed to generate training speech")
    return voices


def tokenizer_path(model_name: str) -> str:
    return hf_hub_download(model_name, "tokenizer.model")


class Model:
    """The base model plus the pieces training needs: tokenizer and frozen-encoder features."""

    def __init__(self, model, model_name: str):
        self.model = model
        self.tokenizer = spm.SentencePieceProcessor()
        self.tokenizer.Load(tokenizer_path(model_name))

    def encode(self, audio: np.ndarray) -> mx.array:
        mel = get_logmel(mx.array(audio), self.model.preprocessor_config)
        features, _ = self.model.encoder(mel)
        return features

    def own_transcript(self, features: mx.array) -> list[int]:
        hypothesis, _ = self.model.decode(features, mx.array([features.shape[1]]))
        return [token.id for token in hypothesis[0]]


def cache_key(**fields) -> str:
    return hashlib.sha1(json.dumps({"recipe": RECIPE, **fields}).encode()).hexdigest()[:16]


def plan_clips(word: str, say_as: str, voices: dict[str, list[str]], rng: random.Random):
    langs = ["en", "de"] if voices["de"] else ["en"]

    def clip(slot_written: str, slot_spoken: str, filler: bool) -> dict:
        lang = "de" if "de" in langs and rng.random() < GERMAN_FRACTION else "en"
        template = rng.choice(TEMPLATES[lang])
        written = template.replace("{W}", slot_written)
        if filler:
            written = written[0].upper() + written[1:]
        return {
            "written": written,
            "spoken": template.replace("{W}", slot_spoken),
            "voice": rng.choice(voices[lang]),
            "rate": rng.choice(RATES),
            "filler": filler,
        }

    clips = [clip(word, say_as, False) for _ in range(CLIPS_PER_WORD)]
    for _ in range(round(FILLER_FRACTION * CLIPS_PER_WORD)):
        lang = rng.choice(langs)
        filler = rng.choice(FILLERS[lang])
        clips.append(clip(filler, filler, True))
    return clips


def speak(clip: dict, path: Path) -> np.ndarray:
    subprocess.run(
        [
            "say",
            "-v",
            clip["voice"],
            "-r",
            str(clip["rate"]),
            "-o",
            str(path),
            f"--data-format=LEI16@{SAMPLE_RATE}",
            "--",
            clip["spoken"],
        ],
        check=True,
        capture_output=True,
    )
    audio, _ = sf.read(str(path), dtype="float32")
    return audio


def add_noise(audio: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    snr_db = rng.uniform(15, 40)
    power = np.mean(audio**2) / (10 ** (snr_db / 10))
    return (audio + rng.normal(0, np.sqrt(power), audio.shape)).astype(np.float32)


def save_rows(path: Path, rows: list[dict]) -> None:
    """Writes beside the target and renames, so a cancelled run never leaves half a cache."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp.npz")
    np.savez(
        tmp,
        **{f"f{i}": row["features"] for i, row in enumerate(rows)},
        tokens=np.array(json.dumps([row["tokens"] for row in rows])),
    )
    tmp.replace(path)


def load_rows(path: Path) -> list[dict]:
    data = np.load(path)
    tokens = json.loads(str(data["tokens"]))
    return [{"features": data[f"f{i}"], "tokens": t} for i, t in enumerate(tokens)]


def spoken_form(word: dict) -> str:
    return word.get("say_as") or word["word"]


def word_cache(word: dict, voices, cache_dir: Path) -> Path:
    key = cache_key(word=word["word"], say_as=spoken_form(word), voices=voices)
    return cache_dir / "words" / f"{key}.npz"


def word_rows(word: dict, voices, model: Model, cache_dir: Path, on_clip) -> list[dict]:
    path = word_cache(word, voices, cache_dir)
    if path.exists():
        return load_rows(path)

    rng = random.Random(path.stem)
    noise = np.random.default_rng(int(path.stem, 16))
    clips = plan_clips(word["word"], spoken_form(word), voices, rng)
    with tempfile.TemporaryDirectory() as tmp, ThreadPoolExecutor(8) as pool:
        audios = pool.map(
            lambda pair: speak(pair[1], Path(tmp) / f"{pair[0]}.wav"), enumerate(clips)
        )
        rows = []
        for clip, audio in zip(clips, audios, strict=True):
            if rng.random() < 0.5:
                audio = add_noise(audio, noise)
            features = model.encode(audio)
            tokens = (
                model.own_transcript(features)
                if clip["filler"]
                else model.tokenizer.EncodeAsIds(clip["written"])
            )
            rows.append({"features": np.array(features[0].astype(mx.float16)), "tokens": tokens})
            on_clip()
    save_rows(path, rows)
    return rows


def example_rows(example: dict, model: Model, cache_dir: Path) -> list[dict]:
    key = cache_key(id=example["id"], text=example["text"])
    path = cache_dir / "examples" / f"{key}.npz"
    if path.exists():
        return load_rows(path)
    audio, sr = sf.read(example["audio"], dtype="float32")
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    if sr != SAMPLE_RATE or len(audio) > MAX_EXAMPLE_SECONDS * SAMPLE_RATE:
        return []
    features = model.encode(audio)
    rows = [
        {
            "features": np.array(features[0].astype(mx.float16)),
            "tokens": model.tokenizer.EncodeAsIds(example["text"]),
        }
    ]
    save_rows(path, rows)
    return rows


class Head(nn.Module):
    def __init__(self, model):
        super().__init__()
        self.decoder = model.decoder
        self.joint = model.joint
        self.blank = len(model.vocabulary)
        self.durations = model.durations

    def __call__(self, features, tokens, frames, labels):
        embed = self.decoder.prediction["embed"](tokens)
        # Inference starts the decoder from a zero vector, not from an embedded token
        embed = mx.concatenate([mx.zeros_like(embed[:, :1]), embed], axis=1)
        prediction, _ = self.decoder.prediction["dec_rnn"](embed)
        logits = self.joint(features, prediction)
        likelihood = tdt_log_likelihood(
            logits[..., : self.blank + 1],
            logits[..., self.blank + 1 :],
            tokens,
            frames,
            labels,
            self.durations,
        )
        return -likelihood.sum() / (labels + 1).sum()


def batches(rows: list[dict], rng: random.Random) -> list[list[dict]]:
    """Groups similar lengths, since padding to the longest clip dominates step time."""
    rows = rows[:]
    rng.shuffle(rows)
    out = []
    for i in range(0, len(rows), BATCH * 8):
        group = sorted(rows[i : i + BATCH * 8], key=lambda r: len(r["features"]))
        out += [group[j : j + BATCH] for j in range(0, len(group), BATCH)]
    rng.shuffle(out)
    return out


def collate(batch: list[dict]):
    frames = max(len(r["features"]) for r in batch)
    labels = max(1, max(len(r["tokens"]) for r in batch))
    features = np.zeros((len(batch), frames, batch[0]["features"].shape[1]), np.float32)
    tokens = np.zeros((len(batch), labels), np.int32)
    for i, row in enumerate(batch):
        features[i, : len(row["features"])] = row["features"]
        tokens[i, : len(row["tokens"])] = row["tokens"]
    return (
        mx.array(features),
        mx.array(tokens),
        mx.array([len(r["features"]) for r in batch]),
        mx.array([len(r["tokens"]) for r in batch]),
    )


def head_weights(model) -> dict[str, mx.array]:
    return {
        name: value
        for name, value in tree_flatten(model.parameters())
        if isinstance(value, mx.array) and name.startswith(("decoder.", "joint."))
    }


def train(model, rows: list[dict], on_step) -> dict[str, mx.array]:
    for module in (model.decoder, model.joint):
        module.update(tree_map(lambda p: p.astype(mx.float32), module.parameters()))
    head = Head(model)
    optimizer = optim.AdamW(learning_rate=LEARNING_RATE, weight_decay=0.0)
    loss_and_grad = nn.value_and_grad(head, head)
    rng = random.Random(0)
    for _ in range(EPOCHS):
        for batch in batches(rows, rng):
            loss, grads = loss_and_grad(*collate(batch))
            optimizer.update(head, grads)
            mx.eval(head.parameters(), optimizer.state, loss)
            on_step()
    return {name: value.astype(mx.bfloat16) for name, value in head_weights(model).items()}


def write_head(weights: dict[str, mx.array], head_path: Path, meta_path: Path, spec) -> None:
    head_path.parent.mkdir(parents=True, exist_ok=True)
    tmp = head_path.with_name(".head.tmp.safetensors")
    mx.save_safetensors(str(tmp), weights)
    tmp.replace(head_path)
    # Written last: a crash in between leaves a stale spec, which only means one more retrain
    meta_tmp = meta_path.with_name(f".{meta_path.name}.tmp")
    meta_tmp.write_text(json.dumps({"spec": spec, "trained_at": time.time()}))
    meta_tmp.replace(meta_path)


def run(job_path: Path, model, model_name: str) -> None:
    job = json.loads(job_path.read_text())
    cache_dir = Path(job["cache_dir"])
    words = job["words"]
    voices = installed_voices()
    base = Model(model, model_name)

    missing = sum(not word_cache(word, voices, cache_dir).exists() for word in words)
    clips_per_word = CLIPS_PER_WORD + round(FILLER_FRACTION * CLIPS_PER_WORD)
    prepare_units = missing * clips_per_word
    done = 0

    rows = []
    for word in words:

        def on_clip():
            nonlocal done
            done += 1
            emit(progress=0.6 * done / max(prepare_units, 1))

        rows += word_rows(word, voices, base, cache_dir, on_clip)
    for example in job["examples"]:
        rows += example_rows(example, base, cache_dir) * EXAMPLE_WEIGHT

    steps = EPOCHS * len(batches(rows, random.Random(0)))
    start = 0.6 if prepare_units else 0.0
    step = 0

    def on_step():
        nonlocal step
        step += 1
        emit(progress=start + (1 - start) * step / steps)

    weights = train(model, rows, on_step)
    # A run the app stopped but that kept going must not overwrite a newer head
    if json.loads(job_path.read_text())["spec"] != job["spec"]:
        raise RuntimeError("Superseded by a newer training run")
    write_head(weights, Path(job["head_path"]), Path(job["meta_path"]), job["spec"])
