"""Keep the two products sharing one kernel instead of sharing code paths.

`docs/product-split.md` splits the repository into **BongoCat Desktop** (local save,
Live2D model, keyboard and mouse credit) and **BongoCat Cloud** (account, cloud save,
sessions only). Both run `crates/bongocat-pomodoro`; nothing else is shared.

A desktop crate reachable from the cloud build would not fail to compile. It would
quietly pull a window system, a GPU or a file writer into a wasm module or a
mini-program cloud function, and the first symptom would be a build break on a
target nobody develops on. So the direction of every dependency, and the product
ownership of every workspace member, is pinned here: adding a crate means deciding
which product owns it.
"""

import re
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CRATES = ROOT / "crates"
WORKSPACE_MANIFEST = ROOT / "Cargo.toml"

KERNEL = "bongocat-pomodoro"
CLOUD_BRIDGE = "bongocat-pomodoro-web"
DESKTOP_ENTRY = "bongocat-app"

DEPENDENCIES = "[dependencies]"
DEV_DEPENDENCIES = "[dev-dependencies]"

# Third-party crates the kernel may name. Anything else it reaches has to be a
# `bongocat-` crate, and only the one listed in `KERNEL_LOCAL` is allowed.
KERNEL_THIRD_PARTY = frozenset({"serde", "serde_json", "thiserror"})

# `document.rs` writes the desktop save file through the storage helper, so the
# kernel reaches exactly one desktop crate. Moving that write behind the caller is
# a separate change; until then this is the only allowed crossing, and it is named
# here instead of being swallowed by a wildcard.
KERNEL_LOCAL = frozenset({"bongocat-storage"})

# The cloud build is the kernel plus JSON. `serde` is only there because the bridge
# hands back a `serde_json::Value`.
CLOUD_BRIDGE_ALLOWED = frozenset({KERNEL, "serde", "serde_json"})

# Everything in `crates/` that belongs to the desktop product. Reachable from the
# app binary, never from the wasm bridge or the mini-program cloud function.
DESKTOP_ONLY = frozenset(
    {
        "bongocat-app",
        "bongocat-audio",
        "bongocat-config",
        "bongocat-i18n",
        "bongocat-input",
        "bongocat-live2d",
        "bongocat-live2d-playback",
        "bongocat-live2d-render",
        "bongocat-log",
        "bongocat-model",
        "bongocat-model-store",
        "bongocat-overlay",
        "bongocat-packaging",
        "bongocat-platform",
        "bongocat-render",
        "bongocat-runtime",
        "bongocat-storage",
        "bongocat-ui",
        "bongocat-ui-protocol",
        "bongocat-update",
    }
)
SHARED = frozenset({KERNEL})
CLOUD_ONLY = frozenset({CLOUD_BRIDGE})

# A dependency is declared either bare (`serde = { .. }`) or as a dotted key
# (`bongocat-storage.workspace = true`), so the crate name is the first segment.
DECLARATION = re.compile(r"^(?P<name>[A-Za-z0-9_-]+)(?:\.[A-Za-z0-9_-]+)*\s*=")


def read(path):
    return path.read_text(encoding="utf-8")


def section_body(source, header):
    """Return the body of one `[section]`, or fail when the section is absent.

    A missing section is an error rather than an empty body on purpose: a manifest
    that moved its dependencies under `[target.'cfg(...)'.dependencies]` would
    otherwise look like a crate with no dependencies at all, and every rule below
    would pass against nothing.
    """
    name = header.strip("[]")
    match = re.search(
        rf"^\[{re.escape(name)}\]\s*\n(?P<body>.*?)(?=^\[|\Z)",
        source,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"{header} is missing")
    return match.group("body")


def dependency_names(manifest, header=DEPENDENCIES, missing_ok=False):
    """The dependency keys declared in one section of a manifest.

    `missing_ok` is for the optional sections only: a crate may legitimately have
    no dev-dependencies, but it may never have no dependencies, so the required
    section still fails closed.
    """
    try:
        body = section_body(read(manifest), header)
    except AssertionError:
        if missing_ok:
            return set()
        raise
    names = set()
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        declaration = DECLARATION.match(stripped)
        if declaration is not None:
            names.add(declaration.group("name"))
    return names


