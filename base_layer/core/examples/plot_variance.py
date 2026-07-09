#!/usr/bin/env python3
"""Plot LWMA difficulty variance from `variance.csv`.

The CSV is produced by the `variance` experiment in the `lwma_sim` example
(`cargo run -p tari_core --example lwma_sim -- variance`). It has two `#` comment
header lines followed by one data row per block window:

    window,mean,median,stddev,cv_pct,p5,p95,min,max,<diff_0>,<diff_1>,...,<diff_N>

Block solve times are drawn from an exponential distribution (a Poisson mining
process), so the only variable is the block window. Two figures are produced:

  * difficulty_response.png — one line per window, difficulty (or % deviation
    from target) as blocks are added: the raw jitter.
  * spread_vs_window.png    — the headline result: coefficient of variation and
    the p5/mean/p95 deviation band versus block window (responsiveness vs
    stability tradeoff).

A legacy CSV (`window,<diffs...>` with no stats columns) is still handled.

Usage:
    python3 plot_variance.py [variance.csv] [--raw] [--out-dir plots]
                             [--windows 45 90] [--target 100000000] [--show]
"""
import argparse
import csv
import os

import matplotlib
# Use a non-interactive backend by default so the script saves PNGs headlessly
# (e.g. over SSH / in CI) without needing a display.
if not os.environ.get("DISPLAY") and os.name != "nt":
    matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

TARGET = 100_000_000  # Difficulty::from_u64(100_000_000) used in the experiment

# Number of leading stat columns in the current CSV format:
# window,mean,median,stddev,cv_pct,p5,p95,min,max
STAT_COLS = 9


def read_columns_header(path):
    """Return the parsed `# columns:` header list, or None if absent (legacy)."""
    with open(path, newline="") as f:
        for line in f:
            line = line.strip()
            if not line.startswith("#"):
                break
            if "columns:" in line:
                cols = line.split("columns:", 1)[1].strip()
                return [c.strip() for c in cols.split(",")]
    return None


def load(path):
    """Return (rows, is_new_format).

    Each row is a dict: window, difficulties, and — for the new format — the
    precomputed mean/median/stddev/cv_pct/p5/p95/min/max stats.
    """
    header = read_columns_header(path)
    is_new = header is not None and "stddev" in header
    rows = []
    with open(path, newline="") as f:
        for raw in csv.reader(f):
            if not raw or raw[0].lstrip().startswith("#"):
                continue
            # Tolerate a trailing empty field from a trailing comma.
            cells = [c for c in raw if c.strip() != ""]
            if len(cells) < 2:
                continue
            window = int(cells[0])
            if is_new and len(cells) >= STAT_COLS:
                rows.append({
                    "window": window,
                    "mean": float(cells[1]),
                    "median": float(cells[2]),
                    "stddev": float(cells[3]),
                    "cv_pct": float(cells[4]),
                    "p5": float(cells[5]),
                    "p95": float(cells[6]),
                    "min": float(cells[7]),
                    "max": float(cells[8]),
                    "difficulties": [int(c) for c in cells[STAT_COLS:]],
                })
            else:
                # Legacy format: window followed by raw difficulties only.
                rows.append({
                    "window": window,
                    "difficulties": [int(c) for c in cells[1:]],
                })
    rows.sort(key=lambda r: r["window"])
    return rows, is_new


def plot_response(rows, args):
    """Time series of difficulty (or % deviation) per block window."""
    plt.figure(figsize=(12, 6))
    for r in rows:
        if args.windows is not None and r["window"] not in args.windows:
            continue
        diffs = r["difficulties"]
        if not diffs:
            continue
        if args.raw:
            y = diffs
        else:
            y = [(d / args.target - 1.0) * 100.0 for d in diffs]
        plt.plot(range(len(y)), y, label=f"window {r['window']}", linewidth=0.8)

    plt.title("LWMA difficulty response (exponential/Poisson block times)")
    plt.xlabel("block number")
    if args.raw:
        plt.ylabel("difficulty")
        plt.axhline(args.target, color="black", linestyle="--", linewidth=0.8, label="target")
    else:
        plt.ylabel("deviation from target (%)")
        plt.axhline(0, color="black", linestyle="--", linewidth=0.8)
    plt.legend(fontsize=8, ncol=2)
    plt.grid(True, alpha=0.3)
    plt.tight_layout()

    out = os.path.join(args.out_dir, "difficulty_response.png")
    plt.savefig(out, dpi=130)
    return out


