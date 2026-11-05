"""Keep PROJECT_MAP.md honest about the Rust workspace.

The map once covered only `tools/` — 616 Rust files were invisible to it and the
dependency graph was empty. Refresh it with `tools/generate-project-map.py`.
"""

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAP = ROOT / "PROJECT_MAP.md"


def crate_names():
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    names = []
    for member in manifest["workspace"]["members"]:
        data = tomllib.loads((ROOT / member / "Cargo.toml").read_text(encoding="utf-8"))
        names.append(data["package"]["name"])
    return names


class ProjectMapTests(unittest.TestCase):
    def test_every_workspace_crate_is_mapped(self):
        text = MAP.read_text(encoding="utf-8")
        missing = [name for name in crate_names() if f"`{name}`" not in text]
        self.assertEqual(missing, [], f"PROJECT_MAP.md omits crates: {missing}")

    def test_the_dependency_graph_is_not_empty(self):
        text = MAP.read_text(encoding="utf-8")
        block = text.split("<!-- BEGIN GENERATED: dep-graph -->", 1)[1].split(
            "<!-- END GENERATED: dep-graph -->", 1
        )[0]
        self.assertGreater(
            block.count("→"), 20, "the dependency graph regressed to an empty block"
        )

    def test_the_map_names_the_generator_that_can_refresh_it(self):
        text = MAP.read_text(encoding="utf-8")
        self.assertIn("tools/generate-project-map.py", text)
        self.assertTrue((ROOT / "tools" / "generate-project-map.py").is_file())


if __name__ == "__main__":
    unittest.main()
