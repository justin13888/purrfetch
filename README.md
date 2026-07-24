# purr

Fast, universal, cross-platform fetching tool written in Rust.

Perfect for sharing your [rice](https://www.reddit.com/r/unixporn/) or showing stats on terminal startup.

<p align="center">
  <a href="https://crates.io/crates/purrfetch"><img src="https://img.shields.io/crates/v/purrfetch.svg" alt="crates.io"></a>
  <a href="https://github.com/justin13888/purrfetch/actions/workflows/ci.yml"><img src="https://github.com/justin13888/purrfetch/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/purrfetch"><img src="https://img.shields.io/crates/d/purrfetch.svg" alt="downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/crates/l/purrfetch.svg" alt="license"></a>
</p>

> **purr is v1 — stable and actively maintained.** A fast, memory-safe, drop-in
> successor to the archived [neofetch](https://github.com/dylanaraps/neofetch).

<p align="center">
  <img src="assets/purr.svg" alt="purr output on Fedora, themed Catppuccin Macchiato" width="700">
</p>

## Why purr?

[neofetch](https://github.com/dylanaraps/neofetch) is archived and no longer maintained. If its look is part of your terminal-startup ritual — the thing you see every time a shell opens — purr keeps exactly that: the same fields, the same `${c1}`..`${c6}` ASCII, the same vibe. The difference is it's **instant** instead of the visible pause neofetch takes to start. It's a drop-in successor that's [actively maintained and packaged for most platforms](#installation) (with more package managers on the way).

- **Fast**: probes run in parallel on native Rust; a typical run finishes in ~20 ms — roughly **91× faster** than neofetch's ~2 s
- **Cross-platform**: Linux, macOS, and Windows
- **neofetch-compatible**: matches neofetch's commonly-used info fields, styling, configuration, and `${c1}`..`${c6}` ASCII format. The [parity matrix](docs/neofetch-parity.md) records exactly what's covered and what's intentionally deferred
- **Highly customizable**: TOML config plus CLI flags for separators, colours, per-field options, color blocks, ASCII overrides, JSON output, and a Kitty image backend
- **Modern neofetch replacement**: memory-safe, maintained, and distributed via native package managers across Windows, macOS, and Linux

## Gallery

Every render below is a real `purr` run against a curated example preset. The
presets are compiled only when the default-off `example` Cargo feature is
enabled, so you can try them on any machine with:

```bash
cargo run --features example -- --example <preset>
```

The hero image above uses the `fedora-desktop` preset.

| | |
|---|---|
| <img src="assets/examples/arch.svg" alt="purr --example arch"> `purr --example arch` | <img src="assets/examples/nixos.svg" alt="purr --example nixos"> `purr --example nixos` |
| <img src="assets/examples/gentoo.svg" alt="purr --example gentoo"> `purr --example gentoo` | <img src="assets/examples/debian-server.svg" alt="purr --example debian-server"> `purr --example debian-server` |
| <img src="assets/examples/ubuntu.svg" alt="purr --example ubuntu"> `purr --example ubuntu` | <img src="assets/examples/macos.svg" alt="purr --example macos"> `purr --example macos` |
| <img src="assets/examples/void.svg" alt="purr --example void"> `purr --example void` | |

## How purr compares

purr's goal is narrow on purpose: **be the neofetch you already know, without
the wait or the abandonware.** If you're choosing a fetch tool:

- **[neofetch](https://github.com/dylanaraps/neofetch)** — the original, archived
  since 2024. purr replicates its default look, fields, flags, and `${c1}`..`${c6}`
  ASCII format (the [parity matrix](docs/neofetch-parity.md) tracks every field),
  while starting in ~20 ms instead of ~2 s. If you want neofetch, maintained and
  fast, that's purr.
- **[fastfetch](https://github.com/fastfetch-cli/fastfetch)** — an excellent,
  very featureful neofetch successor in C with its own JSONC configuration and a
  broader module set. purr trades that breadth for neofetch-style TOML/flag
  compatibility, a smaller single binary, and memory safety (Rust).
- **[macchina](https://github.com/Macchina-CLI/macchina)** — a minimal,
  aesthetics-focused fetch tool in Rust with its own distinct look. purr shares
  its probe engine ([libmacchina](https://github.com/Macchina-CLI/libmacchina))
  but targets neofetch's look and configuration surface instead.

What purr deliberately does **not** do: neofetch's arbitrary-bash `print_info`
scripting, its 60+ package-manager matrix, or the w3m-era image backends
(kitty is supported; the rest are [intentionally deferred](docs/neofetch-parity.md)).
Benchmarks against all three are reproducible via
[`scripts/bench-compare.sh`](scripts/bench-compare.sh).

## Installation

### Cargo

```bash
cargo install --locked purrfetch
```

### Prebuilt binaries

Download a binary for your platform from the [latest release](https://github.com/justin13888/purrfetch/releases/latest), or use the install script:

```bash
# Linux & macOS
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/justin13888/purrfetch/releases/latest/download/purrfetch-installer.sh | sh
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://github.com/justin13888/purrfetch/releases/latest/download/purrfetch-installer.ps1 | iex"
```

<!-- TODO(packaging): re-enable Alpine once the APKBUILD is accepted into alpinelinux/aports (needs the v1.0.0 release tarball + `abuild checksum`). -->
### Alpine Linux

_Planned._ The [`APKBUILD`](packaging/alpine/APKBUILD) is available to build from in the meantime; upstreaming into the Alpine repositories (`apk add purr`) is pending.

<!-- TODO(packaging): re-enable AUR once an AUR account + AUR_SSH_PRIVATE_KEY secret are set up; aur.yml then deploys purr-bin/purr-git automatically on each release. -->
### Arch Linux

_Planned._ AUR recipes for `purr-bin` (prebuilt) and `purr-git` (build from source) live in [`packaging/aur/`](packaging/aur/); publishing to the AUR (`paru -S purr-bin`) is not yet live.

### Debian/Ubuntu and derivatives

Download the `.deb` for your architecture from the [latest release](https://github.com/justin13888/purrfetch/releases/latest) and install it:

```bash
sudo dpkg -i purrfetch_*_amd64.deb
```

### Fedora

Grab the `.rpm` from the [latest release](https://github.com/justin13888/purrfetch/releases/latest) and install it:

```bash
sudo dnf install ./purrfetch-*.rpm
```

<!-- TODO(packaging): re-enable COPR once the copr project justin13888/purr is created with a Git-built package (.copr/Makefile) + GitHub webhook. -->
_A Fedora COPR repository (`sudo dnf copr enable justin13888/purr && sudo dnf install purr`) is planned._

### Nix

Run it directly, or install it into your profile (the flake also exposes an overlay and `packages.default`):

```bash
nix run github:justin13888/purrfetch
nix profile install github:justin13888/purrfetch
```

### Homebrew (macOS & Linux)

```bash
brew install justin13888/tap/purr
```

### Winget (Windows)

```powershell
winget install justin13888.purr
```

<!-- TODO(packaging): re-enable Scoop once packaging/scoop/purr.json is accepted into a bucket (see packaging/README.md). -->
### Scoop (Windows)

_Planned._ The manifest lives at [`packaging/scoop/purr.json`](packaging/scoop/purr.json);
submission to a scoop bucket (`scoop install purr`) is pending. In the meantime it
installs directly: `scoop install https://raw.githubusercontent.com/justin13888/purrfetch/master/packaging/scoop/purr.json`.

> The native packages (Debian/Ubuntu `.deb`, Fedora `.rpm`, Nix) and Homebrew also install the `man purr` page and bash/zsh/fish shell completions. On Windows a PowerShell completion (`purr.ps1`) ships in the archive/MSI — it is not auto-loaded, so dot-source it from your `$PROFILE`.

### Git

Note: This method is suggested for one of the following reasons:

1. Latest `purr` version
2. Native package manager is unsupported or not preferred

To install via Git, follow these steps:
1. Clone this repository.
2. Run `cargo install --path .` in the repository root.

## Usage

Run `purr` with no arguments for the neofetch-style output. Useful flags:

| flag | effect |
|---|---|
| `--all` | show every probe |
| `--json` | structured JSON output |
| `-L`/`--logo`, `--off` | logo only · no logo |
| `--ascii_distro <name>` | force a distro logo |
| `--ascii_colors "4 6 1"` | recolour the logo |
| `--separator <s>`, `--no_bold`, `--colors "..."` | text styling |
| `--memory_unit gib`, `--uptime_shorthand tiny`, `--cpu_cores physical` | per-field options |
| `--backend kitty --source <img.png>` | Kitty image backend |
| `--stdout` | plain output (honours `NO_COLOR`) |

The optional `--example [preset]` flag renders curated data instead of live
system information. Install a feature-enabled binary or run it directly:

```bash
cargo install --locked --features example purrfetch
cargo run --features example -- --example <preset>
```

See the [Gallery](#gallery) for available presets.

Run `purr --help` for the full list, or `man purr` for the manual page (also
checked in at [`man/purr.1`](man/purr.1) and bundled in release archives).

### Configuration

purr reads a TOML config (`purr config-path` prints its location; `purr generate`
writes a starter file). Precedence is **defaults < config file < CLI flags**.
Each probe is a labelled entry — either a terse string or a table of options:

```toml
[Neofetch]
title = true
separator = ":"
bold = true

[[Neofetch.probes]]
OS = "OS"                      # terse form

[[Neofetch.probes]]
[Neofetch.probes.CPU]          # rich form
label = "CPU"
cores = "physical"
```

Use a `[Json]` table (or `--json`) for JSON output.

### Parity & supported systems

purr targets neofetch [`ccd5d9f`](https://github.com/dylanaraps/neofetch/blob/ccd5d9f52609bbdcd5d8fa78c4fdb0f12954125f/neofetch):

- [`docs/neofetch-parity.md`](docs/neofetch-parity.md) — dated, field-by-field parity with deferred features
- [`docs/os-support.md`](docs/os-support.md) — the 50 shipped logos and the pruned distro list

## Development

- Clone this repository
- Run `mise install` to provision the dev tools and git hooks (see [Tooling](#tooling))
- Run `cargo build` to build the project
- Use `mise run start` (or `cargo run`) to run the project

### Tooling

Dev tools (`hk`, `convco`) and task running are managed with [mise](https://mise.jdx.dev/). Install mise, then provision everything and install the git hooks in one step:

```bash
mise install
```

This installs the tools and, via a `postinstall` hook, runs `hk install --mise` to set up the git hooks. Ensure mise is [activated](https://mise.jdx.dev/getting-started.html) in your shell (or use its shims) so the tools and tasks are on your `PATH`.

Common tasks (run `mise tasks` to list them all):

```bash
mise run start        # build and run purr (forward args: mise run start -- <args>)
mise run test         # test both default and example-feature builds
mise run fmt          # format code in place
mise run lint         # lint both default and example-feature builds
mise run fmt-check    # verify formatting without modifying files
mise run lint-check   # verify clippy lints without modifying files
mise run man          # regenerate man/purr.1 from the CLI definition
mise run completions  # regenerate shell completions in completions/
mise run svg          # regenerate assets/purr.svg + the assets/examples/ gallery (requires freeze)
```

The man page (`man/purr.1`) and the shell completions (`completions/`) are
generated from the `clap` CLI by `examples/gen-man.rs` and
`examples/gen-completions.rs`, so they never drift from `purr --help`. Run
`mise run man` and `mise run completions` after changing any flags
(`mise run man-check` / `mise run completions-check` verify they are in sync).

### Git Hooks

This project uses [hk](https://hk.jdx.dev/) (configured in `hk.pkl`) to manage git hooks, which run through mise:

- **pre-commit** — format and clippy-fix staged Rust files, re-staging the results
- **pre-push** — formatting, lint, and test checks, plus a Conventional Commits check
- **commit-msg** — Conventional Commits linting via `convco`

`mise install` installs these automatically. To (re)install them manually, run `hk install --mise`.

### Commit messages

Commits must follow [Conventional Commits](https://www.conventionalcommits.org/) — enforced by `convco` (commit-msg hook, pre-push, and CI). Version bumps, the `CHANGELOG.md`, and releases are automated from these messages by [release-plz](https://release-plz.dev/).

### Benchmarking

#### End-to-end comparison (hyperfine)

Requires [hyperfine](https://github.com/sharkdp/hyperfine). Compares purr against any of neofetch, macchina, and fastfetch that are installed.

```bash
bash scripts/bench-compare.sh           # warm benchmark
bash scripts/bench-compare.sh --cold    # also cold-cache (requires sudo)
```

Results are written to `bench-results.json` and `bench-results.md`.

#### Probe microbenchmarks (criterion)

Benchmarks each probe individually, grouped by expected cost (fast, I/O-heavy, subprocess). Also measures the cold construction cost of each `libmacchina` readout.

```bash
cargo bench
```

HTML reports are written to `target/criterion/`.

#### Runtime profiling

**Tracing spans** — prints per-probe and per-subprocess timing at `debug` level:

```bash
RUST_LOG=debug cargo run --release -- --all
```

**Chrome trace** — produces `purr-trace.json` viewable in [Perfetto](https://ui.perfetto.dev):

```bash
cargo run --release --features profile -- --all
```

**Flamegraph** — requires `cargo install flamegraph` and `perf` (Linux):

```bash
cargo flamegraph --profile profiling -- --all
```

## Packaging

Package version across repositories:

[![Packaging status](https://repology.org/badge/vertical-allrepos/purrfetch.svg)](https://repology.org/project/purrfetch/versions)

<!-- Repology may file these packages under `purr` once they propagate; if the
badge shows "no data", switch the project name in the two URLs above to `purr`. -->

## FAQ

Q: Why did you write another fetch tool?
A: It's feature-rich, fast, and written in a memory-safe language (Rust). The goal is to make it a modern, well-maintained replacement for neofetch and more.

Q: Why not contribute to an existing fetch tool?
A: I want to start from a clean state, including all the features the community wants, and make it truly universally supported and deployable to all common platforms.

Q: What does purr use to fetch metrics under the hood?
A: purr uses the `libmacchina` crate for most system-related info, plus native probes (GPU driver, GTK font, MPRIS now-playing, …) and a neofetch-compatible renderer on top.

Q: Why threads instead of an async runtime?
A: Each probe runs on its own scoped OS thread and streams its result to the renderer as it completes. Probes are dominated by blocking syscalls and subprocesses, so an async runtime (tokio, smol, …) would add binary size and complexity without improving wall-clock time — the slowest probe bounds the run either way.

## Issues

If you encounter any issues, please open an issue on the GitHub repository.

## Contributing

Feel free to submit an issue or PR on GitHub.

> Notice: Looking for submissions/suggestions of new ASCII arts: <https://github.com/justin13888/purrfetch/issues/1>

## Credits

purr stands on two projects in particular:

- **[neofetch](https://github.com/dylanaraps/neofetch)** — the ASCII distro logos
  (`${c1}`..`${c6}` markers included) and the layout purr replicates are ported
  from neofetch, along with its field-formatting behavior. Thank you, Dylan
  Araps and the neofetch contributors.
- **[macchina](https://github.com/Macchina-CLI/macchina) / [libmacchina](https://github.com/Macchina-CLI/libmacchina)** —
  libmacchina powers most of purr's probes. Instead of forking, purr pushes
  probe performance tweaks upstream to libmacchina directly, so improvements
  land in both projects.

See [NOTICE.md](NOTICE.md) for full third-party attributions.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

purr ports ASCII logos and parity logic from [neofetch](https://github.com/dylanaraps/neofetch) (also MIT). See [NOTICE.md](NOTICE.md) for third-party attributions.
