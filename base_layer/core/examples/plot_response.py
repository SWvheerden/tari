#!/usr/bin/env python3
"""Plot LWMA step-response from `response_noisy.csv` / `response_noiseless.csv`.

The CSV is produced by the `response` experiment in the `lwma_sim` example
(`cargo run -p tari_core --example lwma_sim -- response --noise`). It has two `#`
comment header lines followed by one row per (hash-rate step, block window):

    noisy,window,hash_change,settle_mean,settle_median,settle_p95,non_converged,<diff_0>,<diff_1>,...

`hash_change` is the multiplier applied to the equilibrium difficulty by the
hash-rate step, so the new equilibrium is `base * hash_change`. The trajectory is
one representative trial's difficulty path after the step.

Two figures are produced:

  * settle_vs_hashchange.png — blocks to settle (median, with the p95 tail) versus
    the size of the hash-rate step, one line per block window.
  * response_trajectories.png — per-window: the difficulty converging to the new
    equilibrium (% deviation) after the step, for a curated set of steps.

Usage:
    python3 plot_response.py [response_noisy.csv] [--out-dir plots]
                             [--base 100000] [--tol 0.01] [--show]
"""
import argparse
import csv
import os

import matplotlib
if not os.environ.get("DISPLAY") and os.name != "nt":
    matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

BASE = 100_000  # Difficulty::from_u64(100_000) used as the pre-step equilibrium
STAT_COLS = 7   # noisy,window,hash_change,settle_mean,settle_median,settle_p95,non_converged
# Curated subset of steps drawn as trajectories (kept small so the plot stays readable).
TRAJECTORY_STEPS = [0.5, 0.7, 0.9, 1.1, 1.5, 2.0, 5.0]
U64_MAX = 18_446_744_073_709_551_615  # sentinel written for a non-converged cell


def load(path):
    rows = []
    with open(path, newline="") as f:
        for raw in csv.reader(f):
            if not raw or raw[0].lstrip().startswith("#"):
                continue
            cells = [c for c in raw if c.strip() != ""]
            if len(cells) < STAT_COLS:
                continue
            rows.append({
                "noisy": cells[0].strip().lower() == "true",
                "window": int(cells[1]),
                "hash_change": float(cells[2]),
                "settle_mean": float(cells[3]),
                "settle_median": int(cells[4]),
                "settle_p95": int(cells[5]),
                "non_converged": int(cells[6]),
                "trajectory": [int(c) for c in cells[STAT_COLS:]],
            })
    return rows


def _converged(value):
    """A settle cell of u64::MAX means the trial never converged."""
    return value if value != U64_MAX else None


def plot_settle(rows, args):
    """Blocks-to-settle (median + p95) versus hash-rate step, one line per window."""
    windows = sorted({r["window"] for r in rows})
    plt.figure(figsize=(11, 6))
    cmap = plt.get_cmap("tab10")
    for i, w in enumerate(windows):
        pts = sorted((r for r in rows if r["window"] == w), key=lambda r: r["hash_change"])
        xs = [r["hash_change"] for r in pts]
        median = [_converged(r["settle_median"]) for r in pts]
        p95 = [_converged(r["settle_p95"]) for r in pts]
        color = cmap(i)
        plt.plot(xs, median, marker="o", color=color, linewidth=1.4, label=f"window {w} (median)")
        plt.plot(xs, p95, marker="^", color=color, linewidth=0.8, linestyle="--", alpha=0.7,
                 label=f"window {w} (p95)")

    plt.axvline(1.0, color="black", linestyle=":", linewidth=0.8)
    plt.text(1.0, plt.ylim()[1] * 0.97, " no change", fontsize=8, va="top")
    plt.title("LWMA settling time vs hash-rate step size")
    plt.xlabel("hash-rate step (equilibrium difficulty multiplier)")
    plt.ylabel("blocks to settle")
    plt.legend(fontsize=8, ncol=max(1, len(windows)))
    plt.grid(True, alpha=0.3)
    plt.tight_layout()
    out = os.path.join(args.out_dir, "settle_vs_hashchange.png")
    plt.savefig(out, dpi=130)
    return out


def plot_trajectories(rows, args):
    """Per-window convergence curves: % deviation from the new equilibrium vs block."""
    windows = sorted({r["window"] for r in rows})
    if not windows:
        return None
    fig, axes = plt.subplots(1, len(windows), figsize=(7 * len(windows), 5), squeeze=False)
    cmap = plt.get_cmap("viridis")
    steps = sorted(TRAJECTORY_STEPS)

    for ax, w in zip(axes[0], windows):
        by_step = {r["hash_change"]: r for r in rows if r["window"] == w}
        for j, step in enumerate(steps):
            r = by_step.get(step)
            if r is None or not r["trajectory"]:
                continue
            equilibrium = args.base * step
            y = [(d / equilibrium - 1.0) * 100.0 for d in r["trajectory"]]
            ax.plot(range(len(y)), y, linewidth=0.8, color=cmap(j / max(1, len(steps) - 1)),
                    label=f"x{step:g}")
        ax.axhline(0, color="black", linestyle="--", linewidth=0.8)
        ax.axhspan(-args.tol * 100, args.tol * 100, color="green", alpha=0.12,
                   label=f"±{args.tol * 100:g}% band")
        ax.set_title(f"window {w}: convergence to new equilibrium")
        ax.set_xlabel("block after step")
        ax.set_ylabel("deviation from new equilibrium (%)")
        ax.grid(True, alpha=0.3)
        ax.legend(fontsize=7, ncol=2)

    fig.suptitle("LWMA step response (representative trial)")
    fig.tight_layout()
    out = os.path.join(args.out_dir, "response_trajectories.png")
    fig.savefig(out, dpi=130)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv", nargs="?", default="response_noisy.csv")
    ap.add_argument("--out-dir", default="plots")
    ap.add_argument("--base", type=int, default=BASE,
                    help="pre-step equilibrium difficulty the experiment used")
    ap.add_argument("--tol", type=float, default=0.01, help="settle tolerance band to shade")
    ap.add_argument("--show", action="store_true", help="also display the figures")
    args = ap.parse_args()

    if not os.path.exists(args.csv):
        raise SystemExit(
            f"{args.csv} not found. Generate it first with:\n"
            f"    cargo run -p tari_core --example lwma_sim -- response --noise --out <dir>\n"
            f"then point this script at <dir>/response_noisy.csv"
        )

    rows = load(args.csv)
    if not rows:
        raise SystemExit(f"no data rows found in {args.csv}")
    os.makedirs(args.out_dir, exist_ok=True)

    written = [plot_settle(rows, args)]
    traj = plot_trajectories(rows, args)
    if traj:
        written.append(traj)

    for out in written:
        print(f"wrote {out}")
    mode = "noisy" if rows[0]["noisy"] else "noise-free"
    nonconv = sum(r["non_converged"] for r in rows)
    print(f"parsed {len(rows)} step/window rows from {args.csv} ({mode}); "
          f"{nonconv} non-converged trial(s) total")

    if args.show:
        plt.show()
    plt.close("all")


if __name__ == "__main__":
    main()
