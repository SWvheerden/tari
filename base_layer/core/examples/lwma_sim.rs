// Copyright 2022 The Tari Project
// SPDX-License-Identifier: BSD-3-Clause

//! Simulation harness for the LWMA difficulty-adjustment algorithm.
//!
//! This replaces the old `response_test` / `variance_test` functions that used to live (and
//! `panic!`) inside `lwma_diff.rs`'s test module. It is a standalone, reproducible experiment:
//! nothing here runs during `cargo test`.
//!
//! Two experiments are provided:
//!
//! * **response** — warm the LWMA to steady state, apply a step change in hash rate, and measure
//!   how many blocks it takes to settle within a tolerance of the new equilibrium difficulty. Can
//!   be run noise-free (the canonical impulse response) or with exponentially distributed block
//!   times averaged over many trials (what settling actually looks like on a live, noisy chain).
//!
//! * **variance** — hold the true hash rate constant, drive block times from an exponential
//!   distribution (real proof-of-work inter-arrival times), and measure how much the LWMA-produced
//!   difficulty jitters. Reports standard deviation and percentiles, not just min/max.
//!
//! Run it with, e.g.:
//!
//! ```text
//! cargo run -p tari_core --example lwma_sim -- all
//! cargo run -p tari_core --example lwma_sim -- response --no-noise
//! cargo run -p tari_core --example lwma_sim -- response --noise --trials 200
//! cargo run -p tari_core --example lwma_sim -- variance --seed 7 --samples 20000
//! ```
//!
//! CSV output is written to `<tmp>/lwma_sim/` (override with `--out <dir>`), never the CWD.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rand::{rngs::StdRng, RngExt, SeedableRng};
use tari_core::proof_of_work::lwma_diff::LinearWeightedMovingAverage;
use tari_transaction_components::tari_proof_of_work::{Difficulty, DifficultyAdjustment, MIN_DIFFICULTY};
use tari_utilities::epoch_time::EpochTime;

/// Target block time used for every simulation (arbitrary units, "seconds").
const TARGET_TIME: u64 = 100;
/// Block windows to sweep over.
//const BLOCK_WINDOWS: [usize; 7] = [30, 45, 60, 80, 90, 110, 150];
const BLOCK_WINDOWS: [usize; 2] = [45, 90];
/// Default settle tolerance: a block counts as settled once the (smoothed) difficulty is within
/// this fraction of the new equilibrium. Overridable with `--tol`.
const DEFAULT_TOLERANCE: f64 = 0.01;
/// Cap on response-trial length, as a multiple of the block window. The noise-free response settles
/// in ~1–3 windows; anything beyond this many is reported as non-converged rather than looped
/// forever. This bounds runtime and detects "can't hold the band under noise" quickly.
const MAX_RESPONSE_WINDOWS: usize = 60;
/// Fixed number of blocks recorded for the representative trajectory of every (step, window), so
/// all trajectories in the plot span the same x-range and can be compared directly. Settling-time
/// stats are still measured to the full cap above; only the recorded path is truncated/extended to
/// this length.
const TRAJECTORY_BLOCKS: usize = 600;

fn main() {
    let cfg = Config::from_args(std::env::args().skip(1));
    fs::create_dir_all(&cfg.out_dir).expect("create output dir");
    println!("Output directory: {}", cfg.out_dir.display());
    println!("Seed: {}\n", cfg.seed);

    match cfg.mode {
        Mode::Response => run_response(&cfg, cfg.noise),
        Mode::Variance => run_variance(&cfg, cfg.noise),
        Mode::All => {
            // Showcase both: the clean impulse response and the noisy, multi-trial version.
            run_response(&cfg, false);
            run_response(&cfg, true);
            run_variance(&cfg, true);
        },
    }
}

// -------------------------------------------------------------------------------------------------
// Response experiment
// -------------------------------------------------------------------------------------------------

