# Install the latest purr release.
#
# This bootstrap is deliberately version-independent: it fetches the latest
# release's cargo-dist installer (`purrfetch-installer.ps1`), which verifies the
# archive checksum, and runs it with the arguments and environment given here.
# Script-installed `purr` binaries compile in this file's SHA-256, and
# `purr self-upgrade` only re-runs it while that checksum still matches, so any
# change to this file makes older binaries refuse to self-upgrade. Keep it
# stable.
#
#   powershell -ExecutionPolicy Bypass -c "irm https://github.com/justin13888/purrfetch/releases/latest/download/install.ps1 | iex"
#
# PURRFETCH_RELEASE_URL overrides where the installer is fetched from.
#
# Everything runs inside a script block: under `irm | iex` that keeps these
# variables (and $ErrorActionPreference) out of the caller's session, and
# running the fetched installer as a script block rather than a file keeps it
# clear of execution policy, as cargo-dist's own `irm | iex` one-liner is.

& {
  $ErrorActionPreference = 'Stop'

  $baseUrl = if ($env:PURRFETCH_RELEASE_URL) {
    $env:PURRFETCH_RELEASE_URL
  } else {
    'https://github.com/justin13888/purrfetch/releases/latest/download'
  }
  $installer = Invoke-RestMethod -UseBasicParsing -Uri "$baseUrl/purrfetch-installer.ps1"

  # cargo-dist's installer reports some failures through $LASTEXITCODE rather
  # than an error; surface them so callers (purr self-upgrade) see a failure.
  $global:LASTEXITCODE = 0
  & ([ScriptBlock]::Create($installer)) @args
  if ($global:LASTEXITCODE) {
    throw "purrfetch-installer.ps1 failed with exit code $global:LASTEXITCODE"
  }
} @args
