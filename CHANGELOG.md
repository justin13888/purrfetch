# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
