"""Keep the cultivation state machine split across its source files.

`state.rs` once held 5,339 lines: every view type, every transition and every
surface query in one file. The two later `impl Cultivation` blocks now live in
child modules, and the transition block itself is split by domain. A future
edit must not grow any of them back into a monolith.
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CULTIVATION = ROOT / "crates" / "bongocat-pomodoro" / "src" / "cultivation"
MAX_STATE_LINES = 3000
MAX_ACTIONS_LINES = 700

# The transition block, by domain. `state_actions.rs` keeps 采集/出行/会话/建筑;
# the rest are 园圃与装备, 战斗与破境, 探索, 打工, 试炼与锁妖塔 and the views.
SPLIT_FILES = (
    "state_actions.rs",
    "state_actions_garden.rs",
    "state_actions_battle.rs",
    "state_actions_explore.rs",
    "state_actions_work.rs",
    "state_actions_trial.rs",
    "state_actions_view.rs",
)
# The query block is one file on purpose: it is read-only and does not grow with
# every action, only with every surface that wants a new number.
QUERY_FILES = ("state_queries.rs",)


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
        missing = [
            name
            for name in SPLIT_FILES + QUERY_FILES
            if not (CULTIVATION / name).is_file()
        ]
        self.assertEqual(missing, [], f"split-off impl files are missing: {missing}")

    def test_state_rs_declares_both_child_modules(self):
        source = (CULTIVATION / "state.rs").read_text(encoding="utf-8")
        for name in SPLIT_FILES + QUERY_FILES:
            with self.subTest(module=name):
                self.assertIn(f'#[path = "{name}"]', source)

    def test_each_transition_module_stays_below_its_own_threshold(self):
        # One file per domain, each under 700 lines. Without this cap the split
        # buys nothing: the first busy feature lands in whichever file is open
        # and the monolith comes back wearing a suffix.
        for name in SPLIT_FILES:
            with self.subTest(module=name):
                lines = len((CULTIVATION / name).read_text(encoding="utf-8").splitlines())
                self.assertLessEqual(
                    lines,
                    MAX_ACTIONS_LINES,
                    f"{name} grew past {MAX_ACTIONS_LINES} lines; split it by "
                    "domain rather than appending",
                )

    def test_each_transition_module_is_a_child_impl_block(self):
        # A split file that stopped being `impl Cultivation` would mean the
        # transitions moved somewhere else and this contract guards nothing.
        for name in SPLIT_FILES:
            with self.subTest(module=name):
                source = (CULTIVATION / name).read_text(encoding="utf-8")
                self.assertIn("impl Cultivation {", source)


if __name__ == "__main__":
    unittest.main()
