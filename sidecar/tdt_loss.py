"""TDT (token-and-duration transducer) loss in MLX, matching NeMo's tdt loss with sigma = omega = 0.

alpha[t, u] is the log prob of having emitted the first u labels and standing at frame t.
Blanks advance d > 0 frames; labels advance d >= 0 frames, so within one frame the u axis is a
linear recurrence, solved in closed form with logcumsumexp. The loop runs over frames only.
"""

import mlx.core as mx

NEG = -1e9


def tdt_log_likelihood(
    token_logits: mx.array,  # [B, T, U+1, V+1], blank is the last class
    duration_logits: mx.array,  # [B, T, U+1, D]
    targets: mx.array,  # [B, U] int, padded arbitrarily
    frames: mx.array,  # [B] valid frames per sample
    labels: mx.array,  # [B] valid labels per sample
    durations: list[int],
) -> mx.array:
    assert durations[0] == 0, "first duration must be 0 (label-only)"
    B, T, U1, _ = token_logits.shape
    token_lp = token_logits - mx.logsumexp(token_logits, axis=-1, keepdims=True)
    dur_lp = duration_logits - mx.logsumexp(duration_logits, axis=-1, keepdims=True)

    blank_lp = token_lp[..., -1]  # [B, T, U+1]
    label_lp = mx.take_along_axis(token_lp[:, :, :-1, :], targets[:, None, :, None], axis=-1)[
        ..., 0
    ]
    blank_step = blank_lp[..., None] + dur_lp  # [B, T, U+1, D], index 0 unused
    label_step = label_lp[..., None] + dur_lp[:, :, :-1, :]  # [B, T, U, D]

    def shift_u(x):
        return mx.concatenate([mx.full((B, 1), NEG), x], axis=1)

    alphas = []
    for t in range(T):
        incoming = [mx.full((B, U1), NEG)]
        if t == 0:
            incoming.append(mx.concatenate([mx.zeros((B, 1)), mx.full((B, U1 - 1), NEG)], axis=1))
        for i, d in enumerate(durations):
            if d == 0 or t - d < 0:
                continue
            prev = alphas[t - d]
            incoming.append(prev + blank_step[:, t - d, :, i])
            incoming.append(shift_u(prev[:, :-1] + label_step[:, t - d, :, i]))
        a = mx.logsumexp(mx.stack(incoming), axis=0)
        # Zero-duration labels within frame t: alpha[u] = logsumexp_{v<=u} a[v] + sum_{v<=w<u} L[w]
        cum = mx.concatenate([mx.zeros((B, 1)), mx.cumsum(label_step[:, t, :, 0], axis=1)], axis=1)
        alphas.append(mx.logcumsumexp(a - cum, axis=1) + cum)

    alpha = mx.stack(alphas, axis=1)  # [B, T, U+1]
    batch = mx.arange(B)
    finals = []
    for i, d in enumerate(durations):
        if d == 0:
            continue
        t = frames - d
        valid = t >= 0
        t = mx.maximum(t, 0)
        term = alpha[batch, t, labels] + blank_step[batch, t, labels, i]
        finals.append(mx.where(valid, term, NEG))
    return mx.logsumexp(mx.stack(finals), axis=0)  # [B]
