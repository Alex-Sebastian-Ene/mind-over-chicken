"""Terrain (#109, #129): runs the Node acceptance tests for the Rust home world (sea share,
five or more continents, sites with sea access, every run different) and galaxy planet
surfaces. Skipped when Node isn't installed."""

import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class TerrainTest(unittest.TestCase):
    def test_node_unit_tests(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("node is not installed")
        proc = subprocess.run(
            [node, "--test", str(ROOT / "tests" / "terrain.test.mjs")],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=600,
        )
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)


if __name__ == "__main__":
    unittest.main()
