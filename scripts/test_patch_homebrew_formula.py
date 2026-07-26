#!/usr/bin/env python3
"""Tests for patch-homebrew-formula.py."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name("patch-homebrew-formula.py")
SPEC = importlib.util.spec_from_file_location("patch_homebrew_formula", SCRIPT)
assert SPEC and SPEC.loader
PATCHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PATCHER)


FORMULA = """class Purr < Formula
  def install
    bin.install "purr"

    install_binary_aliases!

    pkgshare.install "completions"
  end
end
"""


class PatchFormulaTests(unittest.TestCase):
    def test_inserts_documentation_after_aliases(self) -> None:
        patched, changed = PATCHER.patch_formula(FORMULA)
        self.assertTrue(changed)
        self.assertIn(PATCHER.BLOCK, patched)
        self.assertLess(
            patched.index("install_binary_aliases!"),
            patched.index("man1.install"),
        )

    def test_is_idempotent(self) -> None:
        patched, _ = PATCHER.patch_formula(FORMULA)
        second, changed = PATCHER.patch_formula(patched)
        self.assertFalse(changed)
        self.assertEqual(second, patched)

    def test_rejects_a_changed_template(self) -> None:
        with self.assertRaisesRegex(PATCHER.FormulaPatchError, "template may have changed"):
            PATCHER.patch_formula(FORMULA.replace("install_binary_aliases!", "aliases!"))

    def test_rejects_a_partial_patch(self) -> None:
        partial = FORMULA.replace(
            PATCHER.ANCHOR,
            f'{PATCHER.ANCHOR}\n    man1.install "purr.1"\n',
        )
        with self.assertRaisesRegex(PATCHER.FormulaPatchError, "incomplete"):
            PATCHER.patch_formula(partial)


if __name__ == "__main__":
    unittest.main()
