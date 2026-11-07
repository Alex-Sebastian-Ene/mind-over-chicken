"""Calibration seed (#54): runs the Node unit tests for the configurable window, the reproducible
random setup and the sensor-to-trait formula. Skipped when Node isn't installed."""

import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class CalibrationTest(unittest.TestCase):
    def test_node_unit_tests(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("node is not installed")
        proc = subprocess.run(
            [node, "--test", str(ROOT / "tests" / "calibration.test.mjs")],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=120,
        )
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)


if __name__ == "__main__":
    unittest.main()