fn run_response(cfg: &Config, noisy: bool) {
    // Separate files per mode so `all` (which runs both) never overwrites one with the other.
    let path = cfg
        .out_dir
        .join(if noisy { "response_noisy.csv" } else { "response_noiseless.csv" });
    let mut csv = String::from(
        "# response experiment: blocks to settle within tolerance of the new equilibrium after a \
         hash-rate step\n# columns: noisy,window,hash_change,settle_mean,settle_median,settle_p95,\
         non_converged,trajectory...\n",
    );

    // Noise-free: the geometric decay never re-leaves the band, so test the raw difficulty
    // (smooth = 1). Noisy: the raw difficulty jitters more than the band, so we test its trailing
    // moving average over the window — a low-pass estimate of where the algorithm has actually
    // settled. This adds ~window/2 blocks of lag, which is expected and comparable across windows.
    let trials = if noisy { cfg.trials } else { 1 };
    let tol = cfg.tol;

    println!(
        "=== response ({}) — {} trial(s), tol ±{:.1}%{} ===",
        if noisy { "noisy / exponential block times" } else { "deterministic / noise-free" },
        trials,
        tol * 100.0,
        if noisy { ", difficulty smoothed over the block window" } else { "" },
    );
    let base = Difficulty::from_u64(100_000).unwrap();

    // Decreasing then increasing hash-rate steps. `hash_change` is the multiplier applied to the
    // equilibrium difficulty (difficulty scales with hash rate).
    let steps = [
        ("reduce", &[0.9f64, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3][..]),
        ("increase", &[1.1, 1.3, 1.5, 1.7, 2.0, 2.1, 5.0][..]),
    ];

    for (label, hash_changes) in steps {
        println!("-- {label} hash rate --");
        for &hash_change in hash_changes {
            for &window in &BLOCK_WINDOWS {
                let mut settle = Vec::with_capacity(trials);
                let mut representative = Vec::new();
                for trial in 0..trials {
                    // Deterministic per (window, hash_change, trial) seed → fully reproducible.
                    let mut rng = StdRng::seed_from_u64(
                        cfg.seed ^ (window as u64) ^ (hash_change.to_bits()) ^ ((trial as u64) << 32),
                    );
                    let smooth = if noisy { window } else { 1 };
                    // Only the representative trial (0) records its path, at a fixed length so all
                    // trajectories line up; the rest record nothing and just contribute settle stats.
                    let record_len = if trial == 0 { TRAJECTORY_BLOCKS } else { 0 };
                    let (blocks, traj) =
                        response_trial(window, base, hash_change, noisy, tol, smooth, record_len, &mut rng);
                    settle.push(blocks);
                    if trial == 0 {
                        representative = traj;
                    }
                }

                let converged: Vec<u64> = settle.iter().filter_map(|b| b.map(|v| v as u64)).collect();
                let non_converged = settle.len() - converged.len();
                let (mean, median, p95) = if converged.is_empty() {
                    (f64::NAN, u64::MAX, u64::MAX)
                } else {
                    let mut sorted = converged.clone();
                    sorted.sort_unstable();
                    let mean = sorted.iter().sum::<u64>() as f64 / sorted.len() as f64;
                    (mean, percentile(&sorted, 0.50), percentile(&sorted, 0.95))
                };

                println!(
                    "  hash_change {hash_change:>4}, window {window:>3}: settle mean {mean:>7.1}, \
                     median {median:>5}, p95 {p95:>5}{}",
                    if non_converged > 0 {
                        format!(", non-converged {non_converged}/{trials}")
                    } else {
                        String::new()
                    },
                );

                let traj = representative
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                csv.push_str(&format!(
                    "{noisy},{window},{hash_change},{mean:.1},{median},{p95},{non_converged},{traj}\n"
                ));
            }
        }
    }

    fs::write(&path, csv).expect("write response csv");
    println!("wrote {}\n", path.display());
}

