#!/usr/bin/env bash
# Compare purr against fastfetch using a matched default module set.
#
# Requirements:
#   - hyperfine (https://github.com/sharkdp/hyperfine)
#   - fastfetch (https://github.com/fastfetch-cli/fastfetch)
#
# Usage:
#   bash scripts/bench-compare.sh
#   bash scripts/bench-compare.sh --cold   # also run cold-cache benchmark (requires sudo)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Prerequisites ---
for tool in hyperfine fastfetch; do
    if ! command -v "$tool" &>/dev/null; then
        echo "Error: $tool is not installed." >&2
        exit 1
    fi
done

if [[ "${1:-}" != "" && "${1:-}" != "--cold" ]]; then
    echo "Usage: $0 [--cold]" >&2
    exit 2
fi

# --- Build purr in release mode ---
echo "Building purr (release)..."
cargo build --release --locked --manifest-path "$REPO_ROOT/Cargo.toml"
PURR="$REPO_ROOT/target/release/purr"

# Match purr's stock neofetch probe set. Both tools bypass user configuration
# and disable colours so the benchmark measures the same non-TTY workload.
FASTFETCH_STRUCTURE="Title:Separator:OS:Host:Kernel:Uptime:Packages:Shell:Display:DE:WM:WMTheme:Theme:Icons:Terminal:TerminalFont:CPU:GPU:Memory"
PURR_COMMAND="$PURR --no-config --stdout"
FASTFETCH_COMMAND="fastfetch --config none --pipe --structure $FASTFETCH_STRUCTURE"

echo ""
echo "Environment:"
echo "  host: $(uname -a)"
echo "  purr: $("$PURR" --version | head -n 1)"
echo "  fastfetch: $(fastfetch --version | head -n 1)"
echo "  hyperfine: $(hyperfine --version)"
echo ""

HYPERFINE_ARGS=(
    --command-name "purr matched/default" "$PURR_COMMAND"
    --command-name "fastfetch matched" "$FASTFETCH_COMMAND"
)

# --- Warm benchmark ---
echo "=== Warm matched benchmark (warmup=10, min-runs=100) ==="
hyperfine \
    --warmup 10 \
    --min-runs 100 \
    --shell=none \
    --export-json "$REPO_ROOT/bench-results.json" \
    --export-markdown "$REPO_ROOT/bench-results.md" \
    "${HYPERFINE_ARGS[@]}"

echo ""
echo "Results saved to bench-results.json and bench-results.md"

# --- Cold-cache benchmark (optional, requires sudo) ---
if [[ "${1:-}" == "--cold" ]]; then
    echo ""
    echo "=== Cold-cache matched benchmark (min-runs=5) ==="
    if [[ "$(uname -s)" != "Linux" ]]; then
        echo "Cold-cache benchmarking is currently supported on Linux only. Skipping."
    elif ! sudo -n true 2>/dev/null; then
        echo "Sudo access required for dropping caches. Skipping cold benchmark."
    else
        hyperfine \
            --warmup 0 \
            --min-runs 5 \
            --shell=none \
            --prepare "sudo sh -c 'sync; echo 3 > /proc/sys/vm/drop_caches'" \
            --export-json "$REPO_ROOT/bench-cold-results.json" \
            --export-markdown "$REPO_ROOT/bench-cold-results.md" \
            "${HYPERFINE_ARGS[@]}"
        echo ""
        echo "Cold results saved to bench-cold-results.json and bench-cold-results.md"
    fi
fi
