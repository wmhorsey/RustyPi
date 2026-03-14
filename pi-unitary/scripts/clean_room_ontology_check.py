#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass
from pathlib import Path


@dataclass
class RunMetrics:
    mode: str
    ticks: int
    grid_size: int
    core_radius: int
    total_ste_initial: int
    total_ste_final: int
    conservation_error_max: int
    core_leak_samples: int
    blocked_total: int
    redirected_total: int
    redirection_ratio: float
    shell_peak_mean: float
    far_peak_mean: float
    shell_to_far_peak_ratio: float
    shell_emergence_latency: int | None


def in_bounds(n: int, x: int, y: int) -> bool:
    return 0 <= x < n and 0 <= y < n


def build_masks(n: int, cx: int, cy: int, core_r: int) -> tuple[list[list[bool]], list[tuple[int, int]], list[tuple[int, int]]]:
    core = [[False for _ in range(n)] for _ in range(n)]
    boundary: list[tuple[int, int]] = []
    far: list[tuple[int, int]] = []
    for y in range(n):
        for x in range(n):
            dx = x - cx
            dy = y - cy
            d2 = dx * dx + dy * dy
            if d2 <= core_r * core_r:
                core[y][x] = True
            # Thin shell just outside core.
            if core_r * core_r < d2 <= (core_r + 2) * (core_r + 2):
                boundary.append((x, y))
            # Far annulus for comparison.
            if (core_r + 18) * (core_r + 18) <= d2 <= (core_r + 24) * (core_r + 24):
                far.append((x, y))
    return core, boundary, far


def initial_density(n: int, cx: int, cy: int) -> list[list[int]]:
    rho = [[0 for _ in range(n)] for _ in range(n)]
    for y in range(n):
        for x in range(n):
            dx = x - cx
            dy = y - cy
            d = math.isqrt(dx * dx + dy * dy)
            # Deterministic radial profile: no random terms.
            val = 420 - 3 * d
            if val < 60:
                val = 60
            rho[y][x] = val
    return rho


def inward_step(dx: int, dy: int) -> tuple[int, int]:
    # Integer direction toward center using dominant axis.
    if abs(dx) >= abs(dy):
        sx = -1 if dx > 0 else (1 if dx < 0 else 0)
        sy = 0
    else:
        sx = 0
        sy = -1 if dy > 0 else (1 if dy < 0 else 0)
    return sx, sy


def tangential_dirs(sx: int, sy: int) -> tuple[tuple[int, int], tuple[int, int]]:
    # Perpendicular directions.
    return (-sy, sx), (sy, -sx)


