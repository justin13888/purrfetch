#!/usr/bin/env python3
"""Add purr's man page and shell completions to a cargo-dist formula."""

from __future__ import annotations

import argparse
from pathlib import Path


ANCHOR = "    install_binary_aliases!\n"
MARKER = "    # purrfetch: install the documentation bundled by cargo-dist.\n"
BLOCK = (
    f"{MARKER}"
    '    man1.install "purr.1"\n'
    '    bash_completion.install "completions/purr.bash" => "purr"\n'
    '    zsh_completion.install "completions/_purr"\n'
    '    fish_completion.install "completions/purr.fish"\n'
)
REQUIRED_LINES = tuple(BLOCK.splitlines())


class FormulaPatchError(RuntimeError):
    """The formula does not match the cargo-dist template we expect."""


def patch_formula(source: str) -> tuple[str, bool]:
    """Return the patched formula and whether it changed."""
    present = [line in source for line in REQUIRED_LINES]
    if all(present):
        if source.count(MARKER) != 1:
            raise FormulaPatchError("the purrfetch formula block appears more than once")
        return source, False
    if any(present):
        raise FormulaPatchError("the purrfetch formula block is incomplete")
    if source.count(ANCHOR) != 1:
        raise FormulaPatchError(
            "expected exactly one cargo-dist install_binary_aliases! call; "
            "the formula template may have changed"
        )
    return source.replace(ANCHOR, f"{ANCHOR}\n{BLOCK}", 1), True


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("formula", type=Path)
    args = parser.parse_args()

    source = args.formula.read_text(encoding="utf-8")
    try:
        patched, changed = patch_formula(source)
    except FormulaPatchError as error:
        raise SystemExit(f"error: {error}") from error
    if changed:
        args.formula.write_text(patched, encoding="utf-8")
        print(f"patched {args.formula}")
    else:
        print(f"{args.formula} is already patched")


if __name__ == "__main__":
    main()
