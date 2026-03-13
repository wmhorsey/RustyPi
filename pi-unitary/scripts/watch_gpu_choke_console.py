#!/usr/bin/env python3
"""
Console-based GPU choke simulation watcher.

Monitors decoded choke node CSV files and prints live phase distribution updates.
Run this while a GPU simulation is running to see the evolution in the terminal.

Usage:
    python scripts/watch_gpu_choke_console.py --csv reports/audit_gpu_scale_probe_smoke3/scale_512-1773430469217/decoded_choke_nodes.csv
"""

from __future__ import annotations

import argparse
import csv
import time
from collections import defaultdict
from pathlib import Path


PHASES = ["free", "formation", "liftoff", "coherence", "drift", "dissolution"]


class ConsoleChokeWatcher:
    def __init__(self, csv_path: Path, update_interval: float = 2.0):
        self.csv_path = csv_path
        self.update_interval = update_interval
        self.last_mtime = 0
        self.data_cache = []
        self.max_tick_seen = 0
        self.last_display_tick = -1

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
                    if tick > self.max_tick_seen:
                        data.append({
                            'tick': tick,
                            'sequence': int(row['sequence']),
                            'node': int(row['node']),
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
                        })
                        if tick > self.max_tick_seen:
                            self.max_tick_seen = tick
        except (IOError, ValueError, KeyError) as e:
            print(f"Error reading CSV: {e}")
            return []

        self.last_mtime = current_mtime
        return data

    def display_update(self):
        """Display current phase distribution."""
        if not self.data_cache:
            return

        # Get data for the latest tick
        latest_tick = self.max_tick_seen
        if latest_tick == self.last_display_tick:
            return  # Already displayed this tick

        current_data = [r for r in self.data_cache if r['tick'] == latest_tick]
        if not current_data:
            return

        # Count phases
        phase_counts = defaultdict(int)
        total_nodes = len(current_data)

        for record in current_data:
            phase_counts[record['phase']] += 1

        # Calculate percentages
        phase_pcts = {}
        for phase in PHASES:
            count = phase_counts.get(phase, 0)
            pct = (count / total_nodes) * 100 if total_nodes > 0 else 0
            phase_pcts[phase] = pct

        # Display
        print(f"\nTick {latest_tick} - Total Nodes: {total_nodes}")
        print("-" * 50)
        for phase in PHASES:
            count = phase_counts.get(phase, 0)
            pct = phase_pcts[phase]
            bar = "█" * int(pct / 2)  # Simple bar chart
            print("12")

        # Summary stats
        active_nodes = total_nodes - phase_counts.get('free', 0)
        active_pct = (active_nodes / total_nodes) * 100 if total_nodes > 0 else 0
        print(".1f"
        self.last_display_tick = latest_tick

    def run(self):
        """Start the console-based monitoring."""
        print(f"Watching {self.csv_path}")
        print("Press Ctrl+C to stop watching.")
        print()

        try:
            while True:
                new_data = self.load_data()
                if new_data:
                    self.data_cache.extend(new_data)
                    self.display_update()

                time.sleep(self.update_interval)

        except KeyboardInterrupt:
            print("\nStopped watching.")


def main():
    parser = argparse.ArgumentParser(
        description="Console-based GPU choke simulation watcher",
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
        default=2.0,
        help='Update interval in seconds (default: 2.0)'
    )

    args = parser.parse_args()

    if not args.csv.exists():
        print(f"Error: CSV file {args.csv} does not exist")
        print("Make sure the GPU simulation is running and has generated decoded output.")
        return 1

    watcher = ConsoleChokeWatcher(args.csv, args.interval)
    watcher.run()
    return 0


if __name__ == '__main__':
    import sys
    sys.exit(main())