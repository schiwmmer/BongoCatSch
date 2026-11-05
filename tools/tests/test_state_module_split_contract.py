"""Keep the cultivation state machine split across its three source files.

`state.rs` once held 5,339 lines: every view type, every transition and every
surface query in one file. The two later `impl Cultivation` blocks now live in
child modules. A future edit must not grow the parent back into a monolith.
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CULTIVATION = ROOT / "crates" / "bongocat-pomodoro" / "src" / "cultivation"
MAX_STATE_LINES = 3000
SPLIT_FILES = ("state_actions.rs", "state_queries.rs")


class StateModuleSplitTests(unittest.TestCase):
    def test_state_rs_stays_below_the_monolith_threshold(self):
        lines = len((CULTIVATION / "state.rs").read_text(encoding="utf-8").splitlines())
        self.assertLessEqual(
            lines,
            MAX_STATE_LINES,
            "state.rs grew back toward a monolith; move a cohesive impl block "
            "into a child module instead",
        )

    def test_the_split_impl_modules_still_exist(self):
        missing = [name for name in SPLIT_FILES if not (CULTIVATION / name).is_file()]
        self.assertEqual(missing, [], f"split-off impl files are missing: {missing}")

    def test_state_rs_declares_both_child_modules(self):
        source = (CULTIVATION / "state.rs").read_text(encoding="utf-8")
        for name in SPLIT_FILES:
            with self.subTest(module=name):
                self.assertIn(f'#[path = "{name}"]', source)


if __name__ == "__main__":
    unittest.main()
