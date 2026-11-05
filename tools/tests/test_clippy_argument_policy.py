"""Keep the long-argument policy in one place.

Runtime worker consumers and settings constructors take their dependencies as
explicit arguments. Twenty scattered `#[allow(clippy::too_many_arguments)]`
attributes once hid that choice; the threshold now lives in `clippy.toml` and
only call sites genuinely above it keep a local, explained exemption.
"""

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / "clippy.toml"
CRATES = ROOT / "crates"
THRESHOLD = 10
MAX_LOCAL_EXEMPTIONS = 4


def exemptions():
    found = []
    for path in sorted(CRATES.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        for index, line in enumerate(lines):
            if "allow(clippy::too_many_arguments)" in line:
                found.append((path, index, lines))
    return found


class ClippyArgumentPolicyTests(unittest.TestCase):
    def test_the_threshold_is_declared_once_at_the_workspace_root(self):
        self.assertTrue(CONFIG.is_file())
        config = tomllib.loads(CONFIG.read_text(encoding="utf-8"))
        self.assertEqual(config.get("too-many-arguments-threshold"), THRESHOLD)

    def test_local_exemptions_stay_bounded(self):
        self.assertLessEqual(
            len(exemptions()),
            MAX_LOCAL_EXEMPTIONS,
            "a new blanket exemption should raise the shared threshold, not "
            "add another per-function allow",
        )

    def test_every_local_exemption_names_its_reason(self):
        found = exemptions()
        self.assertGreater(len(found), 0, "the scan would pass vacuously")
        for path, index, lines in found:
            with self.subTest(file=path.name, line=index + 1):
                preceding = " ".join(lines[max(0, index - 2) : index])
                self.assertIn("threshold", preceding.lower())


if __name__ == "__main__":
    unittest.main()
