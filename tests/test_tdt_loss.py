"""Check the TDT loss against brute-force enumeration of every path on tiny random inputs.

sidecar/venv/bin/python tests/test_tdt_loss.py
"""

import math
import sys
from functools import lru_cache
from pathlib import Path

import mlx.core as mx
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "sidecar"))
from tdt_loss import tdt_log_likelihood  # noqa: E402

DURATIONS = [0, 1, 2, 3, 4]


def log_softmax(x):
    return (
        x
        - np.log(np.exp(x - x.max(-1, keepdims=True)).sum(-1, keepdims=True))
        - x.max(-1, keepdims=True)
    )


def brute_force(tok, dur, y, T, U):
    tok, dur = log_softmax(tok), log_softmax(dur)

    @lru_cache(None)
    def prob(t, u):
        total = 0.0
        for i, d in enumerate(DURATIONS):
            if u < U and t + d < T:
                total += math.exp(tok[t, u, y[u]] + dur[t, u, i]) * prob(t + d, u + 1)
            if d > 0:
                if u == U and t + d == T:
                    total += math.exp(tok[t, u, -1] + dur[t, u, i])
                elif t + d < T:
                    total += math.exp(tok[t, u, -1] + dur[t, u, i]) * prob(t + d, u)
        return total

    return math.log(prob(0, 0))


def main():
    rng = np.random.default_rng(0)
    V = 6
    T_max, U_max = 7, 4
    cases = [(7, 4), (5, 2), (3, 3), (1, 0), (6, 1)]
    tok = rng.normal(size=(len(cases), T_max, U_max + 1, V + 1)).astype(np.float32)
    dur = rng.normal(size=(len(cases), T_max, U_max + 1, len(DURATIONS))).astype(np.float32)
    y = rng.integers(0, V, size=(len(cases), U_max))

    got = tdt_log_likelihood(
        mx.array(tok),
        mx.array(dur),
        mx.array(y),
        mx.array([T for T, _ in cases]),
        mx.array([U for _, U in cases]),
        DURATIONS,
    )
    for b, (T, U) in enumerate(cases):
        want = brute_force(tok[b, :T, : U + 1], dur[b, :T, : U + 1], tuple(y[b, :U]), T, U)
        print(f"T={T} U={U}  dp={float(got[b]):.5f}  brute={want:.5f}")
        assert abs(float(got[b]) - want) < 1e-3

    grad = mx.grad(
        lambda x: tdt_log_likelihood(
            x, mx.array(dur), mx.array(y), mx.array([7] * 5), mx.array([4] * 5), DURATIONS
        ).sum()
    )(mx.array(tok))
    assert bool(mx.all(mx.isfinite(grad)))
    print("ok")


if __name__ == "__main__":
    main()
