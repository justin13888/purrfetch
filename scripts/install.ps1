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
# Everything runs inside a script block so that, under `irm | iex`, these
# variables (and $ErrorActionPreference) stay out of the caller's session. The
# fetched installer also runs as a script block, so its `exit 1` on failure
# ends this script with that code too. Like cargo-dist's own installer, it
# needs an execution policy of Unrestricted, RemoteSigned, or Bypass, hence
# `-ExecutionPolicy Bypass` in the command above.

& {
  $ErrorActionPreference = 'Stop'

  $baseUrl = if ($env:PURRFETCH_RELEASE_URL) {
    $env:PURRFETCH_RELEASE_URL
  } else {
    'https://github.com/justin13888/purrfetch/releases/latest/download'
  }
  $installer = Invoke-RestMethod -UseBasicParsing -Uri "$baseUrl/purrfetch-installer.ps1"
  & ([ScriptBlock]::Create($installer)) @args
} @args
