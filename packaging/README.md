# Packaging and releases

This is the maintainer source of truth for purr's distribution channels,
validation policy, and release runbook.

The crate is named `purrfetch`; the installed command and Homebrew formula are
named `purr`.

## Support matrix

| Channel | Platforms | Status | Release owner |
|---|---|---|---|
| GitHub archives | Linux aarch64/x86_64, macOS Apple Silicon/Intel, Windows x86_64 | Supported | `release-plz.yml` + cargo-dist |
| GitHub installers | POSIX shell, PowerShell, Windows MSI | Supported | `release-plz.yml` + cargo-dist |
| Homebrew | macOS and Linux | Supported | `release-plz.yml` → `justin13888/homebrew-tap` |
| Cargo from crates.io | Rust-supported hosts | Supported | crates.io Trusted Publishing |
| Cargo from Git | Rust-supported hosts | Supported | this repository |
| Mise from crates.io or Git | Rust-supported hosts; Rust is required | Supported interface backed by Cargo | no separate publisher |
| `.deb`, `.rpm`, Nix, winget, Scoop, Alpine, AUR/Arch, COPR | — | Deferred | dormant prototypes only |

Deferred channels have no release automation and no user-facing installation
promise. Their recipes remain in the repository to preserve prior exploration,
but they may be stale. A channel becomes supported only after demonstrated
demand, maintained ownership, and repeatable end-to-end build, installation,
upgrade, and publication validation.

## Owned release workflow

[`.github/workflows/release-plz.yml`](../.github/workflows/release-plz.yml) is
the only release-management workflow:

- A push to `master` only creates or updates the release-plz PR.
- Every pull request runs the complete non-publishing preflight for all five
  targets.
- A manual dispatch with a stable `vMAJOR.MINOR.PATCH` tag runs the same
  preflight, validates publisher credentials, and promotes only after every job
  passes.

cargo-dist is pinned in `Cargo.toml` and is used only to generate artifacts.
It does not generate CI or publish anything. The owned workflow uses this fixed
native matrix:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

The preflight builds every archive, checksum, shell installer, PowerShell
installer, MSI, source archive, and Homebrew formula. It then:

- runs every native binary;
- downloads through both installers from a local artifact server;
- silently installs, runs, and uninstalls the MSI;
- installs the final formula on Linux and macOS and checks the command, man
  page, and bash/zsh/fish completions;
- runs Cargo package and publish dry-runs; and
- dry-runs both supported Mise specifications.

[`scripts/patch-homebrew-formula.py`](../scripts/patch-homebrew-formula.py)
adds the bundled man page and completions to cargo-dist's formula. It is
idempotent and deliberately fails if the formula template changes. Its tests
live in
[`scripts/test_patch_homebrew_formula.py`](../scripts/test_patch_homebrew_formula.py).

## Promotion and retry policy

Promotion is one serialized job. It performs these operations in order:

1. Create or validate the tag at the exact current `master` commit.
2. Create or reuse a draft GitHub release and upload the complete artifact set.
3. Publish or verify the exact crates.io crate through Trusted Publishing.
4. Push or verify the exact Homebrew formula.
5. Publish the GitHub release.
6. Confirm the tag, release assets, crate checksum, and tap formula.

The supported destinations are required. Missing credentials and unmet
publisher prerequisites fail the run; no publisher self-skips.

External registries cannot participate in a distributed transaction. The
workflow therefore gates every build and validation before the first
publication, then promotes serially and fails loudly. Retrying the same tag is
safe: existing tags, releases, crate versions, and formulas count as complete
only after their commit, asset bytes, checksum, version, or content is verified.
The workflow never overwrites a mismatched published crate or formula.

## One-time repository setup

- Enable **Allow GitHub Actions to create and approve pull requests** so
  release-plz can maintain its PR.
- Configure the crates.io Trusted Publisher for crate `purrfetch`, owner
  `justin13888`, repository `purrfetch`, and workflow `release-plz.yml`.
- Add `HOMEBREW_TAP_TOKEN`, scoped to push to
  `justin13888/homebrew-tap`.
- Protect the promotion environment or workflow dispatch permission according
  to the repository's release-approval policy.

The crate already exists on crates.io. If this process is reused for a new crate,
crates.io requires its initial version to be published before Trusted Publishing
can be configured.

## Release runbook

1. Confirm the release-plz PR contains the intended stable version and
   changelog, its complete PR preflight passes, and then merge it.
2. Wait for `master` CI and the next release-plz PR update to settle.
3. Dispatch the owned workflow from `master`:

   ```bash
   gh workflow run release-plz.yml --ref master -f release-tag=v1.1.0
   ```

4. Watch the run. Do not create or push the tag manually and do not publish any
   individual channel out of band.
5. Confirm the final promotion summary and independently check the GitHub
   release, crates.io version, and `justin13888/homebrew-tap` formula.

If promotion fails after it starts, correct the prerequisite or transient
failure and dispatch the same tag again. Do not bump the version merely to retry;
the workflow verifies and resumes already-completed destinations.

Only stable releases are supported. Prerelease tags require an explicit policy
for all supported channels before they can be enabled.

## Dormant prototypes

| Prototype | Files |
|---|---|
| Debian and RPM downloads | `Cargo.toml` package metadata |
| Fedora RPM/COPR | `packaging/rpm/purrfetch.spec`, `.copr/Makefile` |
| AUR/Arch | `packaging/aur/` |
| Alpine | `packaging/alpine/APKBUILD` |
| Scoop | `packaging/scoop/purr.json` |
| Nix | `flake.nix`, `flake.lock` |
| winget | no separate recipe; the supported MSI could seed future work |

These files are not checked by release CI and must not be presented as current
installation instructions. Validate and refresh them before any experiment.

## Maintainer checks

Before changing release infrastructure:

```bash
python3 scripts/test_patch_homebrew_formula.py
dist plan
cargo package --locked --allow-dirty
cargo publish --dry-run --locked --allow-dirty
mise use --dry-run 'cargo:purrfetch@latest'
mise use --dry-run \
  'cargo:https://github.com/justin13888/purrfetch@branch:master'
git diff --check
```

Run `actionlint` against `.github/workflows/release-plz.yml`; the container form
is useful when it is not installed locally:

```bash
podman run --rm -v "$PWD":/repo:Z -w /repo rhysd/actionlint:latest
```

After release changes, also perform a local cargo-dist build and installer smoke
test. Pull requests provide the authoritative cross-platform preflight and must
exercise all five native targets without publishing.
