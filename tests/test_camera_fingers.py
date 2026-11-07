"""Camera finger actions (#67): runs the Node unit tests for the One Euro smoothing, the dead zone,
the per-player hand range, pinch detection, the pointing / pinch-and-release controller and the
debug tab's rate meters. Skipped when Node isn't installed."""

import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class CameraFingersTest(unittest.TestCase):
    def test_node_unit_tests(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("node is not installed")
        proc = subprocess.run(
            [node, "--test", str(ROOT / "tests" / "camera_fingers.test.mjs")],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=120,
        )
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)


if __name__ == "__main__":
    unittest.main()
