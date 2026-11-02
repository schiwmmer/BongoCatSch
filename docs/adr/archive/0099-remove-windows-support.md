# ADR-0099: 移除 Windows 支持，仅保留 macOS

状态：已定稿并落地（2026-10-29）

> **本 ADR 记录一次架构级收缩**：项目从「Windows + macOS 双平台」改为「macOS-only」。它既是决策记录，
> 也是本次移除范围内**惰性保留项**的权威清单——凡是代码里还留着 Windows 语义的东西，都在 §4 里
> 给出了保留理由，不留「为什么还在」的悬念。

## 1. 背景

项目从诞生起就是双平台目标（Windows 10 1903+ 与 macOS 12+），技术设计（ADR-003/0089 等）长期
并行维护两套后端：Windows 走 Win32 / D3D11 / DirectComposition / Raw Input / NSIS 安装器，
macOS 走 AppKit / Metal / NSPanel / CGEventTap / .app 打包。

本轮决策：**项目只做 macOS**。Windows 的维护成本（两套渲染后端、两套输入适配、两套安装器、
`#[cfg(target_os = "windows")]` 分散在 183 处）在只有 macOS 用户的实际情况下不再值得。
收缩释放的是**每一条 `cfg` 分支的持续认知成本**——双平台让每一处平台边界都必须同时想两遍。

## 2. 决策

删除 Windows 平台代码、CI、打包与资源；文档架构改 macOS-only；保留 ADR / benchmark 历史以供追溯。
分类处理：

| 类别 | 谓词真值（macOS） | 处置 |
| --- | --- | --- |
| A | FALSE | 删 `#[cfg(target_os = "windows")]` 属性 + 条目 |
| B | — | 重写为 `#[cfg(test)]`（测试专用） |
| C | TRUE | 仅删属性，保留条目（macOS 也走这里） |

删除清单：

1. **平台 cfg**：183 处 `#[cfg(target_os = "windows")]` 与 `cfg(windows)` → 清零。
2. **模块删除**：`crates/bongocat-app/src/taskbar_settle.rs` 及其测试模块——Windows 任务栏图标
   settle 机制（`show_taskbar_icon` 在 macOS 从不读取）。
3. **平台代码删除**：`crates/bongocat-platform/src/local_time.rs` 的 `#[cfg(windows)] fn read_offset`
   （Windows 用 `localtime_s`），保留 `#[cfg(unix)]` 实现。
4. **平台二进制删除**：`vendor/cubism/5-r.5/Core/dll/windows/` 与 `Core/lib/windows/` 的
   `Live2DCubismCore.dll` / `.lib` / `Live2DCubismCore_MD.lib`（无任何构建引用）。
5. **文档改写**：README 双语言、`AGENTS.md`、`docs/technical-design.md`、`docs/product-runtime.md`、
   `docs/TODO.md`、`shared/config/contract.md`、`shared/behavior/input-semantics.md` 全 macOS-only；
   删 Windows Raw Input / Win32 / D3D11 / NSIS / WM_* / RegisterHotKey / DPI / window_mode / taskbar 等整节。
   `docs/update-signing-guide.md` 头部已自注「保留作历史对照」，不改。
6. **配置与依赖**：根与 crates `Cargo.toml` 无 Windows feature；`.github` CI、`justfile` 无 Windows
   残留；无 `.nsi` / `.bat` / `.cmd` / `.ps1` / `.rc` 脚本。
7. **spike 清理**：删除 `spikes/input-windows/`、`spikes/overlay-windows/`（纯 Windows 试验项目）；
   `spikes/gpui-overlay-macos`、`spikes/gpui-settings`、`spikes/config-store` 的 Windows cfg 分支清零
   （含引用已删 `bongocat_overlay_windows_spike` 的死代码）。spike `Cargo.lock` 的 `windows-sys` 等
   为第三方 crate 平台条件传递依赖，保留。

## 3. 为什么只保留 macOS

- **单一后端**：渲染（Metal）、输入（CGEventTap）、窗口（NSPanel）、打包（.app）、安装（拖放拷贝）
  各剩一条路，每个平台边界只需想一遍。
- **释放双平台认知成本**：删掉的不是「一段可跳过的代码」，而是**每处 `cfg` 分支的持续核对义务**——
  `cargo check --target x86_64-pc-windows-msvc`、双后端渲染、双输入矩阵、双安装器不再需要。
- **与既有事实一致**：项目当前用户全部为 macOS；Windows 路径在近期版本已无验证资源。

## 4. 惰性保留项（兼容性强制，非疏漏）

