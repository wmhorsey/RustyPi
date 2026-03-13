#!/usr/bin/env python3
"""
Real-time GPU choke simulation watcher.

Monitors decoded choke node CSV files and displays live phase distribution plots.
Run this while a GPU simulation is running to see the evolution in real-time.

Usage:
    python scripts/watch_gpu_choke.py --csv reports/audit_gpu_scale_probe_smoke3/scale_512-1773430469217/decoded_choke_nodes.csv

Dependencies:
    pip install matplotlib numpy
"""

from __future__ import annotations

import argparse
import csv
import time
from collections import defaultdict
from pathlib import Path

try:
    import matplotlib
    import matplotlib.pyplot as plt
    import matplotlib.animation as animation
    from matplotlib import style
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False
    print("Warning: matplotlib not available. Install with: pip install matplotlib")

# Use a clean style
if HAS_MATPLOTLIB:
    style.use('seaborn-v0_8')

# Phase colors matching the existing visualization
PHASE_COLORS = {
    "free": "#4c78a8",
    "formation": "#f58518",
    "liftoff": "#e45756",
    "coherence": "#72b7b2",
    "drift": "#54a24b",
    "dissolution": "#b279a2",
}

PHASES = ["free", "formation", "liftoff", "coherence", "drift", "dissolution"]


class ChokeWatcher:
    def __init__(self, csv_path: Path, update_interval: float = 1.0):
        if not HAS_MATPLOTLIB:
            raise ImportError("matplotlib is required for real-time watching")

        self.csv_path = csv_path
        self.update_interval = update_interval
        self.last_mtime = 0
        self.data_cache = []
        self.seen_rows = set()
        self.max_tick_seen = 0

        # Set up the plot
        self.fig, (self.ax1, self.ax2) = plt.subplots(2, 1, figsize=(12, 8))
        self.fig.suptitle(f'GPU Choke Simulation: {csv_path.name}', fontsize=14)

        # Phase distribution over time
        self.ax1.set_title('Phase Distribution Over Time')
        self.ax1.set_xlabel('Tick')
        self.ax1.set_ylabel('Node Count')
        self.lines = {}
        for phase in PHASES:
            line, = self.ax1.plot([], [], label=phase, color=PHASE_COLORS[phase], linewidth=2)
            self.lines[phase] = line
        self.ax1.legend(loc='upper right')
        self.ax1.grid(True, alpha=0.3)

        # Current phase pie chart
        self.ax2.set_title('Current Phase Distribution')
        self.pie_wedges = None
        self.pie_labels = None

        plt.tight_layout()

    def load_data(self) -> list[dict]:
        """Load data from CSV file, returning new records since last load."""
        if not self.csv_path.exists():
            return []

        current_mtime = self.csv_path.stat().st_mtime
        if current_mtime <= self.last_mtime:
            return []  # No new data

        data = []
        try:
            with self.csv_path.open('r', encoding='utf-8', newline='') as f:
                reader = csv.DictReader(f)
                for row in reader:
                    tick = int(row['tick'])
                    sequence = int(row['sequence'])
                    node = int(row['node'])
                    row_key = (tick, sequence, node)
                    if row_key in self.seen_rows:
                        continue

                    record = {
                        'tick': tick,
                        'sequence': sequence,
                        'node': node,
                        'phase_tick': int(row['phase_tick']),
                        'coherence': int(row['coherence']),
                        'energy': int(row['energy']),
                        'shell_ring_ticks': int(row['shell_ring_ticks']),
                        'spin_bias': int(row['spin_bias']),
                        'phase_id': int(row['phase_id']),
                        'phase': row['phase'].strip().lower(),
                        'pathway_id': int(row['pathway_id']),
                        'pathway': row['pathway'].strip().lower(),
                        'drive': int(row['drive'])
                    }
                    data.append(record)
                    self.seen_rows.add(row_key)
                    if tick > self.max_tick_seen:
                        self.max_tick_seen = tick
        except (IOError, ValueError, KeyError) as e:
            print(f"Error reading CSV: {e}")
            return []

        self.last_mtime = current_mtime
        return data

    def update_plot(self, frame):
        """Update function for matplotlib animation."""
        new_data = self.load_data()
        if not new_data:
            return self.lines.values(), []

        # Add new data to cache
        self.data_cache.extend(new_data)

        # Group by tick for time series
        by_tick = defaultdict(lambda: defaultdict(int))
        current_tick_data = defaultdict(int)

        for record in self.data_cache:
            tick = record['tick']
            phase = record['phase']
            by_tick[tick][phase] += 1

            # For current distribution, use the latest tick
            if tick == self.max_tick_seen:
                current_tick_data[phase] += 1

        # Update time series lines
        ticks = sorted(by_tick.keys())
        if ticks:
            for phase in PHASES:
                counts = [by_tick[tick].get(phase, 0) for tick in ticks]
                self.lines[phase].set_data(ticks, counts)

            self.ax1.set_xlim(0, max(ticks) + 1)
            max_count = max((max(phase_map.values()) if phase_map else 0) for phase_map in by_tick.values())
            self.ax1.set_ylim(0, max_count * 1.1)

        # Update pie chart
        if current_tick_data:
            self.ax2.clear()
            self.ax2.set_title(f'Current Phase Distribution (Tick {self.max_tick_seen})')

            sizes = [current_tick_data.get(phase, 0) for phase in PHASES]
            colors = [PHASE_COLORS[phase] for phase in PHASES]
            labels = [f'{phase}\n{size}' for phase, size in zip(PHASES, sizes)]

            if sum(sizes) > 0:
                wedges, texts, autotexts = self.ax2.pie(
                    sizes,
                    labels=labels if sum(sizes) > 0 else None,
                    colors=colors,
                    autopct='%1.1f%%',
                    startangle=90
                )
                plt.setp(autotexts, size=8, weight="bold")
                plt.setp(texts, size=8)

        return list(self.lines.values())

    def run(self):
        """Start the real-time visualization."""
        print(f"Watching {self.csv_path}")
        print("Close the plot window to stop watching.")

        ani = animation.FuncAnimation(
            self.fig,
            self.update_plot,
            interval=self.update_interval * 1000,  # Convert to milliseconds
            cache_frame_data=False
        )

        plt.show()


def main():
    if not HAS_MATPLOTLIB:
        print("Error: matplotlib is required but not installed.")
        print("Install with: pip install matplotlib")
        return 1

    parser = argparse.ArgumentParser(
        description="Real-time GPU choke simulation watcher",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        '--csv',
        type=Path,
        required=True,
        help='Path to decoded choke nodes CSV file'
    )
    parser.add_argument(
        '--interval',
        type=float,
        default=1.0,
        help='Update interval in seconds (default: 1.0)'
    )

    args = parser.parse_args()

    if not args.csv.exists():
        print(f"Error: CSV file {args.csv} does not exist")
        print("Make sure the GPU simulation is running and has generated decoded output.")
        return 1

    watcher = ChokeWatcher(args.csv, args.interval)
    watcher.run()
    return 0


if __name__ == '__main__':
    import sys
    sys.exit(main())