/// Run a single response trial. Returns `(blocks_to_settle, difficulty_trajectory)`, with
/// `blocks_to_settle == None` if the (smoothed) difficulty never reached the band within the cap.
///
/// `smooth` is the length of the trailing moving average tested against the band: `1` tests the raw
/// difficulty (right for the noise-free response); the block window low-passes the jitter under
/// noise so a well-defined settling point exists.
///
/// `record_len` is how many leading blocks of the difficulty path to record: pass `0` for the
/// throw-away trials whose path is not plotted (cheap), or `TRAJECTORY_BLOCKS` for the one
/// representative trial so every trajectory has the same length. Settle detection always runs to the
/// full cap regardless, so the stats are unaffected by `record_len`.
fn response_trial(
    window: usize,
    base: Difficulty,
    hash_change: f64,
    noisy: bool,
    tol: f64,
    smooth: usize,
    record_len: usize,
    rng: &mut StdRng,
) -> (Option<usize>, Vec<u64>) {
    let mut lwma = warm_up(window, base);
    let old_diff = lwma.get_difficulty().expect("warmed LWMA yields a difficulty");
    // New equilibrium difficulty after the hash-rate step (clamped to the valid range).
    let stop = clamp_difficulty((old_diff.as_u64() as f64 * hash_change).round() as u64);
    let stop_f = stop.as_u64() as f64;

    let mut time = warm_up_time(window);
    let mut cur = old_diff;
    let mut trajectory = Vec::with_capacity(record_len);
    // Trailing window of raw difficulties for the moving-average test.
    let mut trailing = std::collections::VecDeque::with_capacity(smooth);
    let mut trailing_sum = 0u128;
    let max_blocks = window * MAX_RESPONSE_WINDOWS;
    let mut settled_at = None;

    for block in 1..=max_blocks {
        // Mean solve time when mining at `cur` against the post-step hash rate:
        // target * (chosen difficulty / true equilibrium difficulty).
        let mean_time = TARGET_TIME as f64 * cur.as_u64() as f64 / stop_f;
        time += solve_time(rng, mean_time, noisy);
        lwma.add(EpochTime::from(time), cur).expect("add block");
        cur = match lwma.get_difficulty() {
            Some(d) => d,
            None => clamp_difficulty(0), // fell below MIN_DIFFICULTY; pin to the floor
        };
        if block <= record_len {
            trajectory.push(cur.as_u64());
        }

        trailing.push_back(cur.as_u64());
        trailing_sum += cur.as_u64() as u128;
        if trailing.len() > smooth {
            trailing_sum -= trailing.pop_front().unwrap() as u128;
        }
        // Only judge once the trailing window is full, so the average isn't biased by the step edge.
        if settled_at.is_none() && trailing.len() == smooth {
            let smoothed = trailing_sum as f64 / smooth as f64;
            if (smoothed / stop_f - 1.0).abs() <= tol {
                settled_at = Some(block);
            }
        }
        // Stop once we've both found the settle point and recorded the full fixed-length path.
        if settled_at.is_some() && block >= record_len {
            break;
        }
    }
    (settled_at, trajectory)
}

// -------------------------------------------------------------------------------------------------
// Variance experiment
// -------------------------------------------------------------------------------------------------

