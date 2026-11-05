"""Keep architecture decisions in one place so one ADR cannot drift into two copies.

A decision record was once left at `docs/0100-resource-economy-retune.md` while the
numbered copy lived in `docs/adr/`. Both were byte-identical at the time, but only
one gets edited, so the next revision would have left two authorities behind.

Other `NNNN-*.md` design drafts legitimately live beside `docs/adr/`, so the rule is
about *duplicate decisions*, not about filenames that look numbered.
"""

import hashlib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"
ADR_DIR = DOCS / "adr"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class AdrLocationTests(unittest.TestCase):
    def test_no_adr_filename_is_duplicated_beside_the_adr_directory(self):
        duplicates = sorted(
            path.name for path in ADR_DIR.glob("*.md") if (DOCS / path.name).exists()
        )
        self.assertEqual(
            duplicates,
            [],
            f"the same ADR file exists in docs/ and docs/adr/: {duplicates}",
        )

    def test_no_adr_content_is_duplicated_beside_the_adr_directory(self):
        outside = {digest(path): path.name for path in DOCS.glob("*.md")}
        collisions = sorted(
            f"{path.name} == {outside[digest(path)]}"
            for path in ADR_DIR.glob("*.md")
            if digest(path) in outside
        )
        self.assertEqual(
            collisions,
            [],
            f"a renamed ADR copy sits beside docs/adr/: {collisions}",
        )

    def test_the_adr_directory_holds_decisions(self):
        self.assertTrue(
            any(path.name[:4].isdigit() for path in ADR_DIR.glob("*.md")),
            "docs/adr/ no longer holds numbered decisions; the scan would pass vacuously",
        )


if __name__ == "__main__":
    unittest.main()
