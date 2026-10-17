# Benchmarks

This page records purr's reference performance numbers on each supported desktop platform. The
README's headline numbers come from here. Tracking issue:
[#39](https://github.com/justin13888/purrfetch/issues/39).

## Budgets

Each OS is judged on its own reference machine:

| Budget | Rule | On a miss |
|---|---|---|
| HARD | purr's median wall time is below fastfetch's median on the same machine | file a perf issue and link it from #39 |
| SOFT | purr's median wall time is ≤ 25 ms | warning only |

The neofetch speedup is reported for information. The README states a single "N× faster" figure
only if it holds on every platform; otherwise it states the figure per OS.

## Method

Run everything with one command:

```bash
mise run bench-compare
```

mise installs the pinned hyperfine, fastfetch, and hyfetch (which provides `neowofetch`, the
maintained neofetch fork) for this task only. Tools that are missing or fail are skipped with a
warning. The run writes `bench-results.json` and `bench-results.md` at the repository root; both
are git-ignored. It measures three things:

1. **End-to-end wall time and peak RSS.** hyperfine runs each tool with 5 warmups and at least 20
   measured runs. User configuration is disabled, and stdout is drained through a pipe. hyperfine 2
   records the median, mean ± σ, min, and max wall time, plus each run's peak resident set size.
   Peak RSS is per process: for script-based tools such as neowofetch it is the largest process
   observed, not the sum of the process tree.
2. **Cold span timings.** purr is built a second time with `--features profile`, into
   `target/bench-profile` so it never replaces the binary hyperfine timed. This build runs 20 times,
   one process per run, and the median and p95 of every tracing span are aggregated: `probe: <field>`,
   `init_readout: <kind>`, `subprocess: <cmd>`, and the top-level `render`. Probes run in parallel,
   so spans overlap and do not sum to the wall time. These runs are not warmed up, so they read
   slower than the hyperfine median. Use them to attribute cost between probes, not as the headline
   number. The Shell probe versions purr's parent process, which here is hyperfine or bench-compare
   rather than your shell.
3. **Binary size** of the stripped release binary (`target/release/purr`).

The verdict is printed at the end of the run and also appears in both result files. It never
changes the exit code.

The `Benchmark` workflow (`.github/workflows/bench.yml`) runs the same command on `ubuntu-latest`
and `macos-14` every week and on manual dispatch, and uploads the result files as artifacts. Shared
runners are noisy, so those numbers are only indicative; the reference numbers below come from
dedicated machines.

For a reference run: plug the machine into power, close heavy applications, check that the load
average is near zero, and run the command twice, keeping the second run.

## Linux

> **Provisional.** This run was measured on a shared machine under heavy background load (load
> average about 30 on 16 threads, `powersave` governor). An earlier run on the same day measured purr
> at a 6.1 ms mean against fastfetch's 4.7 ms, so the two tools are within noise of each other on
> this host. It needs to be re-run on an idle system before the Linux item in #39 can be ticked.

**Machine:** AMD Ryzen 7 7800X3D (16 threads), 92 GiB RAM, MSI MEG X870E GODLIKE, Fedora Linux 44
(Silverblue), kernel 7.2.4-200.fc44.x86_64.

**Tools:** purr 1.1.0, fastfetch 2.69.0, neowofetch (Neofetch 8.1.0), hyperfine 2.0.0.

| Tool | Median (ms) | Mean ± σ (ms) | Min (ms) | Max (ms) | Median peak RSS (MiB) |
|---|---:|---:|---:|---:|---:|
| purr | 4.0 | 4.1 ± 0.4 | 3.3 | 6.2 | 7.5 |
| fastfetch | 4.6 | 4.9 ± 0.9 | 4.1 | 16.8 | 7.4 |
| neowofetch | 1749.0 | 1737.0 ± 365.5 | 766.4 | 2326.0 | 29.1 |

**Binary size:** purr release binary is 6,766,120 bytes (6.45 MiB).

**Verdict:**

- HARD: **PASS**. purr median 4.0 ms vs fastfetch median 4.6 ms (within noise, see the note above).
- SOFT: **PASS**. 4.0 ms ≤ 25 ms.
- neowofetch: purr is about 440× faster (median 1749 ms).

**Slowest cold spans** (20 runs, traced build, same load):

| Span | Median (ms) | p95 (ms) |
|---|---:|---:|
| render (whole probe phase) | 17.74 | 25.92 |
| probe: WM Theme | 10.68 | 15.97 |
| probe: Theme | 9.95 | 16.29 |
| subprocess: gsettings (3 per run) | 9.57 | 15.95 |
| probe: Icons | 8.70 | 16.45 |
| probe: Packages | 8.50 | 16.17 |
| probe: Shell | 2.13 | 7.66 |
| probe: GPU | 1.01 | 7.00 |

On a GNOME session, the `gsettings` subprocesses behind Theme, WM Theme, and Icons, together with
package counting, bound the run. Every other probe finishes in well under 1 ms.

## macOS

> **Provisional.** This run was measured on battery power under heavy background load (load average
> about 15–19 on 12 cores, 25 GiB of 36 GiB memory in use). It needs to be re-run plugged in on an
> idle system before the macOS item in #39 can be ticked. The HARD miss below is too large to be
> explained by load alone: fastfetch ran at about 29 ms under the same conditions, and the miss is
> attributable to a single subprocess (see the span table).

**Machine:** Apple M3 Pro (12 cores), 36 GiB RAM, MacBook Pro (Mac15,7), macOS 26.6.1 (25G76).

**Tools:** purr 1.1.0, fastfetch 2.69.0, neofetch 7.1.0 (Homebrew), neowofetch (Neofetch 8.1.0),
hyperfine 2.0.0.

| Tool | Median (ms) | Mean ± σ (ms) | Min (ms) | Max (ms) | Median peak RSS (MiB) |
|---|---:|---:|---:|---:|---:|
| purr | 229.1 | 210.1 ± 36.9 | 159.4 | 257.6 | 14.2 |
| fastfetch | 28.9 | 29.6 ± 2.3 | 27.3 | 38.0 | 16.8 |
| neofetch | 592.5 | 687.0 ± 344.7 | 497.7 | 2056.9 | 21.7 |
| neowofetch | 7438.4 | 7464.0 ± 353.5 | 7028.1 | 8667.0 | 39.1 |

**Binary size:** purr release binary is 2,015,776 bytes (1.92 MiB).

**Verdict:**

- HARD: **FAIL**. purr median 229.1 ms vs fastfetch median 28.9 ms.
- SOFT: **WARN**. 229.1 ms > 25 ms.
- neofetch: purr is about 2.6× faster than neofetch 7.1.0 (median 592.5 ms) and about 32× faster
  than neowofetch (median 7438.4 ms). bench-compare prefers `neofetch` when it is installed, so the
  printed verdict uses neofetch here; the Linux host had only neowofetch.

**Slowest cold spans** (20 runs, traced build, same load):

| Span | Median (ms) | p95 (ms) |
|---|---:|---:|
| render (whole probe phase) | 166.16 | 233.31 |
| probe: GPU (`subprocess: system_profiler`) | 164.98 | 231.76 |
| probe: Packages | 24.56 | 27.09 |
| subprocess: brew | 20.02 | 22.17 |
| probe: Resolution | 19.07 | 22.71 |
| probe: OS | 18.06 | 20.25 |
| probe: WM Theme | 16.27 | 18.64 |
| probe: Theme | 10.24 | 11.62 |
| subprocess: gsettings (2 per run) | 10.19 | 12.23 |
| probe: Icons | 9.85 | 12.25 |
| subprocess: sw_vers (2 per run) | 8.85 | 12.83 |
| probe: Memory | 8.72 | 11.97 |
| subprocess: defaults (2 per run) | 7.80 | 10.79 |

The GPU probe's `system_profiler` subprocess alone takes about 165 ms and bounds the whole run.
Every other probe finishes within about 25 ms, led by Homebrew package counting (`brew`).
Removing or replacing the `system_profiler` call is what HARD needs on macOS.