| 保留项 | 位置 | 保留理由 |
| --- | --- | --- |
| `window_mode` config 字段 | `config_schema/overlay.rs` | `#[serde(deny_unknown_fields)]`：删除该字段会使含旧键的用户配置**反序列化失败**（配置损坏）。保留为惰性占位（默认 `false`，注释已标明 inert），macOS 不读取其语义 |
| `show_taskbar_icon` config 字段 | `config_schema/system.rs` | 同上，`deny_unknown_fields` 强制保留 |
| `show_taskbar_icon` 功能链 | `settings_commands.rs` / `worker.rs` / `capabilities.rs` / `snapshot.rs` / `ui-protocol` | config 字段保留 ⇒ 其读取/命令/快照/UI 契约一并保留以维持编译与契约完整性。macOS 上由 `UnavailableTaskbarIcon`（恒返回 `TaskbarIconUpdateFailed`）正确建模「无任务栏」，且 settings UI **不渲染**该开关（仅测试默认值 + 错误文案 key） |
| `mver` 的「Windows virtual key」 | `model-store/src/mver/vocabulary.rs` | 模型文件格式的**标识符方案**（非平台代码），删除会破坏既有模型文件的键名兼容；仅散文「live Windows input path」改为「live macOS input path」 |
| `product_windows.rs` 模块 | `bongocat-app/src/` | 模块名中的 `windows` 指**产品打开的窗口**（settings/update/pomodoro 窗口 + Dock 图标状态），非 Windows 平台。全文无 `cfg(windows)`、无 Win32 引用 |
| `updateWindows()` | `overlay/src/macos/renderer.rs` | AppKit 方法名，非 Windows 平台提及 |
| ADR-0010 / 0023 / 0089 | `docs/adr/` | ADR 历史按边界**保留可追溯**（正文含「Windows」字样是记录而非当前承诺） |
| `benchmark/windows-window-mode-2026-10-08.md` | `docs/benchmark/` | benchmark 历史，保留 |
| ADR-013 / 043 / 023 / 035 / 034 | `docs/adr/` | 正文保留作历史记录，**标注「随 Windows 支持的移除（ADR-0099）…决策失效」** |

> 第 1、2 行的 `deny_unknown_fields` 是硬约束：**保留字段是兼容性的唯一正确解**，不是「留一手」。
> 若未来要彻底移除，需先给旧配置提供迁移路径（读取时剥除未知键或走 schema 迁移）。

## 5. 影响面

- `crates/bongocat-app`：删 `taskbar_settle` 模块及测试；main.rs / projection.rs / snapshot.rs /
  input_diagnostics 相关散文改 macOS-only；product_windows 保留。
- `crates/bongocat-platform`：`local_time.rs` 删 Windows `read_offset`；依赖无 Windows。
- `crates/bongocat-ui-protocol` / `bongocat-model-store`：散文与 doc 注释改 macOS-only；
  `mver` 格式标识符保留。
- `vendor/cubism`：删 Windows DLL/LIB（3 个二进制）。
- 文档：`README.md` / `README.zh-CN.md` / `AGENTS.md` / `docs/technical-design.md` 全 macOS-only。
- 本 ADR：新增；ADR-013/023/034/035/043 标注失效。

## 6. 兼容性（AGENTS §3.1）

- **用户配置**：`window_mode` / `show_taskbar_icon` 字段保留（§4），旧配置读取不受影响；
  未删除任何 config 键，无迁移需求。
- **模型文件**：`mver` 键名方案未动，既有模型文件兼容。
- **构建**：`cargo check`（EXIT=0）与 `cargo check --tests`（EXIT=0）在删除后通过；
  仅剩 macOS 单一 target，无 `--target x86_64-pc-windows-msvc` 验证义务。

## 7. 采纳门槛

| 字段 | 内容 |
| --- | --- |
| `decision` | 项目收缩为 **macOS-only**：删除全部 Windows 平台代码 / CI / 打包 / 资源与文档散文（183 处 cfg 清零、taskbar_settle 模块、local_time Windows 块、3 个 Cubism 二进制）；**保留** ADR / benchmark 历史，以及 §4 的 `deny_unknown_fields` 强制惰性字段与 `mver` 格式标识符 |
| `evidence_level` | **L1** —— 删除由 `cargo check` 与 `cargo check --tests` 双绿（EXIT=0）背书；cfg 残留复核为 0 处 `target_os = "windows"`、0 处 `cfg(windows)` |
| `primary_bottleneck` | 不是技术问题，是**维护成本权衡**：双平台让每处平台边界必须同时想两遍；收缩是成本决策而非能力缺失 |
| `released_variables` | 本轮移除的是一整类平台（Windows）而非单变量：平台代码、CI、打包、资源、文档、cfg 分支。`held_constant`：macOS 行为（Metal / CGEventTap / NSPanel / .app / Dock 图标）一律不动 |
| `success_criteria` | 已达成：(a) crates 内 `target_os = "windows"` 与 `cfg(windows)` 均 0 处；(b) `cargo check` 与 `cargo check --tests` 均 EXIT=0；(c) vendor Cubism Windows 二进制已删且无构建引用；(d) README / AGENTS / technical-design 文档全 macOS-only；(e) `cargo clippy --all-targets` 与全量 `cargo test --all` 均全绿；(f) CI 验证的 `spikes/config-store` 经 `cargo clippy -D warnings` + `cargo test` 本地复现全绿 |
| `rollback_trigger` | 若日后需要重新支持 Windows，**回滚路径**：`git revert` 本提交（或恢复 §2 删除项）+ 撤销 §4 保留项标注即可，ADR-013/023/034/035/043 的「失效」标注回退为有效 |

## 8. 开放项

- **无**。本次为收缩决策，删除范围、保留项、兼容性均已在本 ADR 钉死；回滚路径见 §7 `rollback_trigger`。
