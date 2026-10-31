# 架构、平台与设置框架

> 本文件由 `docs/adr/` 下 **35 个 ADR** 合并而成（2026-10-31）。
> 每个 ADR 保留原始编号（`ADR-XXXX`）与完整正文，可继续按编号引用；原始单文件已归档到 `docs/adr/archive/`，仅作考古与回滚，勿直接编辑。

---
<a id="adr-0001"></a>
# ADR-0001: Single Rust Application

状态：Accepted
日期：2026-08-28

## Context

BongoCat 需要在 Windows 和 macOS 上统一实现设置 UI、实时输入、动画、模型管理、窗口、渲染和系统集成，同时保持平台 API 的直接控制能力。

## Decision

BongoCat 自有应用代码统一使用 Rust 2024 edition，并在一个 Cargo workspace 中组织。业务模块通过强类型 Rust 接口协作；平台差异进入明确的 platform adapter。

官方 Cubism Core 平台二进制是唯一允许的厂商 FFI 例外。该例外只提供模型计算能力，不承载 BongoCat 业务逻辑。

## Consequences

- 状态、配置、模型和行为只实现一次。
- Windows/macOS 的窗口、输入和 GPU backend 仍分别实现。
- 平台 handle、裸指针和 `unsafe` 不能进入共享业务模块。
- Linux 可在首发后通过新增 platform/renderer backend 评估。

## Verification

Phase 0 必须证明 GPUI、独立 overlay、输入服务和 Cubism wrapper 可以在同一应用生命周期内可靠启动和停止。

---
<a id="adr-0002"></a>
# ADR-0002: GPUI for Settings UI

状态：Accepted, pending Phase 0 validation
日期：2026-08-28

## Context

BongoCat 需要一套跨 Windows/macOS、可主题化、支持复杂表单和模型管理的 Rust 设置界面。设置 UI 不是实时 Live2D 渲染表面。

## Decision

使用 GPUI 构建设置、模型管理、快捷键、权限、更新和诊断界面。首个 spike 固定 `gpui = "=0.2.2"` 并提交 `Cargo.lock`。

GPUI `Entity` 只拥有临时视图状态。运行时配置和业务状态通过强类型 command/snapshot 边界访问。

## Consequences

- 项目维护一套设置 UI 和 design system。
- 不依赖 Zed 应用内部 UI crate 或 GPUI renderer 私有接口。
- GPUI 升级必须独立验证，不能无约束跟随上游。
- GPUI 设置窗口关闭后，输入、动画和 overlay 必须继续运行；设置和更新窗口的普通 close 都销毁
  当前 GPUI 窗口，下一次打开重新创建。
- 设置窗口的侧边栏当前页面由 app coordinator 持有 process-local `SettingsNavigationMemory` 记忆，
  窗口重建时恢复；该 UI 状态不写入配置，应用重启后回到 Appearance。
- GPUI 0.2.2 的 Windows `WM_DESTROY` 回调会同步重入已借用的 `AsyncApp`；显式 Quit 必须先完成
  全部 BongoCat owner 的 shutdown/join，再由平台适配器跳过有缺陷的最终 GPUI 析构。普通设置/更新
  窗口销毁仍须通过双平台原生 smoke 验证，不得用进程退出代替或提前截断业务 shutdown。

## Verification

Phase 0 验证双平台字体、中文输入法、焦点、键盘导航、辅助功能、DPI/Retina、窗口销毁/重建、侧边栏页面记忆、后台生命周期和 shutdown。

历史证据（2026-08-31）：Windows CI run `33328391234`、job `99302481796` 证明保留窗口后的真实
`WM_DESTROY` 仍以 `0xC0000409` fast-fail；同期检查确认 Zed commit
`399258feeaf90ad8a3a208c99221ee87b6452f38` 的 `main` close callback 仍执行同步
`handle.update`。该证据限定上述兼容措施，并不解除未来恢复正常 GPUI 析构的门禁。

---
<a id="adr-0003"></a>
# ADR-0003: Native Overlay Renderers

状态：Accepted, macOS validated; Windows pending Phase 0 validation
日期：2026-08-28

## Context

模型窗口需要透明、置顶、穿透、低延迟、高 DPI/Retina 和可控 GPU 生命周期。GPUI 没有为双平台 Live2D 外部纹理提供稳定的公共合成接口。

## Decision

模型窗口使用独立原生 overlay：

- Windows：Win32 + D3D11 + DXGI + DirectComposition/DWM。
- macOS：AppKit `NSPanel` + Metal + `CAMetalLayer`。

Renderer 只消费不可变 `RenderSnapshot`，不访问 GPUI、配置或输入服务。

## Consequences

- 设置 UI 与实时渲染拥有独立的窗口和 frame loop。
- 两个平台分别实现 GPU backend，但共享模型求值和 render contract。
- 首发不为了后续 Linux 强行统一 GPU API。
- 窗口、renderer 和 GPU object 必须有明确的 owner 与析构顺序。

## Verification

macOS spike 已验证 `NSPanel` + `CAMetalLayer` 的透明 clear/present、独立于 GPUI renderer 的窗口生命周期，以及设置窗口与 overlay 同时存在时的显示/隐藏/退出流程。

Windows Win32 + D3D11 尚未在 Windows 实机验证；macOS 结果不能替代 Windows 结论。双平台模型绘制、真实 frame source、device lost/swapchain recovery 和 100 次真实窗口创建/销毁仍是 Phase 0 未完成项。

---
<a id="adr-0004"></a>
# ADR-0004: Reconcilable Input State

状态：Accepted, pending platform validation
日期：2026-08-28

## Context

系统快捷键、锁屏、安全桌面、权限变化或输入队列异常可能导致应用收到按下边沿却收不到释放边沿。只依赖事件配对会产生永久 pressed state。

## Decision

输入状态由三类信号共同维护：

1. 低延迟的按下/释放事件。
2. 对当前 pressed set 的系统状态校正。
3. 锁屏、睡眠、设备移除和服务重启等生命周期 `Reset`。

Key/button edge 使用可靠有序队列。鼠标移动和手柄轴使用 latest-value 合并，不得阻塞释放边沿。

Windows 以 Raw Input 为主路径，并用 `GetAsyncKeyState` 校正。macOS 使用 CGEventTap，并用 `CGEventSourceKeyState` 校正。

平台 adapter 统一使用单调时钟调度校正：默认间隔为 `250 ms`，同一个本地 pressed key 必须连续 `2` 次系统快照缺失才生成释放。单次查询异常只增加该 key 的待确认次数；后续快照确认仍按下时清零待确认次数。正常 `KeyUp` 和生命周期 `Reset` 立即清理待确认状态，`Reset` 不等待确认阈值；时钟回退不得推进校正调度游标。该策略由平台无关状态 contract 固定，平台只负责提供候选 pressed-set。

经过的时间不是第四条路径。早期的 `input.keyboard.release_fallback_timeout_ms` 按键释放超时已被移除：
用户仍按住的按键与丢失的松开事件在时长上无法区分，用时长区分等于把一帧变慢翻译成一个猫不再
相信的按键。捕获键盘按键因此没有超时配置，runtime 也不维护按键期限或与之相关的匿名计数。

唯一例外是 macOS 的 CapsLock。该键是锁存键，平台实测只投递「锁存翻转」事件而完全不投递物理松开
（见 Technical Design 的 FlagsChanged 条目），它的物理释放在平台层不存在，因此没有「用户仍按着」
这一状态需要区分。平台按产品既定的「短暂触发」语义在固定 `100 ms` 后合成释放边沿。该时长是按键
行为本身，不是超时配置、不进 schema、不暴露设置项，也不得推广到任何其它键。

## Consequences

- 单个释放事件丢失不会永久卡键。
- 队列溢出必须可观测，不能静默丢弃边沿。
- 丢失 `KeyUp` 的按键只能由状态校正或生命周期 `Reset` 释放；校正延迟上限由平台查询周期和确认
  次数共同决定，实机应分别测量权限、锁屏和睡眠恢复下的延迟。
- macOS CapsLock 的释放由 adapter 合成，`CGEventSourceKeyState` 反映锁存状态、无法替代它；该
  合成边沿走 `InputSource::Capture` 通道，runtime 侧因此把它计入 `captured_up`。这是已知且接受的
  精度损失，平台侧计数器不声称该边沿来自系统。
- Renderer 不直接查询系统键盘状态。

## Verification

Windows 强制覆盖 PixPin `Ctrl+Alt+A`、Win+L、PrintScreen 和 UAC 返回。双平台覆盖锁屏、睡眠、设备变化、服务 restart 和输入压力测试。

---
<a id="adr-0005"></a>
# ADR-0005: Cubism Core FFI Boundary

状态：Accepted, pending SDK and license validation
日期：2026-08-28

## Context

BongoCat 需要加载既有 `.moc3` 模型。官方 Cubism Core 以平台二进制形式提供，应用必须控制其版本、所有权、分发和错误边界。

## Decision

通过窄 Rust sys layer 调用官方 Cubism Core，并在其上建立 safe Rust wrapper：

```text
Cubism Core binary -> raw bindings -> Moc/Model wrappers -> model evaluation
```

原始指针不得离开 wrapper。Rust owner 必须保证 Moc、Model 和 backing buffer 的存活与析构顺序。BongoCat 的动作、表达式、物理、资源和状态逻辑保留在 Rust 模块中。

## Consequences

- 构建清单必须记录 SDK/Core 版本、来源、hash、架构和许可证。
- FFI callback 不执行阻塞工作，不允许 panic 穿越边界。
- 模型切换采用 prepare/validate/commit，失败保留当前模型。
- 不通过扩大非 Rust bridge 来绕过兼容性问题。

## Verification

Phase 0 使用三个预置模型验证 moc/model 生命周期、drawable 数据、motion、expression、physics/pose 需求和重复销毁，并形成 SDK 分发结论。

---
<a id="adr-0006"></a>
# ADR-0006: Linux Is a Post-Launch Evaluation

状态：Accepted
日期：2026-08-28

## Context

BongoCat 的首发目标是 Windows 和 macOS。Linux 的透明窗口、窗口层级、全局输入、托盘和桌面集成能力会随 X11、Wayland、portal 与 compositor 改变，不能从 Windows/macOS backend 的结果推断等价支持。

同时，runtime、配置、模型、动画和行为 fixture 本身不需要依赖具体桌面系统。首发实现不应为了未验证的 Linux 能力统一平台 backend，也不应让共享业务类型依赖 Win32、AppKit 或 GPU handle。

## Decision

Linux 不进入首发支持矩阵，不阻塞 Windows/macOS 的 Phase 0、实现和发布。

共享 Rust crate 保持平台无关：业务接口使用项目自有类型，平台能力通过 adapter 和 capability reporting 提供。Windows 使用 D3D11，macOS 使用 Metal；首发不为了 Linux 预先引入统一窗口、输入或 GPU abstraction。

Linux 只在首发后按独立 spike 评估。评估必须分别覆盖 X11 与 Wayland，并允许结论是某些桌面环境、compositor 或全局输入能力不受支持。尤其不得承诺 Wayland 下存在与 Windows Raw Input 或 macOS CGEventTap 等价的全局键鼠捕获。

## Consequences

- 首发 CI 可以对平台无关 crate 执行 Linux `cargo check`，但该结果不代表 Linux 产品支持。
- 新增共享业务 API 时不得暴露 HWND、Objective-C 对象、X11 handle、Wayland object 或 backend 专用 GPU 类型。
- Linux backend 可以在首发后新增，不要求修改已冻结的输入、配置和模型语义。
- 若 Linux 平台能力不足，产品必须显式降级或限制支持范围，不能用静默丢失输入代替能力声明。

## Verification

Phase 1 CI 对共享 crate 执行 Linux compile check。首发后按照 TODO 的 Linux backlog 建立 X11/Wayland 能力矩阵，并在全局输入、透明 overlay、渲染和系统集成均达到门槛后，另行提交支持决策 ADR。

---
<a id="adr-0007"></a>
# ADR-0007: Single Rust Runtime Environment

状态：Accepted
日期：2026-08-28

## Context

BongoCat 的输入、pressed state、动画、当前模型和配置协调需要单一事实来源。如果生产应用同时保留多个业务 runtime，或通过 IPC 在 UI、输入和模型状态之间复制事实，会扩大时序、恢复、版本兼容和 shutdown 风险。

历史实现仍用于行为观察和模型资源兼容证据，但不应进入 BongoCat 的生产执行链路。旧配置只作为已归档考古样本，不属于产品输入。

## Decision

生产应用使用一个 Rust 进程和一个业务 runtime owner。GPUI 设置界面、平台输入服务、Live2D 求值、原生 overlay 和系统集成都属于同一 Rust 应用生命周期，并通过强类型 Rust command、event 和 snapshot 协作。

不得引入第二套常驻业务运行环境、跨语言共享业务 core、sidecar 业务进程或用于同步核心状态的 IPC 边界。历史应用只作为行为与资源对照，不随 BongoCat 生产应用启动，也不作为功能 fallback。

官方 Cubism Core 平台二进制仍是 ADR-0005 定义的窄 FFI 例外。它只提供模型计算能力，不成为独立业务 runtime，不拥有应用状态。

安装或更新若因操作系统权限必须使用短生命周期 helper，需要在系统集成阶段单独威胁建模和 ADR；helper 必须使用 Rust、不得持有持续业务状态，也不得改变本 ADR 的单一应用 runtime 所有权。

## Consequences

- pressed state、动画、当前模型和配置协调只存在一个可变 owner。
- GPUI 关闭或重建不会创建第二份业务状态；UI 只发送 command 并读取带 revision 的 snapshot。
- 输入 callback、renderer 和平台服务不能绕过 runtime 直接维护产品状态。
- 发布切换前保留历史源码、tag 和样本；生产包不包含历史 runtime、Web 资产或旧配置读取器。

## Verification

Phase 0 验证 GPUI、输入服务、runtime、Cubism wrapper 和独立 overlay 可以在同一进程中按既定顺序启动、停止和重建。Phase 1 对 release dependency tree 与产物内容执行检查，确认不存在第二套业务 runtime、WebView 或 JavaScript runtime。

---
<a id="adr-0008"></a>
# ADR-0008: Application Identity and Isolated Storage Environments

状态：Accepted
日期：2026-08-28

## Context

BongoCat 需要稳定的应用身份，同时开发构建不能读取、覆盖或锁住用户的生产数据。旧版配置字段和存储布局不再是新应用的兼容目标，因此新配置可以直接采用清晰、一致的 Rust 领域命名。

仅依赖 `debug_assertions` 或可变命令行参数选择数据目录会让错误构建访问生产数据。只隔离 `config.json` 也不充分：模型、备份、日志、锁和其他可变文件仍可能互相污染。

## Decision

BongoCat 的 Bundle ID 固定为：

```text
com.ayangweb.bongo-cat
```

构建产物携带不可变的 `BuildEnvironment`：

```text
Development
Production
```

工作区内直接执行的 Cargo 命令默认选择 Development；Production build/package 必须显式启用
`bongocat-app/production` Cargo feature。环境选择不依赖进程环境变量；应用启动后不能通过 CLI、
环境变量或设置项切换环境。packaging 的 `--environment` 只作为产品 CLI 校验并映射为上述 feature。
正式启动 API 只从当前平台与该编译期环境解析数据根，不接受调用方传入 `StorageLayout` 或路径。
需要隔离临时根目录的进程级测试必须显式启用 `storage-test-injection`，该能力不进入默认产品
CLI/API，并且 Production 构建与该 feature 的组合在编译期失败。

持久数据根目录为：

| 平台    | Development                                                         | Production                                                         |
| ------- | ------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Windows | `%APPDATA%\com.ayangweb.bongo-cat\development\`                     | `%APPDATA%\com.ayangweb.bongo-cat\production\`                     |
| macOS   | `~/Library/Application Support/com.ayangweb.bongo-cat/development/` | `~/Library/Application Support/com.ayangweb.bongo-cat/production/` |

两个环境使用相同的目录结构和 schema：

```text
<data-root>/
  config.json
  window-state.json
  models/
  backups/
  logs/
  updates/
  locks/
