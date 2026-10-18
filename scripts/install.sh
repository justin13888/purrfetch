#!/bin/sh
# Install the latest purr release.
#
# This bootstrap is deliberately version-independent: it fetches the latest
# release's cargo-dist installer (`purrfetch-installer.sh`), which verifies the
# archive checksum, and runs it with the arguments and environment given here.
# Script-installed `purr` binaries compile in this file's SHA-256, and
# `purr self-upgrade` only re-runs it while that checksum still matches, so any
# change to this file makes older binaries refuse to self-upgrade. Keep it
# stable.
#
#   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/justin13888/purrfetch/releases/latest/download/install.sh | sh
#
# PURRFETCH_RELEASE_URL overrides where the installer is fetched from.

set -eu

base_url="${PURRFETCH_RELEASE_URL:-https://github.com/justin13888/purrfetch/releases/latest/download}"
installer_url="$base_url/purrfetch-installer.sh"

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT INT TERM
installer="$tmp_dir/purrfetch-installer.sh"

# Only an explicit override (e.g. a local test server) may use plain HTTP.
protocols='=https'
if [ -n "${PURRFETCH_RELEASE_URL:-}" ]; then
    protocols='=https,http'
fi

if command -v curl >/dev/null 2>&1; then
    curl --proto "$protocols" --tlsv1.2 -fsSL "$installer_url" -o "$installer"
elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$installer" "$installer_url"
else
    echo "install.sh: need curl or wget to download $installer_url" >&2
    exit 1
fi

sh "$installer" "$@"
