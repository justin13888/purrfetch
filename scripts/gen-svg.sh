#!/usr/bin/env bash
# Regenerate the README screenshots from purr's demo presets:
#   assets/purr.svg            — the hero (fedora-desktop preset)
#   assets/examples/<p>.svg    — one per remaining `purr --example` preset
# All themed Catppuccin Macchiato in JetBrains Mono.
#
# Idempotent and deterministic: every value comes from `--example`'s curated
# presets (fictional identities, constant uptime/memory/…), so re-runs produce
# byte-identical SVGs and the gallery never churns in git diffs.
#
# Requirements:
#   - freeze (https://github.com/charmbracelet/freeze)
#       Provisioned by `mise install` (declared in mise.toml). To install it
#       manually instead, see https://github.com/charmbracelet/freeze#installation
#
# Usage:
#   mise run svg
#   bash scripts/gen-svg.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Prerequisites ---
if ! command -v freeze &>/dev/null; then
    echo "Error: freeze is not installed." >&2
    echo "  Install: mise install" >&2
    echo "  Or: https://github.com/charmbracelet/freeze#installation" >&2
    exit 1
fi

# --- Build purr in release mode so the screenshots reflect current output ---
echo "Building purr (release)..."
cargo build --release --features example --manifest-path "$REPO_ROOT/Cargo.toml"
PURR="$REPO_ROOT/target/release/purr"

# The hero preset renders to assets/purr.svg; the rest go to assets/examples/.
HERO="fedora-desktop"
GALLERY=(arch nixos gentoo debian-server ubuntu macos windows void)

mkdir -p "$REPO_ROOT/assets/examples"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# Render one preset to one SVG.
#
# Pipe purr (not `freeze --execute`) so purr uses its plain, newline-based
# layout: under a PTY it positions text with cursor-movement escapes that
# freeze's ANSI parser can't replay, collapsing everything onto huge lines.
# Piped, purr still emits ANSI colour (colour is gated on NO_COLOR, which we
# clear here, not on whether stdout is a TTY). WAYLAND_DISPLAY is cleared so
# the DE line's live " (Wayland)" suffix can't leak into demo output.
#
# freeze's `--theme` only sets the background; it renders captured ANSI with a
# fixed VGA palette (pure #0000ff blue, etc.) that is harsh and low-contrast on
# a dark background. Remap those 16 palette colours (plus freeze's default
# foreground #C5C8C6) to Catppuccin Macchiato. The window-control and border
# chrome use distinct hexes and are left untouched. Tied to the palette emitted
# by the freeze version pinned in mise.toml.
render_svg() {
    local preset="$1" out="$2"
    local raw="$TMP_DIR/$preset.svg"
    echo "Rendering $out (purr --example $preset)..."
    env -u NO_COLOR -u WAYLAND_DISPLAY \
        "$PURR" --no-config --example "$preset" |
        freeze \
            --output "$raw" \
            --theme catppuccin-macchiato \
            --font.family "JetBrains Mono" \
            --window \
            --border.radius 8 \
            --padding 20 \
            --margin 20
    sed -E '
        s/#000000/#494d64/Ig
        s/#800000/#ed8796/Ig
        s/#008000/#a6da95/Ig
        s/#808000/#eed49f/Ig
        s/#000080/#8aadf4/Ig
        s/#800080/#c6a0f6/Ig
        s/#008080/#8bd5ca/Ig
        s/#c0c0c0/#b8c0e0/Ig
        s/#808080/#5b6078/Ig
        s/#ff0000/#ed8796/Ig
        s/#00ff00/#a6da95/Ig
        s/#ffff00/#eed49f/Ig
        s/#0000ff/#8aadf4/Ig
        s/#ff00ff/#c6a0f6/Ig
        s/#00ffff/#8bd5ca/Ig
        s/#ffffff/#a5adcb/Ig
        s/#c5c8c6/#cad3f5/Ig
    ' "$raw" > "$out"
}

render_svg "$HERO" "$REPO_ROOT/assets/purr.svg"
for preset in "${GALLERY[@]}"; do
    render_svg "$preset" "$REPO_ROOT/assets/examples/$preset.svg"
done

echo "Wrote assets/purr.svg and ${#GALLERY[@]} gallery SVGs to assets/examples/"
