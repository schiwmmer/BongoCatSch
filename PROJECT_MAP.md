# 📂 Bingocat — PROJECT MAP

*(由 `python3 tools/generate-project-map.py` 自动生成于 2026-11-06 01:32)*

## One-Line Overview

BongoCat 桌宠：Rust 2024 workspace（22 个 crate，GPUI 设置 UI + Product/Live2D runtime），另有浏览器版 `web/`、微信小程序版 `miniprogram/`、`tools/` 下的 Python 契约与打包校验脚本。

## 数据流

```
cargo 构建 → just check / verify.yml（Rust 测试 + Python 契约 + wasm + 小程序校验）
  → bongocat-packaging 出包与发布说明 → cargo-packager 出 .app/.dmg → 发布
浏览器版 / 小程序版共用 bongocat-pomodoro-web 编译出的同一份 wasm 内核
```

<!-- BEGIN GENERATED: dep-graph -->
```
# workspace 内部依赖边（60 条；A → B 表示 A 依赖 B）
bongocat-app → bongocat-audio
bongocat-app → bongocat-config
bongocat-app → bongocat-i18n
bongocat-app → bongocat-input
bongocat-app → bongocat-live2d
bongocat-app → bongocat-live2d-render
bongocat-app → bongocat-log
bongocat-app → bongocat-model
bongocat-app → bongocat-model-store
bongocat-app → bongocat-overlay
bongocat-app → bongocat-platform
bongocat-app → bongocat-pomodoro
bongocat-app → bongocat-render
bongocat-app → bongocat-runtime
bongocat-app → bongocat-storage
bongocat-app → bongocat-ui
bongocat-app → bongocat-ui-protocol
bongocat-app → bongocat-update
bongocat-config → bongocat-input
bongocat-config → bongocat-storage
bongocat-live2d → bongocat-live2d-playback
bongocat-live2d → bongocat-live2d-render
bongocat-live2d → bongocat-log
bongocat-live2d → bongocat-model
bongocat-live2d → bongocat-model-store
bongocat-live2d → bongocat-render
bongocat-live2d → bongocat-storage
bongocat-live2d-render → bongocat-model
bongocat-live2d-render → bongocat-render
bongocat-log → bongocat-storage
bongocat-model-store → bongocat-model
bongocat-model-store → bongocat-render
bongocat-model-store → bongocat-storage
bongocat-overlay → bongocat-input
bongocat-overlay → bongocat-model
bongocat-overlay → bongocat-platform
bongocat-overlay → bongocat-render
bongocat-overlay → bongocat-runtime
bongocat-platform → bongocat-config
bongocat-platform → bongocat-input
bongocat-platform → bongocat-runtime
bongocat-platform → bongocat-storage
bongocat-pomodoro → bongocat-storage
bongocat-pomodoro-web → bongocat-pomodoro
bongocat-render → bongocat-input
bongocat-runtime → bongocat-audio
bongocat-runtime → bongocat-input
bongocat-runtime → bongocat-live2d
bongocat-runtime → bongocat-live2d-playback
bongocat-runtime → bongocat-live2d-render
bongocat-runtime → bongocat-model
bongocat-runtime → bongocat-model-store
bongocat-runtime → bongocat-render
bongocat-ui → bongocat-config
bongocat-ui → bongocat-i18n
bongocat-ui → bongocat-platform
bongocat-ui → bongocat-pomodoro
bongocat-ui → bongocat-ui-protocol
bongocat-ui-protocol → bongocat-input
bongocat-update → bongocat-config
```
<!-- END GENERATED: dep-graph -->

<!-- BEGIN GENERATED: file-table -->
## Crate 概览

共 22 个 crate、616 个 .rs 文件、171882 行。

