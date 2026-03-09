from __future__ import annotations

from math import cos, pi, tau

TAU = tau


def wrap_tau(rad: float) -> float:
    x = rad % TAU
    if x < 0.0:
        x += TAU
    return x


def phase_delta(a: float, b: float) -> float:
    """Smallest angular distance in radians within [0, pi]."""
    a_w = wrap_tau(a)
    b_w = wrap_tau(b)
    raw = abs(a_w - b_w)
    return min(raw, TAU - raw)


def coherence_gate(current: float, target: float, window_rad: float) -> float:
    """Cosine coherence score in [0, 1]."""
    if window_rad <= 0.0:
        return 0.0
    d = phase_delta(current, target)
    if d >= window_rad:
        return 0.0
    x = d / window_rad
    return 0.5 * (1.0 + cos(pi * (1.0 - x)))