```

锁、单实例命名、诊断和更新 channel 也必须包含环境身份。任何环境都不得探测、读取或回退到另一个环境的目录。

BongoCat 不读取或导入旧 Tauri/Pinia 配置。配置 JSON 键统一使用 `snake_case`，名称从当前产品领域语义定义，不保留旧字段 alias。

初始版本是全新的，当前完整 `config.json` 与 `window-state.json` 都以 `schema_version: 1`
开始。在首次正式发布前，新增字段直接修改当前 v1，不保留开发中间结构，也不实现迁移、
旧数据转换或历史版本兼容判断。解析边界保留版本字段并拒绝非 v1 数据，避免静默改写未知格式。
首次正式发布后，后续版本才以实际发布的 v1 为基线单独设计顺序、幂等迁移。

`window-state.json` 的领域名称明确限定其只承载 settings 与 overlay 的可恢复窗口状态，不能作为
通用 application state 容器。开发期旧路径不属于首版兼容输入；产品不读取、迁移或 fallback 到它。

## Alternatives Considered

- Cargo profiles and `debug_assertions` were rejected: both describe the Cargo profile, not the immutable product environment. A release Development smoke and a release Production package must remain distinguishable.
- Build-metadata crates such as `vergen`, `shadow-rs`, and `built` were rejected: they expose Git, Cargo, Rust, and system metadata, but they do not own or enforce BongoCat's mutually exclusive Development/Production data-root semantics. Adopting one would retain the current custom mapping and validation while adding a dependency and lockfile surface.
- `cfg_aliases` was rejected because it only aliases conditions; the project can express the two states directly with one Cargo feature and `cfg`.
- A build script that reads an environment variable was rejected in favor of Cargo's native feature resolution. Cargo now owns the selection path and feature propagation; the remaining explicit `compile_error!` represents the required product invariant that Production cannot include storage test injection.

## Consequences

- 首次启动 BongoCat 时生成当前环境的全新配置。
- 开发构建可安全使用合成数据和自定义模型，不影响生产安装。
- 旧配置目录和字段只保留为历史行为参考，不进入生产 dependency graph 或发布产物。
- 用户模型仍可通过受验证的显式导入流程加入；不会根据旧配置路径自动发现或搬运。
- 工作区直接 Cargo 命令默认 Development；Production build/package 必须显式启用
  `bongocat-app/production`。
- Production 构建不能携带存储根注入能力；需要隔离临时数据根的进程级测试必须使用独立 Development 测试产物并显式启用 `storage-test-injection`。
- 开发过程中产生的旧 schema 或中间数据需要删除并重新生成，不属于产品兼容输入。

## Verification

- 对 Windows/macOS path resolver 分别测试 Development 和 Production，断言根目录不同且内部相对结构一致。
- 工作区直接 `cargo check` 成功并编译为 Development；启用 `production` feature 后编译为
  Production，且 Production 与 `storage-test-injection` 的组合必须失败。
- 默认产品参数拒绝存储测试入口；CI 证明 Production + `storage-test-injection` 构建失败。
- 在两个环境写入不同 sentinel，重启后只读取各自数据。
- 验证锁、日志、备份、模型目录和更新 channel 均无跨环境访问。
- 发布产物验证 Bundle ID 精确等于 `com.ayangweb.bongo-cat`。
- 扫描发布依赖和运行日志，确认没有旧 Tauri/Pinia 配置探测。
- 扫描产品代码，确认不存在 schema migration、字段 alias 或旧结构转换路径。

---
<a id="adr-0009"></a>
# ADR-0009: GPUI Accessibility Gate

状态：Proposed；历史 release/UI-completion gate，项目自有 AccessKit 部分由 ADR-0054 取代
日期：2026-08-28

> 后续修订（2026-09-23）：ADR-0054 将设置 UI 收口为 visual-first。项目自有 AccessKit tree/native bridge/action 不再是 UI 完成或发布门槛；`gpui-kit` 传递语义可保留，但不是 BongoCat 语义契约。

## Context

设置 UI 必须向 macOS 辅助功能 API 暴露可识别的 label、value、role、错误和进度。`spikes/gpui-settings/` 在 macOS 26.5.2 上的人工 smoke 能识别应用、标准窗口、标题栏按钮和菜单，但不能识别 GPUI 绘制的 `Appearance`、`Theme`、模型名称输入框或 `Refresh` 控件。可见截图不能替代辅助功能树证据。

GPUI 0.2.2 的公开源码和 feature 列表没有为普通 element 提供 role/label/value 的通用 accessibility API。直接依赖私有 Zed UI crate、修改 GPUI 私有 renderer 或以隐藏原生控件伪造整棵表单，都会破坏已冻结的依赖和 UI 边界。

## Decision

GPUI 继续作为当前首选设置 UI，但在辅助功能 gate 通过前保持 provisional：

- 允许建立正式 UI crate 和最小产品窗口，但在门禁通过前不实现或宣称完整、可发布的设置界面；
- 不引入 GPUI 私有 API、未维护 fork 或隐藏控件 workaround；
- 使用 GPUI 公开的 raw window handle 安装项目自有 AccessKit semantic tree；macOS/Windows adapter 只负责系统协议，语义 snapshot 由 UI crate 维护；
- 辅助技术 action 必须通过有界强类型 channel 回到 GPUI 主线程，不允许平台 callback 直接修改 Entity 或 runtime；
- 可见控件和语义节点使用同一份 UI 状态，必须覆盖 role、label/title、value、focus、selected、loading/error 和支持的 action；
- 若 GPUI 仍无法提供基础语义，才启动 Iced 替代 spike。Iced 只作为此门槛的 fallback，不与 GPUI 并行进入产品。

截至本 ADR 日期，Iced 0.14.0 的公开 crate feature/source 中也未发现可直接满足 role/label/value 的通用 accessibility API，因此它是待验证候选，不是当前 go 结论。不能因为换 UI 框架名称就视为问题已解决。

## Gate

以下条件是 ADR-0054 前的历史验收定义。满足后才能将本 ADR 标记 Accepted；“项目自有 AccessKit tree”的当前完成与发布门槛已由 ADR-0054 退役。当时要求：

1. 选定版本的公开 API 能为设置表单提供稳定 role、label、value、错误和进度节点；
2. macOS AX tree 能读取主题选择、模型名称输入框、Refresh、错误和 loading 状态；
3. Windows UI Automation 能读取同等语义；
4. 键盘导航、真实 IME、剪贴板、缩放和窗口重建回归仍通过；
5. 方案不依赖私有 crate、未审阅 patch 或平台特定的业务状态副本。

若以上历史条件在完整设置 UI 实现前仍不满足，形成明确的 `NO-GO` 或 `GO WITH CONDITIONS` 决策，并为 Iced 或其他后续方案指定独立 owner、截止阶段和回退条件。ADR-0011 允许其他正式产品模块继续开发。ADR-0054 后，项目自有 AX tree 的部分不再阻塞当前 visual-first UI。

## Evidence

- GPUI 版本固定为 crates.io `0.2.2`，见 `spikes/gpui-settings/Cargo.toml`；
- macOS 本机通过 AppKit AX API 读取 8 个项目语义节点，并通过 `accessibilityPerformPress` 将 Dark radio action 送回 GPUI；
- commit `fd9ad85` 的 push run `33255204781`、job `99107586036` 使用进程外 UI Automation client 读取同等节点，并通过 `SelectionItem.Select` 验证 Dark radio 的 selected/action 闭环；
- 2026-08-30 的 macOS `.app` smoke 读取 Reset 的 help text，并验证打开确认框后只暴露 modal `AlertDialog` 子树、初始 Cancel 焦点、键盘关闭与语义树恢复；commit `45b8dba` 的 push run `33273470907`、job `99156013603` 通过同一实现的 Windows UI Automation dialog 门禁；
- macOS `.app` 的延迟 runtime probe 已通过 AX value/invalid 状态观察到 `Runtime Ready · revision 1` -> `runtime probe failed` -> `Runtime Ready · revision 2`；commit `21ee8aa` 的 push run `33291750411`、job `99204478369` 与 pull request run `33291751558`、job `99204481348` 已从 Windows UI Automation 依次读取 loading、注入错误、retry 和 revision 2 恢复。runner 托管 UIA client 不提供 `AriaPropertiesProperty` 标识，因此仍无法据此验证 AccessKit 的 `busy=true` 投影；VoiceOver/Narrator 操作和宣读仍未验证；
- 双平台设置 spike 通过原生合成 mouse-move 进入 GPUI 平台事件链，验证 Reset hitbox、500ms tooltip delay/build 和 hover exit；物理 pointer 与 VoiceOver/Narrator tooltip 朗读仍未验证；
- Iced 候选版本与 feature 清单通过 `cargo info iced@0.14.0 --verbose` 获取，源码检索未发现通用 accessibility surface。

---
<a id="adr-0011"></a>
# ADR-0011: Progressive Implementation With Release Gates

状态：Accepted
日期：2026-08-30

> 后续修订（2026-09-23）：ADR-0033 已退役 Windows 原生 ARM64，Windows on ARM 改由 x64 仿真覆盖；因此本文的 Windows ARM64 desktop Core 发布门槛只作历史记录。

## Context

Phase 0 已通过独立 spike 验证 runtime、配置隔离、GPUI 设置窗口、原生 overlay
和可校正输入的核心所有权模型。剩余证据主要依赖厂商授权、标准 Cubism SDK、
物理设备、辅助技术、目标系统和签名环境。把这些外部证据全部作为创建产品
workspace 的前置条件，会阻止已经具备自动化契约的业务模块进入正式实现。

维护者已提供仓库外的 `CubismSdkForNative-5-r.5.zip`，并明确授权本地技术
验证和正式产品代码开发；公开发布授权和实机验收延后到产品完成后处理。

## Decision

项目采用渐进式实现，并把开发门禁与发布门禁分离：

- 允许建立正式 Rust workspace，并逐步实现 runtime、配置、模型、UI、平台和
  renderer；每项仍必须满足自己的自动化测试和代码边界。
- Phase 0 未完成项继续保持未勾选。缺失的 Cubism 行为、物理设备、辅助功能、
  GPU、签名和目标系统证据不能由编译或合成测试替代。
- Cubism 发布授权、最终 SDK/二进制清单、Windows ARM64 desktop Core、实机矩阵、
  签名、notarization、更新回滚和 soak 结果全部是 stable 发布门禁，不再阻止
  不公开分发的本地开发。
- 维护者批准把功能开发所需的最小 Core、header、生成 bindings 和预置模型固定到
  `vendor/` 与 `resources/`，使本地开发和目标 ABI 检查可复现。完整
  SDK ZIP 与 Framework 源码不进入仓库；公开安装包仍在发布阶段核对 attribution、
  再分发范围和最终合规清单。
- 产品构建默认不联网，也不从未固定来源下载 SDK。缺少本地 SDK 时，与 Cubism
  无关的 workspace、测试和 CI 必须保持可用。
- 完整设置 UI 仍受 ADR-0009 的辅助功能条件约束；允许建立 UI crate 和最小产品
  窗口，但不得把未通过的交互或辅助功能宣称为可发布功能。

## Supplied SDK Disposition

当前本地基线是 Cubism SDK for Native `5-r.5`，archive SHA-256 为
`7ff3a4bbc19c0a8728965aa522ab77eb11b252916453e68a8a78d3b71188bb12`，Core
版本为 `06.00.0001`。它包含 Windows x64 和 macOS arm64/x64 Core，不包含
desktop Windows ARM64 Core。

此前提供的 `CubismSdkMotionSyncPluginForNative-5-r.2.1.zip` 是可选 MotionSync
插件，SHA-256 为
`26baaf30b0ebb26bc6253884aa61b010666e2ecc2491e3a7fff6d43eb64d6548`。它不是
标准 Native SDK，不能加载 `.moc3`，当前不进入产品范围。

## Consequences

正式实现可以从已验证的 runtime/config 契约开始，不再等待外部测试人员或合规
流程。发布状态必须单独判断；“功能已实现”不表示“可以公开分发”。如果后续发布
核对、SDK ABI 或实机验证失败，受影响的发布目标保持阻塞，并通过后续 ADR 调整
能力或目标矩阵，不能用错误架构的 artifact 或模拟结果绕过。

---
<a id="adr-0012"></a>
# ADR-0012: JSON Localization

状态：已接受（2026-09-10；2026-10-06 修订）

## 决策

BongoCat 使用 `rust-i18n = 4.2.4` 加载编译期嵌入的 JSON 语言资源。应用层资源由独立的 `bongocat-i18n` crate 管理，默认语言为 `en-US`，已落地的语言为 `zh-CN`、`zh-TW`、`ar-SA`、`vi-VN`、`pt-BR`、`ko-KR` 与 `fr-FR`。

**语言命名采用带地区子标签的提名式**，与 `en-US` 一致：一份语言只发一份 catalog，地区子标签提名这份文案所依据的主要变体，并不声称 catalog 未携带的地区特化。`en-US` 本身也是这种名字——它同样服务 `en-GB`。

**解析走 RFC 4647 的 language-subtag fallback**（浏览器 `Intl`、CLDR 与操作系统自身的做法）：
按 primary subtag 匹配已发布 catalog，`ar`/`ar-EG`/`ar_MA`/`ar-SA` 全部落到 `ar-SA`，`vi` 与
`vi-VN` 全部落到 `vi-VN`，`pt`/`pt-PT`/`pt-BR` 全部落到 `pt-BR`，`ko` 与 `ko-KR` 全部落到 `ko-KR`，
`fr`/`fr-CA`/`fr-BE`/`fr-CH` 全部落到 `fr-FR`，`en`/`en-GB` 全部落到 `en-US`。
因此机器实际上报的 tag 几乎总不是 catalog 自身的名字，而 primary subtag 匹配让它们收敛到同一份
文案，而不是各自回退英文。`pt-BR` 尤其说明这件事的必要性：葡萄牙语没有单一写法，catalog 只发
一种变体，报 `pt-PT` 的机器读到的也是它。

**语言枚举的变体名按语言本身命名，不带地区**：`Arabic`、`Vietnamese`、`Portuguese`、`Korean`、
`French` 各自服务整个语言，地区子标签只是提名，`PortugueseBrazil` 这类名字会声称 catalog 并不携带的地区特化。
语言下拉里的名称按 endonym 规则写成本地人自称语言的名字，同样不附地区后缀——`Português` 而不是
`Português (Brasil)`，`العربية` 而不是 `العربية (السعودية)`，`한국어` 而不是 `한국어(대한민국)`，
`Français` 而不是 `Français (France)`，对应的 i18n key 也只写 `portuguese`、`arabic`、`korean`、
`french`。`ChineseSimplified` 是唯一保留限定词的名字，
因为它命中的确实是脚本分叉：`zh` 下的简繁是两种书写体系。

`zh` 是唯一不能只看 primary subtag 的情形：简繁是不同书写体系而非地区变体，因此两种写法各发
一份 catalog（`zh-CN` 简体、`zh-TW` 繁体），并在 `zh` 分支内按书写体系子标签直接分流，不进入
下面的 subtag 循环。`hant` 与 `TW`/`HK`/`MO` 判为繁体，`hans` 与其余 `zh` 标签判为简体——两种
拼写都要覆盖，因为平台只报地区子标签时 `TW`/`HK`/`MO` 是唯一可用的线索。
`bongocat-config` 的 `Language::from_system_locale` 与 `bongocat-i18n::locale_code` 各做一次
同样的判定：两个 crate 之间不存在依赖方向，不能共用一个 helper，行为必须由测试对齐。

语言文件放在 `crates/bongocat-i18n/locales/`，每种语言一个 JSON 文件，使用 `_version: 1`
和真正嵌套的领域结构。`rust-i18n` 在编译期将嵌套路径解析为查找 key；JSON 源文件本身不得使用
点号分隔的扁平 key。Rust UI 在文案实际使用处直接引用稳定的领域路径，不内嵌翻译文本或维护
enum 到 key 的集中映射。

新增一种语言是一次**数据加注册**的改动，不改变本 ADR 的任何结构决定：写一份与 `en-US` 同
key、同占位符的 JSON，然后在 `build.rs` 的 `CATALOGS`、`Language`/`SettingsLanguage` 两个
枚举及其 `code`/`catalog_locale`/`from_system_locale`/`resolve`、`settings_language_display_name`
和 `tools/validate-locales.py` 的 `EXPECTED_LOCALES` 中各加一项。语言下拉里每种语言用自身
文字书写（endonym），而不是当前窗口语言的名字。

## 约束

- 翻译 key 使用小写 `snake_case`，按 `navigation`、`settings`、`models`、`shortcuts`、
  `diagnostics`、`about`、`actions`、`status` 和 `errors` 等领域分层；字段名必须表达具体上下文。
- 插值使用 `rust-i18n` 的 `%{name}` 语法；所有语言必须保持相同的占位符集合。
- 找不到语言或 key 时回退到 `en-US`；`system` 在配置/平台层先解析为受支持语言。
- GPUI 语言切换通过已有带 revision 的设置 snapshot 触发重绘；本地化查询不进入 overlay frame loop。
- 新文案必须先加入 JSON，再由 Rust 使用 key；禁止在 `.rs` 中新增自然语言翻译文本。
- `bongocat-i18n` 是唯一调用 `rust_i18n::i18n!` 的 catalog owner。UI 使用其 `text(locale, key)`
  facade，并在使用点写出 key；不得为 UI crate 再初始化同一份 catalog 或依赖全局 locale。
- 测试/CI 必须检查 JSON 可解析、语言 key 集合相同、值为非空字符串且占位符集合一致。
  语言清单只在 `crates/bongocat-i18n/src/tests/mod.rs` 的 `LOCALES` 一处枚举，各条比较型测试
  都遍历它，因此新增语言不会让某条比较悄悄少覆盖一种语言。
- UI 文案的书写约定见 `docs/localization-copy-conventions.md`。当前已落地的一条：**省略号一律写
  单个 `…`（U+2026，视觉上是三个点）**，禁止中文排版习惯的 `……`（六个点）与拉丁写法的 ASCII
  `...`——同一个 key 由所有语言共用，写法必须与语言无关。该规则由 `tools/validate-locales.py`
  机械强制，不依赖 review。

## 取舍

`gpui-kit` 的底层 `gpui-component` 也使用 `rust-i18n`，因此依赖生态一致。`rust-i18n` 与传递的 `rust-i18n-macro`／`rust-i18n-support` 必须停在同一个补丁序列：4.2.4 的宏展开调用 `rust_i18n::replace_patterns_cow`，只存在于 4.2.4 的运行时 crate，跨版本混用会让 `gpui-component` 与 `bongocat-i18n` 同时编译失败。JSON 比 YAML/TOML 更适合现有前端资源、翻译工具和跨语言校验；本方案不引入 Fluent 的复数/选择语法。若后续产品需要复杂 ICU/Fluent 消息，应另行提交 ADR，不在业务代码中混用第二套格式。

---
<a id="adr-0013"></a>
# ADR-0013: Startup Item Capability and Environment Isolation
> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

状态：Superseded by ADR-0043
日期：2026-08-31

## Context

BongoCat 支持 macOS 12+，但 `SMAppService.mainAppService` 只在 macOS 13+
可用。Development 与 Production 又固定使用相同 Bundle ID；若开发构建直接注册 main
app login item，会覆盖同一用户的生产启动项系统状态。Windows 的当前用户启动项没有同样
的 Bundle ID 限制，但仍必须按构建环境隔离名称与目标命令。

启动项是可选平台能力，失败不能阻止 runtime、overlay 或设置窗口启动。UI 需要区分未启用、
已启用、路径过期、等待系统批准和当前环境不支持，而不能把所有情况压缩成一个布尔值。

## Decision

- 共享平台 API 使用稳定的 `StartupItemEnvironment`、`StartupItemState` 和
  `StartupItemError`，不暴露 registry handle、Objective-C object 或第三方错误类型。
- Windows 使用当前用户 `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`，
  Development 与 Production 使用不同 value name。启用时写入当前 executable 的带引号绝对
  路径和 `--run-seconds 0`；状态检测必须区分完全匹配与 stale value；禁用只删除当前环境的
  value，不读取或修改另一环境。
- macOS 13+ Production `.app` 使用 `SMAppService.mainAppService`。状态映射保留
  not-registered、enabled、requires-approval 和 not-found；`not-found` 是可显式尝试注册的
  预注册状态，不在 adapter 或 UI 提前判为无效。注册/取消注册错误只向上返回稳定匿名 code。
- macOS 12 返回明确的 unsupported capability，应用其余功能继续运行。不得回退到已废弃的
  `SMLoginItemSetEnabled`、自行写 LaunchAgent 或修改用户登录项数据库。
- macOS Development 构建只报告 unsupported build environment，不注册或取消注册
  `mainAppService`，从而不改变 Production 的系统登录项状态。真实变更 smoke 使用临时签名的
  Production `.app` 和隔离测试用户/CI runner，并从 `/Applications` 下的临时唯一安装目录
  启动，使 LaunchServices 与 `SMAppService` 验证的是安装态 main application。
- 启用、禁用只能由显式用户 command 触发；正常启动只读取状态。平台失败不改变配置，也不
  阻塞 input/runtime/renderer。

## Consequences

- macOS 12 用户可使用产品核心功能，但设置页会明确显示登录启动不可用。
- 开发构建不能直接演练 macOS 注册变更；平台 contract test 与 Production bundle smoke 负责
  覆盖，公开发布前仍需签名构建和 System Settings 批准路径实测。
- Windows 安装位置变化会显示 stale，而不是误报 enabled；用户再次启用可原子覆盖当前环境
  value。
- 启动项状态属于平台 snapshot，不进入配置 schema，也不由 runtime 动画状态持有。

## Verification

- 纯 contract test 覆盖所有 state/error code、环境命名和 Windows command 编码。
- Windows 临时当前用户 value smoke 覆盖 disabled -> enabled -> stale -> disabled，并保证只
  触及当前环境 value。
- macOS 12 availability probe 不引用缺失 class；macOS 13+ Production bundle smoke 从
  `/Applications` 的临时安装目录覆盖 status、register/unregister 和 requires-approval 映射，
  结束时恢复原状态并注销/删除临时 bundle。首次 `not-found` 注册后取消注册允许规范化为
  `not-registered`，因为两者都不留下启用的登录项。
- 双平台 smoke 后运行完整 format、Clippy、workspace test 和 release check。

---
<a id="adr-0014"></a>
# ADR-0014: Formal Settings Accessibility Bridge

状态：Accepted（2026-08-31）

> 后续修订（2026-09-23）：ADR-0054 已退役项目自有设置语义桥。本文保留作为当时的 AccessKit/action 实现证据；当前设置 UI 只验收可见控件、焦点和键盘导航。

## Context

The settings window needs project-owned accessibility semantics before the startup-item UI can
close its accessibility gate. GPUI 0.2.2 does not provide a stable public element-level semantic
API, while platform accessibility adapters must not become a second business-state owner.

## Decision

- `bongocat-ui` builds an `AccessibilityTree` from its existing page, snapshot, pending and focus
  state. Node IDs, roles, values, toggles and supported actions are project-owned types.
- `bongocat-platform` owns the AccessKit macOS/Windows adapter and receives only a copied raw
  window handle plus the project tree. It does not read settings, runtime or config state.
- AccessKit Click/Focus actions cross a bounded capacity-32 channel and are applied by the GPUI
  entity. Clicks reuse the existing typed settings commands; focus is applied through existing
  `FocusHandle`s on the next render.
- Disabled/loading/unsupported controls do not expose mutation actions. Adapter diagnostics count
  forwarded and rejected actions without recording labels, key identities or user content.
- Product smoke may inspect the native AX/UIA role, value, enabled/focusable state and advertised
  action pattern. It must not toggle the startup item; platform lifecycle smoke remains responsible
  for mutation and restoration.
- The bridge is attached after the GPUI native handle exists but while `WindowOptions::show` is
  still false, then the window is activated. It is retained by `SettingsView` and dropped before
  the corresponding GPUI window is destroyed; attachment failure leaves the window hidden and is
  returned to the product coordinator.

## Consequences

The startup switch now has a formal switch role, value, toggle state and action mapping on both
platform adapters. This does not claim VoiceOver/Narrator or physical assistive-technology
verification; those remain release evidence gates. If GPUI later exposes an equivalent stable API,
the platform adapter can be removed without changing runtime or settings command contracts.

---
<a id="adr-0015"></a>
# ADR-0015: Separate UI Snapshot and Configuration Revisions

状态：已接受

## 背景

设置 snapshot 需要在 runtime、平台状态、输入诊断和模型目录变化时刷新。配置写入同时需要
compare-and-swap，防止窗口中的旧编辑覆盖后台配置。若两者共用一个 revision，纯诊断变化会
错误拒绝仍然有效的配置编辑。

## 决策

- `SettingsSnapshot.revision` 是 UI 可观察内容的单调版本，只用于丢弃过期异步结果和刷新视图。
- `SettingsSnapshot.config_revision` 是当前环境持久化配置的版本；没有可编辑配置 snapshot 时为 `None`。ADR-0054 后，损坏配置按备份/默认值 fallback，恢复路径仍进入普通设置窗口，不再有 recovery-only 版本。
- 所有已接入的配置/模型选择 command（overlay settings、motion audio、model selection）携带
  `expected_config_revision`，settings worker 在调用 Application 写入前比较它。Overlay visibility
  是 runtime-only command，不写配置；它仍携带当前 config revision 作为 stale-view guard。
- 比较失败返回匿名 `SnapshotOutdated`，不改变 runtime、配置文件或任何 revision；UI 读取最新
  snapshot 并保留可操作错误。

## 后续边界

模型选择、启动项和其他配置域接入 optimistic concurrency 时继续使用 `config_revision`；纯
平台/诊断状态不得改变该 token。`revision` 仍可因这些状态变化推进。

---
<a id="adr-0019"></a>
# ADR-0019: gpui-component Design System

状态：已被 ADR-0020 取代（2026-09-04）

> 后续修订（2026-09-23）：除 ADR-0020 的 facade 取代外，ADR-0054 进一步退役了项目 AccessKit bridge；本文的组件与语义细节均为历史证据。

## 背景

设置窗口需要与固定的 `gpui 0.2.2` 兼容的主题、输入和桌面控件。维护者决定将现有
`guise-ui 1.5.3` 替换为 `gpui-component`，并指定其官方组件文档作为 API 事实来源。

## 决策

- workspace 使用 GPUI Kit 官方开发版 `gpui-component 0.5.2` revision
  `c0946e6acdc9e2f984f317ef7f998ee2c79f1a87`，关闭未使用的可选 feature；其许可证为
  Apache-2.0，依赖 GPUI `0.2.2`。GPUI Kit 的 manifest 使用官方 Zed git source，提交的
  `Cargo.lock` 固定当前解析到的 GPUI revision `55c0cc36d18cf06c6c54f640a87ce83d09413ae4`，
  避免组件与应用加载两份 GPUI 类型；`gpui_platform` 使用同一 Zed source。
- 设置窗口在首次创建组件前调用 `gpui_component::init`，以官方 `Root` 作为窗口根视图，并在
  系统外观变化时同步 `Theme`。
- 该 GPUI revision 默认会为窗口安装自己的 AccessKit adapter，而现有项目桥接仍承载已验证的
  macOS AX/Windows UIA 语义和 action contract。过渡期所有 GPUI 应用入口使用
  `Application::new_inaccessible(gpui_platform::current_platform(false))`，仅禁用重复的 GPUI
  adapter，避免两套 `accesskit_macos` 向同一 NSView 注册同名 Objective-C 类并触发 abort；
  项目桥接继续提供辅助功能。迁移到 GPUI 原生 element-level 语义后必须同时删除项目桥接、
  直接 AccessKit 依赖和此兼容构造，并重跑双平台辅助功能门禁。
- 图标资源使用官方 `gpui-component-assets = 0.5.1` 的 `Assets`（Lucide SVG），通过
  `Application::with_assets` 注册给 GPUI；应用不自维护重复的 `icons/*.svg` 文件。
- UI 使用组件库的 `Settings`、`SettingPage`、`SettingGroup`、`SettingItem`、`SettingField`，以及
  `Button`、`Switch`、`Tag`、`Input` 和 `NumberInput`。偏好设置页面由官方 Settings
  sidebar 管理，页面内按职责使用 SettingGroup；Settings 自带的搜索会按 SettingItem 标题、描述和显式关键词过滤，页面/分组名称由 ADR-0066 作为关键词提供，业务关键词不再塞进可见描述。
  `InputState` 只持有编辑状态；`InputEvent`/`NumberInputEvent` 转换为现有 typed command，
  runtime snapshot 仍是持久状态事实来源。
- `gpui-component 0.5.2` 没有普通 `Card` primitive；`HoverCard` 是悬浮预览组件而非内容容器。
  Settings 分组使用官方 `GroupBoxVariant::Outline`，模型管理和诊断等复杂内容通过官方
  `SettingField::element` 嵌入，不另建设置框架。
- 第三方类型不进入 runtime、config 或 model 公共 API。停止维护或 GPUI 版本不兼容时，替换
  边界保持在 `bongocat-ui` 和窗口初始化代码内。

## 影响

`gpui-component` 提供的控件范围更广，但传递依赖和编译成本高于 Guise。锁文件必须提交，后续
升级继续按仓库依赖规则审计版本、许可证、平台构建和完整输入/窗口 smoke。双平台辅助功能、缩放
及真实交互证据仍是 UI 完成门禁，本 ADR 不将这些 TODO 标记完成。

---
<a id="adr-0020"></a>
# ADR-0020: GPUI Kit 统一依赖入口

状态：已接受（2026-09-04）

> 后续修订（2026-09-24）：ADR-0054 退役项目自有设置语义桥。上游已合并 `SettingGroup::variant()`，但包含该能力与 `Popover::arrow()` 的 crates.io release 尚未发布；当前直接依赖上游固定 revision `500852f449c05dc01920ec82f3ae2656a61d0387`，不再使用 `[patch.crates-io]` 或维护者 fork。对应 release 发布后恢复纯 registry 精确 pin。

> 后续修订（2026-09-28）：上游已发布 `gpui-kit v0.7.0`，该 release 同时包含
> `SettingGroup::variant()` 与 `Popover::arrow()`。按上一条与 ADR-0056 的既定条件，直接依赖
> 切回 crates.io 精确 pin `=0.7.0`，`longbridge/gpui-kit` git source 与 `deny.toml` 的对应
> 放行项一并删除。GPUI 依赖随之上移到 `gpui-pre 0.3.7`。
>
> 后续修订（2026-10-06）：升级到 crates.io 精确 pin `=0.7.1`，GPUI 依赖随之上移到
> `gpui-pre 0.3.8`。仍然只依赖同一个 registry 版本，不改变本 ADR 的替换边界。
>
> 后续修订（2026-10-12）：升级到 crates.io 精确 pin `=0.7.3`，GPUI 依赖随之上移到
> `gpui-pre 0.3.9`（Zed 快照 `93d745668805029476e6d7fbb19652116fed51de`）。仍然只依赖同一个
> registry 版本，不改变本 ADR 的替换边界。

## 背景

ADR-0019 直接组合 Zed git source 的 `gpui`、`gpui_platform` 与 GPUI Component 开发版及
assets。应用必须手工保证四个 source 的类型一致，manifest、import 和升级边界分散。GPUI Kit
随后在 crates.io 发布 `0.6.0`，提供 GPUI、platform、base、component 和 assets 的统一入口。

## 决策

- workspace 只直接依赖 `gpui-kit`，当前为 crates.io 精确 pin `=0.7.3`，并提交完整
  `cargo update` 后的 `Cargo.lock`。不再直接声明 `gpui`、`gpui_platform`、`gpui-component`
  或独立 assets crate，也不使用其它 git source 混装 GPUI。
- 代码从 `gpui_kit` 根使用 GPUI 类型，从 `gpui_kit::platform`、`gpui_kit::component` 和
  `gpui_kit::assets` 使用对应层；组件初始化统一调用 `gpui_kit::init`。
- 当前固定的 `gpui-kit` 使用 Apache-2.0 许可证。它的 GPUI 依赖
  通过 crates.io `gpui-pre` 同步包交付；当前 lockfile 解析到 `0.3.9`（v0.7.3 起
  `gpui-kit` 以 `=0.3.9` 精确 pin 该家族，`gpui-pre-reqwest` 仍为 `0.12.15`），包元数据声明
  Zed `gpui 0.2.2` revision `93d745668805029476e6d7fbb19652116fed51de`。因此项目不再直接
  覆写 GPUI，但也不把该传递包误记为 crates.io 包名 `gpui = 0.2.2`。
- 2026-09-19：`gpui-kit` 由 `=0.6.1` 升级到 `=0.6.4`（v0.6.2 功能版与 v0.6.4 补丁的
  最新稳定版；无 breaking API 变化），升级范围仍在 `bongocat-ui` 与 `bongocat-app`，
  不改变本 ADR 的替换边界。
- 2026-09-22：`gpui-kit` 由 `=0.6.4` 升级到 `=0.6.6`（crates.io 最新非 yanked 稳定版；
  v0.6.5/v0.6.6 为补丁：masked label 跳过高亮，以及把 `gpui-pre` 改为 `=0.3.6` 精确
  pin，无 breaking API 变化）。lockfile 中 `gpui-kit` 本体之外，gpui-base/
  gpui-component/gpui-kit-assets `0.6.6` 与 `gpui-pre 0.3.6`（Zed 快照
  `bcf6582ce3500df93a8a39366640173e6786cea6`，`zed-version` 仍为 `0.2.2`）在本次
  `cargo update` 前已由此前一次完整 `cargo update` 解析到位，本次无 diff；v0.6.6 的
  `=0.3.6` 约束恰好与既有解析一致。升级范围仍在 `bongocat-ui` 与 `bongocat-app`，
  不改变本 ADR 的替换边界。
- 2026-09-24：`cargo search gpui-kit --limit 1` 与 `cargo info gpui-kit` 仍显示 crates.io
  最新稳定版为 `0.6.6`，但该 release 不含上游后来合并的 `SettingGroup::variant()` 与
  `Popover::arrow()`。上游 merge commit `500852f449c05dc01920ec82f3ae2656a61d0387`
  的 package 元数据仍为 `0.6.5`，因此本仓库直接固定该完整 commit；lockfile 中
  `gpui-kit`、`gpui-component`、`gpui-base`、`gpui-component-macros` 与
  `gpui-kit-assets` 统一为同一 git source，`gpui-pre` 仍为 crates.io `0.3.6`。
- 同一 revision 由 `Root` 自动挂载 dialog、sheet 与 notification layer；业务根视图删除旧的
  `Root::render_*_layer` 调用。`PopConfirm` 转发 `Popover::arrow(bool)`，模型删除确认启用
  anchor-aligned arrow。对应上游 release 发布后，本条临时 git source 决策自动失效并恢复
  crates.io 精确 pin。
- 2026-09-28：`gpui-kit v0.7.0` 发布，包含上述两项 API 与 `Root` 的自动挂载行为，因此
  切回 crates.io 精确 pin `=0.7.0`，删除 `longbridge/gpui-kit` git source 与 `deny.toml`
  的对应放行项。lockfile 中五个 suite package 统一解析为 `0.7.0`，GPUI 同步包升到
  `gpui-pre 0.3.7`（v0.7.0 以 `=0.3.7` 精确 pin 该家族）。项目未使用 release notes 中
  breaking change 涉及的 Chart/Plot、DatePicker/TimeField、Table、TextView heading 与
  表单/表格/弹层 primitive 迁移面，源码零改动；`Root` 与 `WindowExt` 仍由业务窗口入口
  直接使用。
- 2026-10-06：`gpui-kit` 由 `=0.7.0` 升级到 `=0.7.1`（crates.io 最新非 yanked 稳定版）。
  suite 五个 package 统一为 `0.7.1`，GPUI 同步包升到 `gpui-pre 0.3.8`（v0.7.1 以 `=0.3.8`
  精确 pin 该家族；同版本内含 continuation-indent API 适配，保持原有换行行为）。release notes
  里的两处行为变化——chart 首次绘制动画默认开启、Questionnaire 单选即确认——只涉及项目未使用的
  组件；新增的 speech/ColorSelect 与 `RenderedText::source` 也是增量 API。IME 候选框定位、输入框
  单行裁剪、disabled Radio、菜单键盘高亮、CJK 标点换行等修复落在设置界面实际使用的组件上，
  源码零改动。workspace 范围的 `cargo update` 同时把 `rust-i18n-macro`/`rust-i18n-support` 带到
  4.2.4，与 `gpui-component` 展开 `t!` 所需的 `rust-i18n` 运行时不再匹配，因此直接 pin 由 `=4.2.2`
  上调到 `=4.2.4`（ADR-0012）。
- 2026-10-12：`gpui-kit` 由 `=0.7.1` 升级到 `=0.7.3`（crates.io 最新稳定版；上游发布说明
  声明 v0.7.3 取代 v0.7.2 并包含其全部改动）。suite 五个 package 统一为 `0.7.3`，GPUI 同步包
  家族统一为 `gpui-pre 0.3.9`（v0.7.3 以 `=0.3.9` 精确 pin 该家族）。lockfile 随之移除
  `gpui-pre` 不再需要的传递依赖——`tracing-subscriber`/`sval`/`value-bag` 系列、
  `bindgen`/`clang-sys`/`cexpr`/`prettyplease`、`bzip2` 与 `libbz2-rs-sys`、`cgl`、
  `core-graphics2`、`core-video`、`io-surface`、`objc2-core-media`、
  `objc2-screen-capture-kit`、`async-compression`/`compression-*`——并新增 v0.7.3 引入的
  可选后端 `gpui-fast 0.1.7` 家族与 `Diff` 组件所需的 `similar`。`libbz2-rs-sys` 已不在依赖图
  中，`deny.toml` 为它保留的 `bzip2-1.0.6` 例外一并删除（`cargo deny` 会把它报成
  unmatched license exception）。
- v0.7.3 的两处行为变化落在项目实际使用的组件上：`TextView` 默认排版改为按继承字号计算标题
  字号（0.7.1 是相对硬编码 `px(14.)` 的绝对 rem 阶梯，更新窗口以 `.text_xs()` 渲染更新日志时
  二级标题会到 21px），段落间距由 `1rem` 收到 `0.75rem`；设置列表的横向内边距移进列表行，
  焦点环不再被列表裁剪。两者都是外观变化，不涉及项目代码的 API 迁移面——Dialog 入场动画、
  `TooltipDefaults`、DataTable `header_row_height`、Editor 补全、`InputState::add_selection`
  等均为项目未使用或纯增量能力，源码零改动。
- `gpui-fast` 是 v0.7.3 新增的可选 feature，启用后整套 GPUI 实现由 `gpui-pre` 换成 wgpu 后端的
  `gpui-fast`。本 ADR 不启用它：后端替换需要单独的 ADR、双平台实机证据与设置窗口、IME、缩放、
  窗口重建回归，不能作为依赖升级的附带结果。
- GPUI Kit 默认 component/assets feature 正好覆盖当前设置窗口。tree-sitter、decimal、
  inspector 和 test-support 等可选 feature 不启用。其 native facade 还会引入配套 HTTP client
  与 TLS 传递依赖；这些依赖不得进入 BongoCat 的业务 API。
- 当时保留项目 AccessKit bridge、typed command/snapshot 和独立 overlay 边界；`Application::new_inaccessible` 通过 `gpui_kit::platform` 构造。ADR-0054 后项目桥接已退役，`new_inaccessible` 只作为禁用 GPUI adapter 的兼容选择保留，不再形成 BongoCat 的辅助功能 contract。
- 替换边界限定在 `bongocat-ui` 与 `bongocat-app` 的窗口入口。上游停止维护、许可证变化或
  GPUI 版本不兼容时，只替换这一 UI 边界，不向 runtime、config、model 或 renderer 扩散
  GPUI Kit 类型。

## 影响

manifest 和 Rust import 只有一个版本入口，避免应用与组件解析到两套 GPUI 类型。来源在
2026-09-28 回到 crates.io 精确 pin，lockfile 记录 registry checksum，`deny.toml` 不再放行
GPUI Kit 的 git 仓库。代价是 GPUI Kit
统一管理整套传递版本，升级必须作为单独变更重跑双平台构建、设置窗口、IME、辅助功能、缩放、
窗口重建和 shutdown smoke；ADR-0054 后“辅助功能 smoke”不再是项目自有 UI 完成条件。本 ADR 不把仍缺少实机证据的 UI TODO 标记完成。

---
<a id="adr-0024"></a>
# ADR-0024: macOS TCC Capability Boundary

状态：已接受（2026-09-05）

> 后续修订（2026-09-23）：ADR-0054 退役了项目自有 AccessKit/AppKit 设置桥；Input Monitoring 与 Accessibility TCC 的边界仍然有效。

## 背景

BongoCat 需要在应用失焦时监听键盘和鼠标边沿，以驱动 BongoCat overlay；同时设置窗口必须向
VoiceOver 等辅助技术公开自身的可访问语义。两种能力不能因为都被 macOS 归入隐私或辅助技术领域而
混为同一项 TCC 请求。

## 决策

- 全局键盘、鼠标和修饰键监听只使用 Input Monitoring。`bongocat-platform` 仅以
  `CGPreflightListenEventAccess` 查询状态，并只在由用户发起的明确设置操作中允许调用
  `CGRequestListenEventAccess`；启动、轮询和服务恢复不得弹出请求或重复请求。
- listen-only `CGEventTap`、`CGEventSourceKeyState` 和 `CGEventSourceButtonState` 都属于上述
  Input Monitoring 用途。权限拒绝或撤销必须让输入服务进入匿名 `PermissionDenied` 状态并可靠 Reset；
  overlay、设置窗口和本地配置仍继续运行。
- 设置窗口的 AccessKit/AppKit adapter 只公开 BongoCat 自己窗口的可访问树和 action channel。它不
  读取、控制或自动化其它应用，不调用 Accessibility trust/prompt API，因此不得为此请求 Accessibility
  TCC 权限。
- `NSOpenPanel`、`NSWorkspace` URL 打开和 pasteboard wrapper 保持各自最小能力边界；它们不得借由
  Input Monitoring 或 Accessibility 状态扩大访问范围。
- Settings 必须分别显示 Input Monitoring 的当前状态和输入服务状态。状态变化只更新 snapshot/诊断，
  不把“已授权”表述为输入服务已经重启；重新启动输入服务仍必须经过显式、受控的 owner 生命周期。

## 验证

- 静态检查确认平台输入路径只使用 listen-event preflight/request API，AccessKit bridge 不链接或调用
  Accessibility trust API。
- macOS 实机矩阵覆盖未授权、拒绝、授权和撤销 Input Monitoring；确认不出现 Accessibility 提示，且
  拒绝时 overlay/settings 保持可用。
- 后续 TCC UI 刷新实现必须分别测试 permission snapshot 与 service status，确保授权变化不会被误报为
  已运行的 event tap。

## 引导流程与逐次重置（2026-09-30，ADR-0078）

ADR-0078 把 macOS 的引导动作换成 `permission-flow` 的浮动面板，并在**每次进入授权流程前**执行
`tccutil reset ListenEvent com.ayangweb.bongo-cat`。本 ADR 的 Input Monitoring 边界因此有两处修订：

- 「只在由用户发起的明确设置操作中允许调用 `CGRequestListenEventAccess`」仍然成立，且引导流程
  整体位于用户点击之后，没有引入启动或轮询路径上的 TCC 请求。
- 但「不重复请求」的语义改变了：逐次重置会主动把已有授权清回未授权，因此用户每次进入引导都要重新
  授权。这是产品明确要求的行为，不是实现偏差。
- 「权限拒绝或撤销必须让输入服务进入匿名 `PermissionDenied` 状态并可靠 Reset」中，**进入
  `PermissionDenied` 这一半已有覆盖**（tap 被禁用 → 重启 → 进程内 `CGPreflightListenEventAccess`
  复查 → `PermissionDenied`），但**重新授权之后没有自动重启输入服务的触发点**：重置使运行中的
  tap 失效，用户重新授权后输入要等应用重启才恢复。这一缺口记录在 ADR-0078 的「未完成项」。
- 能力面保持最小：`permission-flow` 虽然暴露 8 类权限，但产品只使用 `INPUT_MONITORING` 一项，
  且其类型不离开 `bongocat-platform` 的私有 adapter。

---
<a id="adr-0028"></a>
# ADR-0028: Rust i18n catalog structure

状态：已接受（2026-09-10）

## 背景

BongoCat 最初把所有文案放在 `ui.*` 扁平 key 下。随着设置、模型、快捷键和诊断页面
增加，这种命名无法表达页面上下文，也让相同词语在不同语义下被误复用。`rust-i18n` 会在
编译期把嵌套 JSON 展平为查找路径，因此可以同时获得可读的源文件层级和稳定的运行时 key。

## 调研结论

- [Zed](https://github.com/zed-industries/zed) 的桌面 UI 资源按产品域和页面区域组织，状态与
  操作不是一个无意义的 `common` 大对象。
- [Joplin](https://github.com/laurent22/joplin) 的 locale 资源强调稳定语义标识和参数契约，
  复数/参数消息与页面结构分离，便于翻译工具和校验脚本处理。
- [Nextcloud](https://github.com/nextcloud/server/tree/master/core/l10n) 的大型桌面/服务界面
  以功能上下文维护消息，避免把视觉组件名称作为 key；同一文案只有在语义确实相同才复用。
- [KeePassXC](https://github.com/keepassx/keepassx/tree/master/share/translations) 的桌面对话框
  和菜单动作按交互上下文组织，说明按钮、菜单项和错误提示需要稳定的领域归属，而不是泛化
  的 `label`/`text`。

这些项目没有被直接照搬；BongoCat 根据自己的页面和 runtime 状态建立独立目录。

## 决策

locale 文件保留根级 `_version: 1`，所有叶子 key 使用 `snake_case`，并按以下顶层领域组织：

| 领域          | 内容                                            |
| ------------- | ----------------------------------------------- |
| `navigation`  | 设置窗口导航项及页面简介                        |
| `settings`    | 外观、overlay、模型交互、输入、应用和运行时设置 |
| `models`      | catalog、身份、导入流程、行为和校验             |
| `shortcuts`   | 快捷键范围、录入、动作和冲突                    |
| `diagnostics` | 预留 runtime、输入指标、配置恢复和导出（当前无叶子） |
| `about`       | 产品信息和用户支持动作                           |
| `actions`     | 跨页面且语义稳定的确认、取消、刷新等动作        |
| `status`      | 跨页面生命周期状态                              |
| `errors`      | 按 settings/models/runtime 归属的错误消息       |

页面区域使用 `title`、`description` 等有限且有上下文的字段；实体或状态继续使用具体名称，
例如 `models.identity.source.built_in` 和 `settings.overlay.maximum_fps.description`，不使用
无上下文的 `common.name`。需要参数的消息保留 `%{name}` 占位符，所有 locale 的 key、层级和
占位符集合必须完全一致。

Rust 在文案实际使用处直接通过 `bongocat-i18n` 查询稳定路径，例如
`"models.catalog.load_failed"`。`bongocat-i18n` 保持唯一的 `rust_i18n::i18n!` catalog owner：
UI 不重复嵌入 JSON，也不使用会在并发窗口间共享状态的全局 locale。动态摘要、错误、快捷键冲突
和诊断指标在负责其展示语义的函数中通过同一 facade 查询，不按语言分支写自然语言。

## 平台覆盖（platform override）

少数文案在两个首版平台（macOS / Windows）上需要不同的表达（例如「菜单栏」与「系统
托盘」是同一概念在不同操作系统的命名）。这种差异交给 catalog 而不是调用点处理：

- 在 catalog 里，把 base key 拆成嵌套对象，例如
  `settings.application.status_icon.label.macos` 与
  `settings.application.status_icon.label.windows`，两个 locale 同时拥有。
- UI 不写 `cfg(target_os = ...)`，统一调用 `bongocat_i18n::platform_text(locale, base_key)`。
- `platform_text` 的查找顺序是：`<base_key>.<current_platform_id>` →
  `<base_key>`。`current_platform_id()` 在编译期取 `target_os`，
  返回 `"macos"` / `"windows"` / `"unsupported"`。

扩展方式：未来再有需要分平台的项，只在两个 locale 文件里新增 `<base_key>.macos`
和 `<base_key>.windows`，调用点保持不动。仍不需要分平台的项继续用 `bongocat_i18n::text`。
未知平台（Linux、CI 主机）会回退到 base key，避免生产 UI 暴露 key 字符串。

## 验证

`bongocat-i18n` 测试递归展开 JSON 后比较语言 key 集合和占位符集合；`rust-i18n` 的 fallback
继续使用 `en-US`。新增语言必须先复制完整结构，再提交翻译，不得增删字段。

平台覆盖的覆盖测试随 `bongocat-i18n` 单元测试一起跑：
`platform_text` 在每个支持的 `current_platform_id` 上都返回对应平台的副本，并在没有
override 时回退到 base key。

---
<a id="adr-0030"></a>
# ADR-0030: Prefer Existing Solutions Before Implementing

状态：已接受（2026-09-13）

## 背景

项目已有依赖版本、许可证和平台边界规则，但缺少一条明确的新功能实现方式顺序。部分文档使用
“自行实现”“独立实现”描述平台能力，容易让贡献者把它理解成默认自研。Rust 生态、操作系统 API
和现有 workspace 已经提供许多经过生产验证的能力，重复实现会放大维护、安全和兼容成本。

本决策参考 [Ponytail](https://github.com/DietrichGebert/ponytail) 的核心思想：先理解问题并追溯
真实流程，再按复用阶梯选择最小可行实现；代码、抽象和依赖都不应为未经证明的未来需求提前建设。

## 决策

实现新功能时按以下顺序停在第一个可行方案：

1. 确认需求确实需要实现，避免 YAGNI 之外的工作。
2. 复用现有仓库内的 helper、contract、类型或模式。
3. 使用 Rust 标准库。
4. 使用操作系统原生能力。
5. 使用 workspace 已安装且满足边界的依赖。
6. 调研并直接采用成熟、可靠且维护良好的第三方库或开源方案。
7. 只有现有方案确实无法满足需求，或其引入成本明显高于自行实现时，才编写满足当前需求的最小
   自有实现，并保留一项可运行的验证。

第三方方案必须继续通过仓库的精确版本、许可证、target、维护状态、unsafe 面积和替换边界评审。
复用不得形成第二套业务 runtime，不得绕过强类型 command/event/snapshot 边界，也不得让第三方
错误、配置或平台类型进入公共 API。若成熟方案的引入成本明显更高，必须说明具体成本，而不是仅以
“减少依赖”为理由拒绝。

## 影响

- 通用能力默认采用成熟方案；自研只保留产品特有语义、可靠性和平台所有权边界所需的适配代码。
- 新依赖仍是显式决策，不因“优先复用”免除版本、许可证、安全性和可维护性检查。
- 现有已经落地且满足契约的实现无需仅为遵循本 ADR 进行重写；后续新增或替换功能按本顺序评估。
- ADR-0029 把更新实现从自研栈切换到 `self_update`，与本 ADR 的方向一致。

## Verification

- 代码评审检查新增功能是否说明复用/自研选择，尤其是已经存在等价 crate 或平台能力的场景。
- 新依赖提交必须同时提供版本、许可证、source、target 和替换边界证据。
- 自研通用机制时，以任务记录或 ADR 说明现有方案无法满足的具体约束或明显更高的引入成本。

---
<a id="adr-0032"></a>
# ADR-0032: Startup Permission Prompt Boundary

状态：已接受（2026-09-14）；同日实机验收发现 macOS 侧必须使用不触碰 AppKit 的 `rfd` 路径，见「macOS 弹框实现修正」；2026-09-15 修正检查执行方式为专用 worker 线程上的非阻塞检查，见「非阻塞执行修正」；同日启用 Windows 自定义按钮文案，见「Windows 按钮文案修正」；2026-09-18 为修复主题跟随把 macOS 提示改为主线程 `NSAlert`，见「主题外观修正」；2026-09-24 收口 Windows 按钮文案，使其准确描述当时的按钮文案；2026-09-26 Windows 提示标题改为「建议以管理员权限运行」，次要按钮改为「稍后设置」并与 macOS 共用同一个 key `startup_permission.later`；同日主按钮改为「退出并前往设置」

## 背景

全局输入需要一项平台能力，而缺省状态下两平台都不会主动告知用户：macOS 需要 Input Monitoring
TCC 授权，Windows 需要提权令牌才能在完整性级别更高的前台窗口存在时继续收到 Raw Input。旧版
Tauri 实现把这两件事做成产品自有弹窗，并记录用户是否已经处理过；BongoCat 此前只在
Settings 和 Diagnostics 里投影状态，用户必须自己发现问题。

项目需要一套启动时的检查与引导：使用平台原生弹框，复用既有平台权限检测，不给用户新增需要
维护的提示状态。同时 ADR-0024 已经限定 macOS 的 TCC 请求 API 只能在用户发起的设置操作中调用，
本决策必须与之相容。

## 调研结论

已核实（2026-09-14，源码阅读 `rfd 0.17.2` 与 `windows 0.62.2`）：

- `rfd 0.17.2`（MIT，双平台 target-only，已按 ADR-0030 精确 pin 且 `default-features = false`）
  的消息对话框在没有父窗口时的实现是平台原生的，并与有父窗口时不同：macOS 的**同步**路径走
  `sync_pop_dialog` → `CFUserNotificationDisplayAlert`
  （`src/backend/macos/utils/user_alert.rs`），Windows 走 `MessageBoxW`
  （`src/backend/win_cid/message_dialog.rs`，`not(feature = "common-controls-v6")` 分支）。
  两条路径都不需要产品自建窗口，也不进入 GPUI renderer。
- macOS **同步**路径虽然避开 `NSAlert::runModal`（因此不重入 GPUI 事件循环），但它为了设置激活策略
  与焦点会先构造 `PolicyManager`/`FocusManager`，两者都调用 `NSApplication::sharedApplication`
  （`src/backend/macos/utils/{policy_manager,focus_manager}.rs`）。这会创建共享应用实例，而该实例
  在启动阶段属于谁是有约束的，见下方修正章节；**无父窗口的异步路径**（`async_pop_dialog` →
  `UserAlert::new(opt, None)`）不创建这两个 manager，因此完全不触碰 AppKit，只构建
  `CFUserNotification`。
- macOS 自定义按钮文案会被保留（`OkCancelCustom` 映射为前两个按钮标题）；Windows **不会**保留，
  因为自定义标题来自 `TaskDialogIndirect`，而它只由 ComCtl32 v6 导出。`rfd` 自身文档要求同时
  启用 `common-controls-v6` feature **并**在应用 manifest 中声明 ComCtl32 v6 依赖。原始调研
  认为本产品两者都不具备（`crates/bongocat-app/windows/bongocat-app.rc` 不含该 dependency，
  依赖声明也没有该 feature），所以当时 Windows 只显示系统标准 OK/Cancel 两个按钮；2026-09-15
  复查发现 manifest 前提实际由 `gpui-pre` 静态库满足，只补了 feature，见「Windows 按钮文案
  修正」。
- Windows 提权状态以 `TokenElevation` 为准：`OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY)`
  + `GetTokenInformation(_, TokenElevation, ..)`。`windows 0.62.2` 的 `HANDLE` 未实现 `Drop`
  （只实现 `windows_core::Free`），必须显式 `CloseHandle`。用户按引导勾选「以管理员身份运行此
  程序」后新进程的令牌就是 elevated，与提示承诺的状态一致，同时也能正确识别在提权 shell 中启动
  的情况。
- 现有可复用能力：`bongocat-platform::input_monitoring_permission`
  （`CGPreflightListenEventAccess`，只读）、`request_input_monitoring_permission`
  （`CGRequestListenEventAccess`）、`rfd`（目录选择已使用的既有依赖）、
  `opener 0.9.0` 的 `reveal`（既有依赖且已启用 feature，用于在资源管理器中定位可执行文件）。
  不需要任何新依赖。

## 决策

- 检查与提示全部放在 `bongocat-platform` 的私有 `startup_permission` adapter：
  只暴露项目自有的 `StartupPermissionPrompt`、`StartupPermissionStatus`、
  `STARTUP_PERMISSION_CAPABILITY`、`startup_permission_available()` 和
  `check_startup_permission()`。`rfd`、`windows` token API 和 AppKit 类型都不离开该 adapter。
- 文案由 `bongocat-app` 从翻译目录构造后传入 adapter，沿用 `SystemMenuPresentation` 的既有模式；
  adapter 不读取配置、不持有产品文案。
- 两平台都先做只读查询，再决定是否提示：

  | 平台 | 只读查询 | 提示实现 | 「授权/去设置」动作 |
  | --- | --- | --- | --- |
  | macOS | `CGPreflightListenEventAccess` | 主线程 `NSAlert`（经 `dispatch2::run_on_main` 投递，2026-09-18 起，见「主题外观修正」） | `CGRequestListenEventAccess` + `NSWorkspace` 打开 `x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent` |
  | Windows | `TokenElevation` | `rfd::MessageDialog`（`TaskDialogIndirect`，调用线程，见「Windows 按钮文案修正」） | `opener::reveal` 定位当前可执行文件，正文给出「属性 → 兼容性 → 勾选以管理员身份运行」路径 |

- macOS 曾要求使用 `rfd` 的无父窗口异步实现（见「macOS 弹框实现修正」），2026-09-18 起改为
  主线程 `NSAlert`；同 crate 的同步实现仍被 contract 测试禁止，原因不变（见「主题外观修正」）。

- macOS 先调用 TCC 请求 API 再打开面板：请求是产品出现在「输入监控」列表里的唯一途径，否则用户
  还要手动添加应用。该调用仍严格位于用户点击之后，满足 ADR-0024 「启动、轮询和服务恢复不得弹出
  请求」的约束；启动路径只调用只读 preflight。
- Windows 不自动提权：不调用 `ShellExecuteEx` + `runas`，不写入 HKCU/HKLM，不注册 service 或
  scheduled task。产品继续按 ADR-0023 以 per-user、无提权方式安装和运行，提示只解释兼容性开关
  这一系统标准路径。
- 提示不做任何持久化：不新增配置字段、不写入 `window-state.json`、不新增应用日志事件、不缓存「用户选过
  稍后」。每次启动都重新读取平台当前状态；已授权则完全不提示，未授权则本次启动继续提示。判定依据
  只有平台状态。
- 检查点（2026-09-15 修正）：检查不再阻塞启动流程。产品在 GPUI run loop 内完成 overlay、设置
  服务、系统菜单和 update worker 的启动之后，由主线程 spawn 一个专用 worker 线程执行检查与提示；
  提示未应答、被关闭或检查失败都不影响任何产品窗口的显示与使用。ADR-0054 前由 `--configuration-recovery-mode` 进入的恢复-only 路径不提示；当前配置 fallback 直接走普通设置窗口，自动化 harness 运行仍不提示。原始决策「检查点在 GPUI run loop 之前、提示
  先于猫窗口出现」已被本修正取代，见「非阻塞执行修正」。
- `--startup-permission-smoke` 是只读诊断开关：只输出当前能力名与
  `available`/`missing`，用于双平台可重复验收，不弹框、不写状态。
- 自动化 harness 运行不提示：除 `--run-seconds` 与帮助外的任何参数都表示这是一次 smoke/诊断运行
  （脚本与 CI 上无人应答原生弹框）。产品路径（`--run-seconds 0`，含登录启动项）与 Development
  `cargo run` 始终检查，与打包后的 Production 行为一致，不因构建环境而不同。
- 与旧 Tauri 行为的已知差异：旧版在 Windows 非管理员时提示后不继续运行，本决策改为用户选择次要按钮
  仍可进入应用；2026-09-15 起收窄为「用户选择设置路径且流程成功时退出」，见「Windows 按钮文案
  修正」。macOS 的「请求 + 再次引导」意图保留，但请求时机收紧到用户点击之后（ADR-0024）。旧版
  行为对照仍见 `docs/migration/legacy-behavior-inventory.md`，不在本 ADR 重复维护。

## macOS 弹框实现修正（2026-09-14 实机验收）

首次实机验收在真实打包产物上复现了「启动即 abort」的崩溃，根因是下面这条 GPUI 与 AppKit 的
交互约束。原先「无父窗口同步弹框在 GPUI run loop 之前弹出是安全的」这一条结论是错的：它只检查了
`NSAlert::runModal`，漏看了 `PolicyManager`/`FocusManager` 对共享 `NSApplication` 的创建。

### 现象与证据

- 真实场景：双击 `/Applications/BongoCat.app`（Production 打包产物）→ 权限提示出现 → 用户应答后
  进程 `EXC_CRASH (SIGABRT)`、`abort() called`；应用日志只有 `started` 紧跟 `panicked`
  （release profile 的 `panic = "abort"` 把 panic 变成 abort）。
- 崩溃报告的帧偏移以「同源码重新链接的二进制」反查：本地用同一源码做
  `cargo rustc --release -p bongocat-app --bin bongocat-app -- -C strip=none`（`__text` 大小
  `0xd901e4` 与已安装产物逐字节一致）后用 `atos` 得到：
  `bongocat_app::main` → `gpui::Application::run` → `gpui_macos::platform::MacPlatform::run` →
  `objc::runtime::Object::get_mut_ivar` → panic → abort。
- 用同一份代码重建并触发同一条件，panic 消息为：
  `panicked at objc-0.2.7/src/runtime.rs:503:25: Ivar platform not found on class NSApplication`。

### 约束

- `gpui_macos` 在 `#[ctor]` 中注册 `GPUIApplication`（`NSApplication` 子类）并在其上声明 `platform`
  ivar；`MacPlatform::run` 先 `[GPUIApplication sharedApplication]`，再对该实例
  `set_ivar("platform", ..)`，而 `objc 0.2` 在 ivar 不存在时 panic。
- 进程共享的 `NSApplication` 实例由**第一个** `sharedApplication` 调用决定。若它先在基类
  `NSApplication` 上被调用（`rfd` 的同步 macOS 消息框正是如此），之后
  `[GPUIApplication sharedApplication]` 返回的就是基类实例，GPUI 的 `set_ivar` 随即 panic。
- 因此**在产品建立 GPUI 平台之前运行的任何代码都不得创建共享 `NSApplication`**。
  `NSWorkspace`、`CGPreflightListenEventAccess` 和 `CGRequestListenEventAccess` 已实机验证不创建它。

### 决策修正

- macOS 启动提示改用 `rfd::AsyncMessageDialog`（无父窗口 → `async_pop_dialog` →
  `UserAlert::new(opt, None)` → 不构造 `PolicyManager`/`FocusManager` → 不触碰 AppKit），主线程用
  `async_io::block_on` 等待结果。这与 `directory_picker` 已在用的 `rfd` + `async_io` 用法一致，
  不引入新依赖、不改变提示出现时机（仍在任何产品 UI 之前）。
- Windows 保持同步 `MessageDialog`：`MessageBoxW` 没有该约束，且在调用线程上语义最直观；改用
  `TaskDialogIndirect` 后该路径也保持同步并在 worker 线程上运行，见「Windows 按钮文案修正」。
- 该路径用无点击方式验证过：请求弹框但不等待答复时，产品正常启动并干净退出（exit 0），同时采样确认
  对话框线程真实阻塞在 `CFUserNotificationReceiveResponse`（即弹框确实已提交显示）；同一位置若换回
  同步实现，则必然复现上述 panic。

## 非阻塞执行修正（2026-09-15）

启动实机使用中发现原始检查点（GPUI run loop 之前同步执行）让权限提示成为启动的第一个交互：
用户不应答，模型窗口、菜单栏/托盘和设置 UI 都不会创建。产品要求权限提示只作为辅助提示存在，
于是把「检查的执行方式」从启动路径中拆出来，检查与提示逻辑本身不变。

### 变更

- 移除 `main` 在 `gpui_application.run` 之前的同步调用；语言在主线程解析一次，检查改由 GPUI
  run loop 内（overlay、设置服务、系统菜单、update worker 均已启动、`ProductCoordinator`
  已注册之后）spawn 的专用 worker 线程执行，线程名 `bongocat-startup-permission`。
- 只读平台查询（`CGPreflightListenEventAccess` / `TokenElevation`）与提示文案构造随 worker
  一起移入后台；`CGRequestListenEventAccess` 仍严格位于用户点击引导按钮之后，满足 ADR-0024。
- Worker 线程刻意 detached：原生对话框由 OS 持有、没有可用的远程取消通道，若在退出时 join，
  会把「未应答的弹框阻塞退出」这一原始问题原样搬到 shutdown 路径。该线程只拥有语言与文案
  字符串，不与产品共享任何锁或句柄，用户应答后自行终止；进程退出时 OS 回收线程并随之关闭
  对话框，不构成任务泄漏。
- spawn 失败仅记入启动失败收集，应用照常启动（与「提示失败不阻止启动」同语义）。

### 线程安全证据（2026-09-15 核实）

- macOS 的异步 `rfd` 路径只构建 `CFUserNotification`、不触碰 `NSApplication`，在 worker 线程
  上与 GPUI run loop 并行是安全的；同步路径会在调用线程上运行 `NSAlert` 模态机制，禁止使用，
  原 contract 测试继续固定这一约束。
- `objc2-app-kit 0.3.2` 绑定中 `NSWorkspace::sharedWorkspace()` 与 `openURL` 均未标记
  main-thread-only（无 `MainThreadMarker` 参数），可在 worker 线程执行。
- Windows 的 `MessageBoxW` 是调用线程模态，不阻塞其他线程的消息循环；`OpenProcessToken` 与
  `opener::reveal` 均无线程亲和性要求。
- worker 与产品无共享可变状态，不存在与窗口创建、输入服务或 shutdown 的锁竞争。

## 安全与生命周期不变量

- 提示运行在专用 worker 线程上（2026-09-15 修正，原始的「主线程阻塞等待用户应答」已被取代）：
  主线程继续 GPUI run loop，overlay、输入服务和 runtime 的启动不等待用户应答；worker 只拥有
  语言与文案字符串，不与渲染或输入生命周期共享任何锁。macOS 的对话框在 2026-09-18 起经
  `dispatch2::run_on_main` 在主线程上运行，worker 阻塞等待其结果（见「主题外观修正」）；
  `rfd` 的同步 macOS 路径在该线程上仍被禁止（见上方修正与 contract 测试）。
- 平台对话框的 owner 是 `rfd`，产品不持有任何 dialog handle；不注册回调，不在回调中做阻塞工作。
- macOS 侧只使用 `CGPreflightListenEventAccess`（只读）与用户点击后的
  `CGRequestListenEventAccess`；不调用任何 Accessibility trust/prompt API，不扩大 TCC 面
  （ADR-0024）。
- Windows 侧只读取自身进程令牌，不打开其他进程、不改写系统设置、不请求 UAC。令牌句柄在读取后
  立即关闭；失败路径返回「未提权」而不是 panic。
- 提示内容只包含产品名、权限用途和系统设置路径，不含用户路径、按键、剪贴板或任何诊断载荷。
- 提示失败（对话框后端不可用、`opener` 失败）不阻止应用启动，也不改变产品行为；此时仅相当于用户
  选择了「稍后」。

## 验证

- 单元测试覆盖 `rfd` 结果到选择的映射：自定义标签、标准 OK/Cancel 对以及关闭窗口/未知标签都不能
  被误判为「已授权」，并固定能力名的稳定取值。
- `bongocat-i18n` 目录测试保证中英两份目录键集与占位符一致，`bongocat-app` 侧测试保证本平台用到
  的四个键在两种语言下都有非空、非键名回退的文案。
- `RunOptions` 测试固定：`--startup-permission-smoke` 不打开设置窗口且标记为自动化运行，
  `--run-seconds 0` 保持交互式，其余被接受的参数都判定为 harness 运行。
- macOS contract 测试禁止 `startup_permission` 的 macOS 模块出现 `rfd::MessageDialog::new`；
  该测试做过反向自检（注入该字符串后确实失败），避免变成空断言。
- 崩溃回归的验证方式（自动测试无法覆盖「用户点击原生弹框」）：用同一源码重新链接出带符号的
  release 二进制并用 `atos` 反查崩溃报告帧偏移；以及「请求弹框但不等待答复」的运行确认产品可正常
  启动并干净退出。
- 可重复验收命令：`cargo run -p bongocat-app -- --startup-permission-smoke`，在授权前应输出
  `missing`，授权后应输出 `available`；该命令不弹框、不写入任何产品状态。
- 发布前必须在真实设备补足下列人工验收（当前环境无法代替）：
  - macOS 12+：未授权启动出现原生提示；「打开系统设置」落点为「隐私与安全性 → 输入监控」且产品
    已在列表中；「稍后设置」后猫窗口正常出现且输入服务进入匿名 `PermissionDenied`；授权并重启后
    不再提示。
  - Windows 10 1903+：普通权限启动出现原生提示且按钮为「退出并前往设置」/「稍后设置」两个
    自定义文案（Task Dialog）；「退出并前往设置」能定位可执行文件且产品随后走常规 shutdown
    退出；勾选兼容性开关并重启后不再提示；提权启动时完全不提示。
- Windows 实机矩阵与 macOS TCC 矩阵仍是发布门禁，cross-check 只能证明编译与边界。

## Windows 按钮文案修正（2026-09-15）

> 本节保留 2026-09-15 的实现背景；按钮名称已按最新 catalog 审计同步为当前文案。

产品要求 Windows 提示的两个按钮显示自有文案（「退出并前往设置」/「稍后设置」），而原始决策下
Windows 走 `MessageBoxW`、只显示系统标准 OK/Cancel。「替换边界」曾推断 manifest 前提不满足，实查
后发现只差一半：

- **manifest 前提本已满足**：`gpui-pre` 的静态库（`gpui.lib`）内嵌了一份完整应用 manifest
  （`asInvoker`、Windows 10 `supportedOS`、PerMonitorV2 DPI、SegmentHeap、ComCtl32 v6 的
  Common-Controls 依赖），链接期已进入 executable。原始调研只检查了产品自有
  `bongocat-app.rc`，漏看了这条来源。
- **feature 是唯一缺口**：为 `rfd` 启用 `common-controls-v6`（仅为 `windows-sys` 增加
  `Win32_UI_Controls` gateway，不新增 crate）。

曾尝试在 `bongocat-app.rc` 追加自有 manifest（`1 24`），与 gpui 的 manifest 资源
（同 ID 1）冲突，链接报 `CVT1100 duplicate resource: MANIFEST, name 1`；已回退。不得再为
manifest 另加来源。

### 行为与证据

- 启用后 Windows 提示走 `TaskDialogIndirect`，`OkCancelCustom` 的两个自定义文案成为真实按钮，
  结果以 `MessageDialogResult::Custom(label)` 返回，既有映射逻辑无需修改。
- 官方文档的 Remarks 未要求调用线程初始化 COM；`rfd` 自身的异步 Windows 实现也是在裸
  `std::thread` 上调用 `TaskDialogIndirect`，与本项目专用 worker 线程同构，该路径经 Tauri
  生态长期使用。
- 失败语义保持保守：`TaskDialogIndirect` 绑定失败（激活上下文不可用）时 `rfd` 返回 `Cancel`，
  映射逻辑把它视为选择次要按钮「稍后设置」，绝不会被误判为同意；文案正文（已精简）不引用具体按钮名，
  两种按钮形态下都成立。
- 按钮顺序与默认焦点由 Task Dialog 决定：主按钮「退出并前往设置」在前且为默认按钮（与原
  `MessageBoxW` 默认「确定」一致），Esc/关闭窗口等价于「稍后设置」。
- Windows 的主按钮现在名副其实：权限流程成功（`reveal` 已定位可执行文件）后，worker 通过托盘
  退出使用的同一个 `shutdown_requested` 标志请求退出，产品走常规 shutdown coordinator
  （flush 配置、停 runtime、join 服务）后进程退出，不绕过任何清理步骤；兼容性开关需要重启
  才生效，退出与按钮承诺一致。`reveal` 失败时保持运行（同「提示失败仅相当于选择稍后」的
  既有语义），用户可以再次尝试。macOS 主按钮只打开系统设置，没有退出承诺，行为不变。
- 验证：workspace 构建链接通过；链接产物 `bongocat_app.exe` 内含且仅含一份 RT_MANIFEST，
  内容即 gpui 的 ComCtl32 v6 manifest。

## 主题外观修正（2026-09-18）

ADR-0048 把用户选择的浅色/深色接到原生表面后，实机发现启动权限提示在应用深色下仍是浅色。根因：
macOS 的 `rfd` 无父窗口消息框（同步与异步都是）最终调用 `CFUserNotificationDisplayAlert`
（`rfd 0.17.2` `src/backend/macos/utils/user_alert.rs`），这条路径完全不经过 AppKit，不继承
`NSApplication.appearance`，只会跟随系统外观。ADR-0048 修复的正是 `NSApp.appearance` 继承链，
而本提示当时根本不在链上。

### 变更

- macOS 提示改用 `objc2-app-kit` 的 `NSAlert`（`NSAlertStyle::Warning`，两个自定义按钮文案不变，
  返回值经既有 `requested_permission_flow` 映射，Windows 侧零改动）。`NSAlert` 的窗口继承应用
  appearance，从第一帧起跟随启动时设置的进程级主题（ADR-0048 的 `apply_process_theme`）。
- 呈现通过 `dispatch2::run_on_main` 投递主线程并同步等待结果（`dispatch2 =0.3.1` 因此从
  dev-dependencies 转为正式依赖）：worker 仍负责只读查询与阻塞等待，AppKit 窗口只允许在主线程
  构建。worker 在 GPUI run loop 内 spawn（2026-09-15 修正），`GPUIApplication` 已存在，因此
  09-14「不得触碰共享 `NSApplication`」约束的场景（GPUI 平台建立之前）不适用于呈现阶段；
  `rfd` 同步消息框仍被 contract 测试禁止，原因不变（它会在调用线程上构造共享 `NSApplication`
  的 plumbing）。
- `runModal` 从主队列 block 执行，位于 GPUI 事件之间而非某个事件处理器栈内。模型选择器注释记录的
  `RefCell already borrowed` 重入崩溃发生在「GPUI 事件处理器内同步 `runModal`」，与此处不同；
  `rfd` 自身的文件对话框同样经 `run_on_main` 在主线程上运行模态，是该模式的既有先例。
- 提示出现时调用 `activateIgnoringOtherApps(true)` 把产品带到前台（`-activate` 为 macOS 14+，
  产品支持 12+，故用旧选择器并 `#[allow(deprecated)]`）。这是相对 `CFUserNotification` 的行为
  变化：提示现在是模态的，应答前主线程处于 modal panel run loop（GPUI 的渲染走 display link，
  不受影响），与文件对话框先例一致。
- 响应常量不用 `objc2-app-kit` 的 `NSModalResponseOK`/`NSModalResponseCancel`：该绑定把弃用的
  `NSOKButton`(1)/`NSCancelButton`(0) 值挂在了这两个名字上，而 `-runModal` 对自定义按钮返回
  `NSAlertFirstButtonReturn`(1000)/`NSAlertSecondButtonReturn`(1001)。已对照 macOS SDK 的
  `NSAlert.h` 核实（2026-09-18），crate 内以自有常量 `ALERT_FIRST_BUTTON_RESPONSE` 固定。

### 证据与验证

- 根因证据：`rfd 0.17.2` 源码调用链 + 实机截图（应用深色、提示浅色）与该路径「只跟随系统外观」
  的行为一致；SDK 头文件确认 `NSAlert` 返回值与绑定常量的出入。
- `cargo check` / `cargo clippy --all-targets -p bongocat-platform` 通过；platform 全部单测通过，
  含更新后的 contract 测试：仍禁止 `rfd::MessageDialog::new`、要求 macOS 模块经 `run_on_main`
  呈现、禁止回退 `AsyncMessageDialog`。
- 深浅两色下弹框外观的实机肉眼验收未完成，归入 ADR-0048 既有的「双平台实机主题验收」门禁。

## 设置契约与软件信息报告的能力模型（2026-09-28）

本 ADR 一直把能力表达为「平台自己的能力名 + `available`/`missing`」（见决策第 6 条的
`--startup-permission-smoke` 输出），但 `SettingsInputDiagnostics` 早期用的是 macOS 语汇的
`SettingsInputMonitoringPermission { Unsupported, Denied, Granted }`，Windows 侧因此固定投影成
`unsupported`。该值只在 snapshot、变更日志和用户可复制的软件信息报告里出现，没有 UI 渲染它，
所以它长期无人发现；实机在 Windows 上复制报告时才暴露问题：报告里
`input_monitoring_permission: "unsupported"` 读起来像「该平台没有权限概念」，而事实是
「Windows 的门在提权令牌上，且当前进程未提权」——恰好是决定高完整性前台窗口存在时 Raw Input
是否继续送达的那一个状态（背景第 2 条与本 ADR 决策表）。

因此契约改为 `SettingsInputCapability { name, available }`，`name` 取自
`bongocat_platform::STARTUP_PERMISSION_CAPABILITY`（macOS `input_monitoring`、Windows
`administrator`），`available` 取自 `startup_permission_available()`（macOS TCC preflight、
Windows `TokenElevation`）。带来的约束：

- 平台差异只留在 adapter 内。`bongocat-app` 的投影函数因此不再有 `cfg` 分支，报告方
  （About 页复制的 JSON）也只读 protocol 类型，两个平台的措辞不会各自漂移。
- 应用日志的 `input/permission_unavailable` 与 `input/status_changed` 记录用
  `source=<capability>` 说明是哪一个能力，code 承担「具备/缺失」；不再拼出
  `input_monitoring_*` 这类只对 macOS 成立的字符串。
- 未被观察到的能力（只出现在测试 fixture 与尚未填充的 snapshot）用 `unobserved()` 显式
  构造，`name = "unknown"` 且 `available = false`，不得借用真实平台的能力名。
- 隐私边界不变：能力名是平台属性，不含机器或用户信息。

## 替换边界

替换点只有 `bongocat-platform` 的私有 `startup_permission` adapter 和 `bongocat-app` 的文案
构造。升级 `rfd` 时只需复验 **Windows** 侧：无父窗口消息框仍为原生实现、在
`common-controls-v6` + ComCtl32 v6 manifest 齐备时仍通过 `TaskDialogIndirect` 显示自定义按钮、
manifest 缺失时仍回退到 `MB_OKCANCEL` 且结果被当作「稍后设置」、以及无父窗口路径仍不需要应用
已运行（macOS 提示已不再使用 `rfd`，见「主题外观修正」）。`common-controls-v6` feature 与
executable 内嵌的 ComCtl32 v6 manifest 必须同时存在，缺一会让
Windows 对话框静默失败或按钮回退为系统标准文案。manifest 的唯一来源是 `gpui-pre` 静态库内嵌
的 `gpui.lib` manifest：升级/更换 `gpui-pre` 时必须重新核实该 manifest 仍声明 Common-Controls
v6（可用字节扫描 `Common-Controls` 复核），且不得在产品自有 `.rc` 里再嵌入 manifest
（RT_MANIFEST ID 1 冲突，`CVT1100`）。升级 `opener` 时必须复验 `reveal` feature 与失败语义。
adapter 之外不得依赖 `rfd` 类型。升级 `objc2-app-kit` 时必须复验两件事：`NSAlert` 绑定的方法面
（本提示依赖 `runModal`/`addButtonWithTitle`/`setMessageText`/`setInformativeText`/
`setAlertStyle`）与 `NSModalResponseOK`/`NSModalResponseCancel` 常量值是否仍与
`-runModal` 实际返回值不符（当前不符，crate 内用 `NSAlert.h` 的 1000/1001 自有常量）。
若升级 `gpui-kit`/`gpui-pre-macos`，必须复验 `GPUIApplication` 子类与 `platform` ivar 的约束是否
仍然成立：一旦 GPUI 不再依赖该 ivar，本 ADR 的 macOS 同步/异步选择可以重新评估。

复核记录（2026-10-06，`gpui-kit 0.7.1` / `gpui-pre 0.3.8`）：`resources/windows/gpui.rc` 与
`gpui.manifest.xml` 相对 `0.3.7` 逐字节相同，仍是单条 `RT_MANIFEST 24` 内嵌
`Microsoft.Windows.Common-Controls 6.0.0.0`，产品自有 `.rc` 无需也不再新增嵌入。macOS 侧的
`Ivar platform not found` 约束由 `startup_permission.rs` 的源码 pin 测试固定；同日的 macOS
`just dev-smoke` 通过，设置窗口关闭、销毁并重新打开，未触发该 abort。Windows 侧仍需实机安装、
升级与卸载证据。

复核记录（2026-10-12，`gpui-kit 0.7.3` / `gpui-pre 0.3.9`）：`gpui-pre` 的
`resources/windows/gpui.rc` 与 `gpui.manifest.xml` 相对 `0.3.8` 逐字节相同
（`diff -r gpui-pre-0.3.8/resources gpui-pre-0.3.9/resources` 无输出），仍是单条
`RT_MANIFEST 24` 内嵌 `Microsoft.Windows.Common-Controls 6.0.0.0`，产品自有 `.rc` 无需也不再
新增嵌入。macOS 侧的 `Ivar platform not found` 约束由 `startup_permission.rs` 的源码 pin 测试
固定——该测试断言的是本仓库自己的模块源码，不读上游，因此 GPUI 快照换代不改变它的含义；
同日的 macOS `just dev-smoke` 通过。Windows 侧仍需实机安装、升级与卸载证据。

## macOS 引导流程换成 permission-flow（2026-09-30，ADR-0078）

提示壳不变：仍是主线程 `NSAlert`，文案仍由 `bongocat-app` 从翻译目录构造，worker 生命周期、
「不做任何持久化」「已授权则不提示」的判定依据都不变。变化只有三处：

- macOS 的「授权/去设置」动作由「`CGRequestListenEventAccess` + `NSWorkspace` 打开输入监控面板」
  换成 `permission-flow` 的 `INPUT_MONITORING` 引导面板（带拖拽引导）。`CGRequestListenEventAccess`
  不再由本路径调用：注册到输入监控列表这一步改由面板引导用户完成。
- 该动作前增加一次 `tccutil reset ListenEvent com.ayangweb.bongo-cat`。这使「已授权则完全不提示」
  在流程入口侧被反向使用：每次进入流程都会把授权清回未授权，因此每次都要重新授权，即使此前已授权。
  同时因为 Bundle ID 全环境共用（ADR-0008），它也会清掉已安装 Production 构建的授权。
- `StartupPermissionPrompt` 增加 `locale` 字段，把产品语言交给 adapter，供 macOS 引导面板对齐语言
  （ADR-0078 决策第 5 条）。Windows 侧接受并忽略该参数。

已知后果：重置会让运行中的事件 tap 失效并进入 `PermissionDenied`，而用户重新授权后没有自动重启
输入服务的触发点，输入要等应用重启才恢复；见 ADR-0078 的「未完成项」。本 ADR 原「决策」表中
macOS 一行的「授权/去设置」动作以本节为准。

---
<a id="adr-0043"></a>
# ADR-0043: Startup Item Backend via auto-launch
> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

状态：Accepted
日期：2026-09-17
取代：ADR-0013
修订：ADR-0051 取代了"Development 构建从此支持启动项"这一条决策——开发构建现在按构建环境汇报
`Unsupported(BuildEnvironment)`，不再注册登录项。本文档的其余部分（`auto-launch` 后端、
LaunchAgent plist、HKCU Run、`--run-seconds 0`、契约变体保留、环境隔离命名）继续有效。

## Context

ADR-0013 建立了环境隔离的双平台启动项能力：Windows 使用 HKCU Run，macOS 13+
Production `.app` 使用 `SMAppService.mainAppService`，macOS 12 与 Development
构建明确报告 unsupported。该实现（`P7-STARTUP-ITEM-PLATFORM`、
`P5-STARTUP-ITEM-UI`）已通过双平台 smoke 与 CI 验收。

产品决策改变：启动项是用户可直接感知的功能，macOS 12 用户（产品声明的首发
平台）和 Development 构建不应被排除。同时，维护两套平台后端（HKCU 注册表
raw binding、objc2 ServiceManagement）只为一项可选能力付出不成比例的成本。
经评审选择成熟的开源方案 `auto-launch` crate（crates.io 最新稳定版 0.6.0，
MIT，活跃维护）统一双平台后端。

## Decision

- 引入 `auto-launch = "=0.6.0"` 作为 `bongocat-platform` 的双平台启动项后端，
  替换 `startup_item_macos.rs` 与 `startup_item_windows.rs` 的手写实现。
- macOS 使用 `MacOSLaunchMode::LaunchAgent`：enable 写
  `~/Library/LaunchAgents/{app_name}.plist`（`Label`、
  `AssociatedBundleIdentifiers`、`ProgramArguments`、`RunAtLoad`），disable
  删除 plist，`is_enabled` 检查 plist 存在。该机制在 macOS 12/13+ 行为一致，
  无 TCC 授权、无特权要求、不触碰 SMAppService 状态。注册项在系统设置的
  后台项目列表中可见（经 `AssociatedBundleIdentifiers` 关联应用）。
- Windows 使用 `WindowsEnableMode::CurrentUser`：写入
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`，并同步写入
  `StartupApproved\Run` 启用标记，`is_enabled` 同时尊重任务管理器的禁用覆盖。
- 环境隔离保留：`StartupItemEnvironment` 仍映射为不同的
  `app_name`（`BongoCat Development` / `BongoCat Production`）。macOS 上是
  不同的 plist Label 与文件名，Windows 上是不同的注册表 value name，开发与
  生产互不覆盖，Development 构建从此支持启动项。
- 启动命令统一携带 `--run-seconds 0`（显式产品启动语义）；Windows 侧由
  auto-launch 按 MSVCRT 参数规则对 executable 加引号拼接。
- 共享 contract（`StartupItemState`、`StartupItemError`、
  `StartupItemUnsupportedReason`）保持不变以稳定 UI/typed command 边界；
  后端现只会产生 `Enabled`、`Disabled` 与错误。`Stale`、`RequiresApproval`、
  `NotFound` 与 `Unsupported(OperatingSystem/BuildEnvironment)` 保留为契约
  变体但当前无生产者，UI 对其的处理分支继续作为防御性路径存在。
- 启用、禁用仍只能由显式用户 command 触发；失败不改变配置、不阻塞
  input/runtime/renderer，与 ADR-0013 的隔离与容错原则一致。

## Consequences

- macOS 12 获得启动项支持；macOS 13+ 的注册方式从 SMAppService 变为
  LaunchAgent plist，不再有 requires-approval 状态。
- LaunchAgent 项随登录由 launchd 拉起（`RunAtLoad`），enable 后不立即启动、
  也无 launchctl bootstrap；ProgramArguments 直接运行 bundle 内可执行文件，
  与用户双击 `.app` 的启动语义存在差异（工作目录、环境变量更空），属已知
  且可接受的行为。
- Windows 不再检测 stale（安装位置变化后 `is_enabled` 仍为 enabled，启动时
  才会失败）；用户重新开关一次即可修复。ADR-0013 的 stale 状态由此退役。
- plist 由 auto-launch 以字符串模板生成，路径含 XML 特殊字符时可能生成
  无效 plist；产品路径不含此类字符，作为上游已知边界记录。
- `objc2-service-management` 依赖随 SMAppService 后端一并移除。

## Verification

- 单元测试覆盖环境命名隔离、错误 code 稳定性与后端构造。
- opt-in smoke（`cargo test -p bongocat-platform -- --ignored`）驱动当前
  平台 disabled -> enabled -> disabled 并恢复原状态，且不触及另一环境的
  注册项。
- macOS `--startup-item-smoke` 产品 smoke 继续验证 Production 安装态
  lifecycle（现映射到 LaunchAgent plist）。
- 双平台完整门禁（fmt、严格 Clippy、workspace test）通过；Windows
  实机回归由 CI 承担。

---
<a id="adr-0048"></a>
# ADR-0048: 主题跟随到原生表面

状态：已接受（2026-09-18）
依赖：ADR-0030（优先复用现有方案）、ADR-0031（托盘图标边界）、ADR-0020（GPUI Kit 统一依赖入口）

## 背景

1. **产品要求**：用户选择的浅色/深色要作用于**产品没有自己绘制的**那些表面——窗口标题栏、
   系统弹框、右键菜单、托盘菜单、文件选择框。这些表面的 owner 是操作系统，不是 GPUI。
2. **哪些表面能跟随应用主题，由平台决定，不由产品决定**：macOS 通过进程级
   `NSApplication.appearance` 可以让整棵继承链（窗口 → 弹框 → 菜单 → 面板）一起变；Windows
   只有窗口框有官方 API，ComCtl32 弹框、Win32 菜单和 shell 文件框**没有任何**官方接口可以跟随
   应用主题，最多只能跟随**系统**主题。
3. **调研结论：不存在可以直接用的现成 crate。** 逐个核查过的候选与摘要如下：

   | 候选 | 为什么不能直接用 |
   | --- | --- |
   | `dark-light` | 只**检测**系统外观，不设置任何东西 |
   | `tao` / `winit` | 完整窗口库；引入等于替换 GPUI 的窗口层 |
   | `muda::MenuTheme` | 上游明确只作用于菜单栏，**不含 popup**；Windows 上等于没有 |
   | `tray-icon` / `rfd` | 没有主题 API |
   | `windows` / `objc2-app-kit` | 是 binding，不是方案——但它就是我们需要的那一层 |
   | `winsafe` | 为一个窗口属性引入第二个 Win32 抽象层，不成比例 |
   | 任何 `SetPreferredAppMode` 封装 crate | crates.io 上不存在 |

   所需能力全部已在依赖树内（`windows 0.62.2`、`objc2-app-kit 0.3.2`），因此按 §3.1 阶梯
   停在"已安装依赖 + OS 原生能力"这一级，**不新增第三方依赖**。
4. **根因是结构性的，不是零散 bug。** `bongocat-platform` 此前没有主题入口
   （`crates/bongocat-platform/src/` 下不存在 `theme.rs`），`SystemMenu`、picker、prompt 的构造
   签名里没有任何主题参数。于是主题知识只存在于 UI 层，而原生表面的 owner 在平台层——这是
   "主题 bug 很多、代码越写越乱"的结构性原因。
5. **现状缺陷（带证据，完整清单见调研文档 §4.2）**：

   - **B1（P0 / Windows）** `cx.set_window_appearance` 在 Windows 上是 trait 默认空实现
     （`gpui-pre-0.3.5/src/platform.rs:235`；macOS 有覆写，Windows 后端 0 处覆写）。用户在
     Windows 上选暗色，**任何原生表面都不会变**。
   - **B2（P0 / macOS）** gpui 的 macOS 覆写写的是 `NSApplication.setAppearance:`（进程级），而
     `update_window.rs` 硬编码 `SettingsTheme::System`，其 `System` 分支会清掉覆盖 →
     **更新窗口打开就把设置窗口刚设好的暗色清掉**，下一次 render 再设回来（闪烁）。
   - **B3（P1）** 三套并行同步，幂等缓存各不相同（`window/view_state.rs:4` 无缓存、
     `update_window.rs` 用 `applied_theme`、`window.rs` 的 `apply_optimistic_component_theme`
     比对 `cx.theme().mode` 且 `Theme::change(mode, None, cx)` 不刷新任何窗口）。
   - **B4（P1）** 切回 `System` 时先清覆盖、紧接着读 `window.appearance()`，而它返回的是只在
     `appearance_changed()` 里更新的**缓存字段**，同一帧内读到的仍是覆盖前的值。
   - **B5（P1）** 原生弹框与 6 处 picker 全部没有 `set_parent`（全仓库 `set_parent` 命中 0 处）。
   - **B6（P2）** `bongocat-platform` 没有主题入口，原生表面若要跟随主题**无处可接**。

6. **一个影响范围的事实**：设置窗口在 Windows 上**没有原生标题栏**——gpui 对
   `WindowKind::Normal` 只设 `WS_SYSMENU | WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX`，
   从不设 `WS_CAPTION`。所以"标题栏跟随主题"在 macOS 是真实需求，在 Windows 只剩 DWM 的
   边框/圆角/阴影这一项。

## 决策

### 1. `bongocat-platform` 新增 `theme` 模块，作为原生主题的唯一入口

`crates/bongocat-platform/src/theme.rs` 公开四个类型和三个函数：

```rust
pub enum AppTheme { Light, Dark }          // 已解析的取值，System 不往下传
pub enum SystemAppearance { Light, Dark }  // 系统当前外观
pub enum NativeThemeError { .. }           // 稳定 error code，不进 config
pub fn apply_process_theme(theme: Option<AppTheme>) -> Result<(), _>
pub fn apply_theme(window: &impl HasWindowHandle, theme: Option<AppTheme>) -> Result<(), _>
pub fn init_native_theme() -> Result<(), _>
#[cfg(target_os = "macos")] pub fn system_appearance() -> SystemAppearance
```

`apply_process_theme` 是启动入口：`bongocat-app` 在把 `Application` 移入 settings service
之前读取持久化的 `appearance.theme`，并在 `gpui_application.run` 的主线程回调中、创建 overlay
窗口之前调用它。这样 macOS 的 `NSApplication.appearance` 先于模型窗口和其右键菜单生效；
主题不依赖设置窗口是否曾经打开。窗口级 `apply_theme` 仍用于 Windows DWM 窗口框以及设置/更新
窗口创建后的幂等重应用。Windows 的 `apply_process_theme` 是 no-op，系统菜单/弹框/文件框由
`init_native_theme` 预置为跟随系统主题。


`AppTheme` 是**已解析**的取值：调用方决定 `System` 是什么意思，平台层永远收不到 `System`，
所以两处调用不可能各自推导出不同答案。

### 2. `System` 的解析口径收归平台层，并且全 crate 只有一处解析

`SettingsTheme::System` 不再读 `window.appearance()`。macOS 走
`bongocat_platform::system_appearance()`（`NSApplication.effectiveAppearance`），其余平台沿用
gpui 的值（它们从不安装应用级覆盖，gpui 的答案就是系统值）。这直接修掉 B4：平台层查的是
**当前生效**的外观，不是需要等回调刷新的缓存。

`bongocat-ui` 内只有 `resolved_theme_mode(theme, window, cx)` 一个入口做这个解析，render 路径与
smoke 断言都调它。**smoke 此前用 `component_theme_mode(theme, cx.window_appearance())` 独立推导
期望值**，与产品实际走的路径不是同一条；smoke 的职责是证明产品做了什么，用自己的第二套公式去
推导期望值只会让两者悄悄漂移，所以改成调用产品自己的入口。

**为什么 macOS 不复用 gpui 已有的查询（§3.1 评估记录）**：gpui 的 `App::window_appearance()` 与
`Platform::window_appearance()`（`gpui-pre-macos-0.3.5/src/platform.rs:718`）确实已经实时读
`NSApplication.effectiveAppearance`，形式上够用。但它的名字映射
（`gpui-pre-macos-0.3.5/src/window_appearance.rs`）只识别 `Aqua`、`DarkAqua`、`VibrantLight`、
`VibrantDark`，其余一律**打印到 stdout 并回退成 `Light`**。macOS 开启「提高对比度」后 AppKit 报的
是 `AccessibilityHighContrastDarkAqua`，gpui 会把暗色系统判成浅色，产品于是会在暗色系统里画出
浅色 UI。这是 gpui 的既存缺陷，产品不能继承，所以这一处自研（约 15 行）而不是复用。

### 3. macOS：进程级 `NSApplication.setAppearance:`

这是唯一能覆盖全部四行表面的做法。按窗口设置只会让菜单和面板继续留在系统主题上，因为
`NSWindow.appearance` 不会被窗口自己弹出的菜单继承。

`NSAppearanceName*` 常量以字符串字面值写入（`appearanceNamed:` 与 `-isEqualToString:` 都按内容
比较），避免为一个常量引入 `unsafe`。深色判定接受 `DarkAqua`、`VibrantDark` 和
`AccessibilityHighContrastDarkAqua` 三个名字。

### 4. Windows：窗口框用官方 API，其余表面接受"跟随系统主题"

- 窗口框：`DwmSetWindowAttribute` + `DWMWA_USE_IMMERSIVE_DARK_MODE`（唯一有文档的做法）。
  `theme = None`（跟随系统）时以 **gpui 同源的 `UISettings` 查询**重推导系统外观并显式写入
  该属性——该属性一旦被写过便不再自动跟随系统偏好，且写入必须与 gpui 的写入同源，否则两个
  写入方会互相覆盖。见文末「修正（2026-09-18）」。
- 弹框 / 菜单 / 文件框：**产品不自绘**，接受跟随系统主题。为了让它们在系统为暗色时真的变暗，
  进程在创建任何窗口之前调用一次未文档化的 `SetPreferredAppMode`（`uxtheme.dll` 序号 135），
  取 `PreferredAppMode::AllowDark`（= 1）。

  选 `AllowDark` 而不是 `ForceDark` 是刻意的：这些表面的契约是"跟随**系统**主题"，不是"跟随
  应用主题"。用 `ForceDark` 会让系统为浅色的用户在这些表面上看到暗色，与第 4 条的边界矛盾。

  调用点在 `main` 里、`RunOptions::parse` 之后、任何窗口之前，**一次，且不撤销**。

### 5. 启动时先应用进程级主题

主题的全局事实来源是持久化的 `appearance.theme`，不是设置窗口的生命周期。`bongocat-app`
在创建 overlay 之前解析该字段并调用 `bongocat_platform::apply_process_theme`：macOS 直接
设置 `NSApplication.appearance`，因此模型窗口及其右键菜单从第一帧起就继承正确外观；Windows
该入口不做应用级强制，继续由 `init_native_theme` 让系统-owned 表面跟随系统。设置/更新窗口
打开后仍会调用 `apply_theme`，但那是幂等补偿，不是首次生效的前置条件。

### 6. 原生主题失败不致命

`apply_theme` / `init_native_theme` 的失败一律降级为"该表面保持系统外观"，也就是每个平台的
文档化兜底。UI 层因此 `let _ =` 丢弃结果，不向用户报错、不改配置、不阻止启动。理由：能拒绝的
表面恰好就是**本来就无法跟随应用主题**的那些，为一个外观开关拒绝启动或弹出错误更糟。

### 7. 更新窗口改为携带应用外观，而不是硬编码 `System`

`UpdateView` 增加 `appearance_theme` 字段，`open_update_window` 增加同名参数，`ProductCoordinator`
缓存它并在系统菜单轮询里同步。更新窗口的 `observe_window_appearance` 回调改为读
`appearance_theme`：只有当前偏好是 `System` 时才响应系统变化，固定的偏好已经交给平台，不会因为
系统变了而改变。这修掉 B2。

## 明确不做

- **不新增任何第三方依赖**：能力已在 `windows` / `objc2-app-kit` 内，见背景第 3 条。
- **不自绘菜单或弹框**：那等于重写系统控件，且会失去平台的无障碍与输入法集成。
- **不使用 `muda::MenuTheme`**：上游明确它不覆盖 popup，用它只会让人误以为已经解决。
- **不做 Linux**：首发范围外（ADR-0006）；Linux 分支的 `apply` / `init` 是**有文档的 no-op**，
  不是待办。
- **不在平台层做颜色**：这个模块只回答"哪个外观"，不回答"什么颜色"。
- **不给原生表面加配置字段**：`schema_version: 1` 不变，`appearance.theme` 仍是唯一来源。
- **不在本次修 B5（picker/prompt 的 `set_parent`）**：它是模态归属与 UX 缺陷，不是主题缺陷。
  macOS 上无 parent 的弹框继承 `NSApp` 外观，恰好已经能跟随应用主题；Windows 上跟随系统主题。
  修它属于独立改动。

## 残余风险与待验证项（不得当作已确认）

1. **`SetPreferredAppMode` 是未文档化的序号导出。** Microsoft 从未发布头文件或函数名；序号在
   未来的 Windows 版本或补丁里可能被移除或改变语义。已做的处置：用
   `LOAD_LIBRARY_SEARCH_SYSTEM32` 解析（防止被同名 DLL 劫持）、解析结果缓存、缺失时返回
   `NativeCallFailed` 并静默退化。**这不能证明它在所有受支持 Windows 版本上行为一致。**
2. **Windows 分支只经过隔离 crate 编译验证，不是真实目标构建。** workspace 无法交叉编译到
   `x86_64-pc-windows-msvc`（`libdeflate-sys`，来自 `oxipng`，需要 Windows C 工具链）。已用
   `/tmp/theme-win-check` 隔离 crate（`raw-window-handle 0.6.2` + `windows 0.62.2`，同 features）
   抽取 Windows 分支单独编译，`cargo check` 与 `cargo clippy -- -D warnings` 均通过。**类型正确
   不等于运行时正确。**
3. **双平台实机均未验证。** 未在 Windows 上确认 DWM 暗色边框、暗色弹框/菜单/文件框实际生效；
   未在 macOS 上确认标题栏、弹框、托盘菜单、文件面板随应用主题切换。
4. **`NSApplication.appearance` 的作用面比需求更宽。** 它是进程级的，会作用于产品没有列出的
   表面（例如 AppKit 自己画的一切）。当前没有观察到副作用，但这是一个"比要求做得更多"的选择。
5. **更新窗口/设置页的原生表面有滞后。** 设置页下拉切换主题时
   `apply_optimistic_component_theme` 只改组件主题、不碰原生表面（该函数拿不到 `Window`），
   原生表面要等配置往返 + 下一次 snapshot 轮询才更新。滞后一个轮询周期，最终一致。（修正后
   `System` 的**组件半边**在点击瞬间就解析生效——见文末修正第 2 条；原生窗口框仍滞后一个往返，
   该边界不变。）
6. **`AccessibilityHighContrastDarkAqua` 只在提高对比度时出现。** 把它算作暗色是有意为之
   （见决策 2：gpui 会把它判成浅色），但它没有在提高对比度 + 暗色的实机上验证过。
7. **`apply_theme` 要求主线程（macOS），但 UI 层丢掉了结果。** 非主线程调用返回 `WrongThread`
   而不是 panic，`system_appearance()` 在非主线程返回 `Light`（中性兜底，不是真实答案）。
   两者都是刻意的降级，但 `apply_native_theme` 用 `let _ =` 丢弃返回值，所以**一次线程模型的
   回归会表现为"主题悄悄不生效"而不是任何信号**。当前 `apply_component_theme` 只从 render
   调用，不存在这个路径；若要让它可诊断，需要按 ADR-0017 的有界日志契约新增一个固定 code，
   本次未做。
8. **本 ADR 只覆盖"哪个外观"，不覆盖颜色。** 主题色不拆独立 crate，在 `bongocat-ui` 内收口。
9. **`apply_optimistic_component_theme` 与 `resolved_theme_mode` 仍是两个函数。** 前者只服务
   "用户刚点了下拉、配置还没往返" 这一个场景，且拿不到 `Window`，因此没有并入统一入口。B3 的
   三套并行同步已收敛为「一处解析 + 一处乐观」两条职责清晰的路径，但没有收敛成一条。
10. **macOS 的 AppKit 退出路径不会回到 `main`，所以失败必须在可达边界处理。** `App::quit()`
   走 `msg_send![NSApplication, terminate:]`（`gpui-pre-macos-0.3.5/src/platform.rs:557-575`），
   `NSApplication::run()` 不返回；末尾的 `Arc::try_unwrap(failures)` 汇总对 macOS 永远不可达。
   原先 smoke 记录失败却 exit 0，已由 TODO 第 87 项修复：应用自有的 macOS 退出路径在
   `on_app_quit` 中先 `begin_product_shutdown`，await 完整的 `ProductShutdown::finish()`，再按
   最终 accumulator `std::process::exit(product_failures_exit_code(&failures))`；`finish()` 内
   新产生的失败因此也会传播。`exit_after_automated_smoke` 仅保留给 AppKit 自行终止、异步
   shutdown 来不及完成的兜底路径。干净 `--settings-window-smoke` 得到 exit 0；在
   `ProductShutdown::finish()` 内临时注入 `forced finish failure` 得到 exit 1 与
   `product run failed: forced finish failure`，探针已还原。
   **剩余边界**：AppKit 自行终止且 shutdown future 未完成时，兜底 helper 只能看到调用时已记录的
   失败；该路径仍受 OS 终止时序限制。
11. **收敛成单一解析入口后，smoke 的断言只能验证"一致"，不能验证"正确"。** 产品设的值与 smoke
   期望的值来自同一个 `resolved_theme_mode`，所以把一个错误的解析同时喂给两边时断言仍然通过
   （已用变异确认）。解析本身的正确性依赖 `NSApplication.effectiveAppearance` 与
   `appearanceNamed:` 这两个文档化 API，以及两个不变量单测；没有端到端的正确性测试。

## 后记（2026-09-18）：启动权限弹框的例外已修复

本文档的「alerts | application theme」一行只覆盖经 `NSApplication.appearance` 继承链的 AppKit
弹框。落地当天的实机截图显示启动权限提示在应用深色下仍是浅色：该提示当时走 `rfd` 的无父窗口
消息框 = `CFUserNotification`，完全不经过 AppKit，不在继承链上——ADR-0048 的实现没有错，是
这个提示本身不在被修复的路径上。已按 ADR-0032「主题外观修正（2026-09-18）」把它改为主线程
`NSAlert`，本表格自此对产品全部系统弹框成立。

## 修正（2026-09-18）：Windows 上 `System` 回退不再是 no-op，乐观路径覆盖 `System`

实机（Windows，浅色固定 → 切「跟随系统」且系统为暗色）发现两个回归，均由本文档错误的前提
引起，已修复；修复本身又暴露出第二个写入源问题，同日二次修正：

1. **标题栏在手动切换主题后不再跟随系统。** 根因：`DWMWA_USE_IMMERSIVE_DARK_MODE` 一旦被
   显式写入（例如选过浅色），Windows 就按写入值绘制窗口框，**不再自动跟随系统偏好**；初次
   启动能跟随只是因为 gpui 后端在窗口创建时以 WinRT `UISettings` 查询系统外观并显式写入了
   正确值。决策 4 原文「`theme = None` 时什么都不做」的前提因此不成立。修复：Windows
   `apply` 在 `None` 时**以 gpui 同源的 `UISettings.GetColorValue(Foreground)` 查询**（同一
   API、同一亮度公式）重推导系统外观并显式写入该属性，WinRT 不可用时回退
   `AppsUseLightTheme` 注册表值、再失败回退浅色（文档化兜底）。偏好为 `System` 期间系统偏好
   翻转时，设置窗口的 `observe_window_appearance` 回调会重新调用该入口重推导。写入后以
   `SetWindowPos(SWP_FRAMECHANGED)` 触发非客户区重绘，保证可见窗口上的实时切换立即生效。
2. **切换瞬间出现浅色留白/闪烁。** 根因：`apply_optimistic_component_theme` 此前只对固定的
   Light/Dark 生效，`System` 分支完全不动组件主题，UI 要等配置往返 + snapshot 才变暗（即
   残余风险 5 的滞后，在最常见路径上被放大成可见闪烁）。修复：乐观路径对 `System` 也立即
   解析——macOS 先清应用级覆盖再解析（与 `apply_component_theme`「先原生后解析」同序，否则
   读到的还是刚被清除的固定值）；Windows 无进程级覆盖可清，`apply_process_theme(None)` 为
   no-op，窗口框由往返后的 `apply_component_theme` 修正。请求失败时原有
   `applied_theme = None` 的重同步路径保证回滚。
3. **二次修正：跟随系统的查询必须与 gpui 同源。** 第一版修复用 `AppsUseLightTheme`
   注册表值推导，实机发现标题栏仍为浅色：gpui 创建窗口时已按 `UISettings` 写入暗色，而
   注册表推导给出了浅色，**后写的注册表结果覆盖了 gpui 的正确值**。两个写入源不一致时，
   时间在后的一方总是获胜，所以"跟随系统"的推导不允许存在第二个事实来源——现统一为
   gpui 所用的 `UISettings`，注册表值仅作 WinRT 不可用时的兜底。

验证：`cargo check` / `cargo clippy -D warnings`（platform、ui、app）与
`cargo test -p bongocat-platform -p bongocat-ui`（52 + 117 通过）全绿；Windows 实机切换
浅色 → 跟随系统（暗）的标题栏与无闪烁表现待复测。

## 验证

已完成（2026-09-18，本机 macOS arm64，固定工具链 1.97.1）：

- `just check` 六道门全绿：`cargo fmt --all -- --check`；三组严格 Clippy（workspace 排除 app、
  app `storage-test-injection`、app `production`）；`cargo test --locked --workspace`
  **749 passed / 0 failed**；`cargo check --locked --workspace --release`。
- `cargo check --locked --workspace --all-targets` 通过，`bongocat-platform` / `bongocat-ui` /
  `bongocat-app` 无告警。
- Windows 分支交叉编译：`/tmp/theme-win-check` 的 `cargo check --target x86_64-pc-windows-msvc`
  与 `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings` 均通过（以临时注入
  `compile_error!` 的方式确认该文件确实被编译，随后还原）。
- `crates/bongocat-platform/src/theme.rs` 单测：`error_codes_are_stable_and_unique`（4 个 error
  code 唯一且带 `native_theme_` 前缀）、`only_dark_reports_itself_as_dark`。
- `crates/bongocat-ui` 新增 2 个不变量测试（共 117）：
  `only_the_system_choice_lets_the_system_decide` 断言固定偏好对
  `Light`/`Dark`/`VibrantLight`/`VibrantDark` 四种系统外观都解析到同一个模式、且只有 `System`
  允许跟随系统；`the_native_and_component_halves_pin_together` 断言 `pinned_native_theme` 与
  `pinned_theme_mode` 同时固定或同时不固定。两者都用**变异测试**验证过有牙齿：把
  `pinned_theme_mode(SettingsTheme::Light)` 改成返回 `Dark` 后两个测试都失败，随后还原。
- 依赖 feature 变更已记录用途：`objc2-app-kit` 增加 `NSAppearance`，`windows` 增加
  `Win32_Graphics_Dwm`。
- **主题全局启动路径实测**：`--run-seconds 4 --hidden-model-switch-smoke`（不打开设置窗口）
  exit 0，模型 overlay 正常启动并完成 hidden model switch；启动时已在 overlay 创建前调用
  `apply_process_theme`，因此模型窗口及其右键菜单不再依赖设置窗口。
- **macOS 运行时冒烟（`just dev-smoke` = `--run-seconds 4 --settings-window-smoke`）跑通，
  exit 0**：设置窗口打开、辅助功能桥挂上、`show_general_page_for_smoke` 在主线程上执行完
  `apply_component_theme` → `apply_native_theme` → `NSApplication.setAppearance` /
  `effectiveAppearance`，无 panic、无 `WrongThread`、正常 shutdown。现在 exit 0 有基本的
  失败传播保证：automated smoke 若在 quit 前记录失败会 exit 1。
- **退出码变异验证**：2026-09-18 在 smoke 分支注入 `forced smoke failure` 得到 exit 1；
  2026-09-23 在 `ProductShutdown::finish()` 内注入 `forced finish failure`，同一 `--settings-window-smoke`
  命令得到 exit 1 且 stderr 输出 `product run failed: forced finish failure`；两次还原后均得到
  干净 smoke exit 0。这证明应用自有退出路径在最终 shutdown 阶段也有牙齿。
- **主题断言的强度仍有限**：`resolved_theme_mode` 同时被产品路径和 smoke 期望值调用，因此
  smoke 验证的是一致性；解析本身由 macOS `effectiveAppearance`/`appearanceNamed:` 文档化 API
  与不变量单测约束，不是端到端的肉眼主题验证。

**未运行**：双平台实机主题切换（肉眼确认标题栏/弹框/托盘菜单/文件面板的深浅色）；Windows DWM
暗色边框与暗色弹框/菜单/文件框实测；`SetPreferredAppMode` 在 Windows 10 1903 / 11 各版本上的
行为；运行中切换系统主题后的跟随行为；提高对比度 + 暗色组合；AppKit 自行终止且 shutdown
future 未完成时的 macOS 退出码时序。

---
<a id="adr-0051"></a>
# ADR-0051: 开发构建不提供登录时启动

状态：已接受（2026-09-20）
修订：ADR-0043（仅取代其"Development 构建从此支持启动项"这一条决策）；2026-09-23（设置项改为整行禁用呈现）
依赖：ADR-0008（应用身份与存储环境）、ADR-0043（启动项后端统一 auto-launch）

## 背景

ADR-0043 把双平台启动项后端统一到 `auto-launch` 时，把 Development 构建一并纳入支持范围：用不同的
`app_name`（macOS plist Label、Windows HKCU value name）隔离开发与生产的注册项，开发构建因此可以
在本机演练真实的注册生命周期。契约里随之保留了 `StartupItemUnsupportedReason::BuildEnvironment`，
但后端从不产生它（ADR-0043 原文：「保留为契约变体但当前无生产者，UI 对其的处理分支继续作为防御性
路径存在」）。

决定本 ADR 形态的事实：

1. **注册项指向的是"当前可执行文件"，而它的生命周期长于进程。** 登录时启动把
   `current_exe()` 写进操作系统的启动配置，用户下次登录时由系统拉起该路径。
2. **开发构建的可执行文件是构建产物，不是安装好的应用。** 它位于 Cargo 的 `target/` 下，
   `cargo build`、`cargo clean`、切换 profile 或直接删除目录都会替换或移除它。用户在开发构建里
   打开的开关会留下一个指向已过期或不存在的二进制的登录项，而产品里没有任何界面能解释这件事。
3. **契约里已经有一条为这件事准备好的状态。** `Unsupported(BuildEnvironment)` 与
   `SettingsStartupItemState::can_set_enabled()` 早已把"这个构建不支持该能力"表达成不可变更的状态，
   UI 的处理分支也一直存在，只是没有生产者。
4. **可见开关此前与它对外声明的语义不一致。** 没有动作可做时（快照未就绪、有待处理操作、
   平台不支持），辅助功能节点报 disabled，而可见开关仍然可以点击并发出命令（用户报告的正是这类
   "能点但没意义"的控件）。

## 决策

### 1. 门禁放在应用层，按构建环境判定

`bongocat-app` 的 `system_startup_item_state()` 与 `system_set_startup_item_enabled()` 在
`BUILD_ENVIRONMENT != Production` 时直接汇报
`SettingsStartupItemState::Unsupported(BuildEnvironment)`：

- 读取方向返回 `SettingsStartupItemStatus::State(Unsupported(BuildEnvironment))`，不调用
  `bongocat-platform`，因此开发构建连"查询系统启动项"都不会发生。
- 写入方向返回 `Ok(Unsupported(BuildEnvironment))`。这是一个 no-op 而不是错误：会发送该命令的开关
  已经被禁用，而残留窗口或脚本化客户端不应收到一个用户无法处理的失败。两个方向返回同一个值，
  客户端不会观察到能力在读取与写入之间变化。

判定写成"是否为 Production"而不是"是否为 Development"，使将来新增的构建环境默认不可用，
而不是默认获得注册能力。

选择应用层而不是平台层的理由与既有先例一致：`update_check_available()`（开发构建不安装发布产物）
同样把"这个构建产物是否具备该能力"判定在 `bongocat-app`，`bongocat-update` 只被当作能力实现方使用。
`bongocat-platform` 及其 `auto-launch` 后端、`StartupItemEnvironment`、两套 `app_name` 与 opt-in
lifecycle smoke 全部不变，Production 路径继续使用它们。

### 2. UI：整行按统一禁用样式置灰，并保留可见原因

- `StartupItemPresentation` 增加 `disabled: bool`，只有
  `Unsupported(BuildEnvironment)` 为 `true`；其余状态（初始化中、可重试、已启用、已禁用、需批准、
  登录项缺失、平台/系统不支持）都是 `false`。
- 可见设置项只在"这个构建根本不提供该能力"时以
  `SettingItem::disabled(startup_item.disabled)` 整行禁用，与"鼠标悬停延迟"等项目的禁用呈现一致。
  这与"此刻能否操作"是两个问题：后者由 `action` 回答，仍然只由辅助功能节点声明为 disabled /
  不可点击。保持这两者分离，是为了让已发布构建在快照尚未就绪时仍把该行呈现为正常可用——
  这正是本次要求的边界。
- 行文案继续复用目录里已有的 `settings.application.startup.unsupported_build`；开发构建的
  `description` 保留为可见说明，不再挂 Tooltip，也不再手写 `Switch`。

## 明确不做

- **不删除平台契约里的 `StartupItemEnvironment`、Development `app_name` 或 opt-in smoke**：
  Production 路径仍需它们，删掉会让"环境隔离"这件事失去唯一载体。
- **不在平台层再拦一次**：应用层是唯一生产者，重复判定只会产生第二个事实来源。
- **不为该状态新增文案键**：目录里已有同一句，第二条措辞只会带来漂移风险。
- **不做自动清理**：不在启动时扫描并删除历史上的 `BongoCat Development` 登录项。产品不删除它没有
  写入过的系统配置项（ADR-0008 的环境隔离原则），该项由用户手动移除。

## 后果

- 开发构建不再写 `BongoCat Development` 登录项。**已经存在的该登录项不会被自动清理**：
  macOS 是 `~/Library/LaunchAgents/BongoCat Development.plist`，Windows 是 HKCU Run 下的同名 value。
  本机（macOS）检查过该文件不存在，因此本次没有受影响的实际注册项。
- 开发构建不再能演练真实注册生命周期。该覆盖由 Production 构建的 `--startup-item-smoke`
  （`main.rs`，本就要求 `BUILD_ENVIRONMENT == Production`）与 CI 承担。
- `StartupItemEnvironment::Development` 在平台契约中仍然存在，但生产路径不会再构造它——应用层只会把
  `Production` 传下去。这条差异由 `bongocat-app` 的测试固定。
- macOS 12 用户与 Windows 用户不受影响：门禁只看构建环境，与平台和 OS 版本无关。
- 已发布构建里该行的行为不变：它只在"构建不提供该能力"时禁用，开发构建之外没有状态会让它变灰。
  初始化中与有待处理操作时，辅助功能节点仍报 disabled / 不可点击，而可见控件保持可用——这与改动前
  完全一致，本次只对齐了开发构建的整行呈现。

## 残余风险与待验证项（不得当作已确认）

1. **整行禁用样式依赖 gpui-kit 的 `SettingItem::disabled`，没有像素级截图断言。**
   当前测试覆盖 presentation 的 `disabled` 谓词和命令/无障碍语义；视觉一致性沿用 gpui-kit 既有设置项。
2. **未在 Windows 实机确认**：本机无法执行 `cfg(windows)` 路径。本项改动落在应用层与 UI 层，
   没有改平台代码。
3. **历史上已注册的开发登录项需要用户手动移除**，产品不提供入口，也不会提示。
4. **平台侧的 Development 分支现在只被它自己的 opt-in smoke 驱动**：
   `startup_item_native::startup_item_lifecycle_smoke_restores_original_state` 仍在
   `StartupItemEnvironment::Development` 上跑 disabled → enabled → disabled，而产品路径不会再走到
   那里。保留它是因为 Production 路径需要同一套后端构造，同时它仍是"环境命名隔离仍然成立"的证据；
   代价是本仓库里能观察到 Development 注册的地方只剩这个被 `--ignored` 标记的用例。

## 验证

已完成（2026-09-23，本机 macOS / aarch64）：

- `bongocat-app`：`login_startup_is_gated_on_the_build_environment` 断言
  `startup_item_available()` 等于 `BUILD_ENVIRONMENT == Production`（同一用例在开发与
  `--features production` 两种特性下都成立）；
  `a_development_build_reports_login_startup_as_unavailable` 断言开发构建的读取返回
  `State(Unsupported(BuildEnvironment))`，且 `true`/`false` 两个写入方向都返回
  `Ok(Unsupported(BuildEnvironment))` 而不是错误。
- `bongocat-ui`：`the_startup_row_is_disabled_exactly_where_the_build_cannot_offer_it` 对全部平台
  状态 ×`blocked` 断言 `disabled` 恰好等于"构建环境不支持"；
  `a_development_build_disables_the_startup_row_with_visible_copy` 固定开发构建的可见文案
  （中文目录原文）；
  `the_startup_row_stays_operable_in_every_actionable_state` 断言已发布构建能产生的三个可操作
  状态都不禁用。
- 设置窗口 smoke 的启动项断言不变（辅助功能节点按 `action == None` 报 disabled/clickable/focusable），
  与可见行级禁用规则由上面的 presentation 用例覆盖。

**验证命令**：`just check`（format、分段 Clippy、workspace tests、release check）通过。

**未运行**：视觉截图确认整行置灰、Windows 实机。

---
<a id="adr-0054"></a>
# ADR-0054: 配置损坏恢复收口为 back-to-back fallback，设置 UI 采用视觉优先契约

状态：已接受（2026-09-22）
依赖：ADR-0005（配置原子提交与备份）、ADR-0053（开关门禁规则，本 ADR 取代其中项目 AccessKit tree 部分）

## 背景

维护者要求（2026-09-22）把当前应用收敛到“能看见的功能就是功能”，不再为配置损坏维护恢复
窗口、恢复提示、恢复按钮或重启门禁，也不再把屏幕阅读器和项目自有的辅助技术树当成产品功能：

1. 当前 `config.json` 可用时直接使用。
2. 当前配置损坏时，从最新可用备份恢复。
3. 当前配置损坏且没有可用备份时，写入并使用默认配置。
4. Development、Production 和 smoke 的设置窗口必须使用同一套可见组成；不能再出现 smoke 有
   “重置偏好设置”、正常运行却没有的窗口。
5. 只给用户看的文字和控件负责界面；删除只为辅助技术存在的隐藏文案、AccessKit 节点、action、
   native bridge 和直接依赖。

这里的“视觉优先”只描述当前设置 UI 的实现范围。项目只定义和绘制用户可见、可操作的
内容；以后不新增项目自有的无障碍树、原生桥接、隐藏文案或 action。主题、本地化、键盘
操作和 GPUI Kit 自身提供的默认语义仍可保留。

## 决策

### 1. 配置 fallback

`ConfigStore::load_or_default` 是配置加载的唯一入口，行为固定为：

- 当前 v1 配置通过完整 typed validation 时，原样加载。
- 当前文件 parse/validate 失败时，按新到旧读取本环境自有 `config-*.json`，使用第一份完整
  有效的当前 v1 备份；恢复后的当前文件必须重新读取并验证一致。
- 当前配置损坏且没有有效备份时，直接原子写入当前 v1 默认配置，并返回该默认配置。
- 当前配置缺失时直接写入并返回默认配置。
- 每个阶段复用既有 writer lock、原子替换和损坏文件 quarantine；不再有“等待用户点恢复默认”的
  中间状态。

非 v1 当前文件仍按当前 v1 的严格版本入口处理，不属于自动 downgrade/migration。本产品不为旧
Tauri/Pinia、开发中间版本或未知未来版本增加转换器、alias、fallback UI 或兼容分支。

### 2. 设置窗口没有配置恢复 UI

- 不创建 recovery-only 设置窗口，不创建受限 runtime，不做 application operational gate。
- 不显示窗口级、页面级或分组级的配置恢复提示。
- 不提供 `RestoreDefaultConfiguration` 按钮、command、snapshot 字段或“恢复后必须重启”流程。
- `ExportDiagnostics` 和备份目录打开能力可以保留为排障入口，但不得重新长出配置恢复页面。
- 成功加载、备份恢复和默认 fallback 都回到普通启动路径；正常关闭与重启语义不变。

### 3. 设置窗口的可见组成必须一致

- Development、Production 和 smoke 使用同一个设置窗口视图及同一页面/分组/按钮组成。
- smoke 通过测试 seam 触发导航、主题和窗口检查，不得通过隐藏或额外渲染用户控件来制造差异。
- 更新入口若由 application 注入，smoke 也必须注入等价 callback，使其可见组成与产品一致；
  smoke 是否真正发起网络更新不属于可见组成条件。
- 以后新增可见按钮或分组时，产品窗口、development smoke 和 release smoke 必须同时获得它；
  以后删除可见内容时也必须三处一致。

### 4. 设置 UI 的辅助技术范围

- 设置 UI 是 visual-first：项目只定义并绘制用户可见、可操作的内容。
- `bongocat-ui` 和 `bongocat-platform` 不维护项目自有的 AccessKit tree、Accessibility adapter、
  native view bridge、辅助技术 action channel、隐藏标签或 only-for-assistive-technologies 文案。
- 除 `gpui-kit` 的传递实现外，workspace 不直接依赖 `accesskit`、`accesskit_macos` 或
  `accesskit_windows`。
- 可保留的键盘行为包括 tab 顺序、可见 focus、Enter/Space 激活、Escape 取消；这些由可见控件
  和 `gpui-kit` 默认能力承担，不额外复制一份辅助语义状态。
- Tooltip 服务鼠标/触摸等可见交互；若文案只为了让屏幕阅读器读出而存在，则不写。
- ADR-0053 的“可见行、AccessKit tree、mutator 三层同源”收窄为“可见行与 mutator 同源
  `SettingGate`”；GPUI 若自带语义由其组件决定，不建立项目 contract。

### 5. 文档契约

以后实现或评审设置 UI 时：

- 先问“用户是否看得见、是否可以操作”；看不见的实现默认删除。
- 不以 screen reader、VoiceOver、Narrator、UIA、AX tree 或 Accessibility permission 作为
  功能完成条件。
- 不在项目代码新增 `*_accessibility`/`*_a11y` 模块、隐藏 `Status` 节点、`supports_click`
  contract 或屏幕阅读器 smoke。
- 不把 `gpui-kit` 内部可能携带的语义当作 BongoCat 的业务 contract。
- 新增可见功能只保留实现功能所需的最小状态和测试，不预先建立恢复 UI、辅助桥或跨层协议。

### 6. macOS 前台窗口与后台 Dock 偏好

- 设置或更新窗口打开期间（包括最小化）始终使用 `Regular` 并显示 Dock 图标；猫窗口由独立
  overlay 管理，不计入 GPUI 的普通窗口列表。
- `system.show_dock_icon` 保留当前字段和默认值，仅决定普通窗口全部关闭后的 Dock 显示状态。
  UI 文案为「始终显示程序坞图标」；在设置窗口内关闭开关不切换到 `Accessory`，不影响窗口内容。
- 复用普通窗口打开入口与关闭回调计算模式，避免通过枚举原生窗口、重新显示或恢复焦点补救
  切换模式的副作用。打开失败恢复背景模式；配置提交失败恢复旧偏好。

## 后果

- 配置损坏的普通路径收敛为“备份优先，否则默认值”，没有额外窗口和用户操作。
- 产品/开发/smoke 的可见窗口一致，按钮差异不会再来自运行模式。
- 项目自有辅助功能代码、隐藏文案和直接 AccessKit 依赖被删除；`gpui-kit` 仍可透明使用其
  内部默认语义。
- 自动化测试继续覆盖可见功能、输入、配置和 smoke；不再以辅助树断言证明 UI 完成。

## 明确不做

- 不删除操作系统或 GPUI 的辅助能力；只是不把它列为 BongoCat 自有的 UI contract。
- 不删除主题、本地化、键盘导航、可见 focus 和 tooltip。
- 不因“以后可能需要”而提前保留恢复窗口、恢复状态、辅助桥、feature flag 或空模块。
- 以后不在设置 UI 的项目代码里新增无障碍树、原生桥接、隐藏文案或 action。

## 验证

- `bongocat-config`：有效当前配置、有效备份优先、无有效备份回落默认值。
- `bongocat-app` / `bongocat-ui`：无 recovery-only window、无配置恢复 command/snapshot 字段。
- Development、Production、smoke 的 Settings window 查看同一页面与更新入口组成。
- workspace 与 `spikes/` 都不再出现项目自有 `accessibility`、`accesskit` 模块、节点或直接依赖。
  `spikes/gpui-settings` 原本自建的 AccessKit bridge、action channel 与隐藏 `Status`
  节点已随该 ADR 删除；它的 CI smoke 改为纯生命周期检查，Windows 侧不再用 UI
  Automation 驱动探针。

---
<a id="adr-0056"></a>
# ADR-0056: SettingGroup variant 与上游 GPUI Kit 固定 revision

状态：已接受（2026-09-23；2026-09-24、2026-09-25、2026-09-28、2026-10-06、2026-10-12 修订）

## 背景

模型库页面的内容是自绘卡片网格。设置窗口的组容器使用官方
`Settings::with_group_variant(GroupBoxVariant::Outline)`，因此该页在外层多出一个卡片容器：
边框、内边距与圆角包裹在自绘网格外面。`SettingGroup::variant(GroupBoxVariant::Normal)`
可以在不改变其它页面的情况下移除这一层容器。

`gpui-kit 0.6.6` 的文档已经承诺 group variant 可单独覆盖，但实现没有对应 API。维护者向
`longbridge/gpui-kit` 提交 issue #3202 与 PR #3203；PR 已于 2026-09-23 合并，merge commit
为 `500852f449c05dc01920ec82f3ae2656a61d0387`。同一上游 revision 还包含
`Popover::arrow(bool)`，可供项目内 `PopConfirm` 转发。对应能力尚未发布到 crates.io。

修订（2026-09-25）：ADR-0066 将模型对象拆为 Model library 与 Model behavior 两个独立页面。
`SettingGroup::variant(GroupBoxVariant::Normal)` 只用于前者——它承载自绘卡片网格；后者是
独立页面并使用窗口默认 Outline。这个 variant 边界不因模型行为独立成页而扩大。

修订（2026-09-28）：上述能力已随 `gpui-kit v0.7.0` 发布到 crates.io，本 ADR 声明的临时
git source 条件因此解除。

## 决策

- 根 workspace 直接依赖 `gpui-kit` 的 crates.io 精确 pin。2026-09-28 起该 pin 为 `=0.7.0`，
  2026-10-06 随 v0.7.1 补丁版升到 `=0.7.1`，2026-10-12 升到 `=0.7.3`，
  `[patch.crates-io]`、维护者 fork 依赖与 `longbridge/gpui-kit` git source 一并删除；仍禁止
  改用未固定的 `main` branch。
- lockfile 中 `gpui-kit`、`gpui-component`、`gpui-base`、`gpui-component-macros` 与
  `gpui-kit-assets` 全部从同一 registry 版本解析；GPUI 本身仍来自 crates.io
  `gpui-pre` 同步包（`=0.7.3` 要求 `0.3.9`），不引入第二套 GPUI 类型。
- Model library 页面继续调用 `SettingGroup::variant(GroupBoxVariant::Normal)`；独立的
  Model behavior 页面不覆盖 variant，调用点与测试 harness 不因依赖来源或页面拆分而改变。
- `PopConfirm` 增加 `arrow(bool)` 转发，模型删除确认在按钮上方显示并启用 anchor-aligned arrow。上游同时改为由 `Root` 自动挂载 dialog、sheet 与 notification layer，因此业务根视图删除旧的 `Root::render_*_layer` 调用。
- `deny.toml` 2026-09-28 起只允许 `https://github.com/ayangweb/gilrs`；其它 git source 仍拒绝。

## 影响

- 依赖来源最终从维护者 fork patch 切换为上游固定 commit，再切换为上游 crates.io 精确 pin；
  两者都不再维护或信任 fork 分支。
- `Root` layer API 迁移只删除重复挂载；窗口仍使用 `gpui_kit::component::Root`，dialog、
  sheet 与 notification 的 owner 仍为 GPUI Kit。
- `PopConfirm::arrow` 默认关闭以保持调用方显式选择；当前模型删除确认使用 `BottomRight` 向上显示并显式启用箭头。组件几何测试
  固定 arrow 会为 trigger 与 surface 预留额外间距。

---
<a id="adr-0058"></a>
# ADR-0058: Platform-neutral input contract boundary

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0004, ADR-0007, ADR-0044

## Context

`bongocat-platform` originally imported the public input types from
`bongocat-runtime`. That made a platform adapter depend on the complete runtime
graph, including Cubism, audio, model state, and worker lifecycle. The same
runtime crate also contained the pure input state reducer and the platform
input adapters, so the dependency direction did not express the actual
ownership boundary.

## Decision

Create `bongocat-input` as a platform-neutral contract and transport crate.

It owns:

- physical key, mouse, and gamepad identities;
- `InputEvent`, sequencing, reset, and reconciliation vocabulary;
- reliable input producer hand-off and anonymous transport diagnostics;
- cursor and gamepad-axis latest-value transports;
- platform-input diagnostic values and stable error-code validation;
- the canonical `GLOBE_KEY_USAGE` constant.

`bongocat-runtime` continues to own the single mutable `InputState`, model
bindings and `ModelInputSnapshot` projection, command queue, worker, and
shutdown lifecycle. The input crate does not depend on runtime, renderer,
GPUI, configuration, or an operating-system API.

The runtime adapts its command producer through the input crate's
`InputSubmitter` trait. Platform input services use the input crate directly;
they do not depend on the normal `bongocat-runtime` library. Global shortcut
registration likewise accepts an application-owned typed callback, so the
platform crate does not construct runtime commands.

`bongocat-render` re-exports `GLOBE_KEY_USAGE` for existing model/render
vocabulary, but does not own the physical input contract.

## Consequences

- platform input code no longer pulls Cubism, model, audio, or runtime worker
  dependencies;
- input transport and reducer tests are isolated from the runtime worker;
- the runtime remains the only owner of mutable pressed state and shutdown
  ordering;
- existing runtime re-exports may remain temporarily for source compatibility,
  but new consumers should depend on `bongocat-input` directly;
- moving a reducer or model projection out of runtime requires a separate
  decision and is not implied by this crate.

## Rejected alternatives

- Moving `InputState` and `ModelInputSnapshot` into `bongocat-input`: rejected
  because it would make the public input crate depend on renderer vocabulary
  and blur the single runtime owner.
- Keeping platform input types in runtime: rejected because it preserves the
  platform-to-runtime dependency and makes unsafe input adapters compile with
  unrelated Cubism/audio code.
- Creating a separate crate for every transport or reducer submodule: rejected
  as unnecessary fragmentation; these types share one stable input contract.

---
<a id="adr-0059"></a>
# ADR-0059: UI protocol boundary

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0035, ADR-0054

## Context

The settings and update services exposed their typed DTOs, commands, replies,
snapshots, and bounded channel clients from `bongocat-ui`. That made the
application service layer depend on a GPUI crate even though the service never
rendered a window. It also put presentation-only policy (debounce timing,
localized labels, update polling cadence, and filesystem-backed title probing)
next to the wire contract.

## Decision

Create `bongocat-ui-protocol` as the process-local contract crate. It owns:

- settings and update snapshots, stable enums, error codes, and project-owned
  DTOs;
- typed commands, replies, operation controls, state handles, and bounded
  service clients/endpoints;
- `SettingsWindowPlacement`/`SettingsWindowState`, which are shared service
  state rather than GPUI entities;
- pure, path-based model-source title normalization when the adapter supplies
  whether the selected path is a directory.

The crate has one direct external dependency, `async-channel`. It does not
depend on GPUI, an operating-system API, `bongocat-config`,
`bongocat-platform`, `bongocat-i18n`, `bongocat-update`, model, or runtime
crates. The application maps config/platform/update values into these project
types at its service boundary.

`bongocat-ui` keeps GPUI views and presentation policy: the settings patch
debouncer, language display names, update error message keys, the update
window poll interval, and filesystem inspection performed by the picker/view
adapter. It depends on the protocol crate and re-exports it only inside the UI
crate for the existing view-module wiring; application services import the
protocol directly.

The GPUI executable still depends on `bongocat-ui` for windows, views, and
handles. It imports settings/update protocol types from
`bongocat-ui-protocol`; this keeps the composition root explicit without
making the service layer depend on rendering.

## Consequences

- settings and update service tests run without a GPUI test platform;
- app service code no longer obtains protocol DTOs through the rendering crate;
- protocol tests pin stable codes, command ordering, bounded transport, and
  snapshot revisions;
- GPUI tests retain rendering, localization, debounce, and window-handle
  assertions;
- the protocol is in-process and typed; it is not a serialization or IPC
  schema, so no serde dependency is introduced;
- changing a producer's public DTO now requires an explicit protocol change
  rather than an incidental UI module edit.

## Rejected alternatives

- Moving all of `bongocat-ui` into a protocol crate: rejected because it would
  preserve GPUI and presentation dependencies in the service contract.
- Keeping protocol types in `bongocat-ui` with a facade: rejected because it
  leaves the application-to-rendering dependency intact.
- Adding serde or serializing the command channel: rejected because the current
  boundary is an in-process typed queue, not a persisted or network protocol.
- Moving debounce, localization, and render scheduling into the protocol:
  rejected because those are view policy and would make the contract depend on
  presentation details.

---
<a id="adr-0066"></a>
# ADR-0066: 设置侧边栏采用任务导向的信息架构

状态：已接受（2026-09-25）
依赖：ADR-0002（GPUI 设置界面）、ADR-0019（GPUI Kit design system）、ADR-0047（模型库边界）、ADR-0052（快捷键门禁）、ADR-0054（visual-first 设置 UI）、ADR-0056（SettingGroup variant）

2026-09-25 修订：About 保持设置菜单中的最后一个普通页面，但按当前能力补齐软件信息、
项目/反馈链接和日志目录入口；不复用旧 Tauri 字段或旧配置。

## 背景

2026-09-20 的八页拆分解决了“所有设置挤在 General”以及一级菜单粒度过粗的问题，但当前
一级页面仍混用了对象、输入设备、系统模块和用户任务几种分类依据：

- `General` 实际只有主题与语言，名称比内容宽；
- 模型自己的镜像、动作音效与随机行为位于 `Interaction`，模型选择又在 `Models`；
- `Interaction` 的鼠标设置与 `Input` 的键盘、手柄设置描述的是同一类连续控制输入；
- `Application` 同时包含桌面图标、登录启动、自动更新和日志，页面名称无法表达范围；
- `About` 是产品信息与支持入口，不应被实现成脱离设置树的特殊窗口；它仍由同一个 Settings
  页面和组件契约承载。

一级页面数量、命名和设置归属使用不同分类轴，会增加用户在“模型”“模型窗口”“交互”
“快捷键”之间来回确认的成本。页面内分组虽然已经能成为二级导航，但分类边界本身仍需收敛。

## 决策

设置侧边栏采用七个业务分类，加一个最后的普通入口，顺序固定为：

1. **外观与语言 / Appearance & language**：主题、语言；页面标题已经说明范围，不再重复
   一个同义的“外观”分组。
2. **模型库 / Model library**：导入、选择、改名、封面、打开文件夹和删除；只展示模型卡片，
   不与其它模型设置共用正文滚动区域。
3. **模型行为 / Model behavior**：水平镜像、动作音效、随机播放内容下拉（关闭 / 仅表情 /
   仅动作 / 表情和动作，ADR-0072）及间隔。
4. **模型窗口 / Model window**：窗口行为、窗口外观、窗口性能。
5. **输入与交互 / Input & interaction**：鼠标、键盘、手柄。
6. **快捷键 / Shortcuts**：窗口快捷键、模型行为快捷键。
7. **应用与系统 / App & system**：启动与桌面、更新、日志；“启动与桌面”内先显示登录时启动，
   再显示状态图标和任务栏图标。
8. **关于 / About**：作为设置菜单中的最后一个普通页面，产品信息、复制软件信息、项目主页、
   问题反馈、打开日志目录和手动检查更新都用标准 `SettingItem` 控件呈现。About 使用独立的
   Info 图标，始终位于页面列表最后一项。复制内容只包含编译期产品/平台身份，不包含路径、模型名称
   或用户文件。

“模型库”“模型行为”与“模型窗口”仍是三个独立一级入口：前者管理模型对象，第二个管理
模型全局表现，后者管理独立 overlay 窗口。模型行为快捷键仍属于“快捷键”，因为该页拥有
离散命令绑定、冲突规则和 ADR-0052 的两个门禁；它不因模型行为设置独立成页而重复出现。

ADR-0047 中“模型管理页只做选择与元数据编辑”的边界收窄为“模型库页面只做选择与元数据
编辑”。模型卡片仍不承载行为预览、动作/表情目录或快捷键编辑；这些能力分别继续由
Model behavior、Shortcuts 和 runtime command 拥有。

“应用与系统 > 更新”只保存自动检查偏好与间隔；About 中的一次性“检查更新”动作仍保留。
About 的软件信息、项目/反馈链接和日志目录都是当前能力的一次性支持动作，不新增
持久化配置，也不把旧版 Tauri 字段带回新 schema。“更新”和“日志”分组的设置项只保留标题与控件，
不显示重复描述。这继续区分可持久化偏好与即时操作，并让页面标题、设置项标题和控件本身承担可见语义。

## 搜索

GPUI Kit 的搜索只匹配设置项标题、描述和显式关键词，不匹配页面或分组标题。每一组设置项
因此携带当前页面名和分组名的搜索关键词，保证可见层级本身可搜索。重新分类前的名称
（General、Interaction、Input、Application、模型管理等）继续作为只搜索、不显示的别名。

模型库是自定义元素，除页面关键词外还索引当前模型显示名；因此搜索一个模型名可以
打开模型库页面。非空搜索不会重新创建设置项、绕过 typed command 或暴露内部路径。

## 明确不做

- 不增加“常用设置”或“高级设置”页，也不复制其它页面的开关。
- 不创建第三级导航、页面内 Tabs 或新的导航框架。
- 不把模型行为快捷键复制到模型库页，不把鼠标穿透从模型窗口行为页移走。
- 不把主题、语言或日志静默移入其它分类。
- 不修改 config/schema、runtime command、默认值或持久化设置行为；About 新增的支持动作只调用
  已有平台能力或 settings service，不建立新的配置字段。
- 不 fork GPUI Kit，也不使用私有 renderer/component 接口。About 由上游 `Settings` 在同一
  可滚动菜单中按顺序渲染为最后一项；当前公开 API 不提供独立 sidebar footer，因此不额外
  复制一份导航实现来强行固定它。

## 后果

- 一级业务分类从八个页面中的八种混合领域收敛为七个用户任务域，About 保持最后。
- 模型库单独占据一个页面，模型行为单独占据另一个页面；模型窗口仍是第三个独立入口。
- 键鼠、手柄及模型响应集中到 Input & interaction。
- 系统图标与登录启动合并为“启动与桌面”，登录时启动排在状态/任务栏图标之前；自动更新和日志各自成为直接可搜索的分组。
- Appearance 与 About 删除与页面标题重复的操作分组标题；About 只保留产品信息和支持动作，
  不再在设置页展示法律与隐私正文。
- About 的操作行使用标准设置项和按钮，打开日志目录由 settings service 持有路径；UI 不把
  存储路径放进 revisioned snapshot。行描述只在标题与按钮说不清时出现：项目主页行只显示链接，
  日志目录行没有描述。
- Model library 仍是 `GroupBoxVariant::Normal` 的自绘卡片网格；Model behavior 使用窗口默认
  Outline。ADR-0056 只适用于前者。
- 页面名称、图标、顺序及旧名称搜索别名由 `bongocat-ui` 的导航 contract 统一拥有；各分组的
  标题仍在对应页面的 typed `SettingGroup` 装配处定义，并由同一页面的搜索关键词与 smoke
  校验消费，避免复制设置项。

## 验证

- 导航 contract 固定八个页面的顺序、图标和本地化标题，并断言标题非空且互不重复。
- About contract 固定产品信息、复制软件信息、项目/反馈、日志和手动更新行的本地化组成；
  复制文本不包含路径、模型名称或用户文件，settings-service 日志目录 command 覆盖成功/失败路径。
- 搜索 contract 断言页面名、分组名和旧页面名同时进入关键词。
- locale validator 和 `bongocat-i18n` 双向 key 守门保证双语目录及源码引用一致。
- `bongocat-ui` 定向测试覆盖 app/system 文案与范围，release settings smoke 覆盖真实窗口
  组成；双平台 800×600、125/150/200% 与 Retina 的最终目视检查仍按 Phase 5/8 门禁执行。

---
<a id="adr-0067"></a>
# ADR-0067：配置领域命名空间与模型完整身份

- 状态：已接受
- 日期：2026-09-25
- 影响范围：`config.json` v1、settings protocol、Application/runtime shortcut 边界、schema/fixtures

## 背景

初版 BongoCat 的配置把不同领域的状态放在过于宽泛的 `application` 和 `model`
section 中：系统集成与更新策略没有独立归属，输入和 overlay 性能字段被放在模型 section，
模型来源使用 `preset`/`installed`，选中模型由两个可分别缺失的字段表示。这样会产生以下
问题：

- `model.maximum_fps` 看似模型属性，实际由 overlay 与 runtime frame scheduler 消费；
- `model.release_fallback_timeout_ms` 看似模型行为，实际只属于键盘输入可靠性；
- `application.show_*` 与 `application.check_for_updates_*` 属于不同领域；
- `installed` 描述存储实现，不能表达用户看到的模型来源；
- 单独的 `model_id` 不能区分同 id 的内置模型与导入模型，快捷键可能串到错误的模型；
- `overlay.visible` 是会话状态，不应被当作跨重启的用户偏好。

## 决策

当前 v1 直接采用以下领域结构，不保留旧字段 alias、迁移或兼容转换：

```text
schema_version
appearance
overlay
input { gamepad }
logging
model
shortcuts
system
updates
```

具体规则如下：

1. 系统入口可见性放在 `system.show_taskbar_icon` / `system.show_status_icon`；自动更新策略放在
   `updates.check_automatically` / `updates.check_interval_hours`。
2. `overlay.maximum_fps` 属于 overlay/runtime；手柄死区属于
   `input.gamepad.stick_dead_zone` / `input.gamepad.trigger_dead_zone`。`input` 没有 `keyboard`
   命名空间：捕获键盘按键的释放由可靠 `KeyUp`、状态校正和生命周期 `Reset` 决定，没有可配置的
   超时需要归属（ADR-0004）。
3. 模型来源统一为 `imported` / `built_in`。配置字段为 `model.imported_models` 与
   `model.built_in_models`；model-store 内部的 `InstalledModel`、`ModelOrigin::Installed` 和
   `ModelOrigin::Preset` 仍是技术层词汇，不直接序列化到配置。
4. 选中模型是一个 nullable 完整对象：`model.selected_model: null` 或
   `{ "id": "...", "source": "imported" | "built_in" }`。模型行为快捷键的 `model` 也必须保存
   同一完整身份，不能只保存 id。
5. 快捷键配置字段为 `shortcuts.command_bindings` 与
   `shortcuts.model_behavior_bindings`，两个独立门禁为 `shortcuts.commands_enabled` 与
   `shortcuts.model_behaviors_enabled`。活动模型投影和冲突检测以完整模型身份为作用域。
6. `overlay.visible` 从 `config.json` 移除。可见性是 runtime snapshot 的会话状态：每个新进程
   从可见状态启动，隐藏只持续到进程退出；runtime/settings snapshot 仍保留 `overlay_visible`。
7. 随机行为使用 `model.random_behavior.mode` 与
   `model.random_behavior.interval_seconds`，并保持原有范围与默认值；门禁字段已由
   ADR-0072 改为 mode 枚举的 `off` 取值。
8. 模型响应来源门禁使用 `model.ignore_keyboard` 与 `model.ignore_gamepad`；它们属于模型
   交互表现，不属于 `input` 的采集/死区配置。门禁只作用于模型输入投影，不能关闭可靠输入、
   pressed-state 恢复、诊断或独立快捷键。三个模型输入忽略开关分别可由
   `toggle_ignore_mouse_input`、`toggle_ignore_keyboard_input` 和 `toggle_ignore_gamepad_input`
   application shortcut target 切换；这些 target 仍走 settings service 的 typed handoff 和
   revision-checked 持久化。

## 结果与边界

- Rust 类型、JSON Schema、默认值、fixtures、Application 映射、settings snapshot、platform
  shortcut target 和 runtime origin 投影必须使用同一组名称。
- 完整模型身份需要贯穿配置编译、平台注册和 Application dispatcher；runtime snapshot 公开
  当前模型的 storage origin，dispatcher 在触发行为前同时比较 id 与 origin。
- 配置损坏时仍按现有“最新有效备份 → 默认配置”规则处理。旧开发配置不会自动转换；结构不完整的
  文件按既有严格 v1 解析/恢复边界处理。
- `InstalledModel` 等技术类型不因本次用户-facing 命名调整而改名；只有配置和 settings-facing
  来源词汇使用 `imported` / `built_in`。

## 验证

- `shared/config/config.schema.json` 与 31 个 config fixtures 通过 Draft 2020-12 验证。
- `cargo test -p bongocat-config` 覆盖默认 round-trip、未知字段拒绝、完整模型身份、嵌套范围和
  fixture manifest。
- `cargo test -p bongocat-app --lib` 覆盖配置到 runtime、快捷键身份传播、可见性会话语义和
  重启边界。
- 独立 `spikes/config-store` contract 与 workspace 测试验证 Rust 实现和共享 fixture 一致。

---
<a id="adr-0068"></a>
# ADR-0068: 设置窗口销毁与进程内导航记忆

状态：已接受（2026-09-25）
依赖：ADR-0002（GPUI 设置 UI）、ADR-0035（更新 worker 与更新窗口）

## 背景

设置窗口过去关闭后只隐藏，根 `Entity` 和 GPUI 窗口会一直保留到进程退出；更新窗口过去关闭后立即销毁。
两种行为不一致，也让不常打开的设置窗口长期保留窗口级资源。

设置窗口的侧边栏选择由 `gpui-kit` 的 `Settings` 组件保存在 window-keyed state 中。窗口销毁后这份
状态会随窗口一起释放；如果不做桥接，用户每次重新打开设置都会回到第一个页面。

## 决策

### 1. 设置和更新窗口关闭即销毁

- 设置窗口标题栏 close、设置窗口 toggle 和显式关闭路径统一销毁 GPUI 窗口；
- 更新窗口保持既有的关闭即销毁行为；
- 窗口销毁不停止 runtime、输入、音频、frame source、overlay 或更新 worker；
- 两次打开之间不保留窗口实例，下一次打开重新创建 GPUI window、根 `Entity` 和组件状态；
- Windows 任务栏按钮的期望可见性由 coordinator 保留，窗口不存在时先保存意图，下一次创建时应用，
  不因设置窗口已销毁而把合法的配置变更报告为失败。

### 2. 进程内记住设置侧边栏页面

`ProductCoordinator` 持有一个 `SettingsNavigationMemory`。它是进程内、非持久化的单一 owner：

1. 设置页面通过 `SettingPage::title_suffix` 的公开渲染回调报告当前一级页面；
2. 报告只更新 `SettingsNavigationMemory`，不把导航选择写入配置文件或 runtime snapshot；
3. 新设置窗口把 memory 中的 page index 传给 `Settings::default_selected_index`；
4. 应用重启重新创建 coordinator，导航回到 Appearance。

这样既利用了 `gpui-kit` 现有的 sidebar 行为，又不复制或接管第三方 `SettingsState`。导航记忆只覆盖
一级页面；页面内的草稿、搜索词和未完成表单仍随窗口销毁，不跨窗口保存。

### 3. Shutdown 与窗口回调

普通 close 的 UI 清理发生在窗口销毁前：取消快捷键捕获、清理模型拖放并请求 flush 待提交设置。
runtime、输入和后台 worker 的 shutdown 顺序不变。Windows GPUI 0.2.2 的最终 `WM_DESTROY` 兼容
退出路径仍只用于显式 Quit 后的产品 owner 关闭阶段；普通设置/更新窗口销毁必须由双平台原生 smoke
验证。

## 结果

- 不再长期保留不常打开的设置窗口资源；
- 更新窗口行为回到原有的即时销毁语义；
- 设置窗口仍能在同一进程内恢复上次一级侧边栏页面；
- 应用重启后不会恢复旧进程中的 UI 导航选择；
- 窗口级 GPUI state 不跨窗口保存，避免把草稿、搜索和异步操作意外带到下一次打开。

## 验证

- `SettingsNavigationMemory` contract 覆盖默认值、页面更新和 clone 后的进程内保持；
- 设置窗口 smoke 覆盖 close 后 handle/window 被清除、重新 open 使用新实例并恢复 snapshot；
- Windows/macOS 原生 smoke 仍需分别覆盖真实标题栏 close、窗口销毁、重建和侧边栏页面恢复；
- 显式 Quit 的双平台 shutdown 顺序和 GPUI `WM_DESTROY` 兼容门禁保持不变。

---
<a id="adr-0073"></a>
# ADR-0073: 应用级快捷键收窄为五个 command

状态：已接受（2026-09-27）

## 背景

`shortcuts.command_bindings` 的 application command 是一个闭合集合，由
`ShortcutCommand::parse` / `as_str` 双向实现。此前它有八个取值，其中
`toggle_mirror`、`toggle_click_through` 和 `toggle_always_on_top` 只是把三个已经在
别处可见的开关接到键盘上：

- 模型水平翻转：`model.mirror`（设置页「模型行为」）与模型窗口右键菜单。
- 鼠标穿透：`overlay.click_through`（设置页「模型窗口」）与托盘/右键菜单勾选项
  （ADR-0068）。
- 始终置顶：`overlay.always_on_top`，同样在这两处有控件。

也就是说，这三个取值是同一状态在第三个入口的重复暴露。用户要改这三项时手上通常正
拿着鼠标；而真正只能靠键盘完成的场景（不看屏幕地开关模型、打开设置、按设备类型忽略
输入）由保留下来的 `toggle_overlay`、`open_settings` 和三个
`toggle_ignore_*_input` 覆盖。

## 决策

- 闭合集合收窄为五个：`toggle_overlay`、`open_settings`、`toggle_ignore_mouse_input`、
  `toggle_ignore_keyboard_input`、`toggle_ignore_gamepad_input`。三个被移除的
  `as_str` 拼写不再是合法 command。
- 三个对应的设置项和右键菜单项保持不变。收窄的是「可绑定的快捷键」，不是能力：
  `model.mirror`、`overlay.click_through` 和 `overlay.always_on_top` 的默认值、持久化
  路径、runtime 投影和 overlay 窗口行为都不动，ADR-0068 的菜单树与
  `SystemMenuAction` 也不动。
- 该变更直接进入当前 v1，不提供迁移、旧 command 探测或 fallback。一份仍持有这三个
  绑定的 v1 文档按损坏处理，走既有的「最新有效备份 → 默认配置」。默认
  `command_bindings` 是空列表，所以全新配置从未依赖过它们。
- 快捷键页的窗口作用域随之只渲染这五行；`bongocat-ui` 的
  `WINDOW_SHORTCUT_COMMANDS`、本地化 label 映射和
  `shortcuts.command_names.*` 的三个 key 同步删除，而不是留下永远渲染不到的行。

## 验证

- `bongocat-config` 覆盖剩余五个 command 双向拼写仍闭合、被移除的拼写不再被解析。
- `bongocat-ui` 覆盖窗口作用域恰好五行、tab 序号在缩短后的行序上仍不重叠、只有模型
  行为行携带 play 控件；`bongocat-i18n` 双向 key 守门确认三个文案 key 与它们的引用
  同时消失。
- `cargo fmt`、三段 `cargo clippy -D warnings`、`cargo test --workspace`、
  `cargo check --workspace --release` 与 `tools/validate-locales.py` 全部通过。
- 未运行：Windows 10 1903+ 与 macOS 12+ 实机上快捷键注册与触发的目视确认，以及设置
  窗口行的实际排版。这些仍属 ADR-0044 的完成门禁，本 ADR 的自动化证据不替代它们。

## 后果

配置契约少三个 command，UI protocol 少三个 `SettingsApplicationShortcut` 取值，
`apply_application_shortcut` 少三条分支。仍持有这三个绑定的开发配置会整体回退到备份
或默认配置，这是 v1 直接变更的既定代价。

---
<a id="adr-0078"></a>
# ADR-0078: macOS 输入监控引导采用 permission-flow（第二个厂商 FFI 例外）

状态：已接受（2026-09-30）；上游 0.1.40 已接入产品，macOS 12 适配落在 fork 的 rev 上（上游 PR #2 未合并），locale 参数与实机验收未完成，见「未完成项」

补充：ADR-0005（Cubism Core FFI 边界）、ADR-0024（macOS TCC 能力边界）、ADR-0032（启动权限
提示边界）、ADR-0008（应用身份与存储环境）、ADR-0030（实现决策阶梯）

## 背景

ADR-0032 定下 macOS 的启动权限提示：主线程 `NSAlert` 承担提示壳，用户点主按钮后调用
`CGRequestListenEventAccess` 并打开「隐私与安全性 → 输入监控」面板。这条路径只把用户送到系统
设置，之后如何在列表里勾选、如何把应用拖进授权列表，全靠用户自己摸索。

[`veecore/permission-flow`](https://github.com/veecore/permission-flow) 提供带拖拽引导的浮动
面板：打开正确的设置面板、显示目标应用 bundle、给出「把此应用拖入授权列表」的引导文案。本
ADR 记录把它用于 macOS 输入监控引导的决策、实测得到的能力边界，以及已经落地的实现。

### 上游事实（crates.io `permission-flow` 0.1.40，2026-05-03 发布）

- MIT，edition 2024，`rust-version = "1.85"`；许可证在 `deny.toml` 的 allow 列表内。
- **没有任何 normal 依赖**。只有 build-dependency `swift-rs`（feature `build`）和一个
  dev-dependency `iced`（不参与下游构建）。Rust 侧 499 行，Swift 侧 2885 行 / 34 文件，
  其中 `PermissionFlow/` 是 `jaywcjlove/PermissionFlow` 的 vendored 副本。
- 非 macOS 上编译为 no-op 垫片，`build.rs` 在 `CARGO_CFG_TARGET_OS != "macos"` 或 `DOCS_RS`
  时直接 return，因此 Windows 构建不受影响。
- 公开 API：`PermissionFlowController::{new, start_flow, stop_current_flow}`、
  `StartFlowOptions::{new, use_click_source_frame, without_click_source_frame}`、
  `Permission::INPUT_MONITORING` 等 8 项、`Permission::authorization_state()`、
  `AppPath::suggested_host_app()`。
- Swift 侧另有 `PermissionFlowConfiguration.localeIdentifier`，但 shim 把它硬编码为
  `.init(promptForAccessibilityTrust: false)`，`authorize(pane:suggestedAppURLs:sourceFrameInScreen:)`
  也没有 locale 参数，Rust API 更没有暴露——**上游的 Rust 接口无法指定语言**。

### 实测结论（2026-09-30，macOS arm64，Swift 6.4 / Xcode 27，Rust 1.97.1）

1. **Swift 运行时 rpath 必须由下游 binary crate 自己加。** 库依赖的 `rustc-link-arg` 不传递到
   最终链接的二进制。不加时 `otool -l` 里没有 `LC_RPATH`，程序在 `main` 之前就 abort：

   ```
   dyld: Library not loaded: @rpath/libswift_Concurrency.dylib
     Reason: no LC_RPATH's found
   ```

2. **SwiftPM 资源包不会被 swift-rs 发布。** `PermissionFlow_PermissionFlow.bundle` 只留在
   `target/<profile>/build/permission-flow-<hash>/out/swift-rs/…/Products/<Config>/`，可执行文件
   旁没有。SwiftPM 生成的访问器（`resource_bundle_accessor.swift`）依次查找
   `Bundle.main.resourceURL`、`Bundle(for: BundleFinder.self).resourceURL`、`Bundle.main.bundleURL`，
   都不含该 bundle 时执行 `fatalError("unable to find bundle named PermissionFlow_PermissionFlow")`。

3. **嵌套 bundle 的语言由主 bundle 决定。** 实测：系统语言为 `zh-Hans-CN`、应用未声明任何
   `CFBundleLocalizations` 时，嵌套 bundle 的 `preferredLocalizations` 仍是 `["en"]`（development
   region），面板显示英文；在主 bundle 的 `Info.plist` 声明 `CFBundleLocalizations` 之后，主
   bundle 与嵌套 bundle 同时变成 `["zh-Hans"]`；再把 `AppleLanguages` 写进**本应用自己的
   CFPreferences 域**，两者同时变成覆盖值（`zh-Hant` → 「輸入監控」、`ar` → 「مراقبة الإدخال」）。
   `vi` 没有对应 `.lproj`，回落到 `["en"]`。

4. **`AppPath::suggested_host_app()` 在开发二进制上指向错误的应用。** 实测返回启动它的应用
   （`cargo run` 产物不在 `.app` 内，该方法回退到父进程链）。

5. **`authorization_state()` 只回答宿主进程的状态**，不能用作「是否需要引导」的判据；产品现有的
   进程内 `CGPreflightListenEventAccess` 才是。

6. **controller 是 `!Send`/`!Sync`，且 `new()` 必须运行在 macOS 主线程**（shim 用
   `Thread.isMainThread` 判定，非主线程返回 status 3 → `NewControllerError`）。

7. **最低版本是 macOS 13。** crate 的 `build.rs` 写 `MINIMUM_MACOS_VERSION = "13.0"`，两处
   `Package.swift` 都声明 `.macOS(.v13)`，实测 swiftc 收到 `-target arm64-apple-macosx13.0`。
   产品 `LSMinimumSystemVersion` 是 `12.0`。

8. **在 arm64 宿主上无法交叉编译到 `x86_64-apple-darwin`。** 实测
   `cargo build --target x86_64-apple-darwin` 链接失败（Swift 对象是 arm64，Rust 目标要 x86_64）。
   根因在 `swift-rs 1.0.8` 的 `src-rs/build.rs`：Swift 的 `--arch` 取自
   `std::env::consts::ARCH`（宿主架构），且 `cross_compiling` 把 `x86_64-apple-darwin` 判为非交叉。

9. **能力面比「输入监控」这一项大。** shim 导入 `Carbon`、`CoreBluetooth`、`StoreKit`、
   `MusicKit`，`fullDiskAccessAuthorizationState()` 会读 `/private/etc/sudoers`、
   `/Library/Application Support/com.apple.TCC/TCC.db` 等路径，Rust 侧也暴露 8 类权限。

10. **CI 构建的 arm64 包在 macOS 27 上点「打开系统设置」会 SIGTRAP。** 复现（v2.1.0 CI 包，
    arm64，macOS 27.0）：点击引导按钮进入 `request_permission_flow` → `start_flow()`，SwiftUI
    面板的 `NSHostingView` 加入窗口时断言失败，`bongocat-app` 崩溃。崩溃栈均在 SwiftUI 框架
    内（`NSHostingView.didChangeRequiredBridges` → `GraphHost.preferenceValues()` →
    `AGGraphGetValue` → `ViewBodyAccessor.updateBody` 内的 `_assertionFailure`）。同一源码、
    同一 `Cargo.lock` 的本地产物不崩。唯一可复现的差异是构建 SDK：v2.1.0 CI 包
    （`build-provenance.json` 为 arm64、`vtool` 显示 `sdk 26.5`、LD 1267.0）在 macOS 27 上崩，
    本地产物（`sdk 27.0`、LD 27037.1、Xcode 27.0）不崩。也就是说 permission-flow 的 SwiftUI
    面板是「Swift 静态库版本 ↔ 系统 SwiftUI」的运行时不变量敏感代码：用 macOS 26 SDK 编译的
    SwiftUI 客户端跑在 macOS 27 的 SwiftUI 上会触发 AG 图断言，反向（macOS 27 SDK）不会。

## 决策

### 1. 接受 Swift/AppKit 作为第二个厂商 FFI 例外

AGENTS.md §1 目前只允许「官方 Cubism Core 平台二进制」一个厂商 FFI 例外。本 ADR 明确追加第二个
例外，范围严格限定为 **macOS 输入监控权限引导的呈现**，与 ADR-0005 的 Cubism 边界并列：

- `permission_flow` 只出现在 `bongocat-platform` 的私有 `startup_permission` adapter 之后，
  它的类型不进入项目公共 API，也不进入 runtime、render、overlay、input、config、UI；
- 该例外不放宽「不引入 Tauri、WebView、Node.js、JavaScript 或第二套 UI framework」：Swift 面板
  是权限引导的专用表面，设置界面仍然只有 `gpui-kit`。

依赖只声明在 `bongocat-platform` 的 macOS target 下（`permission-flow = "=0.1.40"`），因此
Windows 构建不构建 `swift-rs`，也不受 Swift 工具链影响。

### 2. 保留 ADR-0032 的提示壳，只替换 macOS 的引导动作

`NSAlert` 提示、文案 key、worker 生命周期都不动。`startup_permission` adapter 的 macOS
`request_permission_flow` 换成 guided flow；Windows 侧只多接受一个被忽略的 locale 参数。

### 3. 每次进入授权流程前执行 `tccutil reset ListenEvent com.ayangweb.bongo-cat`

2026-09-30 明确选择，实现为 `reset_input_monitoring_grant()`，结果被忽略（系统没有该 bundle 的
条目时 `tccutil` 报错，而那正是要到达的状态）。**这一项偏离既有决策，必须同步修订 ADR-0024 与
ADR-0032：**

- ADR-0032 的「已授权则完全不提示」语义改变：流程入口会主动把「已授权」打回「未授权」，因此
  **每次点引导按钮都要求重新授权**，即使此前已经授权过。
- ADR-0008 固定 Bundle ID 为 `com.ayangweb.bongo-cat`、Development 与 Production 共用，因此该
  reset 会**同时清除已安装 Production 应用的授权**。这与 ADR-0008「开发构建不得读取、写入或锁住
  生产数据」的精神冲突，本 ADR 记录该冲突并接受它。
- 正在运行的事件 tap 会因此失去授权。既有恢复路径（tap 被禁用 → 重启 → `CGPreflightListenEventAccess`
  复查 → `PermissionDenied`）已覆盖这一状态，但**用户重新授权之后没有自动重启输入服务的触发点**，
  即重新授权后输入要等应用重启才恢复。这是本决策的已知后果，见「未完成项」。
- 重置让提示文案里那段「如果列表中已有 BongoCat，请先选中并点击「−」移除，再点击「＋」重新添加」
  变成描述产品自己已经做完的事：条目在面板打开前就被清掉了。因此
  `startup_permission.input_monitoring.description` 只保留用途那一句，第二段从全部 6 个 locale
  删除（2026-09-30）。Windows 的 `administrator.description` 保留——它的第二段是那条路径上唯一
  说明兼容性开关怎么设的地方。

### 4. 不使用 `AppPath::suggested_host_app()`

拖拽目标取自当前可执行文件所在的 `.app`（`enclosing_bundle`），没有 bundle 时取可执行文件本身。
Swift 侧会把非 `.app` 项过滤掉，因此开发二进制得到一个空的拖拽目标，面板仍显示面板引导。
契约测试禁止 macOS 模块出现 `suggested_host_app`。

### 5. 语言对齐靠 `CFBundleLocalizations` + 本应用域的 `AppleLanguages`

上游 Rust 接口没有 locale 参数，所以按实测结论第 3 条走 macOS 自己的机制：

- `macos/Info.plist` 声明 `CFBundleLocalizations`，列出 Swift 面板实际提供 catalog 的语言
  （`ar`、`en`、`pt`、`zh-Hans`、`zh-Hant`）。缺了它，嵌套 bundle 永远回落到 development region。
- 进入流程前把产品语言写进本应用 CFPreferences 域的 `AppleLanguages`（`apply_flow_language`）。
  locale 由 `bongocat-app` 解析后经 `StartupPermissionPrompt::locale` 传入 adapter，与提示文案
  同源，不会各自漂移。
- 该偏好是持久的，写的是本应用自己的域，等价于「本应用的界面语言」，因此应用的其他 AppKit 表面
  也会跟随——这是语言设置本来就该有的行为。
- `vi-VN` 没有上游 catalog，回落英文，记录为已知缺口。

### 6. 链接与打包的必做项（已实现）

- `crates/bongocat-app/build.rs` 的 `link_swift_runtime()` 在 macOS 目标上输出
  `-Wl,-rpath,/usr/lib/swift`；契约测试固定这一行。
- `bongocat-packaging` 在打包前定位 `permission-flow` 的 `OUT_DIR` 里的 Swift 资源包，映射进
  `Contents/Resources`，并在 `verify_app_bundle` 里断言它存在——丢了这个包会在用户手上崩溃，必须
  在打包阶段失败。
- 开发二进制没有打包步骤，adapter 在资源包不可解析时把它从 Cargo profile 目录复制到可执行文件
  旁（`ensure_resource_bundle_available`），并且**不写入 `.app`**（会破坏签名）。复制失败时不启动
  引导面板，退回 ADR-0032 的原有动作——Swift 面板在找不到资源包时会 abort 整个进程，崩溃不是可
  接受的降级方式。

### 7. 依赖来源：临时固定在 fork 的 rev

macOS 12 支持只能先落在 fork（上游 PR <https://github.com/veecore/permission-flow/pull/2>）：

- fork `ayangweb/permission-flow` 的 `feat/macos-12-support`，rev `fad0354d960cf775ac332e6375916f7cc6424c7b`。
- 根 `Cargo.toml` 的 `permission-flow` 暂时写成 `{ git = …, rev = … }`；`deny.toml` 的 `allow-git`
  相应增加该来源。`required-git-spec = "rev"` 保证不会漂到分支上。
- fork 里的改动只有 macOS 12 相关这几类：`MINIMUM_MACOS_VERSION` 与两处 `Package.swift` 的
  `.macOS(.v12)`、全部 `@available(macOS 12.0, *)`、`PermissionFlowLocalizer` 在 12 上按 region
  推断 script（`Locale.Language` 是 13+）、`SettingsNavigator` 兼容 System Preferences 的旧路径，
  以及隐私面板 deeplink 的面板标识按系统版本切换（macOS 12 只认
  `com.apple.preference.security`，13+ 才是 `com.apple.settings.PrivacySecurity.extension`；
  这一点是实机验证 macOS 12 时发现的）。`PermissionFlowButton` 仍是 13+，因为它用
  `LocalizedStringResource`。
- 上游合并并发布后必须改回 crates.io 精确 pin，并从 `allow-git` 删掉这一行；在此之前
  `Cargo.lock` 记录的是 git source，`cargo deny check sources` 是这一项的守门。

### 8. CI 产物的 SDK 必须匹配产物运行面向的 macOS 大版本（2026-10-03）

v2.1.0 CI 的 arm64 包在 macOS 27 上点权限引导的「打开系统设置」会 SIGTRAP（实测结论第 10 条），
根因是同一 Swift 面板在「macOS 26 SDK 编译、macOS 27 运行」的组合下触发 SwiftUI AG 断言。修复：
release workflow 的 Apple Silicon leg 从 `macos-latest`（当时是 macOS 26 / Xcode 26.x）换成
`xcode-27`（macOS 27 / Xcode 27.0 / SDK 27.0），让 CI 产物与实测不崩的本地产物用同一代 SDK。
Intel leg 保留 `macos-26-intel`：macOS 27 是 Apple silicon 专用（2026-09 公布的兼容列表不含 Intel
机型），Intel 机最高只到 macOS 26，SDK 26.x 产物与运行面匹配。两个 leg 的 Xcode 大版本随平台
能力各自取最新，不再人为对齐；「配置完全一致」的原始意图降级为「同为原生 runner」，见 ADR-0033
修订节。后续 macOS 大版本迭代时，两条 leg 的 Xcode/SDK 必须同步复核。

## 未完成项

- **上游 Rust 接口仍没有 locale 参数**：语言对齐继续依赖决策第 5 条的两半机制，等上游加参数后
  移除。
- **macOS 12 只验证了一部分**：实机（macOS 12）确认面板、拖拽卡片与资源包都正常，并因此发现
  deeplink 的面板标识问题（已在上游 PR #2 修掉）。仍未实机确认：修正后的 deeplink 是否落到
  「安全性与隐私 → 输入监控」、以及浮动面板的拖拽引导在 macOS 12 的复选框列表上是否可用——
  macOS 12 的隐私面板不接受拖拽，只提供「+」按钮，面板文案目前没有为该系统区分。
- **上游 PR 合并前依赖来源是 fork**：见决策第 7 条，合并发布后必须切回上游版本。
- **`x86_64-apple-darwin` 无法交叉编译**：`swift-rs` 让 swiftc 按**宿主架构**编译，所以在
  arm64 机器上构建该目标会链接失败（`_permission_flow_*` 未定义符号）。release workflow 已改为
  两条 macOS leg 各用一台同架构 runner（ADR-0033 修订节），发布不受影响；但本机
  `just build --target x86_64-apple-darwin` 仍打不出来。治本要 `swift-rs` 的 `--arch` 跟随
  Rust target（上游 `Brendonovich/swift-rs`，1.0.8 是当前最新版，尚无此改动）。
- **重新授权后输入服务不会自动重启**：见决策第 3 条。
- **实机验收未做**：面板外观、拖拽引导是否可用、以及它与 `GPUIApplication`（ADR-0032「macOS 弹框
  实现修正」记录的 `Ivar platform not found on class NSApplication` 崩溃类别）共存时是否安全，都
  需要人工在真机观察。自动化测试无法覆盖。
- **语言切换的缓存边界未验证**：`Bundle.preferredLocalizations` 按进程缓存。`AppleLanguages` 在
  流程开始前写入，但若进程内已有更早的读取，面板可能仍用旧值；这一条只能实机确认。

## 验证

- 契约测试：macOS 模块禁止出现 `suggested_host_app`；`bongocat-app` 的 build script 必须含 Swift
  运行时 rpath；macOS 模块必须含 `ListenEvent`；原有的 NSAlert / `run_on_main` / 禁止
  `rfd::MessageDialog::new` 与 `AsyncMessageDialog` 断言不变。
- 打包验证：`verify_app_bundle` 断言 `Contents/Resources/PermissionFlow_PermissionFlow.bundle`
  存在。
- fork 侧的 macOS 12 验证：`swift build -Xswiftc -target -Xswiftc arm64-apple-macos12.0`（含
  `--build-tests`）无 error 无 warning，`swift test` 5 passed，`cargo fmt --all --check` /
  `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace --lib` 通过，
  `otool -l libPermissionFlowShimFFI.a` 显示 `minos 12.0`。
- 可重复的人工验收命令：`cargo run -p bongocat-app -- --run-seconds 0`，在未授权时应出现提示，
  点主按钮后应依次看到 TCC 授权被清除、系统设置打开到「输入监控」、以及浮动的拖拽引导面板。
  `--startup-permission-smoke` 仍然只输出能力状态，不弹框。

## 替换边界

替换点是 `bongocat-platform` 的私有 adapter（macOS 的 `request_permission_flow`、主线程
controller 槽、资源包与语言处理）、`crates/bongocat-app/build.rs` 的 rpath、`macos/Info.plist`
的 `CFBundleLocalizations`，以及 `bongocat-packaging` 的资源映射与校验。升级 `permission-flow`
时必须复验：`MINIMUM_MACOS_VERSION`、`SwiftLinker` 的 `--arch` 是否改为跟随 Rust target、
`Bundle.module` 的查找路径、`PermissionFlowConfiguration` 的 locale 字段、上游 catalog 的语言
集合，以及 Swift 工具链最低版本（上游 `swift-tools-version` 为 6.0 / 6.1，不在
`rust-toolchain.toml` 覆盖范围内，CI 需显式提供）。上游合并发布后，替换点还包括依赖来源本身：
`Cargo.toml` 的 git rev 与 `deny.toml` 的 `allow-git` 条目。

---
<a id="adr-0089"></a>
# ADR-0089: Windows window mode for capture

Date: 2026-10-08
Status: Accepted; Windows implementation and OBS capture verified

## Context

Issue #1130 needs an ordinary Windows model window that OBS can select and
capture, with a solid background for chroma keying. The existing popup uses a
transparent DirectComposition surface and is not an ordinary capture target.

Mver baseline `4da0b9468ad3b6ffaa096eba3f080501d6ab0b5c`,
`BongoCatMver/include/catmain.h`, provides the behavior evidence:
`setWindow()` restores the original caption and thick frame when
`decoration.desktop_pet` is false; `clearCatWindow()` clears to opaque
`decoration.rgb`; desktop-pet movement/resizing and click-through are pet-only.
`src/main.cpp` maintains the model aspect ratio on ordinary window resize.

## Decision

- Add optional v1 `overlay.window_mode` (default false) and
  `overlay.window_background_color` (RGB bytes, default green `[0, 255, 0]`).
  Missing fields keep existing data readable. Shared typed configuration,
  commands and snapshots carry these values; only Windows applies them.
- Windows settings expose a switch and the GPUI Kit color picker. No dependency
  or second UI/rendering framework is added. macOS behavior stays unchanged.
- Window mode uses a caption, resize frame, minimize and close buttons, an
  activatable taskbar window and a D3D11 HWND swap chain with an opaque RGB clear.
  Preserve model-authored backgrounds and draw order. Users can choose a key
  color that differs from their model and its background.
- Suspend click-through, hover/idle hide, presentation opacity and rounded
  clipping while in window mode, without modifying the user's pet preferences.
  Always-on-top and explicit visibility remain independently controllable.
- Keep client dimensions separate from non-client frame dimensions. DPI and
  resize use current window metrics; the renderer sizes to the client area.
  Both Windows presentation modes use one client-size policy, retaining the
  unrounded canvas width/height. Numeric and gesture percentages use the current
  DPI and the 350-logical-pixel width at 100%; never multiply saved width/height
  by a relative ratio. Restore and mode/model changes retain client width and
  derive height from the current canvas before adding the frame. Apply minimum
  and maximum size limits uniformly, and retain the 25% gesture endpoint when
  several percentages map to the same minimum size.
  Native position changes (including DPI suggestions) enforce this policy;
  the independent system caption minimum must not resize one axis afterwards.
  The swap chain follows measured client bounds, including minimized placement.
  Publish scale only after a native sizing gesture changes client dimensions.
  Caption moves (including DPI changes), unchanged and cancelled resizing must
  not reinterpret saved geometry as a new configured scale.
- Closing requests runtime visibility off instead of destroying a live renderer's
  HWND. Minimizing must not overwrite saved geometry or break restoration.
- Model changes prepare/validate before commit and retain the capture HWND in
  window mode. Mode changes may replace presentation resources.

## Verification Gates

- [x] Old v1 data, defaults, RGB bounds, fixture/schema and typed round-trip.
- [x] Windows style, client geometry, close/minimize/restore and DPI contracts.
- [x] Opaque background and actual model pixels from D3D11 readback/capture.
- [x] Model switch and rejected model preserve capture/window state.
- [x] Workspace checks and Development product smoke: `just check`,
  `just dev-smoke` and system-menu smoke passed; the latter verifies
  window-mode toggle and restoration.
- [x] OBS window capture and chroma key inspection on Windows.
- [ ] Manual settings layout and cross-monitor movement at 125/150/200% DPI.
  Automated frame metrics at 120/144/192 DPI passed; actual OBS capture was
  inspected at the current 150% display scale.

The corresponding Phase 4 task is recorded in `docs/TODO.md`. Evidence and
remaining Windows platform checks are recorded in
`docs/benchmark/windows-window-mode-2026-10-08.md`.

---
<a id="adr-0093"></a>
# ADR-0093: 语言只保留中文与英文

状态：已接受（2026-10-19）

## 背景

产品原先随包八份 catalog：`en-US`、`zh-CN`、`zh-TW`、`ar-SA`、`vi-VN`、`pt-BR`、`ko-KR`、
`fr-FR`。其中两个中文只看字形差别，其余六种没有任何一版是逐句校对过的：新增一个键要写
八遍，`stats.stage.name.*`、`cultivation.*` 这类成组新增一次就是几十行重复，而校对只能覆盖
到中英两份，别国用户读到的其实是没人看过的机器翻译。产品只需要中文和英文，多出来的六份
不是能力而是负债。

## 决策

**只发两份 catalog。** `crates/bongocat-i18n/locales/` 只剩 `en-US.json` 与 `zh-CN.json`；
`SHIPPED_LOCALES` 收敛为两项，`build.rs` 的 `CATALOGS` 同步。`tools/validate-locales.py`
的 `EXPECTED_LOCALES` 与打包契约测试都断言这两项，因此「加语言」不再是改文档的事，而是
必须同时改四处并让契约测试通过。

**回退规则按 RFC 4647 主语言子标签。** `locale_code` 只剩一个判断：`zh` 开头的任何标签
（`zh`、`zh-CN`、`zh-Hans`、`zh-TW`、`zh-Hant-HK`）都落到 `zh-CN`，其余一律落到 `en-US`。
繁体标签读简体是刻意的取舍：宁可给一份读得懂的简体，也不给一份读不懂的英文。原本
「简体/繁体各一份」的脚本子标签分支因此消失，`is_simplified_chinese` 删除。

**枚举只留三个值。** `bongocat_config::Language` 与 `SettingsLanguage` 都是
`System | ChineseSimplified | English`，持久化值为 `system` / `zh-CN` / `en-US`，
其余六个变体从枚举、投影（`settings/projection.rs`）、语言选择器与 UI 名称函数里一起删掉。

**安装器跟着收。** Windows NSIS 的 `INSTALLER_LANGUAGES` 只剩 `English`、`SimpChinese`，
仓库自带的 `Vietnamese.nsh` 与 `custom_language_files` 一并删除——两份 catalog 都在
`cargo-packager` 自带的安装器文案里。

## 后果

- 繁体用户、以及任何非中英系统（`de-DE`、`ja-JP`、`ar-EG`……）都落到英文；繁体系统落到简体。
  这条写进 `Language::from_system_locale` 的注释与测试，不再是隐含行为。
- 旧配置里若存着 `"language": "zh-TW"` 之类已经不合法的值，整份配置会解析失败并走既有的
  「备份回滚 / 回到默认」路径：结果是主题与语言回到系统默认，而不是崩溃。这比静默把
  `fr-FR` 读成英文更保守，代价是那一份配置里用户的其他设置也一起回默认。
- 每份 catalog 仍是 377 键，`catalog_keys_are_referenced_by_source` 与
  `source_referenced_keys_exist_in_the_catalog` 双向覆盖不变：键少了，规则没松。
- `settings/appearance/language/options` 下六条 `chinese_traditional`/`arabic`/`vietnamese`/
  `portuguese`/`korean`/`french` 文案删除；语言选择器从 8 项变 3 项（跟随系统 + 简体 + 英文）。
- 后续要加语言，成本是「一份新 catalog + 语言枚举 + 安装器语言 + 两处契约断言」，
  而不是「再翻译一遍」。

## 验证

- `python3 tools/validate-locales.py`：2 份 catalog，377 键，占位符一致。
- `cargo test -p bongocat-i18n`：区域子标签回退表（`en-GB`、`zh_TW`、`zh-Hant` 等）、
  非中英语言落到英文、`SHIPPED_LOCALES` 与磁盘上的文件一致。
- `cargo test -p bongocat-config`：`from_system_locale` 的全部分支与显式选择不跟随系统。
- `python3 -m unittest tools.tests.test_packaging_contract`：catalog 集合与安装器语言集合相等。

---
<a id="adr-0099"></a>
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
