#!/usr/bin/env python3
"""Regenerate PROJECT_MAP.md with the whole Rust workspace, not just tools/.

The previous map was produced by an external tool that only saw `tools/`: 616
Rust source files and all 22 workspace crates were absent, and the dependency
graph was empty. This generator lives in the repository so the map can be
refreshed with one command and checked by a contract test.

Usage: python3 tools/generate-project-map.py
"""

import datetime
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "PROJECT_MAP.md"


def workspace_crates():
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    crates = []
    for member in manifest["workspace"]["members"]:
        directory = ROOT / member
        data = tomllib.loads((directory / "Cargo.toml").read_text(encoding="utf-8"))
        internal = set()
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for key, value in data.get(section, {}).items():
                package = value.get("package", key) if isinstance(value, dict) else key
                if package.startswith("bongocat-"):
                    internal.add(package)
        sources = sorted(directory.rglob("*.rs"))
        lines = sum(
            len(path.read_text(encoding="utf-8", errors="replace").splitlines())
            for path in sources
        )
        crates.append(
            {
                "name": data["package"]["name"],
                "path": member,
                "source_count": len(sources),
                "lines": lines,
                "internal": sorted(internal),
            }
        )
    return sorted(crates, key=lambda crate: crate["name"])


def tree_summary(relative):
    directory = ROOT / relative
    files = sorted(path for path in directory.rglob("*") if path.is_file())
    lines = 0
    for path in files:
        if path.suffix in {".py", ".js", ".wxml", ".wxss", ".json", ".md", ".css"}:
            lines += len(path.read_text(encoding="utf-8", errors="replace").splitlines())
    return len(files), lines


def render(crates):
    names = {crate["name"] for crate in crates}
    edges = [
        f"{crate['name']} → {dep}"
        for crate in crates
        for dep in crate["internal"]
        if dep in names
    ]
    all_rs = sorted(
        (path for path in (ROOT / "crates").rglob("*.rs")),
        key=lambda path: -len(path.read_text(encoding="utf-8", errors="replace").splitlines()),
    )
    stamp = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")

    out = [
        "# 📂 Bingocat — PROJECT MAP",
        "",
        f"*(由 `python3 tools/generate-project-map.py` 自动生成于 {stamp})*",
        "",
        "## One-Line Overview",
        "",
        "BongoCat 桌宠：Rust 2024 workspace（22 个 crate，GPUI 设置 UI + Product/Live2D runtime），"
        "另有浏览器版 `web/`、微信小程序版 `miniprogram/`、`tools/` 下的 Python 契约与打包校验脚本。",
        "",
        "## 数据流",
        "",
        "```",
        "cargo 构建 → just check / verify.yml（Rust 测试 + Python 契约 + wasm + 小程序校验）",
        "  → bongocat-packaging 出包与发布说明 → cargo-packager 出 .app/.dmg → 发布",
        "浏览器版 / 小程序版共用 bongocat-pomodoro-web 编译出的同一份 wasm 内核",
        "```",
        "",
        "<!-- BEGIN GENERATED: dep-graph -->",
        "```",
        f"# workspace 内部依赖边（{len(edges)} 条；A → B 表示 A 依赖 B）",
        *edges,
        "```",
        "<!-- END GENERATED: dep-graph -->",
        "",
        "<!-- BEGIN GENERATED: file-table -->",
        "## Crate 概览",
        "",
        f"共 {len(crates)} 个 crate、{sum(c['source_count'] for c in crates)} 个 .rs 文件、"
        f"{sum(c['lines'] for c in crates)} 行。",
        "",
        "| crate | 路径 | .rs 文件 | 行数 | 内部依赖 |",
        "|---|---|---:|---:|---|",
        *(
            f"| `{c['name']}` | `{c['path']}` | {c['source_count']} | {c['lines']} | "
            f"{', '.join(f'`{dep}`' for dep in c['internal']) or '—'} |"
            for c in crates
        ),
        "",
        "## 最大的源文件（top 12）",
        "",
        "| 文件 | 行数 |",
        "|---|---:|",
        *(
            f"| `{path.relative_to(ROOT).as_posix()}` | "
            f"{len(path.read_text(encoding='utf-8', errors='replace').splitlines())} |"
            for path in all_rs[:12]
        ),
        "",
        "## 非 Rust 目录",
        "",
        "| 目录 | 文件数 | 文本行数 | 说明 |",
        "|---|---:|---:|---|",
    ]
    for relative, note in (
        ("tools", "契约测试、fixture 校验、Cubism SDK 审查"),
        ("web", "浏览器版页面与 wasm 胶水"),
        ("miniprogram", "微信小程序四 tab + 云函数"),
        ("docs", "ADR、设计文档与约定"),
        ("shared", "平台无关的 schema 与 fixture"),
        ("resources", "预置模型、贴图与音频"),
    ):
        files, lines = tree_summary(relative)
        out.append(f"| `{relative}/` | {files} | {lines} | {note} |")
    out += [
        "",
        "<!-- END GENERATED: file-table -->",
        "",
        "<!-- BEGIN GENERATED: env-deps -->",
        "> 依赖以根 `Cargo.toml` 的 `[workspace.dependencies]` 为唯一来源（Rust 2024 / rustc 1.97）；"
        "Python 侧依赖见 `tools/requirements.txt`。",
        "<!-- END GENERATED: env-deps -->",
        "",
        "<!-- BEGIN GENERATED: edit-guide -->",
        "| 你要做什么 | 改哪个文件 |",
        "|---|---|",
        "| 改修仙/番茄钟域逻辑 | `crates/bongocat-pomodoro/` |",
        "| 改 runtime / 输入 / 渲染 | `crates/bongocat-runtime/` |",
        "| 改设置 UI | `crates/bongocat-ui/` |",
        "| 改平台适配 | `crates/bongocat-platform/`、`crates/bongocat-overlay/` |",
        "| 改打包/发布说明 | `crates/bongocat-packaging/`、`CHANGELOG*.md` |",
        "| 改校验契约 | `tools/tests/` |",
        "| 刷新本图 | `python3 tools/generate-project-map.py` |",
        "<!-- END GENERATED: edit-guide -->",
        "",
        "> `PROJECT_MAP.detail.md` 是上一版外部工具的逐函数明细，仅作历史参考；"
        "crate 级事实以本文件为准。",
        "",
    ]
    return "\n".join(out)


if __name__ == "__main__":
    crates = workspace_crates()
    assert crates, "the workspace declares no crates"
    OUTPUT.write_text(render(crates), encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)} for {len(crates)} crates")