| crate | 路径 | .rs 文件 | 行数 | 内部依赖 |
|---|---|---:|---:|---|
| `bongocat-app` | `crates/bongocat-app` | 101 | 24716 | `bongocat-audio`, `bongocat-config`, `bongocat-i18n`, `bongocat-input`, `bongocat-live2d`, `bongocat-live2d-render`, `bongocat-log`, `bongocat-model`, `bongocat-model-store`, `bongocat-overlay`, `bongocat-platform`, `bongocat-pomodoro`, `bongocat-render`, `bongocat-runtime`, `bongocat-storage`, `bongocat-ui`, `bongocat-ui-protocol`, `bongocat-update` |
| `bongocat-audio` | `crates/bongocat-audio` | 17 | 1586 | — |
| `bongocat-config` | `crates/bongocat-config` | 25 | 6879 | `bongocat-input`, `bongocat-storage` |
| `bongocat-i18n` | `crates/bongocat-i18n` | 7 | 959 | — |
| `bongocat-input` | `crates/bongocat-input` | 5 | 2500 | — |
| `bongocat-live2d` | `crates/bongocat-live2d` | 37 | 5800 | `bongocat-live2d-playback`, `bongocat-live2d-render`, `bongocat-log`, `bongocat-model`, `bongocat-model-store`, `bongocat-render`, `bongocat-storage` |
| `bongocat-live2d-playback` | `crates/bongocat-live2d-playback` | 16 | 1571 | — |
| `bongocat-live2d-render` | `crates/bongocat-live2d-render` | 1 | 718 | `bongocat-model`, `bongocat-render` |
| `bongocat-log` | `crates/bongocat-log` | 18 | 1599 | `bongocat-storage` |
| `bongocat-model` | `crates/bongocat-model` | 21 | 4201 | — |
| `bongocat-model-store` | `crates/bongocat-model-store` | 41 | 7673 | `bongocat-model`, `bongocat-render`, `bongocat-storage` |
| `bongocat-overlay` | `crates/bongocat-overlay` | 43 | 8763 | `bongocat-input`, `bongocat-model`, `bongocat-platform`, `bongocat-render`, `bongocat-runtime` |
| `bongocat-packaging` | `crates/bongocat-packaging` | 2 | 4006 | — |
| `bongocat-platform` | `crates/bongocat-platform` | 38 | 8667 | `bongocat-config`, `bongocat-input`, `bongocat-runtime`, `bongocat-storage` |
| `bongocat-pomodoro` | `crates/bongocat-pomodoro` | 29 | 29502 | `bongocat-storage` |
| `bongocat-pomodoro-web` | `crates/bongocat-pomodoro-web` | 2 | 700 | `bongocat-pomodoro` |
| `bongocat-render` | `crates/bongocat-render` | 17 | 1585 | `bongocat-input` |
| `bongocat-runtime` | `crates/bongocat-runtime` | 51 | 12618 | `bongocat-audio`, `bongocat-input`, `bongocat-live2d`, `bongocat-live2d-playback`, `bongocat-live2d-render`, `bongocat-model`, `bongocat-model-store`, `bongocat-render` |
| `bongocat-storage` | `crates/bongocat-storage` | 1 | 189 | — |
| `bongocat-ui` | `crates/bongocat-ui` | 106 | 40463 | `bongocat-config`, `bongocat-i18n`, `bongocat-platform`, `bongocat-pomodoro`, `bongocat-ui-protocol` |
| `bongocat-ui-protocol` | `crates/bongocat-ui-protocol` | 26 | 4608 | `bongocat-input` |
| `bongocat-update` | `crates/bongocat-update` | 12 | 2579 | `bongocat-config` |

## 最大的源文件（top 12）

| 文件 | 行数 |
|---|---:|
| `crates/bongocat-pomodoro/src/cultivation/state.rs` | 5339 |
| `crates/bongocat-pomodoro/src/cultivation/tests.rs` | 3925 |
| `crates/bongocat-packaging/src/main.rs` | 3247 |
| `crates/bongocat-platform/src/macos.rs` | 2345 |
| `crates/bongocat-app/src/main.rs` | 2188 |
| `crates/bongocat-pomodoro/src/cultivation/arts.rs` | 2155 |
| `crates/bongocat-ui/src/window/tests/cultivation_page.rs` | 2129 |
| `crates/bongocat-pomodoro/src/state.rs` | 2006 |
| `crates/bongocat-ui/src/window/render.rs` | 1875 |
| `crates/bongocat-ui/src/window/pomodoro_settings.rs` | 1727 |
| `crates/bongocat-pomodoro/src/cultivation/state_tests.rs` | 1473 |
| `crates/bongocat-ui/src/window/cultivation_settings/records.rs` | 1303 |

## 非 Rust 目录

| 目录 | 文件数 | 文本行数 | 说明 |
|---|---:|---:|---|
| `tools/` | 39 | 5041 | 契约测试、fixture 校验、Cubism SDK 审查 |
| `web/` | 9 | 2912 | 浏览器版页面与 wasm 胶水 |
| `miniprogram/` | 30 | 4629 | 微信小程序四 tab + 云函数 |
| `docs/` | 125 | 24738 | ADR、设计文档与约定 |
| `shared/` | 116 | 9023 | 平台无关的 schema 与 fixture |
| `resources/` | 190 | 2204 | 预置模型、贴图与音频 |

<!-- END GENERATED: file-table -->

<!-- BEGIN GENERATED: env-deps -->
> 依赖以根 `Cargo.toml` 的 `[workspace.dependencies]` 为唯一来源（Rust 2024 / rustc 1.97）；Python 侧依赖见 `tools/requirements.txt`。
<!-- END GENERATED: env-deps -->

<!-- BEGIN GENERATED: edit-guide -->
| 你要做什么 | 改哪个文件 |
|---|---|
| 改修仙/番茄钟域逻辑 | `crates/bongocat-pomodoro/` |
| 改 runtime / 输入 / 渲染 | `crates/bongocat-runtime/` |
| 改设置 UI | `crates/bongocat-ui/` |
| 改平台适配 | `crates/bongocat-platform/`、`crates/bongocat-overlay/` |
| 改打包/发布说明 | `crates/bongocat-packaging/`、`CHANGELOG*.md` |
| 改校验契约 | `tools/tests/` |
| 刷新本图 | `python3 tools/generate-project-map.py` |
<!-- END GENERATED: edit-guide -->

> `PROJECT_MAP.detail.md` 是上一版外部工具的逐函数明细，仅作历史参考；crate 级事实以本文件为准。
