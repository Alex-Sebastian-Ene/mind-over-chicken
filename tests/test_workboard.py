"""Offline tests for scripts/workboard.py: no network, made-up branches."""

import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("workboard", ROOT / "scripts" / "workboard.py")
wb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wb)


def branch(name, files, base="main", pr=None, included_in=None):
    return {"name": name, "base": base, "pr": pr, "files": files, "people": ["someone"], "refs": [],
            "last": "2026-11-07T14:00:00Z", "ahead": 1, "behind": 0, "included_in": included_in}


class WorkboardTest(unittest.TestCase):
    def test_areas(self):
        self.assertEqual(wb.area_of("crates/engine/src/gl.rs"), "Rust engine")
        self.assertEqual(wb.area_of("src/camera/gestures.js"), "camera")
        self.assertEqual(wb.area_of("src/main.js"), "game shell")
        self.assertEqual(wb.area_of("rl/train_ppo.py"), "AI god")

    def test_flags_real_overlaps_only(self):
        engine = branch("rust/render", ["crates/engine/src/gl.rs", "src/main.js", "pkg/x.wasm"], pr={"number": 34, "draft": False})
        galaxy = branch("feat/globe-galaxy", ["src/main.js", "src/cosmos/globe.js", "pkg/x.wasm"], pr={"number": 27, "draft": False})
        stacked_fix = branch("fix/engine-review", ["crates/engine/src/gl.rs"], base="rust/render", pr={"number": 32, "draft": False})
        body = wb.render("main", [engine, galaxy, stacked_fix], [])
        overlaps = body.split("## Overlaps")[1].split("## Claimed")[0]
        self.assertIn("`src/main.js` (hot spot)", overlaps)       # engine vs galaxy: real
        self.assertNotIn("gl.rs", overlaps)                        # fix is stacked on the engine
        self.assertNotIn("pkg/", overlaps)                         # generated files are ignored

    def test_owner_open_is_not_a_claim(self):
        self.assertTrue(wb.UNCLAIMED.search("Owner: open, comment to claim."))
        self.assertFalse(wb.UNCLAIMED.search("Owner: Claude (parallel agent)"))


if __name__ == "__main__":
    unittest.main()