def _stats_for(rows, target):
    """Return per-window spread stats as % deviation from target, computing them
    from the raw difficulties if the CSV didn't carry precomputed columns."""
    windows, cv, p5, mean, p95 = [], [], [], [], []
    for r in rows:
        diffs = r["difficulties"]
        if "cv_pct" in r:
            m, sd = r["mean"], r["stddev"]
            lo, hi = r["p5"], r["p95"]
            cvp = r["cv_pct"]
        elif diffs:
            n = len(diffs)
            m = sum(diffs) / n
            sd = (sum((d - m) ** 2 for d in diffs) / n) ** 0.5
            s = sorted(diffs)
            lo = s[max(0, int(0.05 * n) - 1)]
            hi = s[min(n - 1, int(0.95 * n))]
            cvp = sd / m * 100.0 if m else 0.0
        else:
            continue
        windows.append(r["window"])
        cv.append(cvp)
        p5.append((lo / target - 1.0) * 100.0)
        mean.append((m / target - 1.0) * 100.0)
        p95.append((hi / target - 1.0) * 100.0)
    return windows, cv, p5, mean, p95


def plot_spread(rows, args):
    """Summary: CV% and the p5/mean/p95 deviation band versus block window."""
    windows, cv, p5, mean, p95 = _stats_for(rows, args.target)
    if not windows:
        return None

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(13, 5))

    ax1.plot(windows, cv, marker="o", color="tab:red")
    ax1.set_title("Stability: coefficient of variation vs window")
    ax1.set_xlabel("block window")
    ax1.set_ylabel("difficulty CV (%)")
    ax1.grid(True, alpha=0.3)
    for x, y in zip(windows, cv):
        ax1.annotate(f"{y:.1f}%", (x, y), textcoords="offset points",
                     xytext=(0, 6), fontsize=8, ha="center")

    ax2.fill_between(windows, p5, p95, alpha=0.2, color="tab:blue",
                     label="p5–p95 band")
    ax2.plot(windows, p95, marker="^", color="tab:blue", linewidth=0.9, label="p95")
    ax2.plot(windows, mean, marker="o", color="black", linewidth=0.9, label="mean")
    ax2.plot(windows, p5, marker="v", color="tab:blue", linewidth=0.9, label="p5")
    ax2.axhline(0, color="black", linestyle="--", linewidth=0.8)
    ax2.set_title("Spread: difficulty deviation band vs window")
    ax2.set_xlabel("block window")
    ax2.set_ylabel("deviation from target (%)")
    ax2.legend(fontsize=8)
    ax2.grid(True, alpha=0.3)

    fig.suptitle("LWMA responsiveness vs stability")
    fig.tight_layout()

    out = os.path.join(args.out_dir, "spread_vs_window.png")
    fig.savefig(out, dpi=130)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("csv", nargs="?", default="variance.csv")
    ap.add_argument("--raw", action="store_true",
                    help="plot raw difficulty instead of %% deviation from target")
    ap.add_argument("--out-dir", default="plots")
    ap.add_argument("--windows", type=int, nargs="+", default=None,
                    help="only plot these block windows, e.g. --windows 45 90")
    ap.add_argument("--target", type=int, default=TARGET,
                    help="target difficulty the experiment held constant")
    ap.add_argument("--show", action="store_true", help="also display the figures")
    args = ap.parse_args()

    if not os.path.exists(args.csv):
        raise SystemExit(
            f"{args.csv} not found. Generate it first with:\n"
            f"    cargo run -p tari_core --example lwma_sim -- variance --out <dir>\n"
            f"then point this script at <dir>/variance.csv"
        )

    rows, is_new = load(args.csv)
    if not rows:
        raise SystemExit(f"no data rows found in {args.csv}")
    os.makedirs(args.out_dir, exist_ok=True)

    written = [plot_response(rows, args)]
    spread = plot_spread(rows, args)
    if spread:
        written.append(spread)

    for out in written:
        print(f"wrote {out}")
    print(f"parsed {len(rows)} window(s) from {args.csv} "
          f"({'new' if is_new else 'legacy'} format)")

    if args.show:
        plt.show()
    plt.close("all")


if __name__ == "__main__":
    main()
