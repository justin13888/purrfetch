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

$ErrorActionPreference = 'Stop'

$baseUrl = if ($env:PURRFETCH_RELEASE_URL) {
  $env:PURRFETCH_RELEASE_URL
} else {
  'https://github.com/justin13888/purrfetch/releases/latest/download'
}
$installerUrl = "$baseUrl/purrfetch-installer.ps1"

$tmpDir = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
$null = New-Item -ItemType Directory -Path $tmpDir
try {
  $installer = Join-Path $tmpDir 'purrfetch-installer.ps1'
  Invoke-WebRequest -UseBasicParsing -Uri $installerUrl -OutFile $installer
  # cargo-dist's installer reports failure with `exit`, which only ends that
  # script; surface it so `-File` callers (purr self-upgrade) see a failure.
  $global:LASTEXITCODE = 0
  & $installer @args
  if ($global:LASTEXITCODE) {
    throw "purrfetch-installer.ps1 failed with exit code $global:LASTEXITCODE"
  }
} finally {
  Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
