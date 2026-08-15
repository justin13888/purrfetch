# Releasing

Releasing purr is a **two-step process**: release-plz opens a version-bump PR
automatically, and a maintainer then **manually dispatches** the promotion run
that tags and publishes. Merging the release PR alone publishes nothing.

Both steps live in [`.github/workflows/release-plz.yml`](../.github/workflows/release-plz.yml)
("Release management"). [`release-plz.toml`](../release-plz.toml) deliberately
sets `git_tag_enable = false` and `git_release_enable = false` so that tagging
and publishing stay with the promotion job, which validates every destination.

## Step 1 — merge the release PR

Every push to `master` runs the `release-pr` job, which creates or updates a
`chore: release vX.Y.Z` PR (labelled `release`) bumping `Cargo.toml` and
`CHANGELOG.md`. Review and merge it as usual.

Pull request runs also exercise the full release build — the five target
builds, the global installers, the PowerShell installer, and Homebrew — so a
green release PR means promotion has a good chance of succeeding.

## Step 2 — dispatch the promotion

Once the release PR is merged and `master` is at the release commit:

1. Go to **Actions → Release management → Run workflow**.
2. Select the `master` branch.
3. Enter the `release-tag` input, for example `v1.1.0`.

Or from the CLI:

```bash
gh workflow run "Release management" --ref master -f release-tag=v1.1.0
```

## What promotion enforces

The `prepare` job refuses to proceed unless:

- the tag matches `vMAJOR.MINOR.PATCH` — pre-release and build-metadata tags are unsupported;
- the tag without its `v` equals the `purrfetch` version in `Cargo.toml`;
- the run was dispatched from `master`;
- `HEAD` equals `origin/master`, so you cannot promote a stale commit;
- any pre-existing tag of that name already points at this exact commit.

The `publisher-prerequisites` job (dispatch-only) then checks credentials
*before* anything is published:

- `HOMEBREW_TAP_TOKEN` is set and can push to `justin13888/homebrew-tap`;
- crates.io Trusted Publishing is configured for this repository, verified via
  `rust-lang/crates-io-auth-action` — no registry token is stored.

Promotion runs only after `package-preflight`, all five `build-native` targets
(`{aarch64,x86_64}-apple-darwin`, `{aarch64,x86_64}-unknown-linux-gnu`,
`x86_64-pc-windows-msvc`), `build-global`, `powershell-installer`, and
`homebrew` have all succeeded.

## What promotion does

In order, against the exact commit `prepare` resolved:

1. Creates and pushes `vX.Y.Z`, or validates an existing tag points at that commit.
2. Creates a **draft** GitHub release, uploads the `dist`-planned artifacts, then
   re-downloads each one and compares it byte-for-byte against the local build.
3. Publishes to crates.io via Trusted Publishing, then polls (up to three
   minutes) until the registry reports the expected `.crate` checksum.
4. Updates `Formula/purr.rb` in the Homebrew tap.
5. Flips the GitHub release from draft to published and marks it `latest`.
6. Re-verifies all three destinations — tag commit, release assets, crates.io
   checksum, and the published formula — as a final gate.

## Retrying a failed promotion

Promotion is designed to be re-dispatched with the same tag. Each step is
idempotent and asserts exact equality rather than overwriting:

- an existing tag is accepted only if it points at the same commit;
- an existing **draft** release has its notes and assets refreshed; an already
  **published** release is left alone and only verified;
- a crate already on crates.io is accepted only if its checksum matches the
  local `.crate`; publishing is skipped rather than retried;
- a Homebrew formula already at this version must be byte-identical, and the
  job refuses to overwrite a *newer* version than the one being released.

So the safe response to a mid-way failure is to fix the cause and dispatch the
same tag again. If a step reports a *mismatch* rather than a missing artifact,
do not force it — that means something was published from a different build,
and it needs investigating by hand.
