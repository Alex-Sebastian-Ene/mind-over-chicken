"""Smoke tests for the static game and the headset bridge.

These need no browser and no BrainFlow install: they catch the mistakes that ship a blank
page (a missing file, a broken import path) or a bridge that won't even start.
"""

import py_compile
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMPORT_RE = re.compile(r"""(?:import|export)\s[^;]*?from\s+['"](\.{1,2}/[^'"]+)['"]""", re.S)
ASSET_RE = re.compile(r"""(?:src|href)=["']([^"':#]+)["']""")


class SiteSmokeTest(unittest.TestCase):
    def test_index_assets_exist(self):
        html = (ROOT / "index.html").read_text(encoding="utf-8")
        local = [a for a in ASSET_RE.findall(html) if not a.startswith(("http", "//"))]
        self.assertIn("src/main.js", local)
        for asset in local:
            self.assertTrue((ROOT / asset).is_file(), f"index.html references missing file {asset}")

    def test_js_imports_resolve(self):
        modules = list((ROOT / "src").rglob("*.js"))
        self.assertGreater(len(modules), 5)
        for mod in modules:
            for spec in IMPORT_RE.findall(mod.read_text(encoding="utf-8")):
                target = (mod.parent / spec).resolve()
                self.assertTrue(target.is_file(), f"{mod.relative_to(ROOT)} imports missing {spec}")

    def test_bridge_compiles(self):
        py_compile.compile(str(ROOT / "bridge" / "muse_bridge.py"), doraise=True)
        py_compile.compile(str(ROOT / "scripts" / "serve.py"), doraise=True)


if __name__ == "__main__":
    unittest.main()
