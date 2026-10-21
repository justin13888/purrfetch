# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0](https://github.com/justin13888/purrfetch/compare/v1.1.0...v1.2.0) - 2026-10-21

### Added

- *(cli)* add self-upgrade command for script installs
- *(installer)* add version-independent bootstrap installers
- *(bench)* report peak RSS, cold span timings, and a budget verdict

### Fixed

- *(cli)* scrub installer overrides from self-upgrade's environment
- *(cli)* keep a working purr.exe during Windows self-upgrade
- *(cli)* harden self-upgrade temp dir and resolve symlinked purr
- *(cli)* keep -ExecutionPolicy Bypass for Windows self-upgrade
- *(installer)* run PowerShell installers as script blocks
- *(cli)* isolate Windows PowerShell from pwsh's module path

### Other

- Merge pull request #44 from justin13888/perf/43-macos-gpu-iokit
- *(bench)* record macOS run after the IOKit GPU probe
- note that cargo-installed self-upgrade builds are refused
- describe self-upgrade's safeguards and the pinned installer hashes
- *(cli)* cover self-upgrade refusals and pin bootstrap hashes
- *(cli)* inject self-upgrade's exe, env, and side effects
- *(gpu)* read macOS GPU names from IOKit instead of system_profiler
- Merge remote-tracking branch 'origin/master' into feat/self-upgrade
- *(release)* pin setup-homebrew to its main branch
- *(bench)* record macOS reference results
- note self-upgrade ships after 1.1.0
- document self-upgrade and bootstrap installers
- *(release)* publish and smoke-test bootstrap installers
- *(bench)* record provisional Linux results and cite them in the README
- *(bench)* add a weekly report-only benchmark workflow
- *(bench)* run bench-compare's tests in mise run test
- Add support for Vylen Linux

## [1.1.0](https://github.com/justin13888/purrfetch/compare/v1.0.2...v1.1.0) - 2026-07-26

### Added

- *(demo)* add Windows example preset
- *(assets)* generate a per-distro example gallery from demo mode
- *(cli)* render example presets via --example
- *(demo)* add curated example presets with canned probe values

### Fixed

- *(renderer)* apply title_fqdn consistently across renderers

### Other

- update README
- Merge remote-tracking branch 'origin/master' into chore/32-windows-example
- *(cli)* list Windows example in generated references
- *(readme)* add Windows example to gallery
- *(packaging)* add deferred scoop manifest
- *(readme)* add a comparison section and concurrency FAQ
- credit macchina in NOTICE and add a credits section
- *(build)* strip symbols from release binaries

## [1.0.2](https://github.com/justin13888/purrfetch/compare/v1.0.1...v1.0.2) - 2026-07-02

### Fixed

- *(release)* attribute Homebrew tap commits to github-actions[bot]

### Other

- *(packaging)* note tap commit identity and the tag-push reminder
- *(release)* remind to push the tag when a crate is published

## [1.0.1](https://github.com/justin13888/purrfetch/compare/v1.0.0...v1.0.1) - 2026-07-02

### Fixed

- *(release)* ship completions/man via Homebrew and prep deferred items

### Other

- *(packaging)* document release-plz Trusted Publishing flow
- *(release)* publish to crates.io via release-plz Trusted Publishing
- *(winget)* self-skip until package is bootstrapped in winget-pkgs