def run_case(mode: str, ticks: int, n: int, core_r: int, move_div: int) -> RunMetrics:
    cx = n // 2
    cy = n // 2
    core, boundary_cells, far_cells = build_masks(n, cx, cy, core_r)
    rho = initial_density(n, cx, cy)

    if mode == "void":
        for y in range(n):
            for x in range(n):
                if core[y][x]:
                    rho[y][x] = 0

    total0 = sum(sum(row) for row in rho)
    total_final = total0
    cons_err_max = 0
    core_leak = 0
    blocked_total = 0
    redirected_total = 0

    shell_means: list[float] = []
    far_means: list[float] = []

    for _tick in range(ticks):
        delta = [[0 for _ in range(n)] for _ in range(n)]

        for y in range(n):
            for x in range(n):
                if mode == "void" and core[y][x]:
                    continue

                v = rho[y][x]
                if v <= 0:
                    continue

                dx = x - cx
                dy = y - cy
                sx, sy = inward_step(dx, dy)
                if sx == 0 and sy == 0:
                    continue

                move = v // move_div
                if move <= 0:
                    continue

                tx = x + sx
                ty = y + sy
                if not in_bounds(n, tx, ty):
                    continue

                if mode == "void" and core[ty][tx]:
                    # Inward transport is blocked by void geometry.
                    blocked_total += move

                    (t1x, t1y), (t2x, t2y) = tangential_dirs(sx, sy)
                    a1x, a1y = x + t1x, y + t1y
                    a2x, a2y = x + t2x, y + t2y

                    valid_targets: list[tuple[int, int]] = []
                    if in_bounds(n, a1x, a1y) and not core[a1y][a1x]:
                        valid_targets.append((a1x, a1y))
                    if in_bounds(n, a2x, a2y) and not core[a2y][a2x]:
                        valid_targets.append((a2x, a2y))

                    if valid_targets:
                        # Conservation: move STE from source into tangential neighbors.
                        delta[y][x] -= move
                        q = move // len(valid_targets)
                        rem = move % len(valid_targets)
                        for idx, (nx, ny) in enumerate(valid_targets):
                            d = q + (1 if idx < rem else 0)
                            delta[ny][nx] += d
                            redirected_total += d
                    # If no valid tangential target, keep inventory in place.
                else:
                    # Regular inward transport.
                    delta[y][x] -= move
                    delta[ty][tx] += move

        for y in range(n):
            for x in range(n):
                if mode == "void" and core[y][x]:
                    rho[y][x] = 0
                else:
                    rho[y][x] += delta[y][x]
                    if rho[y][x] < 0:
                        # Guard against arithmetic bugs; this should not happen with inventory-gated moves.
                        rho[y][x] = 0

        if mode == "void":
            for y in range(n):
                for x in range(n):
                    if core[y][x] and rho[y][x] != 0:
                        core_leak += 1

        total = sum(sum(row) for row in rho)
        total_final = total
        err = abs(total - total0)
        if err > cons_err_max:
            cons_err_max = err

        shell_mean = sum(rho[y][x] for x, y in boundary_cells) / max(1, len(boundary_cells))
        far_mean = sum(rho[y][x] for x, y in far_cells) / max(1, len(far_cells))
        shell_means.append(shell_mean)
        far_means.append(far_mean)

    shell_peak = max(shell_means) if shell_means else 0.0
    far_peak = max(far_means) if far_means else 0.0
    ratio = shell_peak / far_peak if far_peak > 0 else float("inf")

    latency = None
    for idx, (s, f) in enumerate(zip(shell_means, far_means)):
        if s > 1.05 * f:
            latency = idx
            break

    redir_ratio = redirected_total / blocked_total if blocked_total > 0 else 0.0

    return RunMetrics(
        mode=mode,
        ticks=ticks,
        grid_size=n,
        core_radius=core_r,
        total_ste_initial=total0,
        total_ste_final=total_final,
        conservation_error_max=cons_err_max,
        core_leak_samples=core_leak,
        blocked_total=blocked_total,
        redirected_total=redirected_total,
        redirection_ratio=redir_ratio,
        shell_peak_mean=shell_peak,
        far_peak_mean=far_peak,
        shell_to_far_peak_ratio=ratio,
        shell_emergence_latency=latency,
    )


def to_dict(m: RunMetrics) -> dict:
    return {
        "mode": m.mode,
        "ticks": m.ticks,
        "grid_size": m.grid_size,
        "core_radius": m.core_radius,
        "total_ste_initial": m.total_ste_initial,
        "total_ste_final": m.total_ste_final,
        "conservation_error_max": m.conservation_error_max,
        "core_leak_samples": m.core_leak_samples,
        "blocked_total": m.blocked_total,
        "redirected_total": m.redirected_total,
        "redirection_ratio": m.redirection_ratio,
        "shell_peak_mean": m.shell_peak_mean,
        "far_peak_mean": m.far_peak_mean,
        "shell_to_far_peak_ratio": m.shell_to_far_peak_ratio,
        "shell_emergence_latency": m.shell_emergence_latency,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Clean-room ontology check: one conserved STE field with geometry-forced boundary dynamics")
    ap.add_argument("--ticks", type=int, default=600)
    ap.add_argument("--grid", type=int, default=129)
    ap.add_argument("--core-radius", type=int, default=12)
    ap.add_argument("--move-div", type=int, default=24, help="Transport divisor; larger means slower flow")
    ap.add_argument("--out", default="reports/clean_room/ontology_check.json")
    args = ap.parse_args()

    control = run_case("control", ticks=args.ticks, n=args.grid, core_r=args.core_radius, move_div=args.move_div)
    void = run_case("void", ticks=args.ticks, n=args.grid, core_r=args.core_radius, move_div=args.move_div)

    acceptance = {
        "conservation": void.conservation_error_max == 0,
        "no_core_leak": void.core_leak_samples == 0,
        "redirection_effective": void.redirection_ratio >= 0.95,
        "shell_emerges": (void.shell_emergence_latency is not None) and (void.shell_to_far_peak_ratio > 1.0),
    }
    acceptance["overall_pass"] = all(acceptance.values())

    payload = {
        "model_contract": {
            "single_substance": "STE",
            "primary_state": "density",
            "deterministic": True,
            "integer_transport": True,
            "geometry_forced_boundary": True,
        },
        "control": to_dict(control),
        "void": to_dict(void),
        "acceptance": acceptance,
    }

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    print(json.dumps(payload, indent=2))
    print(f"wrote={out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
