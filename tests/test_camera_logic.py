"""Camera input (#33, #150, #159): runs the Node unit tests for the spell table, the
gesture->cast debounce, cooldowns and fist-to-open fireball (JS and wasm controllers) and the
blendshape->emotion mapping. Skipped when Node isn't installed."""

import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class CameraLogicTest(unittest.TestCase):
    def test_node_unit_tests(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("node is not installed")
        proc = subprocess.run(
            [node, "--test", str(ROOT / "tests" / "camera_logic.test.mjs"), str(ROOT / "tests" / "camera_casts.test.mjs")],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=120,
        )
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)


if __name__ == "__main__":
    unittest.main()