fn run_variance(cfg: &Config, noisy: bool) {
    let path = cfg.out_dir.join("variance.csv");
    let mut csv = String::from(
        "# variance experiment: steady-state difficulty jitter at constant true hash rate\n\
         # columns: window,mean,median,stddev,cv_pct,p5,p95,min,max,difficulties...\n",
    );

    let truth = Difficulty::from_u64(100_000_000).unwrap();
    let burn_in = cfg.samples / 10; // discard the warmup→random transient before measuring

    println!(
        "=== variance ({}) — {} samples, {} burn-in ===",
        if noisy { "noisy / exponential block times" } else { "deterministic / noise-free" },
        cfg.samples,
        burn_in,
    );

    for &window in &BLOCK_WINDOWS {
        let mut rng = StdRng::seed_from_u64(cfg.seed ^ (window as u64).wrapping_mul(0x9E37_79B9));
        let diffs = variance_run(window, truth, cfg.samples, burn_in, noisy, &mut rng);
        let s = Stats::from(&diffs);

        let pct = |v: f64| (v / truth.as_u64() as f64 - 1.0) * 100.0;
        println!(
            "  window {window:>3}: mean {:>12.1} ({:+.3}%), median {:>10} ({:+.3}%), stddev {:>10.1} \
             (cv {:.3}%), p5 {} ({:+.3}%), p95 {} ({:+.3}%)",
            s.mean,
            pct(s.mean),
            s.median,
            pct(s.median as f64),
            s.stddev,
            s.stddev / s.mean * 100.0,
            s.p5,
            pct(s.p5 as f64),
            s.p95,
            pct(s.p95 as f64),
        );

        let body = diffs.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        csv.push_str(&format!(
            "{window},{:.1},{},{:.1},{:.3},{},{},{},{},{body}\n",
            s.mean,
            s.median,
            s.stddev,
            s.stddev / s.mean * 100.0,
            s.p5,
            s.p95,
            s.min,
            s.max,
        ));
    }

    fs::write(&path, csv).expect("write variance.csv");
    println!("wrote {}\n", path.display());
}

/// Closed-loop steady-state run at constant true difficulty `truth`. Returns the difficulties the
/// LWMA produced, after discarding `burn_in` samples.
fn variance_run(
    window: usize,
    truth: Difficulty,
    samples: usize,
    burn_in: usize,
    noisy: bool,
    rng: &mut StdRng,
) -> Vec<u64> {
    let mut lwma = warm_up(window, truth);
    let mut time = warm_up_time(window);
    let mut cur = truth;

    let mut out = Vec::with_capacity(samples);
    for i in 0..burn_in + samples {
        // Mean solve time = target * (chosen difficulty / true difficulty).
        let mean_time = TARGET_TIME as f64 * cur.as_u64() as f64 / truth.as_u64() as f64;
        time += solve_time(rng, mean_time, noisy);
        lwma.add(EpochTime::from(time), cur).expect("add block");
        cur = lwma.get_difficulty().unwrap_or_else(|| clamp_difficulty(0));
        if i >= burn_in {
            out.push(cur.as_u64());
        }
    }
    out
}

// -------------------------------------------------------------------------------------------------
// Shared model helpers
// -------------------------------------------------------------------------------------------------

/// Warm the LWMA to a full window of constant-difficulty, on-target blocks.
fn warm_up(window: usize, difficulty: Difficulty) -> LinearWeightedMovingAverage {
    let mut lwma = LinearWeightedMovingAverage::new(window, TARGET_TIME).unwrap();
    let mut time = 0;
    while !lwma.is_full() {
        time += TARGET_TIME;
        lwma.add(EpochTime::from(time), difficulty).expect("warmup add");
    }
    lwma
}

/// The wall-clock time reached at the end of [`warm_up`], so post-warmup timestamps keep increasing.
fn warm_up_time(window: usize) -> u64 {
    // `warm_up` adds blocks until full: `window + 1` of them, each `TARGET_TIME` apart.
    (window as u64 + 1) * TARGET_TIME
}