def workspace_members(manifest=WORKSPACE_MANIFEST):
    """The directory name of every workspace member."""
    body = section_body(read(manifest), "[workspace]")
    members = re.search(
        r"^members\s*=\s*\[(?P<body>.*?)\]",
        body,
        re.MULTILINE | re.DOTALL,
    )
    if members is None:
        raise AssertionError("[workspace].members is missing")
    return [Path(path).name for path in re.findall(r'"([^"]+)"', members.group("body"))]


def unexpected(dependencies, allowed):
    return sorted(set(dependencies) - set(allowed))


class ManifestSectionDetectorTests(unittest.TestCase):
    def test_dependency_keys_and_sections_are_read_separately(self):
        source = "\n".join(
            (
                "[package]",
                'name = "demo"',
                "",
                "[dependencies]",
                "bongocat-overlay.workspace = true",
                'serde = { version = "=1.0.0", features = ["derive"] }',
                "",
                "[dev-dependencies]",
                "tempfile.workspace = true",
                "",
                "[lints]",
                "workspace = true",
            )
        )
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text(source, encoding="utf-8")

            dependencies = dependency_names(manifest)
            dev_dependencies = dependency_names(manifest, DEV_DEPENDENCIES)

        self.assertEqual(dependencies, {"bongocat-overlay", "serde"})
        self.assertEqual(dev_dependencies, {"tempfile"})

    def test_a_missing_section_is_reported_rather_than_read_as_empty(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text('[package]\nname = "demo"\n', encoding="utf-8")
            with self.assertRaisesRegex(AssertionError, r"\[dependencies\]"):
                dependency_names(manifest)

    def test_workspace_members_are_read_without_their_directories(self):
        source = "\n".join(
            (
                "[workspace]",
                "members = [",
                '    "crates/bongocat-app",',
                '    "crates/bongocat-pomodoro",',
                "]", "",
            )
        )
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text(source, encoding="utf-8")
            members = workspace_members(manifest)

        self.assertEqual(members, ["bongocat-app", "bongocat-pomodoro"])


class ProductBoundaryContractTests(unittest.TestCase):
    def test_the_kernel_reaches_no_desktop_crate_but_the_save_file(self):
        dependencies = dependency_names(CRATES / KERNEL / "Cargo.toml")
        # A parser that silently found nothing would make the allowlist vacuous.
        self.assertIn("serde", dependencies)
        self.assertEqual(
            unexpected(dependencies, KERNEL_THIRD_PARTY | KERNEL_LOCAL),
            [],
            "crates/bongocat-pomodoro may only use its allowlisted crates",
        )

    def test_the_cloud_bridge_depends_only_on_the_kernel(self):
        for header in (DEPENDENCIES, DEV_DEPENDENCIES):
            dependencies = dependency_names(
                CRATES / CLOUD_BRIDGE / "Cargo.toml", header, missing_ok=True
            )
            self.assertEqual(
                unexpected(dependencies, CLOUD_BRIDGE_ALLOWED),
                [],
                f"{CLOUD_BRIDGE} {header} may only use {sorted(CLOUD_BRIDGE_ALLOWED)}",
            )

    def test_the_desktop_entry_never_reaches_the_cloud_bridge(self):
        for header in (DEPENDENCIES, DEV_DEPENDENCIES):
            dependencies = dependency_names(
                CRATES / DESKTOP_ENTRY / "Cargo.toml", header, missing_ok=True
            )
            self.assertNotIn(CLOUD_BRIDGE, dependencies)

    def test_every_workspace_member_belongs_to_a_product(self):
        classified = DESKTOP_ONLY | SHARED | CLOUD_ONLY
        self.assertEqual(
            sorted(set(workspace_members()) - classified),
            [],
            "a new workspace member has to be classified in docs/product-split.md",
        )

    def test_a_classified_crate_exists(self):
        on_disk = {path.name for path in CRATES.iterdir() if (path / "Cargo.toml").is_file()}
        self.assertEqual(sorted((DESKTOP_ONLY | SHARED | CLOUD_ONLY) - on_disk), [])


if __name__ == "__main__":
    unittest.main()
