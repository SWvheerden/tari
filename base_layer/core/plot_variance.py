#!/usr/bin/env python3
"""Plot LWMA difficulty variance from `variance.csv`.

The CSV is produced by the `variance_lwma` test in `lwma_diff.rs`. Each row is:

    <block_window>,<diff_0>,<diff_1>,...,<diff_N>

Block solve times are drawn from an exponential distribution (a Poisson mining
process), so the only variable is the block window. We produce a single figure
with one line per block window showing how the difficulty deviates from the
target as blocks are added.

Usage:
    python3 plot_variance.py [variance.csv] [--raw] [--out-dir plots] [--windows 45 90]
"""
import argparse
import csv
import os

import matplotlib.pyplot as plt

TARGET = 100_000_000  # Difficulty::from_u64(100_000_000) used in the test


def load(path):
    # list of (window, [difficulties])
    series = []
    with open(path, newline="") as f:
        for row in csv.reader(f):
            # tolerate trailing empty field from the trailing comma
            cells = [c for c in row if c.strip() != ""]
            if len(cells) < 2:
                continue
            window = int(cells[0])
            diffs = [int(c) for c in cells[1:]]
            series.append((window, diffs))
    return series


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv", nargs="?", default="variance.csv")
    ap.add_argument("--raw", action="store_true",
                    help="plot raw difficulty instead of %% deviation from target")
    ap.add_argument("--out-dir", default="plots")
    ap.add_argument("--windows", type=int, nargs="+", default=None,
                    help="only plot these block windows, e.g. --windows 45 90")
    args = ap.parse_args()

    series = load(args.csv)
    os.makedirs(args.out_dir, exist_ok=True)

    plt.figure(figsize=(12, 6))
    for window, diffs in sorted(series):
        if args.windows is not None and window not in args.windows:
            continue
        if args.raw:
            y = diffs
        else:
            y = [(d / TARGET - 1.0) * 100.0 for d in diffs]
        plt.plot(range(len(y)), y, label=f"window {window}", linewidth=0.8)

    plt.title("LWMA difficulty response (exponential/Poisson block times)")
    plt.xlabel("block number")
    if args.raw:
        plt.ylabel("difficulty")
        plt.axhline(TARGET, color="black", linestyle="--", linewidth=0.8, label="target")
    else:
        plt.ylabel("deviation from target (%)")
        plt.axhline(0, color="black", linestyle="--", linewidth=0.8)
    plt.legend(fontsize=8, ncol=2)
    plt.grid(True, alpha=0.3)
    plt.tight_layout()

    out = os.path.join(args.out_dir, "difficulty_response.png")
    plt.savefig(out, dpi=130)
    plt.close()
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