/// Draw a block solve time (whole "seconds") for a block whose difficulty/hash-rate ratio makes its
/// expected solve time `mean_time`.
///
/// * `noisy = true`: sample from an exponential distribution — real proof-of-work block times are
///   the inter-arrival times of a Poisson process, so they are exponentially distributed around the
///   mean, not uniform noise. For `U ~ Uniform(0, 1]`, `-mean * ln(U)` is exponential with the
///   given mean.
/// * `noisy = false`: return the mean itself — the deterministic, noise-free response.
///
/// Floored at 1 so the LWMA never sees a zero or negative solve time.
fn solve_time(rng: &mut StdRng, mean_time: f64, noisy: bool) -> u64 {
    let t = if noisy {
        // `1.0 - random()` lands in (0, 1], excluding 0 so `ln` is finite.
        let u = 1.0 - rng.random::<f64>();
        -mean_time * u.ln()
    } else {
        mean_time
    };
    (t.round() as i64).max(1) as u64
}

/// Clamp a raw difficulty value into the algorithm's valid range.
fn clamp_difficulty(value: u64) -> Difficulty {
    Difficulty::from_u64(value.max(MIN_DIFFICULTY)).expect("clamped into range")
}

// -------------------------------------------------------------------------------------------------
// Statistics
// -------------------------------------------------------------------------------------------------

struct Stats {
    mean: f64,
    median: u64,
    stddev: f64,
    p5: u64,
    p95: u64,
    min: u64,
    max: u64,
}

impl Stats {
    fn from(values: &[u64]) -> Self {
        assert!(!values.is_empty(), "cannot summarise an empty sample");
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();
        // u128 accumulation to avoid overflow across many large difficulties.
        let sum: u128 = sorted.iter().map(|&v| v as u128).sum();
        let mean = sum as f64 / n as f64;
        let variance = sorted.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n as f64;
        Self {
            mean,
            median: percentile(&sorted, 0.50),
            stddev: variance.sqrt(),
            p5: percentile(&sorted, 0.05),
            p95: percentile(&sorted, 0.95),
            min: sorted[0],
            max: sorted[n - 1],
        }
    }
}

/// Nearest-rank percentile of an already-sorted slice. `p` in `[0, 1]`.
fn percentile(sorted: &[u64], p: f64) -> u64 {
    debug_assert!(sorted.windows(2).all(|w| w[0] <= w[1]), "slice must be sorted");
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

// -------------------------------------------------------------------------------------------------
// Config / argument parsing
// -------------------------------------------------------------------------------------------------

enum Mode {
    Response,
    Variance,
    All,
}

struct Config {
    mode: Mode,
    noise: bool,
    seed: u64,
    trials: usize,
    samples: usize,
    tol: f64,
    out_dir: PathBuf,
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Self {
        let mut mode = Mode::All;
        let mut noise = true;
        let mut seed = 42;
        let mut trials = 100;
        let mut samples = 10_000;
        let mut tol = DEFAULT_TOLERANCE;
        let mut out_dir: Option<PathBuf> = None;

        let mut it = args.peekable();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "response" => mode = Mode::Response,
                "variance" => mode = Mode::Variance,
                "all" => mode = Mode::All,
                "--noise" => noise = true,
                "--no-noise" => noise = false,
                "--seed" => seed = next_val(&mut it, "--seed").parse().expect("seed must be a number"),
                "--trials" => trials = next_val(&mut it, "--trials").parse().expect("trials must be a number"),
                "--samples" => samples = next_val(&mut it, "--samples").parse().expect("samples must be a number"),
                "--tol" => tol = next_val(&mut it, "--tol").parse().expect("tol must be a fraction, e.g. 0.02"),
                "--out" => out_dir = Some(PathBuf::from(next_val(&mut it, "--out"))),
                other => {
                    eprintln!("Ignoring unrecognised argument: {other}");
                },
            }
        }

        Config {
            mode,
            noise,
            seed,
            trials,
            samples,
            tol,
            out_dir: out_dir.unwrap_or_else(default_out_dir),
        }
    }
}

fn next_val(it: &mut impl Iterator<Item = String>, flag: &str) -> String {
    it.next().unwrap_or_else(|| panic!("{flag} expects a value"))
}

fn default_out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("lwma_sim");
    let _: &Path = dir.as_path();
    dir
}
