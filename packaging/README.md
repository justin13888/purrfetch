# Packaging

How `purr` is built and distributed, what is automated, and the one-time
owner-only steps each external channel needs. This is the source of truth for
the packaging system; the recipes themselves live alongside this file (and a few
at the repo root where the tools require it).

The crate is **`purrfetch`** (crates.io); the installed **binary/command is
`purr`**. ASCII art is baked into the binary at compile time, so packages only
need to ship the `purr` binary, `man/purr.1`, the `completions/`, and `LICENSE`.

## Release pipeline (already in place)

| Tool | Owns |
|------|------|
| [release-plz](https://release-plz.dev) | version bump, `CHANGELOG.md`, the "release" PR, and the crates.io publish (runs on the default `GITHUB_TOKEN`, no PAT; crates.io via Trusted Publishing OIDC, no token) |
| [dist (cargo-dist)](https://opensource.axo.dev/cargo-dist/) | GitHub Release: binaries for 5 targets + `shell`/`powershell`/`homebrew`/`msi` installers; bundles `man/purr.1` + `completions/` into every archive |

On merge of the release-plz PR, release-plz publishes to crates.io automatically via
Trusted Publishing (OIDC, no token). The GitHub binaries are still **cut manually**: the
owner pushes the `v{version}` tag locally (see the runbook below), which triggers
cargo-dist's `release.yml`; `.deb`/`.rpm`/winget/Homebrew-completions then chain off that
workflow **completing** — a cargo-dist release is created with `GITHUB_TOKEN`, which does
**not** trigger `on: release`, so those workflows use `on: workflow_run` instead. The tag
is pushed by the owner (not release-plz) for the same reason: a `GITHUB_TOKEN`-pushed tag
would not trigger `release.yml`, and a PAT is disallowed by policy.

> **Note on cargo-dist and Linux packages.** cargo-dist produces archives and
> the `shell`/`powershell`/`homebrew`/`msi` installers, but it does **not** build
> `.deb` or `.rpm`. Those are produced by [`cargo-deb`](https://github.com/kornelski/cargo-deb)
> and [`cargo-generate-rpm`](https://github.com/cat-in-136/cargo-generate-rpm)
> (issue #8's note suggesting cargo-dist can emit them is incorrect).

> **Note on Homebrew completions.** cargo-dist's generated formula installs only the
> `purr` binary and dumps the bundled `purr.1` + `completions/` into `pkgshare`, so
> `brew install` alone gives no working completions or `man purr`. `homebrew-completions.yml`
> chains off Release completing and patches the formula in the tap to add
> `man1.install` / `{bash,zsh,fish}_completion.install` (idempotent; fails loudly if
> cargo-dist's template changes). The Windows PowerShell completion (`purr.ps1`) is
> shipped in the `.zip`/`.msi` instead — it is not auto-loaded, so users dot-source it.
>
> Both tap commits — the formula from `release.yml` and the completions patch from
> `homebrew-completions.yml` — are authored by `github-actions[bot]`. `release.yml` is
> dist-generated but edited to override dist's stock `axo bot` identity, so `ci` is
> listed in `allow-dirty` (`[workspace.metadata.dist]`) to keep dist from reverting it.

## Channels

| Channel | Recipe | Built / submitted by | One-time manual setup |
|---------|--------|----------------------|------------------------|
| crates.io | `Cargo.toml` | release-plz `release` job — crates.io Trusted Publishing (OIDC), no token | one-time: configure the crates.io Trusted Publisher + one manual first `cargo publish` (see runbook) |
| GitHub Release (bins, installers) | `[workspace.metadata.dist]` | cargo-dist `release.yml` | none — owner pushes the tag locally |
| Homebrew | cargo-dist `homebrew` installer + `homebrew-completions.yml` | cargo-dist → `justin13888/homebrew-tap`, then the workflow adds completions/man | ✅ tap repo + `HOMEBREW_TAP_TOKEN` (done) |
| winget | `wix/main.wxs` (msi) | `winget.yml` (chains on `workflow_run: Release`) | ✅ `winget-pkgs` fork + `WINGET_TOKEN` (done) |
| `.deb` / `.rpm` (download) | `[package.metadata.deb]`, `[package.metadata.generate-rpm]` | `package-linux.yml` (chains on `workflow_run: Release`) | none (attached to the release) |
| Fedora COPR — *deferred* | `packaging/rpm/purrfetch.spec` + `.copr/Makefile` | COPR (build-from-git) | create COPR project `justin13888/purr` + webhook |
| AUR — *deferred* | `packaging/aur/purr-bin`, `packaging/aur/purr-git` | `aur.yml` (chains on `workflow_run: Release`) | AUR account + `AUR_SSH_PRIVATE_KEY` secret; create the empty packages |
| Alpine — *deferred* | `packaging/alpine/APKBUILD` | manual MR to `alpinelinux/aports` | Alpine developer account |
| Scoop — *deferred* | `packaging/scoop/purr.json` | manual PR to a scoop bucket (`autoupdate` keeps it current once accepted) | pick a bucket: PR to [scoop extras](https://github.com/ScoopInstaller/Extras), or create a `justin13888/scoop-bucket` repo (a personal bucket would need a push token in CI to auto-update — deferred with the token-free policy; extras runs `autoupdate` upstream, no token) |
| Nix | `flake.nix` (repo root) | `nix run github:justin13888/purrfetch` | ✅ works today; optional: submit to nixpkgs |

## Layout

```
Cargo.toml                       # [workspace.metadata.dist], [package.metadata.{deb,generate-rpm,wix}]
wix/main.wxs                     # generated msi definition (dist generate)
flake.nix, flake.lock            # repo root (flakes must be at root)
.copr/Makefile                   # repo root (COPR convention)
completions/{purr.bash,_purr,purr.fish}
packaging/
  README.md                      # this file
  aur/purr-bin/{PKGBUILD,.SRCINFO}
  aur/purr-git/{PKGBUILD,.SRCINFO}
  alpine/APKBUILD
  rpm/purrfetch.spec
  scoop/purr.json
.github/workflows/{package-linux,winget,aur,homebrew-completions}.yml  # chain on `workflow_run: Release` (aur deferred)
.github/workflows/release-plz.yml                  # release PR + crates.io publish (Trusted Publishing)
```

## Self-verification (podman)

Each recipe is validated locally in a throwaway container — no distro tooling on
the host required. From the repo root:

```bash
# .deb / .rpm — build with cargo subcommands, install-test in containers
cargo install cargo-deb cargo-generate-rpm
cargo build --release
cargo deb --no-build && cargo generate-rpm
podman run --rm -v "$PWD":/w:Z -w /w debian:stable \
  sh -c 'dpkg -i target/debian/*.deb && purr --version && man -w purr'
podman run --rm -v "$PWD":/w:Z -w /w fedora:latest \
  sh -c 'dnf install -y ./target/generate-rpm/*.rpm && purr --version'

# Fedora COPR spec
podman run --rm -v "$PWD":/w:Z -w /w fedora:latest \
  sh -c 'dnf install -y rpmlint rpm-build && rpmlint packaging/rpm/purrfetch.spec'

# AUR (non-root build user; namcap + shellcheck + .SRCINFO drift)
podman run --rm -v "$PWD":/w:Z -w /w archlinux:base-devel \
  sh -c 'pacman -Sy --noconfirm namcap shellcheck >/dev/null &&
         useradd -m build && cp -r packaging/aur/purr-git /tmp/p && chown -R build /tmp/p &&
         su build -c "cd /tmp/p && shellcheck PKGBUILD && namcap PKGBUILD &&
                      makepkg --printsrcinfo | diff - .SRCINFO"'

# Alpine APKBUILD
podman run --rm -v "$PWD":/w:Z -w /w alpine:edge \
  sh -c 'apk add atools shellcheck >/dev/null && cd packaging/alpine &&
         shellcheck -s ash APKBUILD; apkbuild-lint APKBUILD'

# Nix flake
podman run --rm -v "$PWD":/w:Z -w /w nixos/nix \
  sh -c 'nix --extra-experimental-features "nix-command flakes" flake check &&
         nix --extra-experimental-features "nix-command flakes" build .#default &&
         ./result/bin/purr --version'

# Workflows
podman run --rm -v "$PWD":/repo:Z -w /repo rhysd/actionlint:latest
```

## Manual runbook (owner-only)

The pipeline is **token-free**: no PAT and no crates.io token live in CI (crates.io uses
Trusted Publishing). The owner drives each release by merging the release PR and pushing
the tag.

**One-time setup**
- ✅ `justin13888/homebrew-tap` repo + `HOMEBREW_TAP_TOKEN` secret — done.
- ✅ `microsoft/winget-pkgs` fork + `WINGET_TOKEN` secret — done.
  - ⚠️ **First submission is manual.** `winget.yml` uses `winget-releaser`, which only
    *updates* a package that already exists in `winget-pkgs`. Submit the initial
    `justin13888.purr` manifest once (e.g. `wingetcreate new`) so future releases can
    auto-update. Until that lands upstream, `winget.yml` self-skips instead of failing
    the release.
- ✅ Repo setting *Settings → Actions → General → "Allow GitHub Actions to create and
  approve pull requests"* — enabled (lets release-plz open the PR on `GITHUB_TOKEN`).
- **crates.io Trusted Publishing.** On crates.io, configure a Trusted Publisher for the
  `purrfetch` crate: owner `justin13888`, repo `purrfetch`, workflow `release-plz.yml`
  (see https://crates.io/docs/trusted-publishing). No token is ever added to CI.
  - ⚠️ **First publish is manual.** Trusted Publishing cannot publish a brand-new crate,
    so publish the very first version once with a local `cargo publish` (`cargo login`
    first; the token stays local). Every version after that publishes automatically in CI.

**Cutting a release**
1. A push to `master` opens/updates the release-plz PR (version bump + `CHANGELOG.md`).
2. Merge that PR. On merge, the `release-plz release` job publishes the new version to
   crates.io via Trusted Publishing (no token). It does **not** tag or build binaries —
   its run summary prints the exact `git tag …` command below as a reminder, since
   forgetting it leaves the crate on crates.io with no matching GitHub release.
3. Locally, push the tag to trigger the GitHub binary release:
   ```bash
   git checkout master && git pull
   git tag v1.0.0
   git push origin v1.0.0   # human push → triggers cargo-dist's release.yml
   ```
   cargo-dist builds the GitHub Release (tarballs, `.msi`, installers, Homebrew
   formula → tap). When it finishes, `package-linux.yml` (`.deb`/`.rpm`),
   `winget.yml`, and `homebrew-completions.yml` (adds completions/man to the tap
   formula) chain off it via `workflow_run`.
4. Confirm the cascade: version live on crates.io, `.deb`/`.rpm` attached to the release,
   winget PR opened, tap updated with a follow-up "install shell completions and man page"
   commit. If a chained job didn't run, re-run it via its `workflow_dispatch`, e.g.
   `gh workflow run package-linux.yml -f release-tag=v1.0.0`.

**Follow-ups (optional / deferred)**
- **AUR.** Create an AUR account; add your SSH public key; create empty packages
  `purr-bin` and `purr-git`; add the `AUR_SSH_PRIVATE_KEY` secret. `aur.yml` then
  deploys on each release (no workflow edit needed — it already chains on Release).
- **COPR.** Create project `justin13888/purr`; add a package built from this Git repo
  (`.copr/Makefile`); add the GitHub webhook for auto-rebuilds.
- **nixpkgs / Alpine aports.** Upstream the `flake.nix` derivation and
  `packaging/alpine/APKBUILD` via their respective review processes.
- **Scoop.** PR `packaging/scoop/purr.json` to [ScoopInstaller/Extras](https://github.com/ScoopInstaller/Extras)
  (their CI then runs `checkver`/`autoupdate` on every release — nothing to
  automate on our side, keeping CI token-free). Test on a Windows box first:
  `scoop install ./packaging/scoop/purr.json`. Bump the pinned `version`/`hash`
  to the newest release before submitting (they match the asset the manifest
  was authored against; `autoupdate` takes over afterwards).
