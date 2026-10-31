# 架构决策总览

> **BongoCat 架构决策逻辑汇总（自包含）**：本文档将仓库全部架构决策记录（ADR）浓缩为一份可独立阅读的逻辑文档，覆盖 ADR-0001~0099 与番茄钟/修仙系列（0090~0098），共 **102 项**。

> 每项保留**原始编号、标题、状态**与**决策 + 关键后果 + 编号逻辑 + 后续修正**；**背景叙述、验证门禁、被否方案**为求精简已丢弃（框架由各章导言提供）。被取代的决策以单行标注其替代，正文不重复。

> **编号碰撞说明**：仓库存在少量不同决策共用同一编号（0012、0065、0066、0068），下文以标题区分，均独立成条。

> **ADR-0100**（资源与采集 v2.2）为独立在途决策，未纳入本汇总。

---

# 一、架构 / 平台 / 设置框架

> 应用架构、平台适配、设置框架与配置/本地化边界。

### ADR-0001 · Single Rust Application

**状态：** Accepted

### Decision

BongoCat 自有应用代码统一使用 Rust 2024 edition，并在一个 Cargo workspace 中组织。业务模块通过强类型 Rust 接口协作；平台差异进入明确的 platform adapter。

官方 Cubism Core 平台二进制是唯一允许的厂商 FFI 例外。该例外只提供模型计算能力，不承载 BongoCat 业务逻辑。

### Consequences

- 状态、配置、模型和行为只实现一次。
- Windows/macOS 的窗口、输入和 GPU backend 仍分别实现。
- 平台 handle、裸指针和 `unsafe` 不能进入共享业务模块。
- Linux 可在首发后通过新增 platform/renderer backend 评估。

### ADR-0002 · GPUI for Settings UI

**状态：** Accepted, pending Phase 0 validation

### Decision

使用 GPUI 构建设置、模型管理、快捷键、权限、更新和诊断界面。首个 spike 固定 `gpui = "=0.2.2"` 并提交 `Cargo.lock`。

GPUI `Entity` 只拥有临时视图状态。运行时配置和业务状态通过强类型 command/snapshot 边界访问。

### Consequences

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

### ADR-0003 · Native Overlay Renderers

**状态：** Accepted, macOS validated; Windows pending Phase 0 validation

### Decision

模型窗口使用独立原生 overlay：

- Windows：Win32 + D3D11 + DXGI + DirectComposition/DWM。
- macOS：AppKit `NSPanel` + Metal + `CAMetalLayer`。

Renderer 只消费不可变 `RenderSnapshot`，不访问 GPUI、配置或输入服务。

### Consequences

- 设置 UI 与实时渲染拥有独立的窗口和 frame loop。
- 两个平台分别实现 GPU backend，但共享模型求值和 render contract。
- 首发不为了后续 Linux 强行统一 GPU API。
- 窗口、renderer 和 GPU object 必须有明确的 owner 与析构顺序。

### ADR-0004 · Reconcilable Input State

**状态：** Accepted, pending platform validation

### Decision

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

### Consequences

- 单个释放事件丢失不会永久卡键。
- 队列溢出必须可观测，不能静默丢弃边沿。
- 丢失 `KeyUp` 的按键只能由状态校正或生命周期 `Reset` 释放；校正延迟上限由平台查询周期和确认
  次数共同决定，实机应分别测量权限、锁屏和睡眠恢复下的延迟。
- macOS CapsLock 的释放由 adapter 合成，`CGEventSourceKeyState` 反映锁存状态、无法替代它；该
  合成边沿走 `InputSource::Capture` 通道，runtime 侧因此把它计入 `captured_up`。这是已知且接受的
  精度损失，平台侧计数器不声称该边沿来自系统。
- Renderer 不直接查询系统键盘状态。

### ADR-0005 · Cubism Core FFI Boundary

**状态：** Accepted, pending SDK and license validation

### Decision

通过窄 Rust sys layer 调用官方 Cubism Core，并在其上建立 safe Rust wrapper：

```text
Cubism Core binary -> raw bindings -> Moc/Model wrappers -> model evaluation
```

原始指针不得离开 wrapper。Rust owner 必须保证 Moc、Model 和 backing buffer 的存活与析构顺序。BongoCat 的动作、表达式、物理、资源和状态逻辑保留在 Rust 模块中。

### Consequences

- 构建清单必须记录 SDK/Core 版本、来源、hash、架构和许可证。
- FFI callback 不执行阻塞工作，不允许 panic 穿越边界。
- 模型切换采用 prepare/validate/commit，失败保留当前模型。
- 不通过扩大非 Rust bridge 来绕过兼容性问题。

### ADR-0006 · Linux Is a Post-Launch Evaluation

**状态：** Accepted

### Decision

Linux 不进入首发支持矩阵，不阻塞 Windows/macOS 的 Phase 0、实现和发布。

共享 Rust crate 保持平台无关：业务接口使用项目自有类型，平台能力通过 adapter 和 capability reporting 提供。Windows 使用 D3D11，macOS 使用 Metal；首发不为了 Linux 预先引入统一窗口、输入或 GPU abstraction。

Linux 只在首发后按独立 spike 评估。评估必须分别覆盖 X11 与 Wayland，并允许结论是某些桌面环境、compositor 或全局输入能力不受支持。尤其不得承诺 Wayland 下存在与 Windows Raw Input 或 macOS CGEventTap 等价的全局键鼠捕获。

### Consequences

- 首发 CI 可以对平台无关 crate 执行 Linux `cargo check`，但该结果不代表 Linux 产品支持。
- 新增共享业务 API 时不得暴露 HWND、Objective-C 对象、X11 handle、Wayland object 或 backend 专用 GPU 类型。
- Linux backend 可以在首发后新增，不要求修改已冻结的输入、配置和模型语义。
- 若 Linux 平台能力不足，产品必须显式降级或限制支持范围，不能用静默丢失输入代替能力声明。

### ADR-0007 · Single Rust Runtime Environment

**状态：** Accepted

### Decision

生产应用使用一个 Rust 进程和一个业务 runtime owner。GPUI 设置界面、平台输入服务、Live2D 求值、原生 overlay 和系统集成都属于同一 Rust 应用生命周期，并通过强类型 Rust command、event 和 snapshot 协作。

不得引入第二套常驻业务运行环境、跨语言共享业务 core、sidecar 业务进程或用于同步核心状态的 IPC 边界。历史应用只作为行为与资源对照，不随 BongoCat 生产应用启动，也不作为功能 fallback。

官方 Cubism Core 平台二进制仍是 ADR-0005 定义的窄 FFI 例外。它只提供模型计算能力，不成为独立业务 runtime，不拥有应用状态。

安装或更新若因操作系统权限必须使用短生命周期 helper，需要在系统集成阶段单独威胁建模和 ADR；helper 必须使用 Rust、不得持有持续业务状态，也不得改变本 ADR 的单一应用 runtime 所有权。

### Consequences

- pressed state、动画、当前模型和配置协调只存在一个可变 owner。
- GPUI 关闭或重建不会创建第二份业务状态；UI 只发送 command 并读取带 revision 的 snapshot。
- 输入 callback、renderer 和平台服务不能绕过 runtime 直接维护产品状态。
- 发布切换前保留历史源码、tag 和样本；生产包不包含历史 runtime、Web 资产或旧配置读取器。

### ADR-0008 · Application Identity and Isolated Storage Environments

**状态：** Accepted

### Decision

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

### Consequences

- 首次启动 BongoCat 时生成当前环境的全新配置。
- 开发构建可安全使用合成数据和自定义模型，不影响生产安装。
- 旧配置目录和字段只保留为历史行为参考，不进入生产 dependency graph 或发布产物。
- 用户模型仍可通过受验证的显式导入流程加入；不会根据旧配置路径自动发现或搬运。
- 工作区直接 Cargo 命令默认 Development；Production build/package 必须显式启用
  `bongocat-app/production`。
- Production 构建不能携带存储根注入能力；需要隔离临时数据根的进程级测试必须使用独立 Development 测试产物并显式启用 `storage-test-injection`。
- 开发过程中产生的旧 schema 或中间数据需要删除并重新生成，不属于产品兼容输入。

### ADR-0009 · GPUI Accessibility Gate

**状态：** Proposed；历史 release/UI-completion gate，项目自有 AccessKit 部分由 ADR-0054 取代

> 后续修订（2026-09-23）：ADR-0054 将设置 UI 收口为 visual-first。项目自有 AccessKit tree/native bridge/action 不再是 UI 完成或发布门槛；`gpui-kit` 传递语义可保留，但不是 BongoCat 语义契约。

### Decision

GPUI 继续作为当前首选设置 UI，但在辅助功能 gate 通过前保持 provisional：

- 允许建立正式 UI crate 和最小产品窗口，但在门禁通过前不实现或宣称完整、可发布的设置界面；
- 不引入 GPUI 私有 API、未维护 fork 或隐藏控件 workaround；
- 使用 GPUI 公开的 raw window handle 安装项目自有 AccessKit semantic tree；macOS/Windows adapter 只负责系统协议，语义 snapshot 由 UI crate 维护；
- 辅助技术 action 必须通过有界强类型 channel 回到 GPUI 主线程，不允许平台 callback 直接修改 Entity 或 runtime；
- 可见控件和语义节点使用同一份 UI 状态，必须覆盖 role、label/title、value、focus、selected、loading/error 和支持的 action；
- 若 GPUI 仍无法提供基础语义，才启动 Iced 替代 spike。Iced 只作为此门槛的 fallback，不与 GPUI 并行进入产品。

截至本 ADR 日期，Iced 0.14.0 的公开 crate feature/source 中也未发现可直接满足 role/label/value 的通用 accessibility API，因此它是待验证候选，不是当前 go 结论。不能因为换 UI 框架名称就视为问题已解决。

### ADR-0011 · Progressive Implementation With Release Gates

**状态：** Accepted

> 后续修订（2026-09-23）：ADR-0033 已退役 Windows 原生 ARM64，Windows on ARM 改由 x64 仿真覆盖；因此本文的 Windows ARM64 desktop Core 发布门槛只作历史记录。

### Decision

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

### Supplied SDK Disposition

当前本地基线是 Cubism SDK for Native `5-r.5`，archive SHA-256 为
`7ff3a4bbc19c0a8728965aa522ab77eb11b252916453e68a8a78d3b71188bb12`，Core
版本为 `06.00.0001`。它包含 Windows x64 和 macOS arm64/x64 Core，不包含
desktop Windows ARM64 Core。

此前提供的 `CubismSdkMotionSyncPluginForNative-5-r.2.1.zip` 是可选 MotionSync
插件，SHA-256 为
`26baaf30b0ebb26bc6253884aa61b010666e2ecc2491e3a7fff6d43eb64d6548`。它不是
标准 Native SDK，不能加载 `.moc3`，当前不进入产品范围。

### Consequences

正式实现可以从已验证的 runtime/config 契约开始，不再等待外部测试人员或合规
流程。发布状态必须单独判断；“功能已实现”不表示“可以公开分发”。如果后续发布
核对、SDK ABI 或实机验证失败，受影响的发布目标保持阻塞，并通过后续 ADR 调整
能力或目标矩阵，不能用错误架构的 artifact 或模拟结果绕过。

### ADR-0012 · JSON Localization

**状态：** 已接受（2026-09-10；2026-10-06 修订）

### 决策

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

### 约束

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
- UI 文案的书写约定见 `localization-copy-conventions`。当前已落地的一条：**省略号一律写
  单个 `…`（U+2026，视觉上是三个点）**，禁止中文排版习惯的 `……`（六个点）与拉丁写法的 ASCII
  `...`——同一个 key 由所有语言共用，写法必须与语言无关。该规则由 `tools/validate-locales.py`
  机械强制，不依赖 review。

### 取舍

`gpui-kit` 的底层 `gpui-component` 也使用 `rust-i18n`，因此依赖生态一致。`rust-i18n` 与传递的 `rust-i18n-macro`／`rust-i18n-support` 必须停在同一个补丁序列：4.2.4 的宏展开调用 `rust_i18n::replace_patterns_cow`，只存在于 4.2.4 的运行时 crate，跨版本混用会让 `gpui-component` 与 `bongocat-i18n` 同时编译失败。JSON 比 YAML/TOML 更适合现有前端资源、翻译工具和跨语言校验；本方案不引入 Fluent 的复数/选择语法。若后续产品需要复杂 ICU/Fluent 消息，应另行提交 ADR，不在业务代码中混用第二套格式。

### ADR-0013 · Startup Item Capability and Environment Isolation

**状态：** Superseded by ADR-0043

> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

### Decision

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

### Consequences

- macOS 12 用户可使用产品核心功能，但设置页会明确显示登录启动不可用。
- 开发构建不能直接演练 macOS 注册变更；平台 contract test 与 Production bundle smoke 负责
  覆盖，公开发布前仍需签名构建和 System Settings 批准路径实测。
- Windows 安装位置变化会显示 stale，而不是误报 enabled；用户再次启用可原子覆盖当前环境
  value。
- 启动项状态属于平台 snapshot，不进入配置 schema，也不由 runtime 动画状态持有。

### ADR-0014 · Formal Settings Accessibility Bridge

**状态：** Accepted（2026-08-31）

> 后续修订（2026-09-23）：ADR-0054 已退役项目自有设置语义桥。本文保留作为当时的 AccessKit/action 实现证据；当前设置 UI 只验收可见控件、焦点和键盘导航。

### Decision

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

### Consequences

The startup switch now has a formal switch role, value, toggle state and action mapping on both
platform adapters. This does not claim VoiceOver/Narrator or physical assistive-technology
verification; those remain release evidence gates. If GPUI later exposes an equivalent stable API,
the platform adapter can be removed without changing runtime or settings command contracts.

### ADR-0015 · Separate UI Snapshot and Configuration Revisions

**状态：** 已接受

### 决策

- `SettingsSnapshot.revision` 是 UI 可观察内容的单调版本，只用于丢弃过期异步结果和刷新视图。
- `SettingsSnapshot.config_revision` 是当前环境持久化配置的版本；没有可编辑配置 snapshot 时为 `None`。ADR-0054 后，损坏配置按备份/默认值 fallback，恢复路径仍进入普通设置窗口，不再有 recovery-only 版本。
- 所有已接入的配置/模型选择 command（overlay settings、motion audio、model selection）携带
  `expected_config_revision`，settings worker 在调用 Application 写入前比较它。Overlay visibility
  是 runtime-only command，不写配置；它仍携带当前 config revision 作为 stale-view guard。
- 比较失败返回匿名 `SnapshotOutdated`，不改变 runtime、配置文件或任何 revision；UI 读取最新
  snapshot 并保留可操作错误。

### 后续边界

模型选择、启动项和其他配置域接入 optimistic concurrency 时继续使用 `config_revision`；纯
平台/诊断状态不得改变该 token。`revision` 仍可因这些状态变化推进。

### ADR-0019 · gpui-component Design System

**状态：** 已被 ADR-0020 取代（2026-09-04）

> 后续修订（2026-09-23）：除 ADR-0020 的 facade 取代外，ADR-0054 进一步退役了项目 AccessKit bridge；本文的组件与语义细节均为历史证据。

### 决策

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

### 影响

`gpui-component` 提供的控件范围更广，但传递依赖和编译成本高于 Guise。锁文件必须提交，后续
升级继续按仓库依赖规则审计版本、许可证、平台构建和完整输入/窗口 smoke。双平台辅助功能、缩放
及真实交互证据仍是 UI 完成门禁，本 ADR 不将这些 TODO 标记完成。

### ADR-0020 · GPUI Kit 统一依赖入口

**状态：** 已接受（2026-09-04）

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

### 决策

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

### 影响

manifest 和 Rust import 只有一个版本入口，避免应用与组件解析到两套 GPUI 类型。来源在
2026-09-28 回到 crates.io 精确 pin，lockfile 记录 registry checksum，`deny.toml` 不再放行
GPUI Kit 的 git 仓库。代价是 GPUI Kit
统一管理整套传递版本，升级必须作为单独变更重跑双平台构建、设置窗口、IME、辅助功能、缩放、
窗口重建和 shutdown smoke；ADR-0054 后“辅助功能 smoke”不再是项目自有 UI 完成条件。本 ADR 不把仍缺少实机证据的 UI TODO 标记完成。

### ADR-0024 · macOS TCC Capability Boundary

**状态：** 已接受（2026-09-05）

> 后续修订（2026-09-23）：ADR-0054 退役了项目自有 AccessKit/AppKit 设置桥；Input Monitoring 与 Accessibility TCC 的边界仍然有效。

### 决策

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

### 引导流程与逐次重置（2026-09-30，ADR-0078）

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

### ADR-0028 · Rust i18n catalog structure

**状态：** 已接受（2026-09-10）

### 决策

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

### 平台覆盖（platform override）

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

### ADR-0030 · Prefer Existing Solutions Before Implementing

**状态：** 已接受（2026-09-13）

### 决策

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

### 影响

- 通用能力默认采用成熟方案；自研只保留产品特有语义、可靠性和平台所有权边界所需的适配代码。
- 新依赖仍是显式决策，不因“优先复用”免除版本、许可证、安全性和可维护性检查。
- 现有已经落地且满足契约的实现无需仅为遵循本 ADR 进行重写；后续新增或替换功能按本顺序评估。
- ADR-0029 把更新实现从自研栈切换到 `self_update`，与本 ADR 的方向一致。

### ADR-0032 · Startup Permission Prompt Boundary

**状态：** 已接受（2026-09-14）；同日实机验收发现 macOS 侧必须使用不触碰 AppKit 的 `rfd` 路径，见「macOS 弹框实现修正」；2026-09-15 修正检查执行方式为专用 worker 线程上的非阻塞检查，见「非阻塞执行修正」；同日启用 Windows 自定义按钮文案，见「Windows 按钮文案修正」；2026-09-18 为修复主题跟随把 macOS 提示改为主线程 `NSAlert`，见「主题外观修正」；2026-09-24 收口 Windows 按钮文案，使其准确描述当时的按钮文案；2026-09-26 Windows 提示标题改为「建议以管理员权限运行」，次要按钮改为「稍后设置」并与 macOS 共用同一个 key `startup_permission.later`；同日主按钮改为「退出并前往设置」

### 决策

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
  行为对照仍见 `legacy-behavior-inventory`，不在本 ADR 重复维护。

### macOS 弹框实现修正（2026-09-14 实机验收）

首次实机验收在真实打包产物上复现了「启动即 abort」的崩溃，根因是下面这条 GPUI 与 AppKit 的
交互约束。原先「无父窗口同步弹框在 GPUI run loop 之前弹出是安全的」这一条结论是错的：它只检查了
`NSAlert::runModal`，漏看了 `PolicyManager`/`FocusManager` 对共享 `NSApplication` 的创建。

#### 现象与证据

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

#### 约束

- `gpui_macos` 在 `#[ctor]` 中注册 `GPUIApplication`（`NSApplication` 子类）并在其上声明 `platform`
  ivar；`MacPlatform::run` 先 `[GPUIApplication sharedApplication]`，再对该实例
  `set_ivar("platform", ..)`，而 `objc 0.2` 在 ivar 不存在时 panic。
- 进程共享的 `NSApplication` 实例由**第一个** `sharedApplication` 调用决定。若它先在基类
  `NSApplication` 上被调用（`rfd` 的同步 macOS 消息框正是如此），之后
  `[GPUIApplication sharedApplication]` 返回的就是基类实例，GPUI 的 `set_ivar` 随即 panic。
- 因此**在产品建立 GPUI 平台之前运行的任何代码都不得创建共享 `NSApplication`**。
  `NSWorkspace`、`CGPreflightListenEventAccess` 和 `CGRequestListenEventAccess` 已实机验证不创建它。

#### 决策修正

- macOS 启动提示改用 `rfd::AsyncMessageDialog`（无父窗口 → `async_pop_dialog` →
  `UserAlert::new(opt, None)` → 不构造 `PolicyManager`/`FocusManager` → 不触碰 AppKit），主线程用
  `async_io::block_on` 等待结果。这与 `directory_picker` 已在用的 `rfd` + `async_io` 用法一致，
  不引入新依赖、不改变提示出现时机（仍在任何产品 UI 之前）。
- Windows 保持同步 `MessageDialog`：`MessageBoxW` 没有该约束，且在调用线程上语义最直观；改用
  `TaskDialogIndirect` 后该路径也保持同步并在 worker 线程上运行，见「Windows 按钮文案修正」。
- 该路径用无点击方式验证过：请求弹框但不等待答复时，产品正常启动并干净退出（exit 0），同时采样确认
  对话框线程真实阻塞在 `CFUserNotificationReceiveResponse`（即弹框确实已提交显示）；同一位置若换回
  同步实现，则必然复现上述 panic。

### 非阻塞执行修正（2026-09-15）

启动实机使用中发现原始检查点（GPUI run loop 之前同步执行）让权限提示成为启动的第一个交互：
用户不应答，模型窗口、菜单栏/托盘和设置 UI 都不会创建。产品要求权限提示只作为辅助提示存在，
于是把「检查的执行方式」从启动路径中拆出来，检查与提示逻辑本身不变。

#### 变更

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

#### 线程安全证据（2026-09-15 核实）

- macOS 的异步 `rfd` 路径只构建 `CFUserNotification`、不触碰 `NSApplication`，在 worker 线程
  上与 GPUI run loop 并行是安全的；同步路径会在调用线程上运行 `NSAlert` 模态机制，禁止使用，
  原 contract 测试继续固定这一约束。
- `objc2-app-kit 0.3.2` 绑定中 `NSWorkspace::sharedWorkspace()` 与 `openURL` 均未标记
  main-thread-only（无 `MainThreadMarker` 参数），可在 worker 线程执行。
- Windows 的 `MessageBoxW` 是调用线程模态，不阻塞其他线程的消息循环；`OpenProcessToken` 与
  `opener::reveal` 均无线程亲和性要求。
- worker 与产品无共享可变状态，不存在与窗口创建、输入服务或 shutdown 的锁竞争。

### 安全与生命周期不变量

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

### Windows 按钮文案修正（2026-09-15）

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

#### 行为与证据

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

### 主题外观修正（2026-09-18）

ADR-0048 把用户选择的浅色/深色接到原生表面后，实机发现启动权限提示在应用深色下仍是浅色。根因：
macOS 的 `rfd` 无父窗口消息框（同步与异步都是）最终调用 `CFUserNotificationDisplayAlert`
（`rfd 0.17.2` `src/backend/macos/utils/user_alert.rs`），这条路径完全不经过 AppKit，不继承
`NSApplication.appearance`，只会跟随系统外观。ADR-0048 修复的正是 `NSApp.appearance` 继承链，
而本提示当时根本不在链上。

#### 变更

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

#### 证据与验证

- 根因证据：`rfd 0.17.2` 源码调用链 + 实机截图（应用深色、提示浅色）与该路径「只跟随系统外观」
  的行为一致；SDK 头文件确认 `NSAlert` 返回值与绑定常量的出入。
- `cargo check` / `cargo clippy --all-targets -p bongocat-platform` 通过；platform 全部单测通过，
  含更新后的 contract 测试：仍禁止 `rfd::MessageDialog::new`、要求 macOS 模块经 `run_on_main`
  呈现、禁止回退 `AsyncMessageDialog`。
- 深浅两色下弹框外观的实机肉眼验收未完成，归入 ADR-0048 既有的「双平台实机主题验收」门禁。

### 设置契约与软件信息报告的能力模型（2026-09-28）

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

### 替换边界

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

### macOS 引导流程换成 permission-flow（2026-09-30，ADR-0078）

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

### ADR-0043 · Startup Item Backend via auto-launch

**状态：** Accepted

> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

取代：ADR-0013
修订：ADR-0051 取代了"Development 构建从此支持启动项"这一条决策——开发构建现在按构建环境汇报
`Unsupported(BuildEnvironment)`，不再注册登录项。本文档的其余部分（`auto-launch` 后端、
LaunchAgent plist、HKCU Run、`--run-seconds 0`、契约变体保留、环境隔离命名）继续有效。

### Decision

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

### Consequences

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

### ADR-0048 · 主题跟随到原生表面

**状态：** 已接受（2026-09-18）

依赖：ADR-0030（优先复用现有方案）、ADR-0031（托盘图标边界）、ADR-0020（GPUI Kit 统一依赖入口）

### 决策

#### 1. `bongocat-platform` 新增 `theme` 模块，作为原生主题的唯一入口

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

#### 2. `System` 的解析口径收归平台层，并且全 crate 只有一处解析

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

#### 3. macOS：进程级 `NSApplication.setAppearance:`

这是唯一能覆盖全部四行表面的做法。按窗口设置只会让菜单和面板继续留在系统主题上，因为
`NSWindow.appearance` 不会被窗口自己弹出的菜单继承。

`NSAppearanceName*` 常量以字符串字面值写入（`appearanceNamed:` 与 `-isEqualToString:` 都按内容
比较），避免为一个常量引入 `unsafe`。深色判定接受 `DarkAqua`、`VibrantDark` 和
`AccessibilityHighContrastDarkAqua` 三个名字。

#### 4. Windows：窗口框用官方 API，其余表面接受"跟随系统主题"

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

#### 5. 启动时先应用进程级主题

主题的全局事实来源是持久化的 `appearance.theme`，不是设置窗口的生命周期。`bongocat-app`
在创建 overlay 之前解析该字段并调用 `bongocat_platform::apply_process_theme`：macOS 直接
设置 `NSApplication.appearance`，因此模型窗口及其右键菜单从第一帧起就继承正确外观；Windows
该入口不做应用级强制，继续由 `init_native_theme` 让系统-owned 表面跟随系统。设置/更新窗口
打开后仍会调用 `apply_theme`，但那是幂等补偿，不是首次生效的前置条件。

#### 6. 原生主题失败不致命

`apply_theme` / `init_native_theme` 的失败一律降级为"该表面保持系统外观"，也就是每个平台的
文档化兜底。UI 层因此 `let _ =` 丢弃结果，不向用户报错、不改配置、不阻止启动。理由：能拒绝的
表面恰好就是**本来就无法跟随应用主题**的那些，为一个外观开关拒绝启动或弹出错误更糟。

#### 7. 更新窗口改为携带应用外观，而不是硬编码 `System`

`UpdateView` 增加 `appearance_theme` 字段，`open_update_window` 增加同名参数，`ProductCoordinator`
缓存它并在系统菜单轮询里同步。更新窗口的 `observe_window_appearance` 回调改为读
`appearance_theme`：只有当前偏好是 `System` 时才响应系统变化，固定的偏好已经交给平台，不会因为
系统变了而改变。这修掉 B2。

### 明确不做

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

### 后记（2026-09-18）：启动权限弹框的例外已修复

本文档的「alerts | application theme」一行只覆盖经 `NSApplication.appearance` 继承链的 AppKit
弹框。落地当天的实机截图显示启动权限提示在应用深色下仍是浅色：该提示当时走 `rfd` 的无父窗口
消息框 = `CFUserNotification`，完全不经过 AppKit，不在继承链上——ADR-0048 的实现没有错，是
这个提示本身不在被修复的路径上。已按 ADR-0032「主题外观修正（2026-09-18）」把它改为主线程
`NSAlert`，本表格自此对产品全部系统弹框成立。

### 修正（2026-09-18）：Windows 上 `System` 回退不再是 no-op，乐观路径覆盖 `System`

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

### ADR-0051 · 开发构建不提供登录时启动

**状态：** 已接受（2026-09-20）

修订：ADR-0043（仅取代其"Development 构建从此支持启动项"这一条决策）；2026-09-23（设置项改为整行禁用呈现）
依赖：ADR-0008（应用身份与存储环境）、ADR-0043（启动项后端统一 auto-launch）

### 决策

#### 1. 门禁放在应用层，按构建环境判定

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

#### 2. UI：整行按统一禁用样式置灰，并保留可见原因

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

### 明确不做

- **不删除平台契约里的 `StartupItemEnvironment`、Development `app_name` 或 opt-in smoke**：
  Production 路径仍需它们，删掉会让"环境隔离"这件事失去唯一载体。
- **不在平台层再拦一次**：应用层是唯一生产者，重复判定只会产生第二个事实来源。
- **不为该状态新增文案键**：目录里已有同一句，第二条措辞只会带来漂移风险。
- **不做自动清理**：不在启动时扫描并删除历史上的 `BongoCat Development` 登录项。产品不删除它没有
  写入过的系统配置项（ADR-0008 的环境隔离原则），该项由用户手动移除。

### 后果

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

### ADR-0054 · 配置损坏恢复收口为 back-to-back fallback，设置 UI 采用视觉优先契约

**状态：** 已接受（2026-09-22）

依赖：ADR-0005（配置原子提交与备份）、ADR-0053（开关门禁规则，本 ADR 取代其中项目 AccessKit tree 部分）

### 决策

#### 1. 配置 fallback

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

#### 2. 设置窗口没有配置恢复 UI

- 不创建 recovery-only 设置窗口，不创建受限 runtime，不做 application operational gate。
- 不显示窗口级、页面级或分组级的配置恢复提示。
- 不提供 `RestoreDefaultConfiguration` 按钮、command、snapshot 字段或“恢复后必须重启”流程。
- `ExportDiagnostics` 和备份目录打开能力可以保留为排障入口，但不得重新长出配置恢复页面。
- 成功加载、备份恢复和默认 fallback 都回到普通启动路径；正常关闭与重启语义不变。

#### 3. 设置窗口的可见组成必须一致

- Development、Production 和 smoke 使用同一个设置窗口视图及同一页面/分组/按钮组成。
- smoke 通过测试 seam 触发导航、主题和窗口检查，不得通过隐藏或额外渲染用户控件来制造差异。
- 更新入口若由 application 注入，smoke 也必须注入等价 callback，使其可见组成与产品一致；
  smoke 是否真正发起网络更新不属于可见组成条件。
- 以后新增可见按钮或分组时，产品窗口、development smoke 和 release smoke 必须同时获得它；
  以后删除可见内容时也必须三处一致。

#### 4. 设置 UI 的辅助技术范围

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

#### 5. 文档契约

以后实现或评审设置 UI 时：

- 先问“用户是否看得见、是否可以操作”；看不见的实现默认删除。
- 不以 screen reader、VoiceOver、Narrator、UIA、AX tree 或 Accessibility permission 作为
  功能完成条件。
- 不在项目代码新增 `*_accessibility`/`*_a11y` 模块、隐藏 `Status` 节点、`supports_click`
  contract 或屏幕阅读器 smoke。
- 不把 `gpui-kit` 内部可能携带的语义当作 BongoCat 的业务 contract。
- 新增可见功能只保留实现功能所需的最小状态和测试，不预先建立恢复 UI、辅助桥或跨层协议。

#### 6. macOS 前台窗口与后台 Dock 偏好

- 设置或更新窗口打开期间（包括最小化）始终使用 `Regular` 并显示 Dock 图标；猫窗口由独立
  overlay 管理，不计入 GPUI 的普通窗口列表。
- `system.show_dock_icon` 保留当前字段和默认值，仅决定普通窗口全部关闭后的 Dock 显示状态。
  UI 文案为「始终显示程序坞图标」；在设置窗口内关闭开关不切换到 `Accessory`，不影响窗口内容。
- 复用普通窗口打开入口与关闭回调计算模式，避免通过枚举原生窗口、重新显示或恢复焦点补救
  切换模式的副作用。打开失败恢复背景模式；配置提交失败恢复旧偏好。

### 后果

- 配置损坏的普通路径收敛为“备份优先，否则默认值”，没有额外窗口和用户操作。
- 产品/开发/smoke 的可见窗口一致，按钮差异不会再来自运行模式。
- 项目自有辅助功能代码、隐藏文案和直接 AccessKit 依赖被删除；`gpui-kit` 仍可透明使用其
  内部默认语义。
- 自动化测试继续覆盖可见功能、输入、配置和 smoke；不再以辅助树断言证明 UI 完成。

### 明确不做

- 不删除操作系统或 GPUI 的辅助能力；只是不把它列为 BongoCat 自有的 UI contract。
- 不删除主题、本地化、键盘导航、可见 focus 和 tooltip。
- 不因“以后可能需要”而提前保留恢复窗口、恢复状态、辅助桥、feature flag 或空模块。
- 以后不在设置 UI 的项目代码里新增无障碍树、原生桥接、隐藏文案或 action。

### ADR-0056 · SettingGroup variant 与上游 GPUI Kit 固定 revision

**状态：** 已接受（2026-09-23；2026-09-24、2026-09-25、2026-09-28、2026-10-06、2026-10-12 修订）

### 决策

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

### 影响

- 依赖来源最终从维护者 fork patch 切换为上游固定 commit，再切换为上游 crates.io 精确 pin；
  两者都不再维护或信任 fork 分支。
- `Root` layer API 迁移只删除重复挂载；窗口仍使用 `gpui_kit::component::Root`，dialog、
  sheet 与 notification 的 owner 仍为 GPUI Kit。
- `PopConfirm::arrow` 默认关闭以保持调用方显式选择；当前模型删除确认使用 `BottomRight` 向上显示并显式启用箭头。组件几何测试
  固定 arrow 会为 trigger 与 surface 预留额外间距。

### ADR-0058 · Platform-neutral input contract boundary

**状态：** —

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0004, ADR-0007, ADR-0044

### Decision

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

### Consequences

- platform input code no longer pulls Cubism, model, audio, or runtime worker
  dependencies;
- input transport and reducer tests are isolated from the runtime worker;
- the runtime remains the only owner of mutable pressed state and shutdown
  ordering;
- existing runtime re-exports may remain temporarily for source compatibility,
  but new consumers should depend on `bongocat-input` directly;
- moving a reducer or model projection out of runtime requires a separate
  decision and is not implied by this crate.

### ADR-0059 · UI protocol boundary

**状态：** —

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0035, ADR-0054

### Decision

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

### Consequences

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

### ADR-0066 · 设置侧边栏采用任务导向的信息架构

**状态：** 已接受（2026-09-25）

依赖：ADR-0002（GPUI 设置界面）、ADR-0019（GPUI Kit design system）、ADR-0047（模型库边界）、ADR-0052（快捷键门禁）、ADR-0054（visual-first 设置 UI）、ADR-0056（SettingGroup variant）

2026-09-25 修订：About 保持设置菜单中的最后一个普通页面，但按当前能力补齐软件信息、
项目/反馈链接和日志目录入口；不复用旧 Tauri 字段或旧配置。

### 决策

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

### 搜索

GPUI Kit 的搜索只匹配设置项标题、描述和显式关键词，不匹配页面或分组标题。每一组设置项
因此携带当前页面名和分组名的搜索关键词，保证可见层级本身可搜索。重新分类前的名称
（General、Interaction、Input、Application、模型管理等）继续作为只搜索、不显示的别名。

模型库是自定义元素，除页面关键词外还索引当前模型显示名；因此搜索一个模型名可以
打开模型库页面。非空搜索不会重新创建设置项、绕过 typed command 或暴露内部路径。

### 明确不做

- 不增加“常用设置”或“高级设置”页，也不复制其它页面的开关。
- 不创建第三级导航、页面内 Tabs 或新的导航框架。
- 不把模型行为快捷键复制到模型库页，不把鼠标穿透从模型窗口行为页移走。
- 不把主题、语言或日志静默移入其它分类。
- 不修改 config/schema、runtime command、默认值或持久化设置行为；About 新增的支持动作只调用
  已有平台能力或 settings service，不建立新的配置字段。
- 不 fork GPUI Kit，也不使用私有 renderer/component 接口。About 由上游 `Settings` 在同一
  可滚动菜单中按顺序渲染为最后一项；当前公开 API 不提供独立 sidebar footer，因此不额外
  复制一份导航实现来强行固定它。

### 后果

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

### ADR-0067 · 配置领域命名空间与模型完整身份

**状态：** —

- 状态：已接受
- 日期：2026-09-25
- 影响范围：`config.json` v1、settings protocol、Application/runtime shortcut 边界、schema/fixtures

### 决策

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

### 结果与边界

- Rust 类型、JSON Schema、默认值、fixtures、Application 映射、settings snapshot、platform
  shortcut target 和 runtime origin 投影必须使用同一组名称。
- 完整模型身份需要贯穿配置编译、平台注册和 Application dispatcher；runtime snapshot 公开
  当前模型的 storage origin，dispatcher 在触发行为前同时比较 id 与 origin。
- 配置损坏时仍按现有“最新有效备份 → 默认配置”规则处理。旧开发配置不会自动转换；结构不完整的
  文件按既有严格 v1 解析/恢复边界处理。
- `InstalledModel` 等技术类型不因本次用户-facing 命名调整而改名；只有配置和 settings-facing
  来源词汇使用 `imported` / `built_in`。

### ADR-0068 · 设置窗口销毁与进程内导航记忆

**状态：** 已接受（2026-09-25）

依赖：ADR-0002（GPUI 设置 UI）、ADR-0035（更新 worker 与更新窗口）

### 决策

#### 1. 设置和更新窗口关闭即销毁

- 设置窗口标题栏 close、设置窗口 toggle 和显式关闭路径统一销毁 GPUI 窗口；
- 更新窗口保持既有的关闭即销毁行为；
- 窗口销毁不停止 runtime、输入、音频、frame source、overlay 或更新 worker；
- 两次打开之间不保留窗口实例，下一次打开重新创建 GPUI window、根 `Entity` 和组件状态；
- Windows 任务栏按钮的期望可见性由 coordinator 保留，窗口不存在时先保存意图，下一次创建时应用，
  不因设置窗口已销毁而把合法的配置变更报告为失败。

#### 2. 进程内记住设置侧边栏页面

`ProductCoordinator` 持有一个 `SettingsNavigationMemory`。它是进程内、非持久化的单一 owner：

1. 设置页面通过 `SettingPage::title_suffix` 的公开渲染回调报告当前一级页面；
2. 报告只更新 `SettingsNavigationMemory`，不把导航选择写入配置文件或 runtime snapshot；
3. 新设置窗口把 memory 中的 page index 传给 `Settings::default_selected_index`；
4. 应用重启重新创建 coordinator，导航回到 Appearance。

这样既利用了 `gpui-kit` 现有的 sidebar 行为，又不复制或接管第三方 `SettingsState`。导航记忆只覆盖
一级页面；页面内的草稿、搜索词和未完成表单仍随窗口销毁，不跨窗口保存。

#### 3. Shutdown 与窗口回调

普通 close 的 UI 清理发生在窗口销毁前：取消快捷键捕获、清理模型拖放并请求 flush 待提交设置。
runtime、输入和后台 worker 的 shutdown 顺序不变。Windows GPUI 0.2.2 的最终 `WM_DESTROY` 兼容
退出路径仍只用于显式 Quit 后的产品 owner 关闭阶段；普通设置/更新窗口销毁必须由双平台原生 smoke
验证。

### 结果

- 不再长期保留不常打开的设置窗口资源；
- 更新窗口行为回到原有的即时销毁语义；
- 设置窗口仍能在同一进程内恢复上次一级侧边栏页面；
- 应用重启后不会恢复旧进程中的 UI 导航选择；
- 窗口级 GPUI state 不跨窗口保存，避免把草稿、搜索和异步操作意外带到下一次打开。

### ADR-0073 · 应用级快捷键收窄为五个 command

**状态：** 已接受（2026-09-27）

### 决策

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

### 后果

配置契约少三个 command，UI protocol 少三个 `SettingsApplicationShortcut` 取值，
`apply_application_shortcut` 少三条分支。仍持有这三个绑定的开发配置会整体回退到备份
或默认配置，这是 v1 直接变更的既定代价。

### ADR-0078 · macOS 输入监控引导采用 permission-flow（第二个厂商 FFI 例外）

**状态：** 已接受（2026-09-30）；上游 0.1.40 已接入产品，macOS 12 适配落在 fork 的 rev 上（上游 PR #2 未合并），locale 参数与实机验收未完成，见「未完成项」

补充：ADR-0005（Cubism Core FFI 边界）、ADR-0024（macOS TCC 能力边界）、ADR-0032（启动权限
提示边界）、ADR-0008（应用身份与存储环境）、ADR-0030（实现决策阶梯）

#### 上游事实（crates.io `permission-flow` 0.1.40，2026-05-03 发布）

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

### 决策

#### 1. 接受 Swift/AppKit 作为第二个厂商 FFI 例外

AGENTS §1 目前只允许「官方 Cubism Core 平台二进制」一个厂商 FFI 例外。本 ADR 明确追加第二个
例外，范围严格限定为 **macOS 输入监控权限引导的呈现**，与 ADR-0005 的 Cubism 边界并列：

- `permission_flow` 只出现在 `bongocat-platform` 的私有 `startup_permission` adapter 之后，
  它的类型不进入项目公共 API，也不进入 runtime、render、overlay、input、config、UI；
- 该例外不放宽「不引入 Tauri、WebView、Node.js、JavaScript 或第二套 UI framework」：Swift 面板
  是权限引导的专用表面，设置界面仍然只有 `gpui-kit`。

依赖只声明在 `bongocat-platform` 的 macOS target 下（`permission-flow = "=0.1.40"`），因此
Windows 构建不构建 `swift-rs`，也不受 Swift 工具链影响。

#### 2. 保留 ADR-0032 的提示壳，只替换 macOS 的引导动作

`NSAlert` 提示、文案 key、worker 生命周期都不动。`startup_permission` adapter 的 macOS
`request_permission_flow` 换成 guided flow；Windows 侧只多接受一个被忽略的 locale 参数。

#### 3. 每次进入授权流程前执行 `tccutil reset ListenEvent com.ayangweb.bongo-cat`

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

#### 4. 不使用 `AppPath::suggested_host_app()`

拖拽目标取自当前可执行文件所在的 `.app`（`enclosing_bundle`），没有 bundle 时取可执行文件本身。
Swift 侧会把非 `.app` 项过滤掉，因此开发二进制得到一个空的拖拽目标，面板仍显示面板引导。
契约测试禁止 macOS 模块出现 `suggested_host_app`。

#### 5. 语言对齐靠 `CFBundleLocalizations` + 本应用域的 `AppleLanguages`

上游 Rust 接口没有 locale 参数，所以按实测结论第 3 条走 macOS 自己的机制：

- `macos/Info.plist` 声明 `CFBundleLocalizations`，列出 Swift 面板实际提供 catalog 的语言
  （`ar`、`en`、`pt`、`zh-Hans`、`zh-Hant`）。缺了它，嵌套 bundle 永远回落到 development region。
- 进入流程前把产品语言写进本应用 CFPreferences 域的 `AppleLanguages`（`apply_flow_language`）。
  locale 由 `bongocat-app` 解析后经 `StartupPermissionPrompt::locale` 传入 adapter，与提示文案
  同源，不会各自漂移。
- 该偏好是持久的，写的是本应用自己的域，等价于「本应用的界面语言」，因此应用的其他 AppKit 表面
  也会跟随——这是语言设置本来就该有的行为。
- `vi-VN` 没有上游 catalog，回落英文，记录为已知缺口。

#### 6. 链接与打包的必做项（已实现）

- `crates/bongocat-app/build.rs` 的 `link_swift_runtime()` 在 macOS 目标上输出
  `-Wl,-rpath,/usr/lib/swift`；契约测试固定这一行。
- `bongocat-packaging` 在打包前定位 `permission-flow` 的 `OUT_DIR` 里的 Swift 资源包，映射进
  `Contents/Resources`，并在 `verify_app_bundle` 里断言它存在——丢了这个包会在用户手上崩溃，必须
  在打包阶段失败。
- 开发二进制没有打包步骤，adapter 在资源包不可解析时把它从 Cargo profile 目录复制到可执行文件
  旁（`ensure_resource_bundle_available`），并且**不写入 `.app`**（会破坏签名）。复制失败时不启动
  引导面板，退回 ADR-0032 的原有动作——Swift 面板在找不到资源包时会 abort 整个进程，崩溃不是可
  接受的降级方式。

#### 7. 依赖来源：临时固定在 fork 的 rev

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

#### 8. CI 产物的 SDK 必须匹配产物运行面向的 macOS 大版本（2026-10-03）

v2.1.0 CI 的 arm64 包在 macOS 27 上点权限引导的「打开系统设置」会 SIGTRAP（实测结论第 10 条），
根因是同一 Swift 面板在「macOS 26 SDK 编译、macOS 27 运行」的组合下触发 SwiftUI AG 断言。修复：
release workflow 的 Apple Silicon leg 从 `macos-latest`（当时是 macOS 26 / Xcode 26.x）换成
`xcode-27`（macOS 27 / Xcode 27.0 / SDK 27.0），让 CI 产物与实测不崩的本地产物用同一代 SDK。
Intel leg 保留 `macos-26-intel`：macOS 27 是 Apple silicon 专用（2026-09 公布的兼容列表不含 Intel
机型），Intel 机最高只到 macOS 26，SDK 26.x 产物与运行面匹配。两个 leg 的 Xcode 大版本随平台
能力各自取最新，不再人为对齐；「配置完全一致」的原始意图降级为「同为原生 runner」，见 ADR-0033
修订节。后续 macOS 大版本迭代时，两条 leg 的 Xcode/SDK 必须同步复核。

### 未完成项

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

### 替换边界

替换点是 `bongocat-platform` 的私有 adapter（macOS 的 `request_permission_flow`、主线程
controller 槽、资源包与语言处理）、`crates/bongocat-app/build.rs` 的 rpath、`macos/Info.plist`
的 `CFBundleLocalizations`，以及 `bongocat-packaging` 的资源映射与校验。升级 `permission-flow`
时必须复验：`MINIMUM_MACOS_VERSION`、`SwiftLinker` 的 `--arch` 是否改为跟随 Rust target、
`Bundle.module` 的查找路径、`PermissionFlowConfiguration` 的 locale 字段、上游 catalog 的语言
集合，以及 Swift 工具链最低版本（上游 `swift-tools-version` 为 6.0 / 6.1，不在
`rust-toolchain.toml` 覆盖范围内，CI 需显式提供）。上游合并发布后，替换点还包括依赖来源本身：
`Cargo.toml` 的 git rev 与 `deny.toml` 的 `allow-git` 条目。

### ADR-0089 · Windows window mode for capture

**状态：** —

### Decision

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

### ADR-0093 · 语言只保留中文与英文

**状态：** 已接受（2026-10-19）

### 决策

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

### 后果

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

### ADR-0099 · 移除 Windows 支持，仅保留 macOS

**状态：** 已定稿并落地（2026-10-29）

> **本 ADR 记录一次架构级收缩**：项目从「Windows + macOS 双平台」改为「macOS-only」。它既是决策记录，
> 也是本次移除范围内**惰性保留项**的权威清单——凡是代码里还留着 Windows 语义的东西，都在 §4 里
> 给出了保留理由，不留「为什么还在」的悬念。

### 1. 背景

项目从诞生起就是双平台目标（Windows 10 1903+ 与 macOS 12+），技术设计（ADR-003/0089 等）长期
并行维护两套后端：Windows 走 Win32 / D3D11 / DirectComposition / Raw Input / NSIS 安装器，
macOS 走 AppKit / Metal / NSPanel / CGEventTap / .app 打包。

本轮决策：**项目只做 macOS**。Windows 的维护成本（两套渲染后端、两套输入适配、两套安装器、
`#[cfg(target_os = "windows")]` 分散在 183 处）在只有 macOS 用户的实际情况下不再值得。
收缩释放的是**每一条 `cfg` 分支的持续认知成本**——双平台让每一处平台边界都必须同时想两遍。

### 2. 决策

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
5. **文档改写**：README 双语言、`AGENTS`、`technical-design`、`product-runtime`、
   `TODO`、`contract`、`input-semantics` 全 macOS-only；
   删 Windows Raw Input / Win32 / D3D11 / NSIS / WM_* / RegisterHotKey / DPI / window_mode / taskbar 等整节。
   `update-signing-guide` 头部已自注「保留作历史对照」，不改。
6. **配置与依赖**：根与 crates `Cargo.toml` 无 Windows feature；`.github` CI、`justfile` 无 Windows
   残留；无 `.nsi` / `.bat` / `.cmd` / `.ps1` / `.rc` 脚本。
7. **spike 清理**：删除 `spikes/input-windows/`、`spikes/overlay-windows/`（纯 Windows 试验项目）；
   `spikes/gpui-overlay-macos`、`spikes/gpui-settings`、`spikes/config-store` 的 Windows cfg 分支清零
   （含引用已删 `bongocat_overlay_windows_spike` 的死代码）。spike `Cargo.lock` 的 `windows-sys` 等
   为第三方 crate 平台条件传递依赖，保留。

### 3. 为什么只保留 macOS

- **单一后端**：渲染（Metal）、输入（CGEventTap）、窗口（NSPanel）、打包（.app）、安装（拖放拷贝）
  各剩一条路，每个平台边界只需想一遍。
- **释放双平台认知成本**：删掉的不是「一段可跳过的代码」，而是**每处 `cfg` 分支的持续核对义务**——
  `cargo check --target x86_64-pc-windows-msvc`、双后端渲染、双输入矩阵、双安装器不再需要。
- **与既有事实一致**：项目当前用户全部为 macOS；Windows 路径在近期版本已无验证资源。

### 4. 惰性保留项（兼容性强制，非疏漏）

| 保留项 | 位置 | 保留理由 |
| --- | --- | --- |
| `window_mode` config 字段 | `config_schema/overlay.rs` | `#[serde(deny_unknown_fields)]`：删除该字段会使含旧键的用户配置**反序列化失败**（配置损坏）。保留为惰性占位（默认 `false`，注释已标明 inert），macOS 不读取其语义 |
| `show_taskbar_icon` config 字段 | `config_schema/system.rs` | 同上，`deny_unknown_fields` 强制保留 |
| `show_taskbar_icon` 功能链 | `settings_commands.rs` / `worker.rs` / `capabilities.rs` / `snapshot.rs` / `ui-protocol` | config 字段保留 ⇒ 其读取/命令/快照/UI 契约一并保留以维持编译与契约完整性。macOS 上由 `UnavailableTaskbarIcon`（恒返回 `TaskbarIconUpdateFailed`）正确建模「无任务栏」，且 settings UI **不渲染**该开关（仅测试默认值 + 错误文案 key） |
| `mver` 的「Windows virtual key」 | `model-store/src/mver/vocabulary.rs` | 模型文件格式的**标识符方案**（非平台代码），删除会破坏既有模型文件的键名兼容；仅散文「live Windows input path」改为「live macOS input path」 |
| `product_windows.rs` 模块 | `bongocat-app/src/` | 模块名中的 `windows` 指**产品打开的窗口**（settings/update/pomodoro 窗口 + Dock 图标状态），非 Windows 平台。全文无 `cfg(windows)`、无 Win32 引用 |
| `updateWindows()` | `overlay/src/macos/renderer.rs` | AppKit 方法名，非 Windows 平台提及 |
| ADR-0010 / 0023 / 0089 | `docs/adr/` | ADR 历史按边界**保留可追溯**（正文含「Windows」字样是记录而非当前承诺） |
| `windows-window-mode-2026-10-08` | `docs/benchmark/` | benchmark 历史，保留 |
| ADR-013 / 043 / 023 / 035 / 034 | `docs/adr/` | 正文保留作历史记录，**标注「随 Windows 支持的移除（ADR-0099）…决策失效」** |

> 第 1、2 行的 `deny_unknown_fields` 是硬约束：**保留字段是兼容性的唯一正确解**，不是「留一手」。
> 若未来要彻底移除，需先给旧配置提供迁移路径（读取时剥除未知键或走 schema 迁移）。

### 5. 影响面

- `crates/bongocat-app`：删 `taskbar_settle` 模块及测试；main.rs / projection.rs / snapshot.rs /
  input_diagnostics 相关散文改 macOS-only；product_windows 保留。
- `crates/bongocat-platform`：`local_time.rs` 删 Windows `read_offset`；依赖无 Windows。
- `crates/bongocat-ui-protocol` / `bongocat-model-store`：散文与 doc 注释改 macOS-only；
  `mver` 格式标识符保留。
- `vendor/cubism`：删 Windows DLL/LIB（3 个二进制）。
- 文档：`README` / `README.zh-CN` / `AGENTS` / `technical-design` 全 macOS-only。
- 本 ADR：新增；ADR-013/023/034/035/043 标注失效。

### 6. 兼容性（AGENTS §3.1）

- **用户配置**：`window_mode` / `show_taskbar_icon` 字段保留（§4），旧配置读取不受影响；
  未删除任何 config 键，无迁移需求。
- **模型文件**：`mver` 键名方案未动，既有模型文件兼容。
- **构建**：`cargo check`（EXIT=0）与 `cargo check --tests`（EXIT=0）在删除后通过；
  仅剩 macOS 单一 target，无 `--target x86_64-pc-windows-msvc` 验证义务。

### 7. 采纳门槛

| 字段 | 内容 |
| --- | --- |
| `decision` | 项目收缩为 **macOS-only**：删除全部 Windows 平台代码 / CI / 打包 / 资源与文档散文（183 处 cfg 清零、taskbar_settle 模块、local_time Windows 块、3 个 Cubism 二进制）；**保留** ADR / benchmark 历史，以及 §4 的 `deny_unknown_fields` 强制惰性字段与 `mver` 格式标识符 |
| `evidence_level` | **L1** —— 删除由 `cargo check` 与 `cargo check --tests` 双绿（EXIT=0）背书；cfg 残留复核为 0 处 `target_os = "windows"`、0 处 `cfg(windows)` |
| `primary_bottleneck` | 不是技术问题，是**维护成本权衡**：双平台让每处平台边界必须同时想两遍；收缩是成本决策而非能力缺失 |
| `released_variables` | 本轮移除的是一整类平台（Windows）而非单变量：平台代码、CI、打包、资源、文档、cfg 分支。`held_constant`：macOS 行为（Metal / CGEventTap / NSPanel / .app / Dock 图标）一律不动 |
| `success_criteria` | 已达成：(a) crates 内 `target_os = "windows"` 与 `cfg(windows)` 均 0 处；(b) `cargo check` 与 `cargo check --tests` 均 EXIT=0；(c) vendor Cubism Windows 二进制已删且无构建引用；(d) README / AGENTS / technical-design 文档全 macOS-only；(e) `cargo clippy --all-targets` 与全量 `cargo test --all` 均全绿；(f) CI 验证的 `spikes/config-store` 经 `cargo clippy -D warnings` + `cargo test` 本地复现全绿 |
| `rollback_trigger` | 若日后需要重新支持 Windows，**回滚路径**：`git revert` 本提交（或恢复 §2 删除项）+ 撤销 §4 保留项标注即可，ADR-013/023/034/035/043 的「失效」标注回退为有效 |

### 8. 开放项

- **无**。本次为收缩决策，删除范围、保留项、兼容性均已在本 ADR 钉死；回滚路径见 §7 `rollback_trigger`。

---

# 二、构建 / 更新 / 交付

> 构建工具链、签名信任模型、更新流水线与发布交付。

### ADR-0010 · Windows CPU Targets

**状态：** 已被 ADR-0033 取代（2026-09-14，Windows ARM64 不再是产品目标，只剩 x64）

### Decision

Windows 产品目标只包括：

- `x86_64-pc-windows-msvc`。

`i686-pc-windows-msvc` 和 `aarch64-pc-windows-msvc` 都不属于 BongoCat
的构建、CI、安装包、更新或测试目标。历史 x86 安装包只作为行为与发布考古证据
保留，不形成兼容承诺。

Windows ARM64 曾作为产品目标保留；ADR-0033 已取消该目标。原因不是"R5 缺少
Core"这一次要阻碍，而是它使产品矩阵多出一个**无法端到端验证**的原生构建：
没有官方可授权的 desktop ARM64 Core，就没有真实的 ABI、模型和 renderer 证据链。
Windows 自身提供 x64 仿真，Windows on ARM 设备运行 x64 构建在正常情况下可用，
因此维持一个不可验证的原生 ARM64 目标只增加成本、CI 面积和发布风险。

### Consequences

- 新的 target matrix、binding fixture 和发布配置不得加入 `i686-pc-windows-msvc`
  或 `aarch64-pc-windows-msvc`。
- `deny.toml` 的 `[graph].targets`、`bongocat-update::UpdateTargetTriple` 与
  发布流水线必须只包含 `x86_64-pc-windows-msvc`；契约测试强制三者一致。
- Windows 平台依赖与 renderer 只在 x64 上验证。ARM64 上的实际体验通过 Windows
  的 x64 仿真覆盖，不再有独立的原生 ARM64 证据要求。

### ADR-0016 · Anonymous Diagnostics Export Contract

**状态：** 已接受（2026-09-01）

### 决策

- 通过 settings service 的 `ExportDiagnostics` 强类型 command 生成 format version 1 JSON。
- 导出目标固定为当前不可变构建环境的 `logs/diagnostics.json`，使用同目录原子写入；UI executor
  不执行文件 I/O。
- 文档只包含 runtime/input/configuration 的稳定 code、匿名计数、模型来源计数和
  settings/config revision。模型 ID、路径、原始按键或事件流、原始配置、时间戳和动态 I/O 文本
  永远不进入导出。
- 导出成功状态只回传格式版本和字节数，作为 UI snapshot 的非配置 revision；失败返回固定的
  `diagnostics_export_failed`，不暴露操作系统文本。
- Diagnostics 页面提供键盘和 AccessKit 可访问的导出操作；“无报告”与成功字节数属于显示状态，
  不影响配置 revision。
- 更新诊断以可选的 `update` 分栏加入导出，仅允许匿名稳定错误码和阶段计数
  （check/download/install 的 started/succeeded/failed）；未接入更新 worker 时该分栏为 `null`。
  provider 的错误码必须来自 `bongocat-update` 统一 catalog；未知值在 Application 边界被丢弃，
  计数仍保留。不导出 endpoint、URL、HTTP 状态正文、artifact 路径、版本或签名材料。
- 导出额外包含 app-owned 和 Cubism Core 两个日志 owner 各自的匿名聚合统计（written、dropped、
  rotated、pruned、bytes 和 retained_files），但不读取或复制任何日志正文、路径或 Core 消息。
- format version 1 是当前完整的匿名结构；消费者必须拒绝非 v1，不能猜测或忽略不兼容的
  导出格式。在首版发布前字段直接修改当前 v1，并同步此 ADR、测试与文档，不保留中间格式的兼容分支。
  首版发布后才为不兼容演进建立新的版本、schema、测试与 ADR。

### 后续边界

应用级日志 writer 的实现与有界清理已记录在 ADR-0017；跨域历史日志合并、预览器和更新系统的
诊断 manifest 仍需单独设计。
实现不得为了导出而放宽日志隐私、环境隔离或原子写入约束。

### ADR-0017 · Bounded Application Logging

**状态：** 已接受（2026-09-01）；2026-09-24 由 ADR-0064 修订文本格式、过滤与统一 retention

### 决策

- `bongocat-app` 持有 app-owned `ApplicationLogHandle`，业务调用只能提交闭合 event catalog 中的
  module、stable code、固定 message 和有界安全上下文。路径、原始按键、剪贴板、配置/模型正文、
  动态 OS 错误文本和密钥不进入日志协议。
- application 与 Cubism Core 使用 `bongocat-log` 的同一套 UTF-8 单行文本 writer、级别过滤、按 UTC
  日切换、单文件大小保护、目录总量/文件数预算与 retention；两类 stream 仍分离，便于支持人员区分
  产品事件和厂商 callback message。
- 文件名固定为 `application-YYYY-MM-DD.log` / `cubism-core-YYYY-MM-DD.log`；轮转段使用
  `.<generation>.log`。单个文件达到 1 MiB 后继续切分，两类日志合计最多 8 MiB/32 个文件，当前
  UTC 日活动文件始终保留，过期或超预算的最旧已轮转文件优先删除。
- 当前 v1 `logging.level` 接受 `error`、`warn`、`info`、`debug`、`trace`，默认 `info`；
  `logging.retention_days` 接受 `1..=30`，默认 `7`。设置先原子持久化，再立即更新共享 controller
  并收敛允许删除的旧文件；不提供 rotation mode、文件大小或目录容量配置。
- Cubism Core callback 只复制至多 512 bytes 到容量 128 的 non-blocking queue；专用 Rust worker
  才调用共享 writer。callback 不执行格式化或文件 I/O；callback-slot contention、queue full 和 stop
  后迟到记录只增加匿名 dropped 计数。shutdown 先注销 callback，再拒绝新记录、排空队列并 join
  worker。Core callback 没有可信 severity，原始 message 固定按 `debug` 过滤；实际 Core/renderer
  失败由 app owner 以稳定 `error`/`warn` event 记录。
- Application startup creates `application-running.marker` inside the environment's log directory.
  The v1 marker contains only a schema version and one fixed phase: `running`, `shutting_down`, or
  `panicked`; it is flushed before services start and is removed only after runtime/audio shutdown
  and the `shutdown_completed` event. The next startup classifies a leftover marker anonymously as
  forced/unknown, shutdown interrupted, or panic, then immediately overwrites it with a new
  `running` marker. This preserves diagnosis without recording payloads or paths and prevents a
  stale marker from causing a repeated recovery loop.
- Diagnostics export includes aggregate counts for the fixed application event codes and a saturated
  application-plus-Core retained-byte/file summary, alongside separate anonymous source counters, as
  part of its current format version 1. Consumers must reject non-v1 data rather than guessing. The
  export never embeds raw log records, panic payloads, paths, key values, or user file content.
- Diagnostics preview 另由 ADR-0027/0064 约束：只从严格命名且可解析的 application `.log` 来源重写
  闭合 code，不复制原日志行；Core 原始 message 永远不进入 preview。

### 后续边界

远程上传、Core message 导出和第三方稳定日志 schema 不在本 ADR 范围。`.log` 面向用户与支持人员，
需要机器处理时使用匿名 `diagnostics.json` 或严格重写后的 preview entry。

### ADR-0021 · Signed Update Manifest Trust Boundary

**状态：** 已被 ADR-0029 取代（2026-09-13）

> 后续修订（2026-09-23）：本文已由 ADR-0029、再由 ADR-0034 取代。当前更新目标为 Windows x64、macOS aarch64 与 macOS x64；Windows 原生 ARM64 已由 ADR-0033 退役。

### 决策

- 新增平台无关且禁止 `unsafe` 的 `bongocat-update` crate。它只执行 manifest、签名、版本、target
  和 artifact 完整性验证，不联网、不写文件、不启动 installer，也不持有 runtime 或 UI 状态。
- manifest v1 使用严格 `snake_case` JSON 和 detached Ed25519 签名。客户端先对收到的原始 manifest
  bytes 执行严格验签，成功后才反序列化；未知字段、非 v1 schema、超过 1 MiB 或无效 JSON 均拒绝。
- manifest 固定包含 Development/Production channel、SemVer release、最低可升级版本、单调
  `release_sequence`、Unix 发布时间和 1 至 8 个 artifact。artifact 明确携带 target triple、arch、
  HTTPS URL、字节数和小写 SHA-256；只接受既定的四个 Windows/macOS target/arch 组合。
- verifier 由不可变构建环境、当前 target、当前版本、已安装 release sequence 和编译期信任公钥集
  构造。公钥以稳定 key ID、环境 channel 和 sequence 有效窗绑定；未知、跨环境、重复或超出轮换窗
  的 key 均拒绝。私钥不进入源码、构建产物或运行时配置。
- manifest sequence 低于已安装值，或新版本没有更高 sequence，视为降级攻击。当前版本低于
  manifest 的最低可升级版本时不尝试就地更新；相同或更旧版本只返回 up-to-date，不产生安装候选。
- 每个环境的最高已验证 sequence 由 `bongocat-update::UpdateSequenceStore` 从不可变
  `StorageLayout` 单独保存为 `updates/update-sequence.json` v1，不进入用户 config 或窗口 state 事务。
  状态固定包含 channel
  和最高 sequence；同目录锁串行化 writer，写入经同目录原子替换并回读验证。未知字段、非 v1、
  跨环境、符号链接、损坏或较低 sequence 一律失败关闭，绝不重置或覆盖已有状态。Unix 在创建或
  重开 `updates/` 时强制 `0700`；Windows 继续以用户 profile ACL 作为同等私有边界。
- 验证成功只返回字段私有、只提供只读 accessor 的项目自有不可变 `VerifiedUpdate`/`VerifiedArtifact`。
  第三方 URL、SemVer、签名或 digest 类型不进入公共 API。下载层必须把流交回 `VerifiedArtifact`
  同时验证精确长度和 SHA-256，通过前不得进入安装阶段。
- `UpdateVerificationSession` 将 verifier 与当前环境的 sequence store 成对创建：打开时读取最高
  sequence，验签和严格 manifest 验证成功后才记录新 sequence；`Available` 与 `UpToDate` 都属于
  成功验证，并立即更新 session 的 rollback 下限。验证失败不得创建或推进 state；该 session
  不联网、不下载或启动 installer。
- 公共 schema 与 accept/reject fixtures 位于 `shared/update/`。签名传输 envelope、endpoint、下载
  恢复、OS 包签名、installer 权限与回滚属于后续独立 contract。

### 依赖与替换边界

- `ed25519-dalek = 3.0.0`（BSD-3-Clause）提供纯 Rust Ed25519 严格验签；关闭默认 zeroize feature，
  只启用 verifier 所需 fast 表。维护仓库为 dalek-cryptography，Rust 1.85+。
- `sha2 = 0.11.0`、`semver = 1.0.28` 和 `url = 2.5.8` 均为当前稳定版，许可证为
  MIT OR Apache-2.0，支持项目 Rust 1.97。它们分别只位于 digest、版本和 HTTPS 解析边界。
- `bongocat-config` 是同一 workspace 的平台无关依赖，只提供不可变的 `StorageLayout` 与
  `BuildEnvironment`，用于把更新 sequence 固定在当前环境的 `updates/` 根；它不向 update
  verifier 暴露配置内容、平台 handle 或 UI 类型。
- 这些依赖均由 `bongocat-update` 封装；若停止维护，可替换实现而不改变 app/runtime/UI 公共协议。
  manifest 算法或字段变化仍需新 schema/ADR，不能在 v1 中静默替换。

### 后续边界

下一步建立环境绑定 endpoint 与 24 小时自动/手动检查调度，再实现有界下载、取消、临时文件清理、
平台 installer、操作系统签名验证和失败回滚。没有这些证据前不得声称更新功能或 stable 发布完成。

### ADR-0022 · Update Manifest Transport Envelope

**状态：** 已被 ADR-0029 取代（2026-09-13）

### 决策

- manifest endpoint 只能经 HTTPS `GET` 获取，客户端不跟随 redirect，只接受 `200` 响应。endpoint
  自身继续由不可变环境发布配置提供，不能来自用户 config、CLI 或运行时输入。
- 响应 body 是不经 JSON 解析、字符转换、解压或重编码的 manifest 原始 bytes，最大为 1 MiB。
- `bongocat-update-key-id` header 携带最多 64 字节的 ASCII key ID，只允许字母、数字、`-`、`_` 和 `.`。
  `bongocat-update-signature-ed25519` header 携带恰好 128 个小写十六进制字符，代表 64-byte detached
  Ed25519 signature。缺失、过长或语法无效的 header 以稳定 update error code 失败关闭。
- transport adapter 只把有限 header 和精确 body 转为 `UpdateManifestEnvelope`。它不验证 manifest、写入
  sequence state、下载 artifact 或调用 installer；envelope 只可交给 ADR-0021 的 verifier/session。
- HTTP、TLS、代理、状态文本和 response 类型不离开 transport adapter。自动/手动调度、取消、下载与
  安装另行实现。

### 后续边界

本 ADR 不选择 HTTP crate、Production endpoint 或公钥注入方式，也不授权下载、包签名、installer 权限或
回滚行为。

### ADR-0023 · Windows Per-User Installer

**状态：** 已接受（2026-09-05）；NSIS toolchain 与 wrapper 边界已由 ADR-0033 取代（2026-09-14）

> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

> 注（2026-09-14）：本 ADR 关于 **per-user 安装、权限面、卸载语义与数据隔离**的决策**仍然有效**。
> 以下内容已随 ADR-0033 作废，不要再据此实现：
> - 由本项目维护 NSIS 脚本、固定官方 `nsis-3.11-setup.exe` 的 MD5、以及通过
>   `BONGOCAT_NSIS_SETUP_PATH` / `BONGOCAT_MAKENSIS_PATH` 注入本机 NSIS 路径；
>   安装器改由 `cargo-packager` 的 NSIS 模板与 `install-mode` 配置生成。
> - "x86 不构建或发布，ARM64 在 Cubism desktop Core 门禁通过前不生成可发布 artifact"：
>   Windows 现在只有 x64（ADR-0010 已更新）。
> - "Windows packaging 仅接受已 Authenticode 签名的 payload"：新流水线在未配置签名
>   身份时会生成未签名安装器并输出警告，签名验证降级为 release workflow 的显式门禁。
> - "未来 Rust update helper 只接收 ADR-0021 验证完成的本 target/arch artifact"一句已随
>   ADR-0029 作废：不再有独立 Rust update helper，替换由 `self_update 1.3.0` 在进程内完成，
>   其权限边界、原子性与失败恢复未经本项目威胁建模。ADR-0021 已标记「已被 ADR-0029 取代」。

### 决策

- Windows 首发采用 per-user NSIS installer。每个受支持 target/arch 生成独立安装 artifact；
  当前只有 `x86_64-pc-windows-msvc`。
- 安装器以当前用户身份安装，不请求管理员权限，不安装 service、driver 或机器级注册项；
  卸载元数据写在 HKCU。
- installer 升级只在固定 product root 内替换旧的 product files，避免遗留 binary。
- 安装、升级和卸载不能读取、导入、迁移或删除 Development/Production 的 config、window state、models、
  backups、logs 或 updates 数据。默认卸载只删除 product files；删除用户数据必须是独立且明确的
  用户操作。
- installer 不联网、不解析 update manifest，也不自行选择 artifact。NSIS 只作为安装 packaging
  tool，不承载 BongoCat 业务逻辑。

### ADR-0025 · Signed Manifest HTTPS Transport

**状态：** 已被 ADR-0029 取代（2026-09-13）

### 决策

- `bongocat-update` 使用精确固定的 `ureq 3.4.0` 作为唯一 signed-manifest HTTPS transport。它只启用
  纯 Rust `rustls` feature，显式关闭默认 features，因而不启用 gzip、charset、cookie 或 proxy feature。
- `UreqUpdateManifestSource` 只接受 `UpdateManifestEndpoint`。endpoint 最多 2 KiB，必须是无 credentials、
  无 fragment 且具有 host 的 HTTPS URL；它来自后续不可变的构建发布配置，绝不来自用户 config、CLI 或
  运行时输入。
- 每次 fetch 只发送一次 GET，限制为 15 秒 global deadline，不跟随 redirect，且只接受 HTTP `200`。
  response body 以 `Read::take(1 MiB + 1)` 有界读取，不进行 JSON/字符集/压缩处理；超过上限和空 body
  均通过既有 `manifest_too_large` verifier code 失败关闭。
- transport 只构造 `UpdateManifestEnvelope`，由其继续校验 key-ID/signature header。endpoint、status、
  transport 和 body-read 错误只公开固定匿名 transport code；底层 URL、HTTP status text、TLS 或 client
  类型不离开 update crate。调用方必须在专用 update worker 运行 blocking fetch，禁止在 GPUI executor
  执行。
- `UpdateCheckCoordinator` 只能以 `UpdateManifestSource` 取得 envelope，再调用既有
  `UpdateVerificationSession`。其顺序固定为 fetch -> signature/schema/target verification -> sequence
  store commit；任何 fetch 或 verification failure 都不能改变当前环境的 anti-rollback state。
- 同一受限 agent 还提供 artifact source，但它只可打开 `VerifiedArtifact` 已验证的 URL，固定 HTTPS、
  无 redirect、`200`，并只返回 raw reader 或既有 `HttpStatus`/`Transport` retry 分类。下载 coordinator
  继续独占重试、取消与 staging，artifact verifier 继续独占长度和 hash 校验。

### 依赖与替换边界

`ureq 3.4.0` 为 MIT OR Apache-2.0、Rust 1.85+ 的公开维护 crate。它是纯 Rust blocking client；只在
`UreqUpdateManifestSource` 私有字段出现。若维护或安全边界不再满足要求，可替换该 source，不改变
`UpdateManifestEndpoint`、`UpdateManifestEnvelope`、verifier、runtime 或 UI contract。

### 后续

本 ADR 不选择 Production endpoint 或公钥，不实现 UI command、自动/手动 dispatch、installer 或 rollback。
它们仍需要各自的发布基础设施与安全验收。

### ADR-0026 · Update Helper Replacement Boundary

**状态：** 已被 ADR-0029 取代（2026-09-13）

### 威胁模型

- 不可信输入包括 update endpoint、manifest transport、artifact bytes、staging directory contents、
  product directory contents、环境变量、当前工作目录和用户可修改的 registry/launch arguments。
- 可信输入只包括编译进当前 app 的环境、target/arch、trust key、当前 installation root，和由
  VerifiedArtifact 只读 accessor 传递的 release identity、精确长度、digest 与 staging file。
  私钥、endpoint 配置和任意用户配置均不进入 helper。
- helper 与 app 同为当前用户权限；它不得请求 elevation、安装 service/driver、写入 HKLM、修改其他
  用户 profile，或以 Development/Production 用户数据目录作为 product replacement source/target。
- 被攻击者替换、删除或锁定的 app/helper/staging file 必须导致失败关闭和保留已知可运行 product，
  而不是重试到另一路径或启动未知 binary。

### 决策

- update helper 是独立、平台限定的 Rust owner，不属于 bongocat-update、runtime 或 GPUI。它只在
  app 完成 shutdown coordinator 的停止 frame tick、输入、runtime、config flush、audio/renderer/overlay
  release 后启动；app 必须等待 helper acknowledgement 或将本次更新报告为未安装。helper 不接受裸
  URL、manifest、target、版本、安装目录或 shell command-line replacement request。
- 调用方传入的 installation root 必须等于平台固定的 current-user product root。Windows 为
  $LOCALAPPDATA\Programs\BongoCat，macOS 为当前已签名 .app bundle；helper 在写入前重新解析
  root、拒绝 reparse point/symlink/非目录和任何与当前 target/arch 不一致的 payload。它绝不从
  StorageLayout 的 config、state、models、backups、logs 或 updates root 导出安装路径。
- helper 只接受同一环境 UpdateDownloadCoordinator 完成的唯一 staging file，重新检查 regular-file
  identity、所有权、无 link、精确 byte length 与 SHA-256，并执行对应 OS package signature/integrity
  verification。解包后的每个 product file 必须保持在 private same-volume work directory，拒绝 link、
  absolute path、path traversal 和已有 uninstaller；不允许把 archive entry 直接写入 product root。
- 替换采用 prepare -> validate -> commit：先在 product root 同卷创建全新的 private candidate directory，
  验证 candidate 的 target/arch、required resources、OS signature 与 executable launchability；再将
  current root 原子重命名为唯一 rollback directory，将 candidate 原子重命名为 current root。任何
  prepare/validate/commit failure 都不得删除 current root。commit 后 launch failure、health timeout 或
  explicit failure acknowledgement 必须在同一 helper invocation 内将 rollback directory 原子恢复；成功
  acknowledgement 后才删除 rollback directory。
- 同一 installation root 一次只允许一个 helper。lock/pid ownership 失效、父进程未退出、超时、unexpected
  child process 或 ambiguous recovery state 都停止替换并保留可诊断的 anonymous failure code。下一次
  launch 仅可在验证 current/rollback 两个固定 sibling 的签名与 target 后恢复其中一个；不得猜测、
  merge 或扫描其他目录。
- helper 不联网、不解析 JSON、不检查更新、不记录 URL、路径、按键、artifact 内容或签名材料。日志只记录
  stable replacement phase/error code、environment、target 和 release sequence；其保留策略服从应用
  diagnostics contract。Windows NSIS installer 继续只用于初始/显式 per-user packaging，不成为
  background update executor。

### 后续边界

本 ADR 不实现 helper、OS signature verification、archive format、health protocol、endpoint、公钥或
release hosting。每个实现需要独立平台代码、自动化与实机证据；在上述验证通过前，更新安装和 stable
发布仍不可宣称完成。

### ADR-0027 · Diagnostics Preview Bundle Boundary

**状态：** 已接受（2026-09-06）；2026-09-24 由 ADR-0064 修订 application event 输入与输出格式

> 后续修订（2026-09-23）：ADR-0054 后设置 UI 不再以 AccessKit 语义作为当前契约；本文的 “AccessKit 语义”验收应理解为当时的历史证据，当前改以可见文案和 typed result 为准。

### 决策

- 预览包是当前不可变环境 `logs/` 中的固定 `diagnostics-preview.zip`。它是 format version 1 的
  ZIP container，内容路径固定为 `manifest.json`、`diagnostics.json` 和
  `application-events.log`；archive entry 不携带宿主绝对路径、环境根、时间戳、模型 ID 或任意
  user-provided name。
- package writer 只枚举严格的 `application-YYYY-MM-DD.log` 与
  `application-YYYY-MM-DD.<generation>.log` 普通文件，并使用 `bongocat-log` 的共享严格文本 parser
  验证固定宽度 UTC timestamp、level、module、stable code、message 与有界 context 语法。历史 JSONL
  与 `cubism-core-*.log` 不是 application event 来源，也不会触发兼容 reader。
- 每个 code 必须来自 app-owned 闭合 catalog，module 必须与 code 的固定 component 相等，level
  必须与 code 的固定 severity 相等。任一记录未知、损坏、UTF-8/大小/普通文件检查失败时，整份来源
  文件跳过并增加匿名 `skipped_source_files`；不从部分文件复制记录。目录本身无法枚举则整个导出
  失败关闭，不伪装成空 preview。
- 重写后的每条 event 固定为 `<LEVEL padded> [<module>] <code>\n`。输出不包含源 timestamp、固定或
  动态 message、context、原始行、来源路径或模型身份；因此 `application-events.log` 既保留旧版
  component/level/code 的可读性，又不会把日志正文或用户邻近数据复制进 support bundle。
- Cubism Core 原始 `.log` 文件和 Core message 永远不进入预览包。Core 的 retained-file/byte 与
  written/dropped/rotated/pruned 聚合值继续只由 ADR-0016 的 `diagnostics.json` 表示。未来若要导出
  Core 历史内容，必须先定义可证明的固定结构和新的 ADR，不能复用路径关键词清洗作为授权。
- ZIP 在当前环境 `logs/` 下以私有同目录 staging file 创建，完成所有 entry 的大小、数量和结构验证
  后 `flush`/`sync_all` 并原子替换目标。取消、解析、写入、同步或替换失败必须删除本次 staging，
  保留上一份有效 preview；Unix 目录/文件分别恢复 `0700`/`0600`，Windows 使用当前 profile ACL。
- package format 继续是首版 v1；在首版发布前字段可直接同步调整 manifest、writer、
  fixture 和测试，不添加任何兼容 reader 或 migration。首版发布后才为不兼容演进建立新的 versioned
  format 与 ADR。
- UI executor 只调用 typed export command。UI 显示 stable result code、format version、entry count
  和 bytes，不直接枚举日志或解压 archive；失败不得显示 OS text、路径、archive entry 或日志正文。

### 后续边界

本 ADR 不实现 remote upload、Core history export 或 support endpoint。具体 archive writer lifetime
必须在实现提交中记录；在完整验证前，Phase 7 日志导出门禁保持未勾选。

首个 writer 使用 `zip = 9.0.2`（MIT，`zip-rs/zip2`，Rust 1.88+），精确 pin 且关闭默认 feature，
只写标准 `Stored` entries；它不提供 encryption、compression 或 archive extraction。替换边界是同样
能在双平台写出并验证此固定 v1 ZIP entry 集合的维护中 Rust crate，不能改变本 ADR 的隐私/原子性约束。

### ADR-0029 · Third-Party Update Library Boundary

**状态：** 已被 ADR-0034 取代（2026-09-14）

取代：ADR-0021、ADR-0022、ADR-0025、ADR-0026

> 后续修订（2026-09-23）：ADR-0033/ADR-0034 已把历史“四个 target”收口为 Windows x64、macOS aarch64 与 macOS x64；本文的 `self_update`、zipsign、四目标和相应验证记录只作历史。

> **本文已失效，正文保留为历史记录。**
>
> 2026-09-14 的换实现表明，本文记录的 `self_update 1.3.0` + zipsign 归档内嵌签名模型有两条
> 硬性阻塞：zipsign 只能签 `.zip` 与 `.tar.gz`，裸 `.exe` / `.dmg` 必然验签失败；且
> `self_update` 的 replace-and-verify 语义不适用于 NSIS 这类系统安装器（上游 `lib.rs:302`）。
> 更新库已改为 `cargo-packager-updater`，信任模型已改为 detached minisign 签名，见
> **ADR-0034**。本文中的库选择、签名方案、依赖清单与验证证据**均不再有效**。
>
> 仍然有效的部分——构建期 channel 隔离、签名密钥缺失时失败关闭、稳定错误码收敛、
> 第三方类型不外泄为项目公共 API——已在 ADR-0034 中重述。
>
> 文末引用的 `archive_layout_capability.rs`、`local_install_rehearsal.rs` 与
> `multi_file_install_capability.rs` 已随该库退役删除，替代品是
> `release_manifest_capability.rs`。

### 决策

- 删除 `bongocat-update` 的全部现有实现（9 个模块、4615 行），改为基于 `self_update 1.3.0`
  的薄封装。保留 crate 名称与诊断契约，其余全部替换。
- 采用 `self_update` 的**完整信任模型**：`zipsign` ed25519 归档签名。不再有 detached 清单签名、
  不再有先验签后解析、不再有 manifest v1 schema 与 1 MiB 上限。
- 采用内置 **`github` 后端**，仓库 `ayangweb/BongoCat`。发行资产须按目标 triple 命名以供匹配。
- 传输使用 `self_update` 的 **`ureq` feature**（而非 `reqwest`）。理由：项目本就固定 `ureq 3.4.0`，
  该选择避免引入 `reqwest` / `hyper` / `h2` / `tower-http` / `cookie_store` /
  `rustls-platform-verifier` / `aws-lc-rs` / `cmake` 整套依赖。实测真实编译图增量为
  **+17 个包**（462 → 479），`reqwest` 方案为 +31。
- 启用 features：`ureq`、`rustls`、`github`、`archive-tar`、`archive-zip`、
  `compression-tar-gz`、`compression-zip-deflate`、`checksums`、`signatures`。
  不启用 `progress-bar`（GPUI 应用无需终端进度条）。
- **channel 隔离保留**：`AGENTS` §10 要求更新 channel 按环境隔离，这是本 ADR 之外的约束。
  Development 构建的 `ReleaseChannel` 不允许联网或安装，`check()` / `install()` 在发出任何请求前
  即失败关闭。
- **签名密钥缺失时失败关闭**：由于上游空公钥集是 fail-open，`RELEASE_SIGNING_KEY` 为 `None` 时
  runtime 拒绝安装并返回 `update_signature_key_missing`。这是本项目对该 fail-open 的补偿。
- **稳定错误码保留**：原 32 个错误码收敛为 13 个，覆盖新实现的实际失败面。其中
  `update_download_transport_failed` 沿用原名，因为它是既有诊断导出契约的一部分
  （`ADR-0016` / `ADR-0027`）且被测试固定。
- 第三方错误、配置与平台类型不得扩散为项目公共 API。`self_update::Error` 在本 crate 边界内
  被映射为自有稳定码；`UpdateDiagnostics` 不含任何库类型。
- 依赖按 `AGENTS` §9 精确 pin（`=1.3.0`）并提交 `Cargo.lock`。
- 随本 ADR 一并移除因验证层退役而失效的 workspace 依赖：`ed25519-dalek`、`ureq`、`sha2`、`semver`。

### 被取代的 ADR 与其损失

以下能力随 ADR-0021 / ADR-0022 / ADR-0025 / ADR-0026 一并退役，`self_update` **没有**对应实现。
记录在此以免日后被误认为仍然有效：

| 退役能力 | 原依据 | 现状 |
| --- | --- | --- |
| detached Ed25519 清单签名、先验签后解析 | ADR-0021 | 由 zipsign 归档签名替代，信任模型不同 |
| 单调 `release_sequence` 防降级 | ADR-0021 | **无替代**。现仅按 semver 比较，降级攻击不再被检测 |
| manifest 层 Development/Production channel 绑定 | ADR-0021 | 改为构建期 `ReleaseChannel` 门禁，弱于原方案 |
| manifest v1 严格 schema、未知字段拒绝、1 MiB 上限 | ADR-0021 | **无替代** |
| 公钥轮换窗（key ID + channel + sequence 有效期） | ADR-0021 | **无替代**。zipsign 为 any-of 多密钥语义 |
| 4 个 target/arch 白名单在清单层强制 | ADR-0021 | 改为资产名匹配 + 构建期 target 常量 |
| 32 个稳定错误码作为公共契约 | ADR-0021 | 收敛为 13 个，语义不同 |
| `https_only`、禁止 redirect、15s 截止、仅接受 200、禁止透明压缩 | ADR-0025 | **无替代**。改由库的传输策略决定 |
| 独立且承担全部校验的 update helper | ADR-0026 | 由库内部编排替代 |
| `updates/` 目录 0700 与原子写入 | ADR-0021 | 随 sequence store 一并退役 |

`AGENTS` §10「更新只允许 HTTPS，校验版本、target、arch、hash 和签名，并提供失败回滚」
仍然成立：库分别以 HTTPS-only 传输、资产 target 匹配、构建期 target 常量、`checksums`
feature 与 `signatures` feature、install 阶段的 stash 回滚覆盖这些点。

### 后续边界

本 ADR 不实现 endpoint、公钥注入、update worker、更新 UI、OS 包签名验证或启动恢复。
在待验证项 1 与 2 完成前，不得声称更新功能或 stable 发布完成。

### ADR-0033 · Build, Packaging and Release Toolchain

**状态：** 已接受（2026-09-14）

取代：ADR-0010 的 Windows target 集合、ADR-0023 的 NSIS toolchain 与 wrapper 边界

#### 发布目标

- `x86_64-pc-windows-msvc` → NSIS 安装器（`.exe`）
- `x86_64-apple-darwin` → `.app` + `.dmg`
- `aarch64-apple-darwin` → `.app` + `.dmg`

Windows ARM64 **不再是产品目标**。Cubism Native R5 没有 desktop ARM64 Core，而
Windows 自身的 x64 仿真可以运行 x64 应用，因此维护一个无法端到端验证的原生
ARM64 目标只增加成本和发布风险。ADR-0010 的 ARM64 结论作废。

### 决策

#### 1. `cargo-packager` 作为唯一打包工具，并以库形式集成

采用 `cargo-packager =0.11.8`（`default-features = false`，仅启用 `rustls-tls`，
Apache-2.0 OR MIT），由 workspace 成员 `crates/bongocat-packaging` 以**库**方式调用
`cargo_packager::package()`。

- 打包配置是仓库内的 Rust 源码，编译期检查，不存在弱类型配置文件。
- 版本由 `Cargo.lock` 精确固定，不要求任何开发者 `cargo install` 全局工具。
- 本地与 CI 执行同一个 crate，接口天然一致。
- 打包工作流完全归 `cargo-packager` 所有：bundle 目录布局、`Info.plist` 生成、
  资源复制、图标转换（`.icns` / `.ico`）、NSIS payload 组装与 installer 生成。

`cargo-packager` 的关键能力与 BongoCat 的对应关系：

| 需求 | `cargo-packager` 能力 |
| --- | --- |
| Windows x64 安装器 | `nsis`，`install_mode = currentUser` 是默认值 |
| macOS `.app` | `app`，自动生成 `Info.plist` 并合并仓库 overlay |
| macOS `.dmg` | `dmg`（本项目按下方第 4 条处理） |
| 桌面 GUI 应用 | 本身就是 Tauri/Wry/Dioxus/egui 等桌面框架使用的打包器 |
| 资源与图标 | `resources`（支持 `src`/`target` 映射）、`icons` |
| 签名 | macOS `signing_identity`、公证凭据；Windows `signtool` / 自定义签名命令 |
| Cargo workspace | 直接消费 cargo target 目录与二进制名 |

#### 2. 删除自研构建与打包脚本

- 删除 `scripts/`（4 个文件，约 14 KB shell / PowerShell）。
- 删除 `windows/installer/BongoCat.nsi`（由 `cargo-packager` 的模板与
  `install-mode` 配置取代）。
- `justfile` 只保留统一入口，不含平台判断、复制、打包或版本处理逻辑：
  `just build [--target ...] [--environment ...] [--formats ...]`。
- `BONGOCAT_NSIS_SETUP_PATH` / `BONGOCAT_MAKENSIS_PATH` 与 NSIS 3.11 的 MD5 固定
  要求一并作废：NSIS toolchain 由 `cargo-packager` 自动获取并缓存。

#### 3. 版本号唯一来源

`[workspace.package].version` 是唯一来源。`bongocat-packaging` 继承该版本，
因此 Cargo 在打包进程启动前就解析完成；`--print-version` 把同一个值暴露给发布流水线，
release workflow 用它校验 tag 与版本一致，不再出现"脚本一个版本、CI 一个版本"的局面。

#### 4. `.dmg` 由操作系统磁盘映像工具产出（有明确退出条件的偏离）

> 2026-09-29 补充：Finder 窗口布局与卷图标由 ADR-0075 决定，本条只负责"镜像由
> 操作系统磁盘映像工具产出"这件事本身。

`cargo-packager` 0.11.8 的 DMG 实现依赖 `create-dmg` 在 2022 年的固定提交
`28867ba`，该脚本在当前 macOS 上**无法完成 DMG 构建**。它用

```text
hdiutil attach ... | grep -E '^/dev/' | sed 1q | awk '{print $1}'
```

解析挂载结果，`sed 1q` 在读到第一行后关闭管道，`hdiutil` 收到 SIGPIPE 后挂载事务
没有完成，脚本随后的 `hdiutil detach <device>` 报
`detach failed - No such file or directory`。

本机实测（macOS 26.5，使用与 `cargo-packager` 完全相同的参数调用该脚本）：

- 原样调用 3/3 失败；
- 把 `hdiutil attach` 的完整输出读完再解析 3/3 成功。

`create-dmg` 的 URL 与 revision 是 `cargo-packager` 的编译期常量，配置无法覆盖；
其 `main` 分支仍然固定同一个 revision，因此没有"换一个受支持的版本"这条路。

最终做法：`.app` 仍然由 `cargo-packager` 产出（bundle 布局、`Info.plist`、资源、
图标、签名全部不变），随后由 `bongocat-packaging` 用 macOS 自带工具把它封装成
DMG：`ditto` 暂存 bundle、加入 `/Applications` 拖放入口、`hdiutil create -format ULMO`
压缩、`codesign` 签名。这恰好是 `create-dmg` 自己封装的东西，只少了它的脚本层。

DMG 压缩格式选 `ULMO`（LZFSE）而不是默认的 `UDZO`（zlib）。`UDZO`/`UDBZ` 按固定
64 KB 分块压缩，看不到 bundle 里重复的预置模型资源；`ULMO` 对整条流做压缩。同一个
2.0.0 arm64 bundle 实测，每种格式都实际挂载并整读校验（可执行文件与三个预置模型
齐全）：`UDZO` 13,795,779 B、`UDBZ` 13,462,431 B、`ULFO` 13,082,584 B、
`ULMO` 10,380,216 B，比 `UDZO` 小 24.8%。挂载并整读镜像平均 0.68 s，`UDZO` 为
0.92 s，更小的镜像同时装得更快；`UDBZ` 读得更快（0.45 s）但大 23%。
`ULMO` 是 Apple 的只读压缩格式，支持版本远早于产品声明的 macOS 12 下限。

**退出条件**：`cargo-packager` 换用可在当前 macOS 工作的 `create-dmg` revision 后，
改回 `PackageFormat::Dmg` 并删除这段代码（窗口布局与卷图标的部分由 ADR-0075 补充，同样
在这条退出条件下一并删除）。

#### 5. Release workflow 由项目自己描述，原生 runner 构建

新增 `.github/workflows/release.yml`，在 tag 推送时于原生 runner 上运行同一套
`just build`：

- `windows-latest` → `just build`（NSIS 安装器）
- `macos-latest` → `just build --target aarch64-apple-darwin`（`.app` + `.dmg`）
- `macos-26-intel` → `just build --target x86_64-apple-darwin`（`.app` + `.dmg`）

两个架构都用 `--target` 显式指定，因此产物集合不依赖 runner 恰好是哪一种架构。runner 则
按目标架构分开，修订见文末「macOS 两条 leg 分 runner 构建（2026-09-30，ADR-0078）」。

### 影响

- 打包行为的一部分由第三方工具决定，项目不再逐行控制 `Info.plist` 与安装器脚本。
  对应地，仓库保留 `macos/Info.plist` 作为**overlay**，只声明 `cargo-packager`
  不生成的键（`LSMultipleInstancesProhibited`、`NSPrincipalClass`）；其余键由工具生成，
  避免两处定义同一个值。
- Windows 安装目录由 `$LOCALAPPDATA\Programs\BongoCat` 变为 `$LOCALAPPDATA\BongoCat`
  （`cargo-packager` 的 currentUser 布局），注册表仍写在 HKCU。首个正式版本发布前，
  不存在需要迁移的已安装实例。
- `cargo-packager` 硬编码 Windows 安装器文件名为
  `{主二进制名}_{version}_{arch}-setup.exe`，且没有可配置项。发布资产名是产品决策，
  因此 `bongocat-packaging` 在打包完成后把已产出的安装器重命名为
  `BongoCat_<version>_x64.exe`（`<version>` 取自 `CARGO_PKG_VERSION`，架构取自
  `ReleaseTarget::architecture`，两者都随构建参数自动解析）。重命名只作用于文件本身，
  安装器内容、`Info.plist` 与 bundle 布局仍完全由 `cargo-packager` 拥有。
- 不再有 NSIS 许可证页面与 DMG EULA 页面（未配置 `license-file`）。
- **本 ADR 接受时没有**产出可更新资产：当时的 `self_update` 方案需要按 target triple 命名、
  macOS 根为 `BongoCat.app/`、Windows 根为 `bongocat-app.exe` 的归档，且
  `RELEASE_SIGNING_KEY` 仍为 `None`（fail-closed）。这是本 ADR 的历史状态，不是当前实现。
  更新方案随后由 ADR-0034 取代为 `cargo-packager-updater` + detached minisign，并由
  `crates/bongocat-packaging` 产出、签名更新载荷；真实发布链路仍是未完成的发布门禁。

### 已接受的残余风险

1. **`cargo-packager` 的 NSIS plugin 获取**：`nsis-3.09.zip` 与 `nsis_tauri_utils.dll`
   有 SHA-1 校验，但 `NSIS-ApplicationID.zip` 只下载、不校验，且来源是
   `tauri-apps/binary-releases` 而不是官方 SourceForge。ADR-0023 原先要求的"官方
   acquisition artifact + MD5 + `makensis /VERSION`"边界被替换成对
   `cargo-packager` 自身供应链的信任。这是明确接受的降级，不能声称等价。
2. **DMG 外观布局**：`cargo-packager` 的 `create-dmg` 路径会在挂载后调用 Finder
   AppleScript 排列图标，需要一次"自动化控制 Finder"授权；`CI=true` 时它会传
   `--skip-jenkins` 跳过。本项目已不经过该路径，因此不受影响；如将来恢复该路径，
   必须重新评估无人值守环境。本项目自己的窗口布局同样不经过 Finder（ADR-0075
   决策第 1 条：构建期直接写 `.DS_Store`），所以这条风险对当前实现不适用。
3. **`x86_64-apple-darwin` 产物在 CI 中不被原生执行**：CI 交叉编译并校验两种架构的
   bundle 结构，但只在宿主架构上运行应用。原生 Intel 机器上的运行验证仍是独立门禁。
4. **Windows 端到端未在本机验证**：Windows 安装器只能在 Windows runner 上验证，
   本次没有在本机执行过 Windows 构建。

### 后续

1. ~~为更新流程产出按 target triple 命名的归档并接入 `zipsign`/`RELEASE_SIGNING_KEY`。~~
   已由 ADR-0034 取代：更新载荷改为共享 manifest 指向的
   `BongoCat-<version>-<arch>.app.tar.gz`（`aarch64` / `x64`）与 NSIS 安装器，并用 detached
   minisign 签名；`RELEASE_SIGNING_KEY` 已内嵌公钥。真实 GitHub 发布与安装链路仍需端到端验证。
2. `cargo-packager` 修复 DMG 后删除本项目第 4 条的偏离。
3. 原生 Intel 机器与干净 Windows 10 1903+ / Windows 11 profile 上的安装、升级、
   卸载与回滚验证。
4. 上报 `create-dmg` / `cargo-packager` 的 `hdiutil` 解析缺陷。

### macOS 两条 leg 分 runner 构建（2026-09-30，ADR-0078）

本 ADR 决策第 5 条原本让两种 macOS 架构在同一个 `macos-latest` 上构建，理由是 SDK、工具链与
配置完全一致。ADR-0078 接入 macOS 输入监控引导面板后这个前提不再成立，而那一段本身要按本节
修订：

- 引导面板链接一个 Swift 静态库，产出它的 build script（`swift-rs`）取的是**宿主架构**而不是
  Rust 目标架构，并且把 macOS 永远当成非交叉编译。因此在 arm64 runner 上构建
  `x86_64-apple-darwin`，链接进二进制的是 arm64 目标文件，链接失败在
  `_permission_flow_*` 未定义符号上；把 runner 换成 x64 则同一问题出现在
  `aarch64-apple-darwin` 那条 leg 上。
- 因此两条 leg 各用一台与目标同架构的 runner：Apple Silicon 用 `macos-latest`
  （`macos-26-arm64`），Intel 用 `macos-26-intel`（同为 macOS 26 的 x64 镜像）。两者
  macOS 版本相同，Xcode 与 Swift 工具链仍对齐，「配置完全一致」的原意得以保留。
- `rustup target add ${{ matrix.triple }}` 保留：它针对的是 `rust-toolchain.toml` 固定的
  工具链，而不是交叉编译——两条 leg 现在构建的都是各自 runner 的宿主 target。
- 代价：本机在 Apple Silicon 上 `just build --target x86_64-apple-darwin` 仍然打不出来，
  Intel 产物只能由 release workflow 或一台 Intel 机器产出。要恢复「单 runner 出两个包」，
  需要 `swift-rs` 让 `--arch` 跟随 Rust 目标（上游仓库 `Brendonovich/swift-rs`），见
  ADR-0078 的「未完成项」。
- 本 ADR「验证与已知限制」第 3 条（`x86_64-apple-darwin` 产物在 CI 中不被原生执行）仍然成立：
  Intel 那条 leg 换成 x64 runner 后，CI 里的构建与结构校验都发生在目标架构上，但应用本身
  仍然只在本机（Apple Silicon）运行过。

补充（2026-10-03，ADR-0078）：上一节「两者 macOS 版本相同，Xcode 与 Swift 工具链仍对齐」的结论
现需修订。v2.1.0 实测显示，用 macOS 26 SDK 构建的 arm64 包在 macOS 27 上点权限引导的「打开系统
设置」会 SIGTRAP（SwiftUI AG 断言），同源码用 macOS 27 SDK 构建则正常。release workflow 因此改为：
Apple Silicon leg 用 `xcode-27`（macOS 27 / Xcode 27.0 / SDK 27.0），Intel leg 保持
`macos-26-intel`（macOS 27 是 Apple silicon 专用，Intel 机最高只到 macOS 26，SDK 26.x 与之匹配）。
两个 leg 的 Xcode 大版本不再人为一致；「配置完全一致」收窄为「同为与目标架构一致的原生 runner」。

### ADR-0034 · Detached Minisign Update Trust Model

**状态：** 已接受（2026-09-14；§3 的 manifest 形状于同日修订，见该节修订说明）

> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

取代：ADR-0029 的更新库与签名部分（正文保留为历史记录）

### 决策

#### 1. 以 `cargo-packager-updater` 取代 `self_update`，保留 `bongocat-update` 的对外契约

采用 `cargo-packager-updater =0.2.3`（`default-features = false`，仅 `rustls-tls`）。
`bongocat-update` 的公开面不变：`ReleaseConfiguration`、`ReleaseChannel`、
`UpdateTargetTriple`、`UpdateRuntime::{check, install, restart}`、`UpdateError`、
`UpdateOutcome`、14 个稳定错误码与 10 项匿名计数。（错误码目录于 2026-09-15 增至 14 个：`ReleaseFetchFailed` 原本兼任「取不到发布信息」与「取到了但读不懂」，ADR-0035 把这两件事拆成两个码。新增码向后兼容，见该 ADR 的「真实端点首次运行的结果」。）

库的 `Error`（`#[non_exhaustive]`）、`Config`、`semver::Version` 与 `Url` 全部映射为项目
自有类型，不出现在 `diagnostics.rs` 或 app/UI 协议中；未识别的库变体降级为
`update_internal_failed`，不把库文本带进诊断导出。

#### 2. 信任模型：detached minisign 签名，密钥缺失时失败关闭

`RELEASE_SIGNING_KEY: Option<&str>` 存放 **base64 的 minisign 公钥盒文本**（即
`cargo packager signer generate-key` 写出的 `.pub` 文件内容）。仓库现已内嵌一把发布公钥
（key ID `DF5E2C9D255DD85E`），Production 构建的 `UpdateRuntime::is_available()` 因而可以为真。
公钥为 `None`、空串或纯空白时，runtime 在**发出任何请求之前**返回
`update_signature_key_missing`；系统菜单不会在没有有效公钥时显示「检查更新」入口。

这条门禁是刻意的第二道防线。库对空 `pubkey` 的行为是解码失败（`Error::Base64` /
`Minisign`），即 fail-closed 而非 ADR-0029 记录的 `self_update` fail-open；但项目仍保留
自己的门禁，因为它产出的是稳定、无路径的错误码，而不是一个库错误。

#### 3. 一份共享 manifest，static 形状

runtime 请求 `releases/latest/download/latest.json`，manifest 是库的 **static** 形状：顶层
`version` 加一个 `platforms` 映射，键为 `<os>-<arch>`，每个条目含 `url`、`signature`、`format`。

`crates/bongocat-packaging` 仍然**每个 target 写一份 fragment**（`<os>-<arch>.json`），因为每个构建
job 只能声明自己产出的载荷；发布前由同一个工具用 `--merge-manifests` 把 fragment 合并成
`latest.json`。合并放在打包工具而不是 CI 里，理由与打包本身一致：manifest 的形状与资产名必须由
一处拥有，工作流只负责调用工具。

平台键拼写必须与库的 `get_updater_target()` / `get_updater_arch()` 一致——是 `macos` 而非
`darwin`，是 `aarch64` 而非 `arm64`。fragment 的文件名就是该键，合并会校验它属于已发布 target，
拼错的文件名会让发布失败而不是产出一个没有 host 能命中的条目。

> **修订（2026-09-14，同日）：** 本节原决定是"每个 target 一份 manifest，dynamic 形状"，理由是
> 避免跨 job 合并。产品要求把检查地址固定为 `.../releases/latest/download/latest.json`，而单一
> 文件名下三个 job 会互相覆盖、最终所有平台拿到同一个载荷，因此必须改为共享 manifest + 合并。
> 平台门禁随之回到 manifest 内：`UpdateErrorCode::NoMatchingAsset` 重新变为可达（发布漏掉某个
> 平台键时会命中它）。fragment 形状不变，只是从"被 runtime 读取的 manifest"降级为"合并的输入"。

#### 4. 更新载荷：macOS 用 bundle 归档，Windows 复用已发布的安装器

| target | 更新载荷 | 签名 |
| --- | --- | --- |
| `aarch64-apple-darwin` / `x86_64-apple-darwin` | `BongoCat-<version>-<arch>.app.tar.gz`（`aarch64` / `x64`），归档根为 `BongoCat.app/` | `.app.tar.gz.sig` |
| `x86_64-pc-windows-msvc` | 已发布的 NSIS 安装器 `BongoCat_<version>_x64.exe` | `.exe.sig` |

macOS 的归档由 `crates/bongocat-packaging` 用 `tar` + `flate2` 从已完成的 `.app` 生成，
`follow_symlinks(false)` 保留 bundle 内部链接。库在 `UpdateFormat::App` 下丢弃归档的根条目
再把其余内容装到 bundle 路径，所以归档根**必须**是 `BongoCat.app/`。

- 补充（2026-09-28）：载荷名里的 target triple 去掉了。macOS 载荷曾经叫
  `BongoCat-<version>-aarch64-apple-darwin.app.tar.gz`，现在叫
  `BongoCat-<version>-aarch64.app.tar.gz`（Intel 侧是 `x64`）。去掉平台后缀不削弱本 ADR 的
  任何一条结论：载荷由 manifest 的 URL 指定，库不按资产名匹配，所以命名只需稳定且可读；
  而该文件只可能出现在它所属的平台上，重复平台名对读者没有信息量，扩展名也已经把它和 Windows
  安装器区分开。Homebrew tap 的 cask 从同一行 `url` 读名字，因此不需要单独同步。

`.dmg` **不是**更新载荷：它是人工安装路径（拖入 `/Applications`），库没有安装它的能力。

#### 5. 签名在 `crates/bongocat-packaging` 内完成，且是最后一步

签名密钥通过 `SIGNING_PRIVATE_KEY` / `SIGNING_PRIVATE_KEY_PASSWORD`
注入，与既有的 `BONGOCAT_MACOS_SIGNING_IDENTITY` 同一形状——**凭据注入点，不是隐藏构建步骤**。
未设置即跳过签名，本地构建因此只产出 bundle 与安装器，不产出更新资产。

密钥对由**同一个入口**一次性生成：`just keygen <file>`（`crates/bongocat-packaging` 的
`--generate-signing-key`）。它用同一份精确 pin 的 `cargo-packager` 调 `generate_key` /
`save_keypair`，因此不需要任何全局 `cargo install`；生成后显式把私钥收紧到 `0600`，并打印公钥
那一行供填入 `RELEASE_SIGNING_KEY`。生成是离线的运维动作，不在 CI 中执行。

release workflow 反过来**断言**发布构建一定签过名：未配置 secret 时 job 直接失败，
并在打包后逐项校验载荷、`.sig` 与 manifest 存在且形状正确。这消除了"发布一批无人能更新的
产物"这一类静默失败。

顺序上签名必须最后发生：minisign 签名覆盖发布时的确切字节，签名之后再改名、重压缩或
`strip` 都会让签名失效。manifest 的 URL 因此在签名之后才写入（载荷文件名已经确定）。

#### 6. 退役 `self_update` 专属的能力测试，改为钉住真实的签名/验签对

`archive_layout_capability.rs`、`local_install_rehearsal.rs` 与
`multi_file_install_capability.rs` 锁定的是 `self_update` 的 `Extract` / `MoveAll` /
`backends::custom` 原语，随该库一并退役。

替代品 `release_manifest_capability.rs` 用 loopback HTTP 服务把
**`cargo_packager::sign::sign_file`（生产端用的签名器）与 `cargo-packager-updater`（应用真正
运行的验签器）**放在一起跑，覆盖：共享 manifest 与它的 `<os>-<arch>` 平台键查找、被篡改的载荷、
由未知密钥签名的载荷、空公钥、以及（macOS）整包替换。

manifest 形状的跨 crate 一致性不靠正则匹配源码，而是由 `crates/bongocat-packaging` 的合并测试
把**自己写出的 `latest.json` 喂给更新库自己的读取类型**（`cargo-packager-updater` 作为 dev 依赖），
因此形状不匹配会在这里失败而不是在用户机器上。`tools/tests/test_update_release_contract.py`
覆盖另一半——资产名与平台键必须与 runtime 声明的一致。

#### 7. 更新源的 GitHub 代理回退（2026-09-15 增补）

国内直连 GitHub 不稳定，一次更新运行在请求共享 manifest 时按固定顺序逐个尝试代理源
（`<proxy>/https://github.com/.../releases/latest/download/latest.json`）：

1. `https://cdn.gh-proxy.org`
2. `https://v6.gh-proxy.org`
3. `https://axisnow.gh-proxy.org`
4. `https://v4.gh-proxy.org`
5. `https://gh-proxy.org`
6. GitHub 官方源（兜底，不加前缀）

一个源**可用**的判定与库对 endpoint 的判定一致：请求返回成功状态**且** body 能被更新库
作为本发布管线的 manifest 反序列化（§3 的 static 形状）。超时、网络错误、HTTP 错误或
内容不可解析都跳到下一个源；全部失败时报最后一个错误，错误码目录不变。
`NoMatchingAsset`（manifest 可读但不含本平台条目）不触发回退——所有源服务的是同一份
release 资产，换源不会改变答案，该诊断必须原样到达用户。

成功的代理贯穿本次更新：`cargo-packager-updater 0.2.3` 的 `Update.download_url` 是公开
字段，runtime 在下载前把 manifest 里的官方 GitHub URL 统一转换为
`<proxy>/<official-url>`；转换只作用于 scheme 为 HTTPS 且 host 为 `github.com` 的 URL，
因此已代理的 URL（host 是代理本身）不会被二次加前缀，非 GitHub URL 不被改动。官方源
成功的运行不做任何转换。`release_page_url`（人工查看 changelog 的页面）不属于传输路径，
不转换。

单次 manifest 请求有独立的 30 秒上界（`UPDATE_MANIFEST_REQUEST_TIMEOUT`），低于载荷
传输的 1800 秒上界（`UPDATE_REQUEST_TIMEOUT`）：源是串行尝试的，一个黑洞连接若占用
传输级超时会把后续每个源都堵死 30 分钟。成功的源确定后，超时在 `Update.timeout` 上恢复
为传输级上界再下载。

信任模型不受影响（§2）：代理只是 URL 前缀转发，能迟滞一次运行、返回过期或伪造的
manifest、把下载指去别处，但无法伪造 minisign 签名，任何被篡改的内容到不了安装步骤；
"旧但签名有效"的降级风险即 §待验证项 5 的既有风险，代理化不改变它。该回退由
`runtime.rs` 的单元测试钉住（代理列表的顺序与字面量、代理 endpoint 的精确形态、
逐源尝试/首个可用即停/官方兜底/全失败报最后错误、`NoMatchingAsset` 短路、下载 URL
转换的幂等性与范围、超时上界），Windows 与 macOS 走同一段平台无关代码。

### 被取代的能力与损失

ADR-0021 / ADR-0022 / ADR-0025 / ADR-0026 的退役能力沿用 ADR-0029 的记录（detached 清单
签名、manifest v1 严格 schema 与 1 MiB 上限、`https_only` / 禁止 redirect / 15s 截止 /
仅接受 200 / 禁止透明压缩、独立 update helper、`updates/` 目录 0700 与原子写入）。
相对 ADR-0029，本次换实现又新增了以下损失，记录在此以免日后被误认为仍然有效：

| 能力 | ADR-0029 时的状态 | 现状 |
| --- | --- | --- |
| 归档完整性校验（`checksums` feature，用 GitHub 公布的 per-asset sha256） | 存在，但上游明确它只是完整性检查、不能替代签名 | **无替代**。`UpdateErrorCode::ChecksumMismatch` 已无产出路径（`diagnostics.rs` 已注明） |
| per-platform 资产匹配（`asset_for` 按 triple 匹配资产名） | 存在 | **无替代**。平台选择改为 manifest 内的 `<os>-<arch>` 键，而不是按资产名匹配；`UpdateErrorCode::NoMatchingAsset` 因此仍然可达（发布漏掉某个平台键时命中） |
| 公钥轮换窗 | **无替代**（zipsign 至少是 any-of 多密钥语义） | **更弱**。`Config.pubkey` 是单把公钥，无 key ID 白名单、无有效期概念 |
| 单调 `release_sequence` 防降级 | **无替代**，仅按 semver 比较 | 同左，未改变 |
| 归档内嵌签名（签名随产物走，不需要额外资产） | 存在 | 改为**分离** `.sig`：每个被签产物 +1 个文件（这正是维护者要求的形状） |

另外两条由库决定、不是项目选择的行为，一并记录：

- 库调用 `minisign-verify` 的 `verify(data, signature, allow_legacy = true)`，因此**同时接受**
  minisign 的 legacy（非预哈希）签名，而不只是 `cargo-packager` 产出的 prehashed 签名。
  这是比 `allow_legacy = false` 更宽的接受面。
- 验签前把整个 payload `read_to_end` 进内存。当前载荷规模（约百 MB 量级）可接受，但它是
  一条随产物增长而增长的常驻内存路径。

### 已接受的残余风险

- **Windows 安装路径未在本机验证。** 本机是 macOS。`install_mode = Quiet` 对应的
  `/S` 静默开关、`/R` 重启语义、以及"安装器替换被占用文件"的可行性均来自源码阅读，
  未在真实 Windows 上跑通。per-user NSIS 安装不需要提权，这一点与 `Quiet` 的前提一致。
- **发布私钥的 CI secret 与真实发布链路尚未验证。** 客户端已内嵌公钥
  (`DF5E2C9D255DD85E`)，但仓库无法得知 GitHub 上 `SIGNING_PRIVATE_KEY` /
  `SIGNING_PRIVATE_KEY_PASSWORD` 是否已设置。未设置时 release workflow 会在
  「Require the update signing key」一步主动失败；这是刻意的：在没有可用密钥的情况下发布，
  比发布失败更糟。真实签名、上传、下载与安装仍需一次完整 release 才能确认。
- **minisign 私钥默认无口令保护。** `generate_key(Some(String::new()))` 产出的私钥是明文
  base64；`SigningConfig.password` 只在生成时设了口令才有意义。密钥的离线备份、权限
  （`0600`）与轮换流程仍是运维职责，不是代码保证。
- **签名/验签从未在真实发布上执行过。** 全部证据来自 loopback fixture 与源码阅读。
- **manifest 合并是本项目新增的自有步骤。** 更新库只消费一份 manifest，不提供合并；共享
  `latest.json` 由 `crates/bongocat-packaging --merge-manifests` 生成。它是本项目代码，因此它的
  正确性（版本一致性、平台键合法性、重复键拒绝）由本项目测试保证，不由上游保证。

### 后续边界

本 ADR 不实现 endpoint 配置界面、公钥注入的自动化、update worker、更新 UI、操作系统包签名
验证或失败启动恢复。在待验证项 1 与 2 完成前，不得声称更新功能或 stable 发布完成。

### ADR-0035 · 更新 worker、更新窗口与发布说明

**状态：** 已接受（2026-09-15；2026-09-25 修订自动检查默认值与间隔）

> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

依赖：ADR-0034（detached minisign 信任模型）、ADR-0029（第三方库边界）、ADR-0016（匿名诊断导出）、ADR-0059（UI protocol 边界）

### 决策

#### 1. 更新 worker 与设置 worker 分开，各自一条线程

新增 `bongocat-app::ApplicationUpdateService`：一条名为 `bongocat-update-service` 的线程独占
`UpdateRuntime`，是**唯一**触碰网络、下载和安装的组件。

不复用 `ApplicationSettingsService` 的命令循环。设置循环是串行阻塞的：把 check/download/install
放进去，会让一次百 MB 级传输期间的所有设置读写排队。两者生命周期也不同——设置服务在退出序列里
先于 overlay 结束，更新服务可以在安装完成后仍在运行。

#### 2. worker 只发布状态，不推送事件

worker 把当前阶段写入 `UpdateStateHandle`（`Arc<Mutex<UpdateSnapshot>>` + revision），窗口按
`UPDATE_STATE_POLL_INTERVAL`（250 ms）读取。理由：

- 窗口关闭、销毁或渲染慢，都不会阻塞或反压 worker；下载进度是覆盖写，不是队列。
- revision 只在阶段真正变化时推进，同一个阶段重复发布不推进 revision，轮询因此不会无意义重绘。
- 命令通道（`UpdateClient` → `UpdateCommand`）保持有界且非阻塞（`try_send`），窗口不会因为
  worker 忙碌而卡住。

**关闭窗口不取消任何操作**，因为操作不属于窗口。这是刻意的：库没有取消钩子（见 §9），一个
"取消"按钮只能让 UI 停止显示，而传输仍在继续——那是在撒谎。窗口因此始终可以关闭。

#### 3. UI protocol 拥有跨层 contract，app 负责适配

`bongocat-ui-protocol` 定义 `UpdateSnapshot` / `UpdatePhase` / `UpdateClient` 等全部更新面
类型，`bongocat-ui` 只负责 GPUI view 和展示策略。`bongocat-ui-protocol` 与 `bongocat-ui` 都不
依赖 `bongocat-update`。`bongocat-app` 做映射，且映射是穷尽 `match`：

- `UpdateStage` → `UpdateFailureStage`
- 14 个 `UpdateErrorCode` → UI 的同名目录

新增一个 stage 或 code 会让 `bongocat-app` 编译失败，直到它被赋予用户可见的含义。这沿用了
`SettingsRuntimeErrorCode` 等既有做法，也让 `cargo-packager-updater` 的类型不越过应用边界。
代价是两份字符串目录，由 `bongocat-app` 的测试逐项比对锁定。

#### 4. 三个阶段可观测，靠拆分库调用实现

窗口需要区分「下载中 / 校验中 / 安装中」，但库的
`download_and_install()` 把三者合成一次调用。因此 runtime 改用
`Update::download_extended()` + `Update::install()` 两次调用：

```text
download_extended(on_chunk, on_finish)   读完全部字节 → on_finish → 验签 → 返回已验签的字节
install(bytes)                           写入安装位置
```

`on_finish` 在验签**之前**触发，返回之后表示验签已通过。于是三个可观测点正好落在
`UpdateEvent::{Progress, DownloadFinished, Verified}` 上，窗口的 `Downloading` / `Verifying` /
`Installing` 分别对应它们，而不是靠猜。

失败阶段不再靠调用方推断：`UpdateError` 现在带一个 `stage` 字段，由 runtime 在失败点设置。
库把下载与验签合在一条错误里，所以下载阶段的错误按 code 再分一次
（`SignatureInvalid` → Verify，其余 → Download，未识别的按 Download 处理——不宣称载荷已通过认证）。

#### 5. 重启要求是平台事实，不是运行时判断

`restart_required_after_install()` 返回 `cfg!(target_os = "macos")`：

- **macOS**：库整包替换 `.app`（`remove_dir_all` + `rename`），运行中的进程此后执行的是已被删除的
  文件。`preset_root()` 虽然只在启动时读取，但 `PresetModelCatalog::list/load` 是**惰性读盘**的，
  所以继续运行会让预设模型切换直接失败。因此 macOS 在安装成功后**自动重启**：先按 §5.3 的顺序
  完成产品 shutdown，再**以新进程**启动新的可执行文件。

  重启必须是新进程，不能用 `exec` 替换进程镜像。`exec` 不结束进程，PID 不变，而 shutdown 序列在
  重启前已经移除了托盘 `NSStatusItem`；替换后的镜像随后创建的 `NSStatusItem` 创建成功、也自报
  visible，但窗口从不被布局，菜单栏里什么都没有——只有用户手动退出再启动才恢复。改为 `spawn`
  之后新进程拿到干净的注册，图标正常出现。`restart_current_process` 的注释记录了完整原因，
  `the_restart_starts_the_updated_build_as_a_new_process` 守住这条不变量。
- **Windows**：库把载荷交给 NSIS 安装器后立即 `process::exit(0)`，`Installed` 在本平台不可观测；
  安装器的 `/R` 负责重启。窗口因此没有重启按钮。

**自动重启由应用侧看门狗触发，而不是由窗口触发。** 窗口可以随时关闭（关闭不取消操作），如果重启
挂在窗口上，用户在传输途中关掉窗口就会留下一个执行着已删除文件的进程。因此应用每 25 ms 观察一次
发布出来的阶段：看到 `Installed { restart_required: true }` 后等 `UPDATE_RESTART_DELAY`（1200 ms，
只为让窗口把结果显示出来）再重启；窗口上的「立即重启」按钮只是把这个等待缩短，两条路径共用一个
`update_restart_started` 标志，因此只重启一次。

#### 6. 自动检查由 GPUI 侧驱动，并在发现更新时打开窗口

开关与间隔值在用户配置里，只有设置服务读得到，所以调度放在 `main.rs` 的 GPUI 侧，而不是 worker 里：
启动后等 10 秒（不与启动争网络和窗口），之后按配置中的整小时数等待下一次检查。自动检查开关
默认 `false`，因此新配置不会主动发起检查；用户显式打开后才启动该调度。间隔默认 `24` 小时，
接受 `1..=8760`；调度器以最近一次实际派发为期限锚点，并通过只读取自动更新设置的轻量轮询重新
安排下一次检查，因此修改间隔会重排期限。关闭自动检查不会清空该值，手动检查也不受它影响。该补充取代本 ADR 最初
记录的“固定 24 小时且间隔未持久化”状态。

发现可用更新且窗口未打开时**打开更新窗口**。替代方案（什么都不做）会让这个开关变成一个
不可观测的设置；这与 §8.4 里"开关存在但无行为"的旧状态没有本质区别。窗口是单例，重复检查
不会叠窗口。

#### 7. 发布说明随 manifest 一起走

`crates/bongocat-packaging` 的 `--merge-manifests` 增加 `--release-notes <file>`，把说明写进共享
`latest.json` 的顶层 `notes`；runtime 把它透传为 `UpdateRelease.notes`，窗口在「更新内容」区域
渲染，并另给一个指向 `releases/tag/v<version>` 的链接。

- **不新增第二次网络请求**：说明和版本、载荷地址来自同一份 manifest，检查更新时一次拿到。
- **不新增依赖**：说明由发布工作流用 `gh api .../releases/generate-notes` 生成（工作流本来就用
  `--generate-notes`），只是改为先生成再同时喂给 manifest 和 `gh release create --notes-file`，
  因此发布页和客户端看到的说明不会分叉。
  - 补充（2026-09-16）：**这一条已被取代**。`generate-notes` 给的是两次 tag 之间的提交摘要（PR 与
    commit 列表），而本项目对外发布的是手写的双语 changelog；两者内容不同，让发布页显示提交摘要
    就等于放弃了 `CHANGELOG` / `CHANGELOG.zh-CN` 作为事实来源。现在由
    `bongocat-packaging --extract-release-notes <file>`（`just release-notes`）按产品版本号从两份
    changelog 中各取同一条目，按「英文正文 → `---` → 中文正文」合成一份文件，同一份文件仍同时喂给
    manifest 与 `gh release create --notes-file`。"一次请求拿到"与"发布页和客户端不分叉"两条结论不变，
    变的只是说明从哪来：不再是 GitHub 的提交摘要，而是仓库里那份 changelog。该步骤因此也不再需要
    `GH_TOKEN`，并且在下载发布产物之前执行。版本在 changelog 里没有条目时合成失败，发布中断——这是
    有意的门禁，理由见下一条。
- **有界**：manifest 每次检查都会被解析，说明因此上限 32 KiB，超长在字符边界截断并追加可见标记，
  而不是让发布失败。空文件按"没有说明"处理，不写空字符串。
  - 补充（2026-09-16）：双语合成后上限仍按 32 KiB 判定，**截断发生在字符边界、不区分语言**：说明
    长到需要截断时，被切掉的会是中文那一段。当前 changelog 的条目约 6 KiB，离上限很远，因此这是
    一个已记录而未处理的边界，而不是一个已知会发生的行为。另需注意：更新窗口渲染的是整份说明，
    不做语言过滤，所以中文用户会先看到英文段、再看到中文段——这与本项目发布页一贯的排版一致，
    是刻意保留的，不是渲染缺陷。
  - 补充（2026-09-28）：每个语言块由工具补一个 `## Changelog` / `## 更新日志` 标题包住 changelog
    条目，块末尾追加一节**生成**的下载链接、模型库和赞助。补标题的理由是层次：条目自己只写
    `###`，生成块三节都是 `##`，不补的话条目会顶着文档开头，读者分不清哪一半是人写的。
    "一次请求拿到"
    这条结论因此多了一个必须一起成立的约束：生成块只能引用 `bongocat-update` **不**需要的资源。
    所以只链接人工安装用的产物（Windows 安装包、两个 macOS 磁盘镜像），不链接签名更新载荷
    `*.app.tar.gz`——它由 updater 从 manifest 取，对读者是同一份程序的第二份下载。链接目标一律
    `https`，因为更新窗口只把 `https` 渲染成可点击控件；生成块不含图片和内联 HTML，
    `update_markdown` 本来就会拒绝解释两者。这些内容不进 changelog，理由见
    `changelog-conventions` §1.3。
- `pub_date` 仍然不写：库把它当可选，新鲜度由版本决定，而打包工具没有日期格式化能力。

#### 8. 传输有界

`UpdaterBuilder::timeout(UPDATE_REQUEST_TIMEOUT)` = 30 分钟。transport 自身没有超时，不设界的话
一条卡死的连接会让 worker 永久阻塞。取值刻意宽松（覆盖整条载荷传输，而不是单次读取），因为它
要逃离死连接，不是约束慢链路；窗口不阻塞在传输上，可以随时关闭。

#### 9. 明确不做的三件事

- **取消下载**：库在 `download_extended` 内部读完全部载荷并验签，没有 abort 钩子。要做到真取消
  只能自研下载与验签——而自研验证层正是 ADR-0029/0034 删掉的东西。因此不提供取消按钮，并在
  runtime 的文档里写明原因。
- **安装前 quiescence**：见下节。
- **操作系统包签名验证（codesign / Authenticode 的客户端校验）**：仍是独立发布门禁，本 ADR 不碰。

### 需要维护者复核的一处偏差

`AGENTS` §10 与 TODO §8.4 的条目是「**安装前**协调 runtime/renderer shutdown，失败可回滚」。
本实现把协调放在**载荷安装完成之后、替换进程之前**：

```text
check → download → verify → install → 按 §5.3 顺序 shutdown → 以新进程启动新构建
```

理由是三条同时成立的观察：

1. **macOS 的整包替换对运行中进程是 inode 安全的**：`rename` 后旧 inode 仍被进程持有，替换本身
   不需要进程先退出。
2. **Windows 的安装路径本来就会终止进程**：库在运行安装器后 `exit(0)`，不存在"安装时进程还在写
   文件"的窗口。
3. **先 quiescence 会让安装失败变成不可恢复**：`Application::shutdown(self)` 消耗 `Application`，
   没有对应的 restart。若在安装前就把 runtime/overlay 停掉，而安装随后失败（磁盘满、权限、
   库在 macOS 上 `remove_dir_all` 后 `rename` 失败），应用会停在"overlay 已销毁、runtime 已停止、
   只能重启"的状态。当前顺序下，安装失败时应用**完好无损**，可以直接重试。

因此当前顺序在失败路径上严格更安全，而在成功路径上同样满足"新构建启动前完成 shutdown"。
**这是一处与既有文档措辞不一致的取舍，需要维护者确认**；若不接受，应改为
"先 shutdown 再 install"，并同时为 `Application` 增加可恢复的 pause/resume 能力——那是另一项
独立工作，不在本 ADR 范围。

### 真实端点首次运行的结果（2026-09-15）

第一次在真实安装产物上运行检查更新时，窗口报 `update_release_fetch_failed`（"无法读取发布信息"）。
排查结论记录如下，因为它同时说明了两件事。

`https://github.com/ayangweb/BongoCat/releases/latest/download/latest.json` **存在且返回 200**，但内容
是**旧 Tauri 版本的 updater manifest**，不是本项目打包流程产出的那份：

| 观察 | 线上文档 | 本项目产出 |
| --- | --- | --- |
| 平台键 | `darwin-aarch64`、`windows-x86_64-nsis`、`linux-*` | `macos-aarch64`、`windows-x86_64` |
| 每个条目 | 只有 `url` + `signature` | 另有必需的 `format` |
| `pub_date` | 有 | 刻意不写 |
| 签名 | `trusted comment: signature from tauri secret key` | 本项目 minisign 私钥 |

库里 `ReleaseManifestPlatform::format` 是**非可选**字段，所以 18 个条目全部缺 `format` 时，整份文档在
平台查找之前就反序列化失败 → `Error::Serialization` → 当时被映射到 `ReleaseFetchFailed`。

也就是说：**更新功能本身没有缺陷，是线上还没有由新流程发布的 release。** 这也是 ADR-0034 待验证项 1
第一次被真正执行到——它暴露的是"端点被旧产物占用"，而不是"客户端有问题"。

排查过程还暴露了一个诊断粒度问题并已修正：

- `ReleaseFetchFailed` 原本同时表示"取不到发布信息"和"取到了但读不懂"。这两件事需要不同的应对
  （前者通常可重试或属运维遗漏，后者说明发布的 manifest 不是本产品的），合并成一个码会让排查必须
  读源码。
- 现在拆成两个稳定码：`update_release_fetch_failed`（`Error::ReleaseNotFound`：没有 release、没有
  manifest 资产、或非成功响应）与 `update_release_manifest_invalid`（`Error::Serialization`，以及
  `Error::Semver`——本构建自己的版本在任何请求之前就已解析，所以库返回的 semver 错误只可能来自
  manifest 的 `version` 字段）。按 ADR-0034 的说明，新增码是向后兼容的。
- 两条新增能力测试在 loopback 上固定了这个故障形态：
  `a_release_without_a_manifest_is_a_fetch_failure` 与
  `a_manifest_from_another_pipeline_is_rejected_as_unreadable`。后者不只断言失败，还做对照实验：
  给同一份文档的每个条目**只补上 `format`**，失败就从 `Serialization` 变成 `TargetNotFound`——这证明
  缺 `format` 是解析失败的真正原因，而平台键拼写是第二个独立问题。

运维含义（不属于代码范围，但必须写下来）：线上 v1.1.0 仍是旧 Tauri 版本，而工作区版本号也是 `1.1.0`。
即使把 manifest 换成正确形状，`release.version > current_version` 对已安装的旧 1.1.0 也不成立，
旧用户会被判为"已是最新"而永远不迁移。新流程的首次发布需要选一个高于旧线的版本号。

### 补充：更新窗口改为按内容定高（2026-09-26）

#### 为什么改

窗口原本是固定 560×460，十个状态共用。逐状态实测（无头渲染，二分查出「内容刚好放得下」的最小高度）
显示，其中四个状态的内容只有一行文字：

| 状态 | 内容所需高度 | 原窗口 | 空出来的部分 |
| --- | --- | --- | --- |
| `Idle` / `UpToDate` / `Available`（无说明） | 159px | 460px | 301px |
| `Checking` | 179px | 460px | 281px |
| `Failed`（无说明） | 187px | 460px | 273px |

四个状态里没有一个用过窗口的一半，而「检查更新」这条最常见的路径正好全程停留在这几个状态里——
用户看到的是一个 560×460 的大框，里面一行字。这是 Sparkle 那类更新对话框采用「按内容定高 + 设上限
+ 超出滚动」的直接原因，也是本次改动的唯一动机。

#### 改了什么

高度在布局后测量、下一帧应用，夹在下限 180 与上限 460 之间。460 是改动前就用的高度，因此窗口不会比
原来更高。实测：单行状态收敛到 187px；带 40 行说明的 `Available` 收敛到 411px，说明区取满
`NOTES_MAX_HEIGHT` 后滚动。

四处是让测量能收敛的必要条件，不是风格选择：

- **说明区按内容取高、以常量封顶**。如果让它跟着窗口长，窗口变高 → 说明区变高 → 窗口再变高，两者
  互相喂给，永不收敛。`the_height_does_not_depend_on_which_height_the_window_opened_at` 从 180 和
  460 两个起点验证同一个结果。
- **内容列不参与收缩**（`flex_shrink_0`）。否则窗口偏小时说明区被静默压缩，帧报告的是「放得下」，
  窗口就永远不会长回去。
- **间距与内边距取自该帧自身的 bounds**，不写死常量。改 `p_4` 或 gap 不会让窗口高度与内容脱钩。唯一
  取不到的是下内边距（窗口太小时它被压掉而不是把动作推出屏幕），因此上内边距被同时用作两侧，
  `the_window_pads_its_content_symmetrically` 守住这个假设。
- **窗口从上限打开再向下收敛**。内容定高只能在开窗后的下一帧才知道，从下限打开会让每个高于下限的
  状态都有一帧动作被裁掉；向上收敛比向下收敛更显眼。

用户拖动窗口后由用户的高度接管（`observe_window_bounds` 比对上次请求的高度），说明区让出空间并滚动。
没有这条，下一次状态变化会悄悄撤销用户的拖动——那才是让「按内容定高」显得有缺陷而不是有用的那种行为。

> 本节以上两条（从上限打开、用户拖动后接管）已被同文件末尾「消除开窗与状态切换时的高度闪烁」一节
> 取代：窗口改为隐藏状态下量好画好再显示，也不再允许拖拽。保留原文是为了记下当时为什么那么选。

### 补充：更新窗口状态浏览器（2026-09-26 已删除）

#### 为什么当时需要它

按内容定高之后，「这个状态看起来对不对」只能靠看，而大部分状态**没法看到**：Development 构建的更新
服务在发出任何请求前就被 `unavailability()` 拦下，所以 `Available` / `Downloading` / `Verifying` /
`Installing` / `Installed` 与各阶段 `Failed` 在构建它的机器上一条都到不了。此前唯一能看到的状态是
`Unavailable`——恰好是内容最少、最不需要看的那个。

合成点击也不可靠：一次 `System Events` 的合成点击会落到别的窗口上，而拿不到按钮的人不该被要求去按
真实的按钮。

#### 做了什么，以及为什么现在删掉

`bongocat-ui::update_state_browser` 开一个开发专用窗口，每个按钮往**共享更新状态**里发布一个状态，
并请求应用把更新窗口叫起来。20 个按钮按「紧凑 / 更新内容 / 已安装 / 失败」分组——分组是重点：哪些
状态高，从状态名上看不出来，而分组把「过一遍」变成一次检查。驱动的是**真实窗口**而不是副本：副本就
不是要发布的那份代码了。

它是定高这项工作的脚手架，不是产品的一部分。定高与闪烁都改完、无头渲染测试已经逐个状态覆盖之后，
它唯一剩下的用途是人工过一遍十个状态——这件事无头测试在做，而且做得更多（每个状态都断言动作、
高度、内容）。留着一个 700 行的开发窗口、CLI 开关和 9 个只为它存在的测试，等于让发布构建多带一个
没人维护的界面。所以删除，`bongocat-ui` 与 `bongocat-app` 随之回到没有它的样子。

#### 当时的验证（2026-09-26，本机 macOS 26.5 / aarch64）

- 9 个测试：点击按钮会发布该按钮自己的状态并请求显示窗口（而不是只把自己标成选中）；`Back to a real
  check` 真的退回真实状态；窗口能渲染的每种状态都有按钮；有两个分支的状态（`Unavailable` /
  `Installed`）与四个失败阶段各有一个按钮；分组名称连续且顺序稳定；面板不会开到屏幕外（含屏幕太窄的
  情况）；脚本入口发布的内容与按钮一致；示例更新内容确实长到窗口放不下。
- 实机截图：紧凑状态约 187px；`Installing` + 完整双语更新内容为 431px，更新内容取满预算后滚动，
  「在 GitHub 上查看」与「关闭」两个动作都在窗口内。这批截图是下面那节定高结论的实机依据。

#### 删除时一并失去的东西

- **未验证**：Windows 上浏览器与更新窗口的并排位置；面板的键盘导航。这两项随浏览器一起消失，
  没有留下结论。
- 定高本身在 Windows 上的行为**不受影响**：它由无头渲染测试与下面的 trace 覆盖，与浏览器无关。
### 补充：消除开窗与状态切换时的高度闪烁（2026-09-26）

#### 现象

用户报告：更新窗口在高度变化过程中会先瞬间变成上限高度，再迅速回退到内容实际需要的高度。

#### 原因不是「算错」，是「算得太晚」

原来的实现把窗口开在上限高度（460），等第一帧测出内容高度再收下来。内容定高只能在布局之后知道，
所以从上限高度开始是当时能想到的唯一安全猜测——上限装得下任何内容，不会裁掉动作。代价就是那一下
可见的收缩。

真正多余的是**中间那一步**。旧代码在窗口比内容矮时返回 `TooSmall`，目标高度直接取上限：意思是
「先长到上限，再收敛回来」。而这个中间高度并非必需——内容列不收缩、空隙塌成 0，所以即使窗口装不下
内容，这一帧报出来的仍然是它**需要**的高度。`Fit::TooSmall` 分支因此整条删掉，测量一步到位。

#### 显示时机：隐藏状态下量好、画好，再显示

目标高度一步到位之后，还剩「窗口出现时是哪个高度」。这一条比看上去难，有两个平台事实挡路：

1. macOS 的 `resize` 是异步的——`PlatformWindow::resize` 把 `setContentSize_` 排进 foreground
   executor，当轮拿不到新尺寸。
2. 窗口没显示时拿不到帧。macOS 只为「遮挡状态判定为在屏幕上」的窗口跑 display link，也就是只发帧
   给它。于是「等一帧画对了再显示」会死等——先做的那版正是这样，窗口永远不出现。

做法是三步，且都不能交给 frame loop：隐藏状态下手动 `draw` 一次量出目标高度，`resize`，然后**再排
一个 foreground executor 任务**，把新高度同步回来、重画、`activate_window`。应用 resize 本身也是同
一个 executor 上的任务，所以后排的任务一定排在它后面——保证来自顺序，不是轮询也不是超时。

用 `cx.update_window`（untyped handle）而不是 `WindowHandle::update`：后者在闭包期间 lease 住 root
view，而 `draw` 会再 lease 一次，直接 double-lease panic。frame loop 用的是 untyped handle，同理。

#### 顺带：窗口不允许拖拽改变尺寸

`is_resizable: false`，一行跨平台：macOS 去掉 `NSWindowStyleMaskResizable`，Windows 去掉
`WS_THICKFRAME`/`WS_MAXIMIZEBOX`。理由与「按内容定高」是同一条——允许拖拽等于让用户选一个下一次
状态变化就会撤销的高度。

于是「用户拖过就接管高度」那条分支（`observe_window_bounds` 比对上次请求的高度、`pinned` 状态、
`start_height_pinning`）连同它的测试一起删除：窗口没有 resize 手柄，那条分支是死代码。

#### 平台取整

trace 里平台把请求的 410.5 变成 411，下一帧量到 410.5、又问一次。平台不给尺寸所以也不会回调，
循环自己停了，但每帧问一次平台给不出的高度是没道理的事，所以「已经在目标高度」允许半像素误差
（`HEIGHT_MATCH_TOLERANCE`）。

#### 边界

- **未验证**：Windows 上的开窗时序。`resize` 在 Windows 上是否同样异步、隐藏窗口是否同样拿不到帧，
  本机无法验证；上面两处都写了 macOS 的具体原因，若 Windows 行为不同则需要按平台分支。
- **未验证**：800x600 与 125/150/200% 缩放下的高度。

### 补充：开窗即显示正在检查（2026-09-26）

#### 现象

用户报告：打开更新窗口时先显示上次检查的结果（“已是最新”、上一次的更新内容或失败原因），
过一会儿才开始显示“正在检查”和进度条。

#### 原因不是“慢”，是“请求发得太晚”

系统菜单与 About 页的「检查更新」都走 `open_update_window_and_check`，而它当时的顺序是
**先开窗并显示，再请求检查**。两件事叠起来才是这一次闪烁：

1. 窗口在 `open_window` 闭包里构造时读的是 `client.snapshot()`，此刻 worker 发布的仍然是上一次
   检查的阶段。窗口的第一帧——也就是用户看到的那一帧——因此画的是上一次的答案。
2. `view.check()` 随后才把 `Check` 命令发出去。命令是发给 worker 线程的消息，worker 取到它之后
   才发布 `Checking`；而窗口按 `UPDATE_STATE_POLL_INTERVAL`（250 ms）轮询共享状态。所以从点击到
   进度条出现之间，屏幕上一直画着旧阶段。

只把请求提前并不能消除它：即使开窗前就把命令发出去，worker 仍是另一个线程，窗口构造那一刻
`Checking` 未必已经发布。第一帧要显示这次检查，就得由窗口自己渲染它。

#### 改了什么

- **窗口带着“开窗要做什么”打开**：`UpdateWindowStart::{Current, Check}`。`Check` 在 view 构造期间
  （即第一帧被绘制之前）就发出请求，`Current` 保持原语义，供自动检查“只展示结果”那条路径使用。
- **请求之后由窗口渲染这次检查**：view 记下请求发出时的 revision（`PendingCheck`）并把阶段渲染成
  `Checking`；`poll_state` 在 revision 还没有前进时**不**采用共享状态里那个仍属于上一次检查的阶段。
  revision 一旦前进就说明 worker 回答了，此时以 worker 发布的阶段为准。命令发不出去（通道满或已关闭）
  时不渲染进度条——没有 worker 在跑的检查不能画成正在跑的检查。
- **这个 revision 是发请求那一刻从共享状态读的**，不是复用窗口上次轮询到的那一个。worker 按自己的
  节奏发布，一个结果完全可能落在两次轮询之间；若用旧 revision 作锚，下一次轮询会因为「revision 已经
  前进」而把那个还没被窗口看到的结果当成对本次请求的回答——同一次闪烁，只是晚了一轮询。
- **`asks_for_a_new_check` 收口三个条件**：没有未完成的请求、当前阶段不忙、当前构建不是
  `Unavailable`。最后一条不是防御性代码而是必需条件：这类构建的 worker 只会重新发布它已有的阶段，
  revision 不前进，照旧渲染的话进度条会永远转下去。同一条件也让「开窗即检查」对已经打开的窗口是
  空操作，因此一个 view 不会有两次在途检查。
- **调用点不需要区分新旧窗口**：`open_update_window_and_check` 末尾那次 `view.check` 仍用于**已经打开**
  的窗口（用户在窗口空闲时再次点击「检查更新」），对刚创建的窗口是空操作。

#### 与既有决定的关系

§2「worker 只发布状态，不推送事件」不变：worker 仍是共享状态的唯一写者，窗口仍然按 250 ms 轮询。
变化的是窗口对**自己刚发出的请求**的本地渲染，它有一个明确且可验证的终止条件（revision 前进），
而不是一个超时或猜测。窗口关闭不取消任何操作也不变：这次渲染的检查在窗口关掉后照常由 worker 完成。

#### 边界

- **未验证**：实机上第一帧的观感。逻辑由无头渲染测试固定，但「打开时立刻看到进度条」这一眼仍然
  需要人看；本次没有启动产品做人工核对。

### ADR-0064 · Human-readable bounded application logging

**状态：** 已接受（2026-09-24）

### 决策

#### 1. 单一人类可读文本方案

application 与 Cubism Core 日志都使用 UTF-8 单行文本，不再写 JSONL。每条记录固定包含：

```text
<UTC timestamp> <LEVEL> [<module>] <stable-code> | <message> | <bounded context>
```

时间使用带 `Z` 的 UTC 时间戳；级别为 `ERROR`、`WARN`、`INFO`、`DEBUG` 或 `TRACE`；module、stable
code 和 message 分离，便于用户阅读和开发者检索。文件名统一使用 `.log`：

- `application-YYYY-MM-DD.log`
- `cubism-core-YYYY-MM-DD.log`

同一 application/Core writer 共享文本格式化、过滤、单文件大小、rotation、数量、目录总容量和 retention
实现。Core callback 仍只做有界复制与非阻塞入队，文件写入只在专用 worker 中执行。

#### 2. 日志过滤与保留策略

当前 v1 配置新增：

```json
{
  "logging": {
    "level": "info",
    "retention_days": 7
  }
}
```

- `logging.level` 是写入阈值，接受 `error`、`warn`、`info`、`debug`、`trace`，默认 `info`。
- `logging.retention_days` 接受 `1..=30`，默认 `7`；配置降低后立即收敛旧文件。
- 日志首先按 UTC 日切换；单个活动文件超过 `1 MiB` 时继续切分编号文件，防止单个文件无界增长。
- 当前 `application` 与 `cubism-core` 日志目录合计最多 `8 MiB` 和 `32` 个文件；超限只删除最旧的
  已轮转文件，当前 UTC 日活动文件始终保留。
- 不提供 rotation mode 配置。日分文件是用户组织日志的稳定边界，大小上限是异常流量保护；只允许用户在
  “按天”和“按大小”之间二选一会制造无实际价值的状态组合。

#### 3. 事件和级别

事件仍由 app-owned 闭合 event catalog 产生，不用自由文本作为公共日志协议。级别按以下语义分配：

- `error`：关键操作失败、panic、shutdown 失败、数据损坏或不可恢复的资源/解析错误。
- `warn`：程序继续但已降级、fallback、重试、回滚、权限不足、临时 GPU/输入服务异常。
- `info`：启动/退出、用户可见的模型/配置/窗口状态变化、更新阶段结果和恢复结果。
- `debug`：服务 start/stop、候选模型 prepare、输入 reset/reconcile 状态变化、窗口状态 flush。
- `trace`：有界、低频且不含输入内容或资源正文的开发诊断；不用于逐事件输入、逐帧渲染或下载进度。

启动、退出、配置读写/恢复、状态持久化、模型准备/切换/导入/删除、资源解析、窗口创建/显隐、
输入服务与权限、更新检查/下载/验签/安装、平台文件/网络错误和 Rust→GPUI command 失败都必须在
app/service 边界进入日志。底层返回稳定错误码后由最近的 app owner 记录一次，避免同一错误在低层、
service 和 UI 重复输出。

#### 4. 隐私和性能边界

日志不得包含用户路径、真实按键序列、剪贴板、配置原文、模型正文、URL、密钥/凭据、签名材料或动态 OS 错误文本。
允许的上下文只包括闭合 stable code、匿名/可移植 model ID、origin、阶段、计数、字节数、revision、
能力/服务状态和构建版本；文本统一转义换行/控制字符并限制长度。panic hook 仍不读取 payload，
使用非阻塞记录。

默认 `info` 只写低频业务和错误边界。输入事件、frame tick、cursor/axis 样本、下载进度、轮询 snapshot
和可恢复的 temporary presentation unavailable 不逐次记录。轮转与清理失败只进入匿名 writer 统计，
不得阻止 overlay/runtime 继续运行。

#### 5. Diagnostics preview

匿名 diagnostics JSON 保持不变。preview bundle 中的 application lifecycle 历史从新的 `.log` 文件
严格解析固定 code，并重新输出为脱敏的 `application-events.log`；每条固定为
`<LEVEL padded> [<module>] <code>`，不复制原始日志行、消息、上下文、路径或时间戳。输入文件或
其中任一记录无法通过共享文本语法、闭合 code/module/level 契约时，整份来源文件跳过并只增加匿名计数。
Cubism Core 原文仍不进入 preview，只保留匿名
written/dropped/rotated/pruned/bytes/file 统计。

### 影响

- 不新增第三方 logging 依赖，也不增加 GPUI/runtime/OS 类型到公共日志 API。
- application/Core 输出、文件发现、retention helper、diagnostics preview 和相关测试必须一起迁移；
  活跃代码不再保留 JSONL writer/解析分支。
- `.log` 是用户可支持格式，不承诺稳定的第三方日志解析 schema；需要机器处理时使用匿名
  `diagnostics.json` 或 preview bundle 中的严格重写记录。
- 日志设置是首版 v1 字段，遵循仓库规则直接更新 schema/default/fixture/实现，不增加迁移或旧字段
  fallback。

### ADR-0069 · Release notes render through gpui-kit TextView behind a claim-based refusal layer

**状态：** —

- Status: accepted
- Date: 2026-09-26
- Depends on: ADR-0020, ADR-0054, ADR-0068
- Amends: none

### Decision

`crates/bongocat-ui/src/update_markdown.rs` keeps three rules and no renderer. It
hands `TextView` the notes, plus a `MarkdownExtensions` registry built from four
plugins:

- `NoRemoteImage` claims every `Node::Image` and `Node::ImageReference` and shows the
  alt text. An image with empty alt text is still claimed, because declining it would
  hand the URL back.
- `LiteralHtml` claims `Node::Html` and shows the source. It is registered twice — once
  as an inline plugin and once as a block plugin — because `gpui-kit` has a separate
  dispatcher for each and a block-level `<div>…</div>` never reaches the inline one.
- `RefusedLink` claims a `Node::Link` whose target fails the HTTPS test and shows the
  link's label as plain text, so a refused target is not underlined and accent-coloured.
  Accepted links are declined to `gpui-kit`, whose click handler still routes through
  the platform opener, which re-checks the scheme.

Each rule is a plain function over the parsed node (`image_alt_text`,
`literal_html_text`, `refused_link_text`) with the plugin as a thin shell, so each
decision is unit-testable without a window.

The input bounds are applied to the raw text, before anything parses it:

- the byte cap stays at 32 KiB;
- a container-marker cap replaces the old block-depth cap. The old cap counted nesting
  while parsing, which the delegated parser no longer exposes. The new cap bounds the
  number of block containers in the source, which bounds tree depth because every
  level of CommonMark nesting is opened by at least one container marker. `TextView`
  walks the tree with one Rust call per level, so tree depth is renderer stack depth
  and this is a sound bound rather than an estimate of apparent depth.

A probe plugin registered after the four production plugins asserts the mechanism
itself: `MarkdownExtensions` resolves plugins in registration order and stops at the
first claim, so a node the refusals claim is never offered to the probe. The test also
reads the rendered text back, because a plugin that claimed more than it was meant to
would leave the changelog blank while still passing any test that only asks whether
something rendered.

### Consequences

- The module drops from a parser plus renderer to a policy layer; `pulldown-cmark` is
  no longer a direct dependency of `bongocat-ui`, since `TextView` owns parsing.
- GFM tables and task lists now render, because `TextView` enables them. The update
  window's changelog is styled by `TextViewStyle::from_theme`, so it follows the theme
  instead of a local token set.
- The update window's link elements no longer carry product-owned element ids;
  `gpui-kit` owns link rendering. The window-level tests now cover what only the window
  can show (the release reaches the window, the notes hold their height budget, the
  phase's actions stay reachable) and the document-level tests cover what each node
  becomes.
- A link *reference* resolves to its target inside `gpui-kit`, so `RefusedLink` cannot
  judge it and a reference link with a non-HTTPS target is presented as a link. It
  still cannot open: the click goes through the HTTPS-only opener. Closing that gap
  needs an upstream hook.
- **This depends on `gpui-kit` resolving plugins before its own handlers.** That is what
  makes the refusals refusals rather than suggestions. A `gpui-kit` revision that
  consulted plugins afterwards would reopen the fetch, which is why the probe test
  asserts the ordering instead of trusting it. The `gpui-kit` revision stays pinned per
  ADR-0020, and switching back to crates.io is a change that has to re-run this test.

### ADR-0074 · 验证流水线按缓存作用域与构建产物拆分

**状态：** 已接受（2026-09-28）

### 决策

#### 1. 只在 `pull_request` 上运行；master push 与手动触发都不保留

这一条是**先加上、实测无效、再撤掉**的，过程本身是结论的一部分。

最初加 `push: master` 的理由不是验证 master（PR 必须通过才能合并，那在验证上是冗余的），
而是缓存：GitHub 的 cache 按 ref 分域，PR 只能读基线分支（`refs/heads/master`）与自己
（`refs/pull/<n>/merge`）的条目，合并后该 PR 的条目就留在一个此后无人读取的作用域里。
同一 PR 内部的复用确实有效——`docs/refresh-readme-status-bar` 的三次连续运行里，
第 1 次 `Cache not found`、`macos-spikes` 16.9 分钟，第 3 次 `Cache restored from key`、
2.9 分钟——但**跨 PR 无效**，而开发者等待的正是每个新 PR 的第一次运行。

**但它没有兑现。** 加了 master 触发之后实测：

| 运行 | `Test workspace (windows)` | 原因 |
| --- | --- | --- |
| 某 PR 的第 2 次（同 PR 作用域有缓存） | 6.4 分钟 | 热 |
| master push | 19.0 分钟 | master 作用域缺该条目 |
| 下一个新 PR 的第 1 次 | 19.0 分钟 | 冷 |

仓库 cache 预算 10 GB，实际占用 **10.69 GB（26 条）**，已超。GitHub 在写入新条目时按 LRU
逐出，被挤掉的恰好是最大的两条——`Windows-workspace` 1.88 GB 与 `macOS-workspace`
1.72 GB——它们在 master 作用域里**根本不存在**。而且每个打开的 PR 都会在自己作用域里存一
整套 master 的副本，占用 ≈ `(1 + PR 数) × 单套大小`，所以同时开两个 PR 就必然再次超预算。
PR #1061 的 `product-smoke` 日志直接写了 `Cache not found`，尽管 master 作用域里那条
`Windows-product` 存在：两次运行并发，master 那次还没写完。

也就是说 master 触发**从未真正交付过预热**，只是让每次合并后多出一轮看起来莫名其妙的验证。
维护者据此决定撤掉它，接受 PR 冷跑约 20 分钟。`workflow_dispatch` 一并去掉。

保留的：`concurrency` + `cancel-in-progress`，同一 PR 上被后续 push 取代的运行不必烧分钟。

`tools/tests/test_verify_documentation_filter.py` 断言解析出的触发事件**恰好只有
`pull_request`**——它读的是 YAML 解析结果而不是文件文本，所以记录这段历史的注释可以照常
提到 `push`。将来若要重新加回，必须是一个有意识的决定。

#### 2. `Test workspace` 拆成四个作业，按产物而非按平台切分

| 作业 | 平台 | 内容 |
| --- | --- | --- |
| `workspace` | Windows + macOS | fmt、clippy、`cargo test --workspace`、`cargo check --workspace --release`、production feature check、provenance |
| `product-smoke` | Windows + macOS | `bongocat-app` release 构建与全部默认 feature 的 product/storage smoke |
| `packaging-smoke` | macOS | `just build` 的两个 bundle smoke 与未知参数拒绝 |
| `windows-platform-smoke` | Windows | startup item、missing-release recovery、D3D11 模型切换 |

切分依据是**产物是否共享**，不是步骤数量：`just build` 为显式 target triple 构建，与
`product-smoke` 不共享任何 artifact；storage smoke 必须跑在默认 feature 构建之后，因此
留在同一个作业里。四个作业的验证集合与拆分前逐条对应，没有任何 smoke 被删除。

#### 3. storage-injection 构建与默认构建共用 target 目录

`storage-test-injection` 只作用于 `bongocat-app` 自身，在同一 target 目录下构建只需重新
链接该 crate（实测本机 8 个 workspace crate、零第三方依赖），构建后把二进制复制到
`target/storage-smoke/`，storage smoke 继续跑属于自己的一份产物。`product-smoke` 中所有
读取默认 feature 二进制的步骤（single-instance、product icon、system menu）因此排在
storage 构建之前。

#### 4. 缓存按作业分键，路径尽量不重叠

仓库 cache 预算是 10 GB，而合并后的单个 `target/` 已经是 3.2 GB，同一份 `target/` 用两个
键各存一次会直接挤爆预算并触发 LRU 逐出。因此每个作业只存自己产出的 profile：

| 作业 | 缓存路径 | 动作 |
| --- | --- | --- |
| `toolchain` | `target/toolchain-stable/`、`target/toolchain-minimum/` | 读写 |
| `workspace` | `target/debug/`、`target/release/`（只含 `cargo check` 的 metadata） | 读写 |
| `product-smoke` | `target/release/`（含 codegen） | 读写 |
| `packaging-smoke` | `target/*/release/`（显式 triple 的产物） | 读写 |
| `windows-platform-smoke` | `target/release/` | **只读**（`actions/cache/restore`） |

`workspace` 的 `target/release/` 只含两个 `cargo check --release` 产生的 metadata，几百 MB，
不含 smoke 作业链接出来的代码；保留它是为了让 release check 不必每次重编 189 个 crate。

`windows-platform-smoke` 只增加两个测试 harness 与一个 overlay 二进制，它们是
`product-smoke` 缓存内容的子集，因此只读不写——否则同一棵 release 树要在仓库预算里再存
一份。删除第二个 target 目录同时让 Windows 的缓存条目从 3.2 GB 降到 1.9 GB 左右
（按目录构成估算，未在 CI 上实测）。

各作业缓存的 `~/.cargo/registry` 仍然重复（约 0.3–0.6 GB × 条目数）。收敛为一个共享
registry 键是明确的优化项，但在条目数与预算都稳定之前不改动，避免同时引入第二个变量。

#### 5. toolchain 作业的 stable canary 改为真实检查

`rust-toolchain.toml` 把 toolchain 钉在 1.97.1，因此作业里裸跑的 `cargo check` 始终解析
到被钉住的版本——"Build workspace with current stable" 与随后的 minimum 检查跑的是同一个
编译器，第二次是空操作（实测 0 秒）。现在 stable 检查显式使用 `cargo +stable`，并单独放在
`target/toolchain-stable`，让这个此前静默失效的门禁真正生效。它在关键路径之外，冷跑多编译
一次 workspace 的代价不进入任何 PR 的等待时间。minimum 半边随后按第 7 条删除，所以本作业
现在只做 stable canary 一件事。

#### 6. `spikes/model-package` 不再在 Linux 上重复跑

`model-package-platforms` 已经在两个发布平台上运行它的 fmt、clippy、test 与 release
check，Linux 上的一份是严格子集，从 `contract-spikes` 的矩阵里移除。

#### 7. 三处只有重复成本、没有覆盖的步骤被删除

按 PR #1059 与 PR #1055 的实测步骤耗时逐条排查后，删掉三处：

1. **toolchain 作业的 minimum 半边。** `rust-toolchain.toml` 钉住 toolchain，所以这一半与
   stable canary 跑的是同一个编译器；而 `workspace` 作业已在两个平台上用 clippy
   `--all-targets`、test 与 release check 编译了整个 workspace。严格子集，删除。
2. **四个 spike 作业里的 `cargo check --release`。** 这些作业的 smoke 跑的是自己
   `cargo build` 出来的 **debug** 可执行文件，release profile 的产物没有任何东西执行过：
   macOS 作业 4.95 分钟，两个 Windows GPUI 作业各约 3 分钟。产品自身的 release profile 由
   `workspace` 的 `cargo check --workspace --release` 与 product smoke 的真实 release 构建覆盖。
   `spikes/model-package` 的 release check 保留：它没有 smoke，那个检查是它唯一的 release
   信号。`spikes/input-windows` 的两条 `--release` 压力运行也保留，那是 issue #47 的证据。
3. **packaging smoke 的第二次 `just build`。** 两个 bundle 的差异只有编译进去的
   `BUILD_ENVIRONMENT` 常量与 bundle 内 provenance 的 `environment` 字段：reopen smoke 断言的
   是 bundle id、图标、签名和 LaunchServices 握手，两项都不读；environment 到 feature 的映射
   已由 `tools/tests/test_packaging_contract.py` 静态断言。留下的是"CI 不再端到端启动一个
   Development bundle"——"bundle 能在 runner 上启动"仍由同作业的 startup-item smoke 覆盖，
   它按 ADR-0051 必须使用 Production bundle。

删除 1 与 2 不在关键路径上，只减分钟；删除 3 把 packaging 作业从 18 分钟降到约 14 分钟。

#### 8. 失败证据契约从固定数字改为按作业推导

`tools/tests/test_failure_evidence_workflow.py` 原先断言 `verify.yml` 里恰好有 10 个
`Collect redacted failure evidence`。拆分后这个数字会立刻过期，而过期的数字无法区分"某个
作业丢了证据步骤"和"某个作业本来就没有"。测试改为解析 `jobs:` 块、断言每个作业各自持有
一对证据步骤。

#### 9. 文档改动跳过编译作业，但只有 `fixtures` 永远运行

编译器占了流水线约 93% 的时间，而 README、ADR、issue 模板这类改动一行代码都碰不到。
新增 `filter-paths` 作业判定"这次改动是否**只**碰了文档"，为 `true` 时其余作业全部跳过，
关键路径从约 9 分钟降到 16 秒。

三条让它不变成静默失守的约束：

1. **单向判定。** 过滤器只能*证明*改动与代码无关才能跳：识别不出的路径、读不到的 diff、
   空的改动集一律判为 `false`（全跑）。反过来那种"作业的输入变了才跑"的写法是**开口**的
   —— 将来新增一个顶层目录不匹配任何输入表，于是所有作业跳过，一次没人验证的改动带着
   全绿的 run 合并。这个方向是本决策里最容易写错的一步。
2. **门禁在作业级，不在 workflow 级。** 跳过的作业报告为 *skipped*，能满足 required check
   而不会卡住；workflow 级 `paths:` 一个都不匹配时**整个 workflow 都不运行**，required check
   永远不上报，PR 永久等待。`workflow_dispatch` 是"我怀疑跳多了"时的手动全量入口。
3. **`fixtures` 永远运行。** 它执行的契约测试会读 `verify.yml`、`release.yml`、两份 changelog、
   `justfile`、`macos/Info.plist`、`deny.toml`、`.github/dependabot.yml` 与
   `product-runtime`——所以"改文档"不等于"不需要验证"。`.github/workflows/` 也因此
   不在文档白名单里：改流水线必须由这条流水线自己验证。

`tools/changed-paths.py` 承担判定，`tools/tests/` 里有两条契约测试守护：一条覆盖分类本身
（含"空改动集判为非文档"这个失败方向），另一条断言 **`verify.yml` 里每个作业必须要么在
永远运行的白名单里，要么带显式 `if:`**——新增作业时不可能"忘了决定它能不能跳"就把门禁关掉。

### 影响

- PR 的墙钟时间从"冷编译 + 全部串行"变成"冷编译 + 并行"，约 50 分钟降到约 20 分钟。
  **每个新 PR 的第一次运行都是冷的**，缓存只对同一个 PR 内的后续 push 有效。
- 拆分后的作业各自 `checkout` 并各自 restore 缓存，runner 分钟数会上升；换来的是墙钟
  时间和每个 PR 的反馈速度。
- 只改文档的改动只跑 `fixtures`，约 16 秒（见决策第 9 条）。
- 14 个作业各自持有失败证据上传，`retention-days: 7` 与脱敏工具不变。
- 本 ADR 只改变 CI 的执行方式。`just check`、`just build` 与本地开发流程不变。

### 已接受的残余风险

1. **PR 冷跑是常态，关键路径约 20 分钟**。曾尝试用 master push 触发预热，实测未兑现
   （决策第 1 条），维护者接受这个时间。同一 PR 内继续 push 会命中该 PR 自己的作用域，
   实测 `Test workspace (windows)` 从 19.0 分钟降到 6.4 分钟。
2. **10 GB 缓存预算已经超**（实测 10.69 GB / 26 条），条目在被写入时 LRU 逐出，最大的两条
   `*-workspace-*` 反复被挤掉。这是 master 触发失效的直接原因，也意味着**缓存不能作为时间
   承诺的依据**。若将来要重新尝试预热，必须先缩小单套体积：把重复十几遍的
   `~/.cargo/registry` 收敛成每个 OS 一条共享键是第一优先，其次是去掉 `workspace` 键里的
   `target/release`（那两个 release check 的 metadata，值 3.3 分钟）。
3. **master 没有任何独立验证**：只有 PR 触发，master 也没有分支保护。绕过 PR 直接推
   master 不会被验证。这是"只保留 PR 触发"的直接代价。
4. **`packaging-smoke` 不与 `product-smoke` 共享缓存**：它为显式 triple 单独构建，冷跑时
   多花约 9 分钟，但该作业不在关键路径上。

#### 关键路径（实测）

| | 改动前 | 改动后 |
| --- | --- | --- |
| 冷跑（每个新 PR 的第一次） | `Test workspace (windows-latest)` 53.6 分钟 | 约 20–28 分钟，关键路径 `Smoke product (windows-latest)` |
| 同一 PR 的后续 push | 53.6 分钟 | 约 20 分钟（该 PR 作用域有缓存） |
| 只改文档 | 53.6 分钟 | 约 16 秒，只跑 `fixtures` |

改动前的数字取自 PR #1055 的逐步实测。改动后的冷跑区间来自合并后两次真实运行
（20.1 与 28.6 分钟），差异来自 10 GB 缓存预算下哪些条目幸存——这正是残余风险第 2 条。
**不要再把缓存命中当作时间承诺**：只有同一 PR 内的后续 push 才是可预期的，那个场景实测
`Test workspace (windows)` 为 6.4 分钟。

### ADR-0075 · 磁盘映像的 Finder 窗口布局与卷图标

**状态：** 已接受（2026-09-29）

补充：ADR-0033 第 4 条（`.dmg` 由操作系统磁盘映像工具产出）

### 决策

#### 1. 布局由构建期写入的 `.DS_Store` 描述，不经过 Finder

窗口布局不在镜像的文件系统里。Finder 在打开卷时读取卷根的 `.DS_Store`，从里面取出窗口
大小、图标位置和图标视图设置；没有这个文件就用它自己的默认值。`create-dmg`（以及 Tauri）
的做法是挂载镜像后用 AppleScript 驱动 Finder 排窗口，Finder 顺手把 `.DS_Store` 写出来。

本项目不走这条路，理由是它在无人值守环境里不成立：

- AppleScript 控制 Finder 需要图形登录会话和一次"自动化控制 Finder"的授权；`create-dmg`
  自己为此提供 `--skip-jenkins`，Tauri 在 `CI=true` 时就会传它——也就是说 Tauri 的 CI
  产物本来就没有这套布局。
- 授权被拒绝时脚本中途失败，留下一张排了一半的镜像。
- 顺带一提，ADR-0033 已经记录了 `create-dmg` 在当前 macOS 上连挂载解析都做不完整。

因此 `crates/bongocat-packaging` 直接写卷根的 `.DS_Store`，写的就是 `create-dmg` 那份
`template.applescript` 会让 Finder 记下的内容：`bwsp`（窗口 bounds 与要隐藏的 Finder
部件）、`icvp`（图标视图设置，含图标与标签尺寸）、`vSrn`（视图设置版本），以及卷根里
每个条目一条 `Iloc`（图标坐标）。条目与坐标由打包工具给出——条目名必须是卷根里真的
存在的名字，否则坐标没有意义——窗口自身的数值（宽、原点、图标与标签尺寸）集中在
`finder_store::WindowLayout::default()`，条目坐标集中在 `build_disk_image` 里。

路径栏刻意不隐藏：它在窗口底部标出卷名与卷图标，Tauri 也没有隐藏它，而
`template.applescript` 里没有对应语句。

#### 2. `.DS_Store` 编码器由本仓库实现

`crates/bongocat-packaging/src/finder_store.rs` 写这个文件。格式是 buddy allocator 的
B-tree：4 字节文件头、32 字节 `Bud1` prelude、三个块（指向树根的目录块、记录全部落在
同一个 4096 字节叶页里的树、allocator 的块表）。`bwsp` 与 `icvp` 的值是 binary plist，
由 `plist` 写出。

调研过的两个第三方选择都没有采用：

- **`ds_parser =0.4.0`**（`.DS_Store` 解析与写入，类型化、带 HFS+ 往返测试）：许可证是
  BlueOak-1.0.0，`deny.toml` 白名单里没有，需要为此放开一条非 OSI 许可；解析器还会带进
  10 个新 crate，其中 `alias_record`（Alias 记录，也就是背景图的别名）本项目一个都用不到。
- **`cargo-codesign` 的 `ds_store` 模块**（MIT OR Apache-2.0，许可证无需放开）：它属于一个
  跨平台签名 CLI，会把 `ed25519-dalek`、`dialoguer`、`clap`、`toml` 等一起带进来，而本项目
  只需要其中一个模块的字节布局。

实际新增的依赖只有两个，且都已经在 `Cargo.lock` 里（`cargo-packager` 与其它 crate 已经
依赖它们），`Cargo.lock` 的净变化是 `bongocat-packaging` 多两行依赖名：

| 依赖 | 用途 | 替换边界 |
| --- | --- | --- |
| `plist =1.10.1` | 写 `bwsp` / `icvp` 的 binary plist | 只在 `finder_store` 内部使用，plist 类型不出现在打包工具的公共面 |
| `tempfile =3.27.0` | 卷的临时挂载点 | 只提供一个会被删除的目录 |

自研编码器的失败模式是有界的：`.DS_Store` 只影响 Finder 读到什么，不影响镜像里的文件，
所以格式写错的后果是安装器窗口退回 Finder 默认布局，而不是安装失败。

#### 3. 卷图标需要一次可写挂载，因此镜像先建可写再转码

Finder 的卷图标是卷根的 `.VolumeIcon.icns` 加上卷根上的一个文件属性。属性只能写在已挂载
的可写卷上，而压缩成 `ULMO` 的镜像挂载后是只读的，所以顺序变成：

```text
hdiutil create -srcfolder <staging> -format UDRW   # 可写
hdiutil attach -nobrowse -noverify -noautoopen -mountpoint <临时目录>
SetFile -a C <挂载点>                              # 卷使用 .VolumeIcon.icns
hdiutil detach
hdiutil convert -format ULMO -o <镜像>             # ADR-0033 选定的压缩格式
codesign
```

`SetFile` 由 macOS 自身在 `/usr/bin` 提供（`create-dmg` 用的是同一个工具），不依赖
Command Line Tools；缺失时构建直接失败，而不是悄悄少一个图标。`.VolumeIcon.icns` 是
`resources/icons/logo-macos.icns` 本身，用 `SetFile -c icnC` 标成图标文件，与
`create-dmg` 的做法一致。

挂载点放在系统临时目录，而不是输出目录：实测把可写卷挂在构建树里会让 macOS 往卷中写入
`.fseventsd`（filesystem-events 日志），产物就多出一个随"当时谁在看着 checkout"变化的隐藏
文件；挂到临时目录下三次实测都没有。挂载由一个守卫对象持有，任何提前返回或 panic 都会
卸载它，构建失败不会在开发者机器上留下已挂载卷。

#### 4. `icvp` 的键集必须与 Finder 自己写的完全一致

这一条是实测出来的：只写 `iconSize`、`textSize`、`arrangeBy` 等少数键时，Finder 打开
窗口仍然是 48 px 图标；把 Finder 自己写的那套键补齐（`backgroundType`、三个
`backgroundColor*`、`gridOffsetX/Y`、`gridSpacing` 也在内）之后，128 px 才生效。图标视图
的默认值显然不是"缺键时沿用已有值"，而是整条记录一起判定。

因此键集是对 Finder 的契约而不是偏好，`finder_store` 的单元测试按整份键名列表断言。

#### 5. 压缩格式、文件系统与文件系统事件都不再改动

`ULMO` 与 ADR-0033 的实测结论保持一致；文件系统仍是 `hdiutil` 的默认（APFS）；镜像里
不再额外删除 `.fseventsd`，因为第 3 条已经让它的来源消失。

### 影响

- **用户可见**：macOS 安装器窗口变成 Tauri 默认外观——660×400、128 px 图标、16 pt 标签、
  app 在左 `Applications` 在右、无工具栏与状态栏、标题栏与路径栏是应用图标。
- 镜像卷根多两个隐藏文件：`.DS_Store`（3,140 B）与 `.VolumeIcon.icns`（应用图标）。
  Finder 默认不显示它们。
- 构建过程多一次可写镜像与一次转码：磁盘上短暂多约 38 MB 未压缩镜像，`hdiutil convert`
  实测 1.6 s。
- 体积没有变大。同一 x64 bundle 实测 11,952,090 B，对照改动前一次成型的 11,990,768 B
  反而小 0.32%——两次调用压缩的差异来自转码路径本身。
- 编码器只在 macOS 上编译（`#[cfg(target_os = "macos")] mod finder_store;`）：磁盘镜像与
  `.DS_Store` 都是 macOS 专有产物，其他平台的打包不读也不写，而 workspace 的 `-D warnings`
  门禁不接受一个无人调用的模块。用 macOS 条件而不是 `#[cfg(unix)]`，是因为 Linux 同样没有
  磁盘镜像与 Finder，若按 Unix 放行会把这段 macOS 实现误判为可跨平台复用。
- 卷根从两个条目变成三个（新增修复命令，ADR-0076）：三个图标排成倒三角（`BongoCat.app`
  与 `Applications` 在上，修复命令居中在下），窗口 660×420、图标 96 px、标签 13 pt
  （Tauri 默认是 660×400、128 px、16 pt，两条目时更合适），卷里多一个可执行命令文件。

### 已接受的残余风险

1. **`.DS_Store` 是 Apple 未公开的格式**，Apple 可以随时更改。后果限于安装器窗口的外观：
   写坏了 Finder 会忽略这个文件并用自己的默认值，安装本身（拖拽 `.app` 到
   `Applications`）不受影响。ADR-0033 的退出条件不变——`cargo-packager` 换到能在当前
   macOS 上工作的 `create-dmg` revision 后，这段代码连同本 ADR 的第 1、3 条一起删除。
2. **单元测试证明的是字节，不是 Finder 兼容性。** `finder_store` 的测试用与写入器对称的
   读取器读回自己的输出，binary plist 由 `plist` 自己解码；Finder 侧是实测（见验证），
   两者不能互相替代。
3. **macOS 27 对 `hdiutil` 的这些调用打印弃用警告**，`hdiutil imageinfo` 也是。它们仍然
   工作，但换 `diskutil` 是一个独立的工具链决定，不夹带在本次外观改动里。
4. **`SetFile` 必须在宿主 macOS 上存在**。它随系统发布在 `/usr/bin`，GitHub 的
   `macos-latest` runner 与开发者机器都有；缺失时构建失败并指名这个工具。
5. **两台机器同时 `just build` 会互相破坏**（共享 `target/package/dmg-stage`）。这是
   本仓库打包入口的既有性质，本次没有改变，也不由本 ADR 授权修复。

### ADR-0076 · 磁盘映像里的修复命令

**状态：** 已接受（2026-09-29）

补充：ADR-0075（磁盘映像的 Finder 窗口布局）、ADR-0033 第 4 条

### 决策

#### 1. 随磁盘映像附带一个命令脚本，双击在终端里跑

安装器窗口第三项是 `Fix Damaged App`（ADR-0075 的倒三角排布）。它是一个带 shebang
且可执行的命令脚本，没有扩展名：

- **名字说的是症状，不是产品。** 遇到这个故障的人，弹窗里读到的是「已损坏」，他
  要找的就是"哪个东西能解决我看到的这句话"。`Fix Damaged App` 16 个字符，在 660 pt
  宽的窗口里与 `Applications` 并排也不换行；`App` 这个名词把它和"磁盘损坏"分开，
  而它正躺在一个磁盘映像上。`Repair BongoCat` 也不错，但更像"维修产品"而不是故障
  出口。脚本在终端里打印的第一行用同样的措辞，让人确认双击的正是窗口里那个条目。

- **在终端里跑**是刻意的。`sudo` 的密码提示读的是终端的 tty，双击打开 Terminal 是
  macOS 自带的机制，不需要任何授权弹窗，也不需要产品能启动——而产品的安装损坏正是
  这个工具要处理的前提。
- **没有扩展名**。macOS 依据 shebang 与可执行位在 Terminal 里运行它；扩展名只会
  在窗口标签里多占一行。Finder 的「隐藏扩展名」是按文件的属性位，而 icon 视图不认它
  （实测：位设上了，标签仍显示扩展名）。
- **打开应用**作为修好的信号，与参考脚本一致。失败时只打印手动命令，不去启动一个
  必然报错的程序。
- **回车即收尾**，脚本随后关掉自己所在的窗口（见第 3 节）。终端窗口是这个工具唯一的
  界面，一个关不掉的窗口会让它看起来像坏了。

#### 2. 只移除隔离属性，不做别的

脚本按顺序做三件事，每一件都有理由：

1. 未安装到 `/Applications` 就说明先安装，退出。
2. **先读属性再决定要不要问密码**：`xattr -p com.apple.quarantine` 是用户本来就有的
   读权限。没有隔离属性时直接告知，退出，不弹密码框。
3. 依次尝试递归形式与普通形式，成功后打开应用。

第 2 条的文案来自实测出的第一版问题：原来只有一句「没有隔离属性，无需修复」。当
`/Applications` 里留着一份旧的、未被隔离的安装时（遇到的正是这种情况：一份几个月前装的
旧版本），双击修复命令的人看到的却是「无需修复」，而他真正没做的事是**从这个映像安装**。
所以现在这一条同时说清两种可能并给出下一步：

- 未安装：`BongoCat.app is not in /Applications yet.` ＋「先把这个映像里的
  `BongoCat.app` 拖进 Applications」。
- 已安装但没有隔离属性：说明「要么这份已经修好了，要么它不是这个映像里的那份」，并让
  人先把映像里的 `BongoCat.app` 拖到 Applications 上覆盖掉现在这份再重跑。

脚本无法分辨这两种情况——`/Applications` 里那份是不是从当前映像来的，它没有依据判断，
而把一种说成另一种比把两种都说清更糟。

明确**不做**的两件事：

- **不关闭 Gatekeeper**。`spctl --master-disable` 是把系统级安全检查对所有应用关闭，
  而这里的故障原因只有隔离属性一个，不需要动系统设置。
- **不重新签名**。`codesign --force --deep --sign -` 修的是另一种故障（安装被中断、
  签名与内容不再一致）。签名校验失败时脚本把它作为**手动命令**打印出来，而不是悄悄
  改写已安装的文件——工具改变的东西越少，它给出的结论就越可信。

#### 3. 脚本自己关掉它所在的窗口

窗口是这里唯一的界面，所以提示里写了「Press Enter to close this window」，就得真能关。
Terminal 的默认行为是命令结束后**保留**窗口（显示 `[进程已结束]`），而「shell 退出时
关闭窗口」是 Terminal 配置文件里的一个选项、默认关闭，脚本不能改读者的配置，于是自己关：

- 只在 `TERM_PROGRAM` 是 `Apple_Terminal` 时尝试，也就是双击打开的那个 Terminal。从
  iTerm 等终端里运行时既不关窗口，提示语也换成 `Press Enter when you are done...`，
  不会去启动一个 Terminal 再关掉别人的窗口。
- 窗口按 **tty ＋ 标题**双重匹配：tty 是这个 shell 的，标题里含这个文件名。双击时
  Terminal 专开一个窗口跑本文件，脚本退出后标题仍保留文件名前缀；手动在自己的窗口里
  跑时标题会变回 `-zsh`，因此不会误关读者正在用的 shell 窗口。
- 关闭动作**延迟一秒**再发出：Terminal 在关闭一个仍有进程运行的窗口前会弹确认框，而
  那一刻本脚本自己就是那个进程。实测立即关闭会弹「是否终止正在运行的进程？」，延迟
  关闭则窗口直接消失、无弹窗。

#### 4. 脚本是 shell 逻辑，这是本决策里唯一的一处例外

仓库对自有代码的要求是 Rust。修复逻辑没有做成产品的一个 run mode，而是留在脚本里，
原因是具体的：

- 它要修的正是**产品装不起来**的情况。让一个可能已经跑不起来的二进制去执行修复，
  逻辑上就自相矛盾。
- 它只用到 macOS 自带的 `sudo`、`xattr`、`open` 与 shell，没有产品代码可以复用的东西。
- 它是随安装包分发的**用户操作脚本**，不是构建逻辑：`AGENTS` §9.2 禁止的是把平台
  判断、目录复制、bundle 组装重新实现到 `Justfile`／CI／自维护脚本里，而这里没有任何
  东西进入构建流程。

代价是这部分行为不在 `just check` 的覆盖内，因此它的每一条分支都用真实 `xattr` 加
PATH 上的 `sudo`／`xattr`／`open` 替身逐条验证（见「验证」）。

#### 4. 文案是英文

安装器窗口里另外两项也是英文名（`BongoCat.app`、`Applications`），第三个条目跟随它们，
脚本内的提示同样是英文。`LANG` 不参与判断：这个脚本没有用户可见的本地化面需要跟随
系统语言，只有 Terminal 里的几行提示。

### 影响

- macOS 磁盘映像卷根从两个可见条目变成三个：`BongoCat.app`、`Applications`、
  `Fix Damaged App`（可执行命令文件，约 3.1 KB）。`ULMO` 压缩后的镜像体积变化在测量
  噪声内。
- 用户可见的新能力：安装后打不开时，双击第三个条目、输入密码、修好，应用自动打开，
  窗口随后自己关闭。
- 修复命令目前使用系统自带的 shell 脚本图标；自定义图标是后续项（见「后续」）。
- Windows 不受影响：磁盘映像是 macOS 专属产物，NSIS 安装器里没有对应条目。

### 已接受的残余风险

1. **脚本不经 `just check` 覆盖行为**。`just check` 现在会跑 `bash -n`（`format!` 里的
   引号错误会被它挡住），但除解析之外的行为只能靠实跑验证（见「验证」）。
2. **它只对 macOS 的隔离属性有效**。签名损坏、文件缺失、用户数据损坏都不在范围内，
   脚本会把「签名校验失败」和对应的手动命令打印出来，仅此而已。
3. **图标未定**。当前显示为系统的 shell 脚本图标。
4. **`sudo` 一次可能提示两次**：第一条命令被拒后第二条会再要一次密码，而 `sudo` 的
   凭据缓存通常让第二次不再提示。无法在无人值守环境里验证这一条。
5. **关窗口这一步依赖 Terminal 的 AppleScript**：只影响提示里承诺的收尾动作，失败时
   窗口留在原地（内容完整，读者可以自行关掉）。非 Apple_Terminal 的终端不尝试，
   提示语也随之换成不承诺关闭的版本。

### 后续

1. 为 `Fix Damaged App` 提供自定义图标（`Icon\r` 或 bundle 图标；实测 APFS 上的普通
   文件不认 resource fork 图标，HFS+ 卷可以——见 ADR-0075 的文件系统讨论）。
2. 未来发行签名与公证落地后（ADR-0033 的发布门禁），评估这个条目是否还有存在的必要。

---

# 三、模型 / Live2D / 渲染

> 模型导入/存储、Live2D 播放与渲染、行为调度与帧校验。

### ADR-0036 · 模型导入的压缩包来源与解压边界

**状态：** 已接受（2026-09-16）→ **已撤回（2026-09-22）**

依赖：ADR-0030（先复用既有方案）、ADR-0027（诊断包已使用 `zip` 写入器）、ADR-0011（渐进实现与发布门禁）

> 本文是已撤回的历史设计记录；当前 `ModelStore`、Mver 转换和用户侧封面 writer 位于
> `bongocat-model-store`，只读包解析与 `PreparedModel` 仍位于 `bongocat-model`。

### 撤回（2026-09-22）

维护者决定**先删掉压缩包来源的实现**，恢复时另行确认。以下内容随之移除：

- `crates/bongocat-model/src/archive.rs` 整个模块（来源识别、`plan_archive`、`extract_archive`、
  条目名规范化、包装目录剥离、文件管理器元数据过滤）；
- `ModelStore` 的归档分支（`import_with_observer`、`inspect_source`、`import_mver_with_observer`）、
  `MverSource::Archive` 变体；
- `ModelStoreDiagnostic::SourceArchiveUnsupported`（`ALL` 由 13 项回到 12 项），以及它在
  `bongocat-app` 两个映射函数与两处穷尽性测试里的分支；
- `ModelPackageLimits::maximum_archive_bytes`；
- `bongocat-model` 对 `zip`（`deflate-flate2`）与 `flate2` 的依赖；`Cargo.lock` 里 `zip 8.6.0`
  因此也不再携带 `flate2`；
- 相关测试：`bongocat-model` 的 11 个归档用例与 `ArchiveBuilder`/`write_archive_tree`/
  `write_package_archive` 测试辅助、`mver.rs` 的归档转换用例、`bongocat-app` 的
  `application_imports_folder_and_archive_sources_of_the_same_model` 与 `archive_fixture` 辅助；
- `examples/model_conversion_smoke.rs` 的 `--source <folder|archive>` 收窄为 `<folder>`。

**本文的全部决策保留**，作为恢复该功能时的设计输入——尤其是决策 1（按内容而非扩展名识别）、
决策 2（压缩包不是第二个解析器，共用 `PreparedModel` 校验）、以及解压前按中央目录校验的安全边界。
**恢复前先与维护者确认**：压缩包上传需要一组本次未做的新功能，不是把代码加回来那么简单。

`bongocat-ui-protocol::model_source_display_name` 的「去掉归档扩展名」规则**保留**：settings service 的
兜底标题仍与它共用，且删掉它会让恢复时的那条契约失去单一实现；调用方负责提供来源是否为目录。

同日更早的两次 UI 修订也一并成为历史记录，保留如下：决策 10 的「一个来源一个按钮」先被单一上传卡片
取代（依据是 `rfd 0.17.2` 新增的 `pick_file_or_folder`——`NSOpenPanel` 可以在同一次调用里既接受
文件又接受目录，决策 10 当时「原生面板没有统一形态」的前提在 macOS 上不再成立；
`pick_model_directory`/`pick_model_archive` 收敛为 `pick_model_source`，
`validate_selected_directory`/`validate_selected_archive` 收敛为 `validate_selected_source`），
随后入口再收窄为只接受文件夹的 `pick_model_folder` + `validate_selected_folder`。这两步的产物
（`pick_model_source`、`validate_selected_source`）现已随本次撤回一并移除，只留下
`pick_model_folder`。

### 原始决策（已撤回，保留作设计输入）

### 决策

#### 1. 来源类型由内容识别，不由扩展名或调用方标志决定

`detect_source_kind` 只看文件系统类型与文件头：目录 → 目录来源；常规文件且以 `PK\x03\x04` /
`PK\x05\x06` / `PK\x07\x08` 开头 → 压缩包来源；其余 → `model_store_source_archive_unsupported`。

因此：归档被改名（`model.package`）仍能导入；叫 `looks-like.zip` 的**目录**仍按目录处理；`.rar`
或损坏下载得到的是一个稳定诊断，而不是"先解压失败、再报包无效"。用户不需要告诉产品他选的是
什么，产品也不需要靠字符串猜。

#### 2. 压缩包不是第二个解析器

压缩包来源与目录来源**共用同一套校验**：解压写满 staging 之后，仍然调用同一个
`PreparedModel::prepare`。压缩包只是"把同样的字节放进 staging 的另一种方式"，因此不可能绕过
路径规范化、符号链接拒绝、JSON/PNG/FLAC 侧车校验、纹理尺寸上限或包级上限。

这是本 ADR 最重要的一条：任何"为压缩包单独写一遍检查"的做法都会在两侧漂移。

#### 3. 解压写进 store 自己的 staging 目录

解压目标就是 `create_staging_directory` 建好的 `.importing-<id>-<pid>-<seq>`，不引入第二个临时
目录、不复用系统 temp。由此得到的性质都是既有的，不需要新机制：

- 提交仍是**一次 `rename`**；失败时 `StagingCleanup` 删掉整个 staging，store 根目录不留痕迹。
- 权限仍是 store 的 owner-only（目录 0700、文件 0600），解压出来的字节不会经过一个"更宽松的
  中间位置"。
- 配置里没有指向临时路径的引用，取消与崩溃恢复沿用既有 `recover_abandoned_operations`。

#### 4. 先校验、后解压：plan → extract 两遍

`plan_archive` 只读中央目录（`by_index_raw`，不解压、不解密），在这一遍里完成：条目名规范化、
条目类型、压缩方法、加密标志、条目数、深度、单文件与整包声明字节。`extract_archive` 才真正
解压。

这样做的直接收益是**压缩炸弹在还只是一个头部数字的时候就被拒绝**：恶意归档不需要被解压一个
字节就会失败，store 也不会先建好 staging 再报错。

代价是文件被读两遍，以及两遍之间归档可能被替换。后者由第二遍的**逐条目复核**处理：索引位置的
条目名、声明大小、`is_file` 必须与 plan 一致，实际写出的字节数必须等于声明值，否则
`model_store_source_changed`。`zip` 默认校验 CRC32，损坏的载荷因此也不会被当作成功。

#### 5. 包装目录剥离

归档一个文件夹必然给每个条目加一层前缀，所以"压缩这个目录"永远是 `X/...`。保留这一层会让入口
发现失败（`model_entry_missing`），因此 `strip_wrapper_directories` 把它去掉，并且**反复**去掉
（`a/b/...` 这种嵌套压缩同样常见），直到剩下的条目不再共享同一个首段。

判定规则是被刻意收窄的：

- 只有当**所有**条目都以同一个首段开头时才剥离；
- 任何一个**文件**直接位于归档根就取消剥离（根目录本身就是包）；
- 一个**目录**条目位于根且等于该首段时不取消剥离——那正是包装目录在声明自己。

最后一条是必要的：`X/` 这个目录条目在每个"压缩文件夹"产物里都存在，如果它取消了剥离，功能
在真实归档上完全不工作。这个缺陷在开发中真实出现过，由
`wrapper_prefix_requires_one_directory_shared_by_every_entry` 固定。

#### 6. 归档工具元数据被丢弃

`__MACOSX/**` 与 `._*` / `.DS_Store` / `.localized` / `Thumbs.db` / `desktop.ini` 不参与包：
它们既不参与包装目录判定，也不被解压。理由与 `ModelStore::list` 忽略同样文件的理由一致——它们
属于文件管理器，不属于模型。副作用是 `__MACOSX/` 与真正的包目录并列时，包装目录仍能被正确
识别（否则这一常见形态会直接导入失败）。

如果一个模型真的引用了被丢弃的 `._*` 文件，它会得到"资源缺失"，这是诚实的结果：包确实不完整。

#### 7. 上限：给"容器"一个自己的界

`ModelPackageLimits` 新增 `maximum_archive_bytes`（默认 1 GiB，与 `maximum_package_bytes` 同值）。

不能复用 `maximum_package_bytes`：那条上限约束的是归档**产出**的包，在归档读取器解析中央目录
**之前**就要用它挡住输入；而"用 `Stored` 存一个 8 字节包上限的归档"本身可能远大于 8 字节。
把两者合并会让包字节上限对归档来源永不可达。也不能复用 `maximum_file_bytes`，否则单文件上限
同样变得不可达。

此外有一个独立的**结构上限**：条目数超过 `maximum_file_count × 4` 直接拒绝。乘数给目录条目与
被丢弃的元数据条目留出余量，同时保证"声明海量条目"的输入不会进入逐条目循环。

#### 8. 诊断码：新增一个，复用其余

新增 `ModelStoreDiagnostic::SourceArchiveUnsupported`（`model_store_source_archive_unsupported`），
只在"这个文件根本不是可用归档"时使用：无法作为 zip 打开、加密、不支持的压缩方法、条目类型
既非文件也非目录、容器超出 `maximum_archive_bytes`、无条目、只有目录没有文件。

其余情形刻意复用既有码，因为它们描述的是同一件事：

| 情形 | 码 |
| --- | --- |
| 条目路径绝对 / 穿越 / 平台前缀 / 重复 / 文件与目录冲突 | `SourceEntryUnsupported` |
| 条目是符号链接 | `SourceSymlinkUnsupported` |
| 条目类型是设备/FIFO 等 | `SourceEntryUnsupported` |
| 深度、条目数、单文件或整包字节超限 | `SourceChanged` |
| 实际字节数与声明不符、归档在两次读取之间变化 | `SourceChanged` |

`SourceChanged` 用于上限与声明不符，是因为目录来源在同一处也用它（"源不再匹配已校验的形态"），
并且对归档而言，声明值就是归档自己的元数据，不一致就是源自相矛盾。这保持了"用户可见结果是
一组稳定码"的收敛性，而不是为同一结果造第二套词汇。

settings 层把新码映射到既有 `SettingsErrorCode::ModelImportSourceUnsupported`
（"模型来源中包含不支持的文件"），因此没有新增用户可见错误码。

#### 9. 依赖：`zip` 的 `deflate-flate2`，并显式依赖 `flate2`

`zip =8.6.0` 已在 workspace 依赖里（诊断包用它写 `.zip`），但当时是 `default-features = false`
且只走 `Stored`，没有解压能力。真实模型归档用 deflate（两个样本的压缩方法都是 8），因此打开
`features = ["deflate-flate2"]`。

`zip` 对 `flate2` 声明的是 `default-features = false`，自己不选后端；而 `flate2` 一个后端都没
选中时以 `compile_error!` 编译失败。因此 `bongocat-model` **直接**依赖 workspace 的
`flate2 =1.1.10`，让 `rust_backend`（miniz_oxide，纯 Rust）成为本 crate 在任何构建图里的确定
事实，而不是依赖"某个兄弟 crate 恰好打开了默认 feature"——那只在测试构建里成立。

其余 feature 一律不开（无 AES、无 bzip2/zstd/lzma/ppmd/deflate64）：模型归档不需要它们，而未
开启的压缩方法会以稳定诊断被拒绝（这也是"加密归档"的拒绝路径）。按 ADR-0030 与 `AGENTS` §9，
两个 crate 的版本都在当次用 `cargo info` 核对了 crates.io 最新非 yanked 稳定版：`zip` 8.6.0
（`9.0.0-pre3` 是预发布，不采用）、`flate2` 1.1.10。

#### 10. UI：一个来源一个按钮（已撤回，见文首）

原生面板没有"选文件夹或文件"的统一形态（macOS 是 `pick_folder` 与 `pick_file` 两个不同面板，
Windows 的 common item dialog 同样区分），所以 Models 页面提供两个按钮：**选择文件夹**与
**选择压缩包**，各自的 tab index、焦点句柄与 AccessKit 节点齐备。

不做的替代方案：一个按钮加一个模式开关（用户要猜当前是什么模式）、或一个按钮先弹选择再弹面板
（多一步且同样要说明）。**由按钮决定来源种类，由字节决定它究竟是什么**。

压缩包按钮的对话框只过滤 `.zip` 作为便利，**不作为判据**：`validate_selected_archive` 只要求
"绝对路径 + 真实常规文件"，因此被改名的归档仍可通过，而"不是归档"由 store 报稳定码——对话框
层拒绝一个能用的归档，会把可导入的文件变成对话框级失败。

状态文案按所选来源区分（`models.import.archive.*`），不再把"压缩包"说成"文件夹"。挑选过程本身
与来源无关的三条文案（取消、取消后保留上次选择、选择器必须在 UI 线程）上移到
`models.import.picker.*`，因为它们描述的是选择器而不是文件夹。

#### 11. picker 模块与稳定码改名

`bongocat-platform` 的 `directory_picker` 模块改名为 `model_source_picker`，
`DirectoryPickerOutcome` / `DirectoryPickerError` 改为 `ModelSourcePickerOutcome` /
`ModelSourcePickerError`，稳定码前缀由 `directory_picker_*` 改为 `model_source_picker_*`，
示例改名为 `examples/model_source_picker_smoke.rs` 并新增 `--kind directory|archive`。

理由是这次改动**引入**了一处不准确：让"压缩包选择器"返回 `DirectoryPickerError` 是错的。与之
相邻的既有模块名一并修正，代价是 4 个文件里的机械替换与一次示例改名。

#### 12. `zip` 归档内容与第三方模型不进入仓库

两个真实样本是第三方模型（含 Live2D 模型数据），不提交到仓库。真实归档的验证改为一个由环境
变量驱动、未设置即跳过的测试：
`imports_the_archive_samples_named_by_the_environment`（`BONGOCAT_MODEL_ARCHIVE_SAMPLES` 指向一个
放 `.zip` 的目录）。仓库内的永久测试用测试期生成的归档（真 deflate 流）覆盖结构与拒绝面。

### 明确不做

- **`.rar` / `.7z` / `.tar.gz`**：只有 zip 被真实需求驱动。新增格式需要各自的边界分析（tar 的
  符号链接与硬链接、gzip 的单流特性、7z/rar 的许可证），不在本 ADR 范围。
- **加密归档**：没有密码输入面，也没有需求。加密条目被拒绝。
- **新增"解压中"进度阶段**：typed 操作契约的 stage 集合（prepare/copy/validate/commit）不变，
  解压计入 `Copying` ——它描述的是"把来源字节物化进 staging"，对归档而言就是解压。新增阶段会
  让 model/ui/app 三处的单调性契约与测试同时改，收益只是一个更细的动词，不值得。
- **把归档解压到已安装目录**：本产品不允许任何就地更新语义；导入永远是"新 id + 新目录"。
- **压缩包内的多模型选择**：一个归档 = 一个模型，仍是入口发现规则（0 个 → 缺失，≥2 个 → 歧义）。

### ADR-0037 · 在应用内导入 BongoCatMver 模型

**状态：** 已接受（2026-09-17）

依赖：ADR-0030（先复用既有方案）、ADR-0036（模型来源按内容识别、压缩包与目录共用校验；
**该 ADR 已于 2026-09-22 撤回**）、ADR-0011（渐进实现与发布门禁）、ADR-0060（model/store 边界）、
ADR-0065（模型模式元数据与卡片 Badge）

修订（2026-09-22）：ADR-0036 撤回后，本文**决策 4「归档来源不解压」**连同它描述的
`ArchivePlan` / `MverSource::Archive` 一并失去实现——Mver 源现在只从用户选中的文件夹就地读取，
`MverSource` 退化为一个持有已 canonicalize 根路径的 struct。其余决策（转换边界、多模型产出、
键位图合成、重编码、诊断码）**全部不变**。归档侧的读取上限逻辑（用声明大小在解压前判定）随实现
删除，但 `LEGACY_RESOURCE_MAXIMUM_BYTES` 仍然约束目录侧的单次读入。

### 决策

#### 1. 检测仍按内容识别，且要求两条独立证据

`ModelStore::inspect_source` 复用 ADR-0036 的 `detect_source_kind`（目录 / zip 签名），然后问该
来源是否带 Mver 键位表。认定条件有两条，缺一不可：

* 根 `config.json` 存在且能解析成 legacy 的 section 形状；
* 这些 section 里至少有一个模式真的在 `<资源根>/<模式>/cat_model/` 下**恰好**有一个
  `.model3.json`。

第二条让误判几乎不可能：转换后的 BongoCat 包把入口放在包根，不在 `cat_model/` 下，两者从外部
无法混淆。`config.json` 读不到、或解析失败、或找不到任何带模型的模式，一律回退到普通包导入，
由那条路径给出它自己的诊断——检测是**试探性**的，不负责报错。

资源根优先取 `img/`（Mver 应用的实际布局），不存在时取源根本身，以容纳把模式文件夹直接摊在根上
的模型包。

修订（2026-09-25）：每个 Mver 转换条目现在在 `imported_models[].input_mode` 中持久化
实际选中的 `standard`/`keyboard`/`gamepad` source section；该字段是模式事实，标题后缀只保留
既有显示兼容语义。普通模型包则由 ModelStore 在 staging 提交前按 `left-keys`/`right-keys` 资源
判定模式，判定失败直接拒绝导入。详情见 ADR-0065。

修订（2026-10-02，issue #1080）：「能解析成 legacy 的 section 形状」现在按该应用自己的读取端
判定，不再按严格 JSON 判定。固定 commit 的 `CatUILauncher/data.cpp` 用 JsonCpp
`CharReaderBuilder` 读 `config.json`，而其 `setDefaults()` 明确设置 `allowComments = true`
（`jsoncpp.cpp` 的 `CharReaderBuilderDefaults`），因此 `//` 与 `/* */` 注释是这个格式接受的内容。
带注释的键位表此前被严格读取拒绝，而检测是试探性的、拒绝不报错——文件夹被当成普通包，
再以「模型包无效」结束，用户拿到的解释与模型本身无关。读取前先剔除字符串字面量之外的注释：
字符串里的 `/` 是数据，`\"` 不结束字面量，块注释保留换行以免错误位置漂移，多字节字符不被
拆分。放宽只到注释为止——不是 config 的文件仍然不是 config。

同一次修订：转换逐字节复制模式的 Live2D 包这一约定，增加一个唯一例外。issue #1080 的真实模型
在 `keyboard` 模式的 model3.json 里声明了 `Sound: live2d_motion1.flac`，而整个包里没有任何
flac 文件——该模式是从 Cubism 官方 `demomodel2` 示例包改装的，留下了示例的音频名，Mver 实际
从自己的每模式 `sounds/` 列表播放音效。保留这个名字会让整个模式过不了包校验（ADR-0060 的
引用必须解析），丢掉它不损失任何东西。因此沿用决策 5 的既有判断（"少一张键位图不等于这个模型
不可用"），转换时以源实际携带的文件集合判定悬空并丢弃该 `Sound` 键，其余内容保持不变。**只
丢弃缺失的引用**：无法规范化的引用（越出包根、绝对路径）保持原样，交由包校验报告，不因为
转换而抹掉一条路径安全发现；没有悬空引用时 entry 逐字节不变。这条修订只在 Mver 转换路径
生效，普通包导入的引用校验不变严格。

#### 2. 一个源产出多个模型，每个模型独立安装

每个模式转换成一个 BongoCat 包，各自分配一个随机 UUID v4 存储键，各自写一条
`imported_models` 元数据记录和对应的 `input_mode`，因此三种模式成为模型列表里三个可以独立
启用、改名、删除的普通条目。模式值来自本次导入选择的 legacy section，不从转换后目录形状
或标题反推。

**逐模式提交**，不做整体回滚：某个模式的键位图损坏时，已经转换成功的模式保留下来。理由是这些
模型彼此独立，把已经转换成功的模式一起丢弃比留下一个用户可重试的部分结果更糟。

标题是「用户标题 · 本地化模式名」（`我的猫 · 标准模式`），即社区导出模型已经在用的命名。模式名
在**截断之后再拼接**：模式名是三个模型之间唯一的区别，长来源名不能把它挤掉，所以先按标题上限
减去后缀长度裁剪来源名。

#### 3. 转换写进 store 自己的 staging，与包来源共用提交尾部

`ModelStore` 的导入尾部（`PreparedModel::prepare` 校验 + 单次 `rename` 提交 + 失败清理 staging）
被抽成 `commit_installed_staging`，目录复制、归档解压与 legacy 转换三条路径共用它。因此转换
结果不可能绕过任何包校验，也不存在第二个临时位置：失败的转换不留痕，成功的转换仍是一次原子
rename。

#### 4. 归档来源不解压（**已随 ADR-0036 撤回，见文首**）

目录来源就地读取；归档来源**只按需读取被命名的条目**（`ArchivePlan::read_file`，仍逐条目复核
名字/声明大小/实际字节数）。导入一个 Mver `.zip` 不会把整个应用——exe、DLL 和用不到的模式——
解压到磁盘上，只把转换真正需要的那几十个文件读进内存。

读取有独立上限 `LEGACY_RESOURCE_MAXIMUM_BYTES`（64 MiB）：它约束的是转换期临时读入的量，与
约束"可被安装的包"的 `ModelPackageLimits::maximum_file_bytes` 是两件事。归档侧在解压前用声明
大小判定，因此超限只花一次头部读取。

#### 5. 键位图合成 = 最小画布上的 Porter-Duff over

合成画布取两层中较小的宽高，两层都锚在原点绘制，因此比画布大的一层被裁切而不是缩放——这与
canvas `drawImage` 的行为一致。合成用整数形式的 `over` 算子：不透明来源整体替换、全透明来源
不改变，只有抗锯齿边缘像素会四舍五入一次（浮点形式在此处最多每通道差 1 档）。

模式里根本没有 `keyboard/` 图集时，不合成，直接按字节安装 paw 图（复古模型可能已经把两层画在
一起了）。某个绑定的 paw 或配套键帽缺失时该绑定被跳过而不是让整次转换失败——与参考实现一致，
且"少一张键位图"不等于"这个模型不可用"。

#### 6. 输出图使用 lossless 重编码，`oxipng` 是唯一选择

`image` 的 PNG 编码器本身不做滤波与熵编码优化，直接用它的输出会比源文件更大。生成的合成图
交给 `oxipng 10.2.1` 做**无损**重编码：位深/颜色类型/调色板/灰度缩减都保持解码后像素不变，
`optimize_alpha` 只改写全透明像素的颜色通道（肉眼不可见），交错被关闭。

测得的取舍（样本 15 张 612×354 合成图，release 构建）：

| 后端 | 总字节 | 用时 |
| --- | --- | --- |
| 仅 `image` 编码 | 160 641 | — |
| oxipng + libdeflate | 91 470（−43%） | 0.68 s |
| oxipng + Zopfli | 86 605（−46%） | 10.25 s |

Zopfli 多 5% 换 15 倍时间，对一次交互式导入不划算，因此只开 `oxipng` 的库入口（`zopfli`、
`parallel`、`binary` 三个 feature 全关）。作为对照，仓库内既有预置模型的键位图是 6.7–9.5 KB，
本次转换产出的同类文件是 4.1–8.4 KB，量级一致。

**明确排除 `imagequant`**（pngquant 的 Rust 绑定，TinyPNG 式有损量化）：它的许可证是
`GPL-3.0-or-later`（pngquant.org 的商业授权另售），与本项目 Apache-2.0 的 `deny.toml` 白名单冲突。
用户要的"保持尺寸与视觉质量基本不变"由无损重编码完全满足，不必引入许可证风险。

重编码失败时写回普通编码结果而不是让转换失败：文件在两种情况下都是合法 PNG，只有体积有差别。

#### 7. 键位名必须落在产品自己的词汇表里

转换输出的文件名是 `bongocat-live2d-render` 从 HID usage 解析出的名字（`KeyA`、`Return`、
`BackQuote`、`ControlLeft`/`Control`……），不是第二套命名。一处刻意偏离参考实现的写法：

* `0x08`：参考表写作 `BackSpace`，产品运行时与预置模型都写作 `Backspace`，采用产品写法。

`gamepad` 使用 XInput 按钮序号映射到产品自己的十六个按钮名（`GamepadButton::key_image_name`），
不是映射到任何 backend 的控制名。XInput 顺序是 0–3 面部键、4/5 肩键、6/7 模拟扳机、8/9 菜单键、
10/11 摇杆键、12–15 方向键（上/下/左/右）。

> **修订（ADR-0070）**：本节原先把 `gamepad` 的输出名定义为"预置 gamepad 模型已经装载的名字"，
> 那套名字是旧 Tauri 输入层从第三方 backend 派生的（`DPad*`、`LeftTrigger` = 肩键、
> `LeftTrigger2` = 模拟扳机、`LeftThumb`/`RightThumb` = 摇杆键），并且当时的序号表有六个条目按错
> 位置。预置模型已按产品词表重命名，序号表已按真实按钮顺序重写，转换结果与
> `resources/models/gamepad/{left-keys,right-keys}` 的文件集合**逐个同名**这一条仍然成立。

无法命名的控制码（鼠标按键、产品没有 overlay 的键）不产出图片，也不报错：它本来就没有可显示的
键位图，产出 `undefined.png` 之类的东西比不产出更糟。

#### 8. 诊断码：新增一个 store 码，复用既有用户可见码

新增 `ModelStoreDiagnostic::SourceConversionFailed`
（`model_store_source_conversion_failed`，`ALL` 由 12 增至 13），只用于"已经认出这是
BongoCatMver 源，但无法把它变成 BongoCat 包"：键位图不是可读 PNG、合成图无法编码、绑定引用的
资源超出读取上限、请求的模式不在此源中。

它刻意与 `InvalidPackage` 区分：后者是"按包读入并且包校验失败"，前者从未成为包。settings 层
把它映射到既有 `SettingsErrorCode::ModelImportSourceUnsupported`（沿用 ADR-0036 §8 的做法），
因此没有新增用户可见错误码；具体原因留在 `detail`/`resource` 里，进入日志与诊断包。

#### 9. 进度跨模型保持单调

settings 的 `report_progress` 会丢弃回退的更新，而 store 每个模型都从 `Preparing` 与 0 重新
开始。`Application` 因此在跨模型处折叠进度：累计已完成模型的文件数与字节数、并对 stage 取
最大值后再上报。于是用户看到的是一个不倒退、且最终等于三个模型总和的一条序列，而不是卡在
第一个模型的终值上。

#### 10. 模式选择与卡片投影

Mver 导入仍由服务层执行阻塞文件与模型解析；用户在检查来源后通过现有模式选择对话框选择要转换
的 section。选择的每个 section 生成一个独立模型，并将该 section 写入 installed metadata 的
`input_mode`。Models 页面只消费 `SettingsModelEntry.input_mode`，不在 UI 线程重新检查资源或从
标题反解析模式。`Application::import_models*` 仍是唯一的导入入口。

#### 11. 按键图按字节安装，legacy 窗口坐标由渲染器重框（issue #1129）

决策 5 把按键图合成成"画布上的一层"，但合成只是把两层叠成一张图，它并不改变这张图的坐标系。
BongoCatMver 的 `mode98_live2d_standard.cpp` 用
`projection.Scale(l2d_correct, l2d_correct * window_size[0] / window_size[1])` 画模型，因此画布**居中**
落在 `window_size` 窗口里；而 `drawhand`/`drawkeyboard` 对 `hand/*.png` 只调用 `window.draw(sprite)`，
没有 `setPosition` 也没有 `setScale`，按键图是**窗口原点锚定、一图素对一窗口像素**的。按键图的尺寸
恰好等于画布在窗口里的尺寸（参考模型是 986×1000 对 1000×1000 窗口，官方样例是 612×354 对 612×352
默认窗口），所以两者之间差的正是窗口左右各留的那条边距
`(window_size[0] − 按键图宽) / 2`。

产品把按键图铺在 `ModelBounds::from_canvas` 得到的画布四边形上，于是按键图从**窗口**坐标系被当成
**画布**坐标系使用，每张按键图都整体偏右这条边距。实测（issue #1129 的白露模型，模板匹配模型自己的
键帽贴片）：G/R/E/W/Q 五个键的落点一致偏右 6.2–7.2 图素，与 `(1000 − 986) / 2 = 7` 吻合。

修法放在渲染器而不是转换器：`bongocat-render::legacy_key_overlay_bounds` 由按键图自身尺寸算出这条边距
（窗口宽取该图较大的一边——按键图就是画布在窗口里的尺寸，窗口不会比它窄，作者不按画布尺寸设窗口时
用的是包含它的正方形），双平台渲染器对每张按键图各建一个四边形缓冲。这样选而不是在转换时平移像素，
有三个理由：转换产出的字节除合成外保持"源的像素"，与决策 5/6 的契约一致；已经装在 store 里的模型
不需要重写资源即可正确显示（旧包没有记录 `window_size`，转换器事后无法还原这条边距）；出厂预置模型
与所有宽按键图模型算出 0 偏移，行为逐字节不变。

### 明确不做

- **普通包导入后的资源重判**：模式只在 staging 提交前判定一次并写入 metadata；卡片和 runtime
  不在每次渲染时重新扫描资源。
- **让模式 Badge 隐式切换 runtime 输入绑定**：`input_mode` 在 ADR-0065 中首先是导入/构建来源
  元数据；若要让 gamepad metadata 驱动 installed Mver 的实际输入，需要独立 runtime/overlay 任务。
- **鼠标按键 overlay**：`standard.mouse_left/right/side` 与 `mouse*.png` 不转换。产品当前没有
  鼠标按键的 overlay 通道（`InputControl::Mouse` 不产生 `KeyPress`），参考实现同样忽略它们。
- **`face/`、`sounds/`、`arm*.png`、`tablet*.png`**：参考实现不处理，产品格式里也没有对应位置；
  模式自带音效已经通过 `cat_model/` 里的 `Sound` 引用随包复制。修订（2026-10-02）：`Sound`
  引用指向源中不存在的文件时按决策 5 的判断丢弃该引用（issue #1080），`sounds/` 目录本身仍不处理。
- **有损量化 / 降分辨率 / 减帧**：用户明确要求不牺牲画质，且 GPL 有损库被许可证排除。
- **`.rar`/`.7z`**：只有 zip 被真实需求驱动，与 ADR-0036 的范围一致。
- **把 Mver 源就地"升级"成 BongoCat 模型**：本产品不做就地更新，导入永远是"新 id + 新目录"。
- **修改运行时让 F1–F12 逐键解析**：转换输出按 `F1`…`F12` 命名以保留模型自己的区分度，但
  `bongocat-live2d::key_name_candidates` 目前把功能键统一回退到 `Fn` 别名，因此逐键 F 图**暂时
  不可达**。这是运行时的既有缺口，不由本次改动引入；未改运行时是因为仓库内没有任何数据能验证
  这个改动（预置模型只有 `Fn.png`，真实样本的键位表也不含功能键），先记录为后续项。

### ADR-0047 · 模型库只做选择与元数据编辑

**状态：** 已接受（2026-09-18；2026-09-25 修订）

依赖：ADR-0036（模型导入的压缩包来源与解压边界）、ADR-0037（在应用内导入 BongoCatMver 模型）、ADR-0020（GPUI Kit 统一依赖入口）、ADR-0065（模型模式元数据与卡片 Badge）

修订（2026-09-25，任务导向导航）：ADR-0066 将模型对象拆为 Model library 与 Model behavior
两个独立一级页面。因此本 ADR 中“模型管理页只做选择与元数据编辑”的准确范围收窄为
**模型库页面**：导入卡片和模型卡片仍只负责选择、标题、封面、位置与删除，不恢复行为预览、
动作/表情目录或快捷键编辑。独立的 Model behavior 页面不改变模型卡片的职责边界。

修订（2026-09-22）：决策 1 与「不做拖拽导入」中描述的导入入口已变：导入卡片不再走
「选择文件夹 / 选择 .zip」两个按钮加标题输入，而是按下即打开一个文件夹选择器并自动开始导入
（ADR-0036 文首修订）。**入口只接受文件夹**：压缩包上传本次不做，选择器与文案都不再提及压缩包。
当时“不做拖拽导入”的结论仅适用于本修订之前的实现；窗口级拖放入口由 2026-09-24 修订取代。
本 ADR 关于页面只做选择与元数据编辑的决策未变。

修订（2026-09-24，窗口级文件拖放）：设置窗口根视图现在接收 GPUI `ExternalPaths`，拖入任意页面都
显示覆盖整个窗口的导入蒙层。一次只接受一个文件夹；拖放路径先在 background executor 复验并
canonicalize，再进入既有的来源检查、转换选择、导入和封面截取流程。蒙层自身也注册 drag/drop handler，
避免渲染后遮住根视图导致真实松手失效；离开窗口则由 paint-phase 的 window listener 捕获
`FileDropEvent::Exited`，不依赖已不再 hovered 的 hitbox。正常、忙碌和多项目选择分别显示可操作、无等待和拒绝提示；
拒绝使用稳定 `SettingsErrorCode::ModelImportDropInvalid` 通知，不把路径带入 UI 文案或日志。

修订（2026-09-22，卡片编辑改为原地）：决策 1 里卡片的「编辑」形态已变。编辑器不再把封面按钮、
标题输入框和保存/取消三行插在封面下面，而是在**卡片已有的行里原地替换**：

- 标题行：卡片只建一个标题行（固定为控件高度，`Size::Medium`）；显示态装模型名，编辑态装同一尺寸
  的 GPUI Kit `Input`（保留组件自身的边框、背景与聚焦环），所以标题行的盒子不变，多出来的只是
  那个输入框，以及名字自身向右让出的 10px 内边距。
- 封面：更换封面的按钮改为叠加在封面右下角的浮层按钮，不再占一整行。
- 操作行：「使用 / 打开模型文件夹 / 编辑 / 删除」在编辑态换成「保存 / 取消」，行本身不变。

因此卡片在两个面几何完全一致（同宽同高、标题行与封面位置不变），打开编辑器不会改变卡片高度、
不会拖动整行、也不会移动下面的卡片。卡片里唯一随编辑态变化的行内容是状态行：它两面都保留，
因为它在编辑态消失同样会改变高度。文案随之把 `models.edit.cover.choose`/`.replace` 合并为
`models.edit.cover.label`（「选择封面」/「Choose cover」）——这个按钮永远作用在卡片的封面
区域上（有封面图是它，只有占位符也是它），区分「选」和「换」已无意义。本 ADR 关于页面只做选择与
元数据编辑的决策未变。

### 决策

#### 1. 页面 = 网格首位导入卡片 + 封面卡片

模型库页面只承担两件事：**选择模型** 与 **编辑该模型的标题/封面**。导入卡片占据网格第一格，点击
后仍走既有的来源选择流程（撰写时为「选择文件夹 / 选择 .zip」按钮加标题输入，2026-09-22 起为
单一文件夹选择器加自动导入，压缩包入口暂缓）。设置窗口根视图同时接收 `ExternalPaths` 文件拖放，
因此拖入不必先切换到 Model library 页面，也不要求指针落在导入卡片上；拖放成功后仍由同一来源检查、导入、
取消和封面截取流程处理。其余每张卡片是
`cover.png` + 模式 Badge + 标题 + 可用性 + 操作行（选中 / 打开所在文件夹 / 编辑 / 删除，删除前弹出确认浮层）。

#### 2. 预置模型可以改名和换封面，但与删除无关

预置模型随产品分发，它**没有**任何导入、删除或裁剪路径；改名与换封面则在两个 origin 上完全一致：
`ModelRowActions::can_edit` 不再区分 origin（`!commands_blocked`），`can_delete` 仍只对 installed
为真，`Application::set_model_title`/`set_model_cover` 也按 origin 路由而不是拒绝。

定制内容必须落在用户侧，因为预置包位于 app 包内（macOS `Contents/Resources/models`、Windows
安装目录 `resources/models`），签名包与 Program Files 都不可写：

- **标题**写进 `config.model.built_in_models`，该列表只保存 `id` 与 `title`，但列表各自独立
  判重、各自独立生命周期：导入创建 installed 记录、删除移除它，预置记录只由改名创建、谁都不删。
  列表为空表示所有预置都还用构建给的名字。预置模式由稳定 id 派生，不写入用户配置。
- **模式**由 `SettingsModelEntry.input_mode` 投影到封面上方的 GPUI Kit `Badge` + `Tag` 组合；
  页面不扫描资源、不解析标题。installed 的模式在导入 metadata 中持久化，普通包若无法通过
  `left-keys`/`right-keys` 资源判定则不会进入 store；详见 ADR-0065。
- **封面**写进 `StorageLayout::model_overrides`（`<data>/model-overrides/<id>/resources/cover.png`），
  由 `bongocat-model-store::PresetCoverStore` 提供唯一写入口，布局与包内封面共用
  `package_cover_path`。快照投影优先取这份替换封面，包内封面只作为回退。
- 预置**包本身永远不被写入**：`crates/bongocat-app/src/settings.rs` 的
  `service_renames_and_covers_a_model_of_either_origin` 断言替换后包内 `cover.png` 逐字节不变。

#### 3. 封面位置成为跨 crate 契约

`bongocat-model` 公开布局事实，`bongocat-model-store` 提供两个用户侧写入口：

- `PACKAGE_RESOURCES_DIRECTORY`、`PACKAGE_COVER_FILE`、`package_cover_path(root)` 是唯一来源，
  BongoCatMver 转换改用同一组常量（原先的私有 `OUTPUT_*` 常量删除）。
- `ModelStore::replace_cover(id, bytes)` 负责「同目录临时文件 + rename」的原子替换，并按既有
  Unix 私有模式落盘；失败时不留半写的封面。`PresetCoverStore::replace_cover(id, bytes)` 是同一
  形状的写入口，只是根目录在用户侧（决策 2），因此两边的封面路径都由同一个
  `package_cover_path` 推出。
- 设置页**不推导路径**：它只消费 `SettingsModelEntry.directory`/`cover`，两者都由 app 层读取磁盘
  后填充（`cover` 为 `None` 表示这个模型没有任何封面，页面画占位符而不是猜一条不存在的路径）。

#### 4. 封面只接受 PNG，字节原样落盘

封面是显示用美术，不是模型数据，因此校验只到「PNG 签名 + 不超过包的每文件上限
（`ModelPackageLimits::maximum_file_bytes`）」；通过后按原字节写入，与转换安装旧版 `cat.png`
的做法一致，不做重新编码。这样也避免为 JPEG 等格式引入解码 feature（workspace 的 `image` 只开
`png`）。

#### 5. 「打开所在文件夹」沿用备份目录的能力注入

`bongocat-platform::open_directory` 已有实现（`opener` + 绝对路径/存在性校验），复用即可；
settings 服务通过 `ModelLocationCapability` 注入，与配置备份目录同一 seam。原因是单元测试必须能
断言「打开所在文件夹」的结果，而真实实现会启动系统文件管理器。

#### 6. 模型页的错误只用通用 Notification 组件

页面级错误（源选择器失败、导入失败、封面选择器失败、目录读不出、改名/换封面/打开位置失败）一律
经 `Window::push_notification` + `NotificationType::Error` 呈现；内联 `Tag`/文本只表达**进度与选择
状态**（未选择/已选择/正在选择/导入进度/完成/取消），失败状态的标签留空。目录读取失败只在
「从可读转为不可读」时推送一次，页面用 `model_catalog_error_reported` 记住已经说过，避免每次
快照重复同一条消息。

#### 7. 行为预览退出模型页

模型页不再列出/预览行为，`SettingsCommand::PreviewModelBehavior`、
`SettingsErrorCode::ModelBehaviorPreviewUnavailable/Failed` 及其文案随之删除。行为目录仍由快捷键
页消费（未激活或未声明的行为不生成编辑入口）。runtime 侧的预览能力
（`Application::preview_motion`/`set_expression`）保留不动，恢复设置页预览是新增一个命令的事。

#### 8. 窗口级拖放使用单文件夹契约

设置窗口的根视图和临时蒙层都注册 GPUI `ExternalPaths` 的 drag/drop handler；另有一个 paint-phase
的 window listener 接收 `FileDropEvent::Exited`，不依赖离开窗口后已经失效的 hitbox。蒙层只在文件
拖入期间存在，覆盖设置窗口的完整 viewport，因此当前页面、滚动位置和指针所在的子控件不会改变导入
语义；蒙层出现后仍需保留自己的 drop handler，不能只依赖根视图的命中测试。

- `Ready`：恰好一个路径且当前没有其它模型来源操作；松手后先在 background executor 调用
  `bongocat_platform::validate_model_folder`，只把 canonical directory 交给既有 `InspectModelSource`
  流程。`Ready`、`Busy` 和 `InvalidSelection` 都只显示各自标题，不显示额外说明。
- `Busy`：已有 picker、drop 校验、来源检查、转换对话框、导入或封面截取；松手不启动第二条流程。
- `InvalidSelection`：零个或多个路径；蒙层立即说明“一次一个模型文件夹”，松手后通过稳定错误通知
  结束，不把用户路径写入通知或日志。单个路径若是常规文件、已不存在或无法 canonicalize，则在
  background 复验失败后走同一稳定通知。

拖放只改变来源的进入方式，不复制模型解析/复制逻辑，也不绕过 `ModelStore` 的安全校验、取消和
原子提交。拖放路径的目录/类型/canonicalize 检查属于平台 adapter；UI executor 不遍历、复制或解析
模型文件。

### 明确不做

- **不为预置模型提供删除**：`delete_model` 的预设拒绝（`PresetModelDeletion`）保持不变。改名与
  换封面自 2026-09-22 修订起对预置开放（决策 2），删除仍然只属于用户导入的模型。
- **不把封面写进配置**：封面是图片字节，不是配置字段；用户侧替换封面是布局相同的文件，配置里
  只有标题这一行文本。installed 的模式元数据是例外，它是强类型小字段而不是图片资源。
- **不在 UI 重新判断普通包模式**：ModelStore 在 staging 提交前按 `right-keys`、`East`、`DPad*`
  等资源规则完成一次判定并把结果写入 metadata；页面不重复扫描资源，判定失败直接拒绝导入。
- **不重编码封面、不新增图片依赖**：只校验 PNG 签名字节。
- **不在模型页恢复任何行为/表情入口**：表情列表已在快捷键页。
- **拖放不支持多项目或常规文件**：拖放契约一次只接受一个目录；多选在蒙层阶段拒绝，单路径的
  文件、缺失或无法 canonicalize 的目录在后台复验后拒绝，二者都以稳定通知说明边界。压缩包仍不属于
  当前来源。
- **不让 renderer 或 runtime 参与封面**：封面只被设置页读取。

#### 修订（2026-09-22，卡片编辑改为原地）已完成

本机 macOS arm64，固定工具链 1.97.1：

- `bongocat-ui` 新增 `opening_a_models_editor_does_not_change_the_card`：用真实卡片渲染模型页，
  记录卡片与标题行的 `Bounds`，经 `ModelRowAction::Edit` 打开编辑器后重新渲染，断言卡片与标题行
  的 bounds 完全相等、输入框自身的盒子（含边框）等于标题行的盒子；另外断言卡片高度等于
  `MODEL_CARD_MIN_HEIGHT`（否则两面可能只是在被网格拉伸过的单元格里相等），以及封面按钮落在
  标题行上方（即画在封面上，而不是下方新增一行）。
- 文案：`models.edit.cover.choose`/`.replace` 合并为 `models.edit.cover.label`，两个 locale 同步
  （239 键 × 2），`tools/validate-locales.py` 通过；`bongocat-i18n` 的两条目录扫描测试通过
  （新字面量可解析、无孤立键）。
- `just check` 六道门全过：`cargo fmt --all -- --check`、三组严格 Clippy、`cargo test --locked
  --workspace`（`bongocat-ui` 141，全绿）、`cargo check --locked --workspace --release`。
- Python：`tools/validate-fixtures.py`、`tools/validate-json-schema.py` 与 `tools/tests`（63 项）通过。

**未运行**：双平台实机点击（打开编辑器、点「更改封面图」、保存/取消）、深色主题下的实机外观检查、
`--settings-window-smoke`。

#### 修订（2026-09-22，预置模型同样可编辑）已完成

本机 macOS arm64，固定工具链 1.97.1：

- `bongocat-model` 新增 4 个 `preset_covers` 测试：替换落在
  `<override 根>/<id>/resources/cover.png`（与 `package_cover_path` 同形）、不留 `.new` 暂存文件、
  二次替换覆盖前一次、未替换时读路径不创建任何东西、且根目录/`<id>`/`resources` 为 `0o700`、
  文件为 `0o600`。
- `bongocat-app` 改写 `service_renames_and_covers_a_model_of_either_origin`（原
  `service_renames_and_recovers_an_installed_models_title_and_cover`）：走完整服务链路断言
  预置改名后快照标题变化、`config.model.built_in_models` 恰好一条记录（installed 记录不受影响）、
  换封面后快照指向用户侧覆盖文件且字节一致，并断言**包内 `cover.png` 逐字节不变**。
- `bongocat-ui`：`model_row_actions_preserve_origin_availability_and_active_identity` 断言预置
  `can_edit == true` 且 `can_delete == false`；新增
  `a_preset_models_card_opens_the_same_in_place_editor` 打开预置卡片的编辑器，断言草稿的
  origin/id/初始标题正确、并在重新投影后存活，且卡片同时画出标题输入框与封面按钮。
  这条用例覆盖了**两处编译器抓不到的 origin 守卫**（`begin_model_edit` 直接拒绝预置、
  `sync_model_row_focus` 在下次投影时丢弃非 installed 的草稿）；两处分别用变异验证：
  改回任一个即变红。smoke 的模型页检查由「预置不得暴露编辑」改为「必须至少有一个可编辑的预置」。
- 配置契约：`shared/config/config.schema.json` 保留 `built_in_models` 的 `id/title` 形状，
  `imported_models` 使用含必填 `input_mode` 的专用定义；14 个既有 fixture 补 `"built_in_models": []`，
  另有 installed mode accept/reject fixtures，`tools/validate-json-schema.py` 的语义检查按列表
  各自判重并拒绝缺失/未知模式。
- 错误口径：删除 `SettingsErrorCode::PresetModelMetadataImmutable`（`ALL` 36 → 35）与两个 locale
  键；`ModelNotInstalled` 因同时服务两个 origin 而改名 `ModelNotFound`（文案改为「找不到该模型」/
  "The model was not found"），`ApplicationError` 同名改动。
- 顺带修掉一处**既有假断言**：`selecting_a_model_keeps_the_behavior_bindings_the_user_recorded`
  用 `config_revision() > revision_before` 判断「配置被重写」。配置 revision 是持久化文档的
  内容哈希（`revision_for_bytes`），大小关系没有语义；新增字段改变了哈希，这条断言随即失败。
  已改为 `!=` 并注明原因。
- `just check` 六道门全过（`cargo test --locked --workspace`：ui 142、app 138 + bin 22、model 88、
  config 57、runtime 75、platform 64、live2d 55、overlay 52、i18n 11 等，34 个测试二进制无失败）；
  Python 三个校验器与 `tools/tests`（63 项）通过。
- 实机：`cargo run --locked -p bongocat-app --release --features storage-test-injection
  --target-dir target/storage-test-injection -- --models-page-smoke --run-seconds 4` 退出 0 且
  stderr 为空。它用真实窗口、真实设置服务与仓库内三个预置跑完整条模型页检查，覆盖了本决策新增的
  「必须至少有一个可编辑的预置」smoke 断言。

**未运行**：双平台实机点击（改名/换封面预置卡片、确认包内文件未被写入）、深色主题实机外观、
`--settings-window-smoke` 与 `--models-page-smoke` 之外的 opt-in smoke。

#### 修订（2026-09-24，窗口级文件拖放）已完成自动化部分

- `bongocat-platform` 公开 `validate_model_folder`，复用原生文件夹选择器的绝对路径、目录类型和
  canonicalize 复验；调用点明确放在 `background_executor`，UI 不执行文件 I/O。
- `bongocat-ui` 新增 `ModelImportState::ValidatingDrop`、窗口级 `ModelDragOverlayState` 和主题化蒙层。
  选择器取得路径后的 `Inspecting` 状态与拖放复验共用
  `models.import.step.validating_source` 的文件夹读取文案。
  根视图与蒙层都注册 `ExternalPaths` drag/drop handler，paint-phase window listener 接收离开事件；
  `Ready`、`Busy`、`InvalidSelection` 三种状态各有独立标题，多项目 drop 不启动导入并通过
  `SettingsErrorCode::ModelImportDropInvalid` 通知说明一次一个文件夹的边界。提示内容区去掉边框和
  背景色，只保留主题化图标与文字层次。
- 自动化覆盖：完整 viewport 蒙层与离开清理（含指针移出 viewport）、忙碌状态不启动第二条来源流程、
  单目录进入既有 `InspectModelSource` command、多项目拒绝，以及选择器取得路径后与拖放共用文件夹读取
  文案和状态转换；`bongocat-platform`、`bongocat-ui-protocol`、`bongocat-ui` 定向测试和 locale validator
  已通过。
- **未运行**：Windows Explorer 与 macOS Finder 的真实文件拖放、拖动时不同页面的目视命中、双平台
  DPI/Retina 主题布局和真实键盘/指针交互。因此本 ADR 的实现决策已进入代码，但双平台实机验收仍未完成，
  不应把自动化测试结果写成跨平台行为已确认。

### ADR-0055 · 导入后的模型封面由渲染一帧截取

**状态：** 已接受（2026-09-22）

依赖：ADR-0002（GPUI for Settings UI）、ADR-0003（Native Overlay Renderers）、ADR-0037（在应用内导入 BongoCatMver 模型）、ADR-0047（模型库只做选择与元数据编辑）

### 决策

#### 1. 导入成功后为每个安装的模型排队一次截取

`ApplicationMainThreadSignals`（由 `ApplicationShortcutSignals` 更名，因为它现在承载的不只是快捷键信号）
新增一条 cover capture 队列。settings worker 在 import command 成功后，把每个已安装模型连同它在
settings 协议里的身份（`SettingsModelKey`，origin 恒为 installed）入队。worker 只排队：它不渲染、不读
封面、不写任何字节，也不为截取失败而改变导入结果。

#### 2. 由 GPUI 线程在 50 ms 轮询里完成渲染

新的 GPUI 任务每 `COVER_CAPTURE_POLL_INTERVAL_MS = 50` 毫秒取空队列，对每个请求：

1. 用 `bongocat_overlay::ModelCoverCaptureSession` 逐帧截取（隐藏窗口 + GPU 读回，见决策 3 与
   2026-09-22 修订：每步只画一帧，帧间让出主线程，不整段阻塞）；
2. 用 `SettingsCommand::ReplaceModelCover`（bytes 载荷）把结果交回 settings worker 落盘；
3. 让设置窗口 `finish_model_cover_capture` 丢掉该模型的图像缓存，并解除该次导入的显示门禁。

第 3 步与 ADR-0047 残余风险 3 是同一条依赖：替换保持相同的包内路径，GPUI 的图像缓存按路径命中，
不显式失效就会继续画旧字节。

设置页同时把本次导入安装的条目挡在网格之外，直到这次回调到达：模型页的导入
卡片把「正在导入模型…」**替换**为「正在截取封面…」（一次只显示当前一步，不逐步叠加），卡片下
出现的就是封面已就绪的模型，而不是先画源包占位图再换图。这个门禁属于设置页的展示状态，不进入
settings 协议。导入成功通知在本次导入最后一次封面回调到达、卡片揭示到网格时一起发出，而不是在包
安装结果首次到达时发出。~~截取失败同样完成该门禁并允许成功，保持既有静默失败行为。~~
（2026-09-23：失败仍完成该门禁，但不再允许成功——模型在这一步就被删除，见文末修订。）

#### 3. 两个平台后端共用一份「帧 → 封面」逻辑

`bongocat-overlay/src/cover.rs` 承担与 GPU 无关的一半，因此能在任何平台上被普通单元测试覆盖：把一帧
预乘 BGRA 读回转成直通 alpha RGBA、按可见像素裁掉空白边、把最长边降到 640、编码 PNG。
`macos.rs` 与 `windows.rs` 各自只负责产生那一帧：

- 私有 runtime 激活该模型（`ActivateModel`）并等待准备完成；
- 建立尺寸取自模型画布、**永不显示**的窗口（macOS 从不 `orderFrontRegardless`，Windows 不设
  `WS_VISIBLE` 也从不 `ShowWindow`）；
- 首帧按已提交模型的标准做 smoke 校验（渲染不出内容必须失败，不能把空白图当成封面），随后绘制
  `COVER_CAPTURE_FRAMES = 30` 帧让 idle 姿态与物理收敛，每帧只读回、不 present；
- 读回、裁切、编码后关掉 runtime 与窗口。整个过程的耗时上限是 `COVER_CAPTURE_TIMEOUT = 5s`。

捕获的是窗口显示的东西（背景图 + drawable + 键位层），也就是模型自己的样子。模式之间的差异来自模型的
背景图，因此不主动按下按键、不出现按键高亮。

#### 4. 截取失败不改变任何东西（~~2026-09-23 起改为终止导入~~，见修订）

失败时保留源封面、不重试、不阻塞导入、不经 UI 报错。封面是显示用美术而非模型数据（ADR-0047 决策 4），
错误的图不是坏掉的模型。可观测性为零这件事是本节的一部分，见残余风险 2。

**本节已被 2026-09-23 的修订取代**：截取失败现在会撤销这次导入并报「模型导入失败」。原文保留为当时
的决策记录；取代它的理由（截取与激活共用同一条 GPU 准备路径）见文末修订。

#### 5. 封面写入仍走同一条 store 通道

`Application::set_model_cover_bytes` 与 `set_model_cover` 共享同一个 `install_model_cover` 契约：PNG 签名
+ 不超过包的每文件上限，然后 `ModelStore::replace_cover` 原字节落盘。`SettingsCommand::ReplaceModelCover`
与 `SetModelCover` 的差别只是载荷（bytes 还是 path），因为捕获出来的封面在被安装之前根本不是一个文件。

#### 6. `capture-cover` 子命令作为可复现证据

`bongocat-overlay capture-cover <standard|keyboard|gamepad> <out.png>` 让整条链路（隐藏窗口、GPU 读回、
裁切、编码）脱离导入流程单独执行，因此平台 backend 可以在没有 UI 的情况下被检查与回归。

### 修订（2026-09-22，当日）

**截取改为逐帧可让步会话，不再整段占用 GPUI 线程**（消除残余风险 1）。用户实测：导入卡片在
「正在截取封面…」一步时 Spinner 停转，偏好设置窗口与 overlay 同时卡住。

- 原因与决策 2、3 相同：截取窗口只能建在拥有产品窗口的线程上（macOS 主线程 / Windows 的
  overlay 所在线程），而整段同步的截取让该线程在约 30 帧内无法回到自己的事件循环。
- 新增 `bongocat_overlay::ModelCoverCaptureSession`（`macos.rs` / `windows.rs` 各有
  `CoverCaptureSession` 后端）：`start` 完成私有 runtime、隐藏窗口与首帧 smoke 校验，`step` 每次
  只泵一次事件、同步最新帧并画一帧，`finish` 关闭 runtime 并编码 PNG。帧数（`COVER_CAPTURE_FRAMES`）、
  超时（`COVER_CAPTURE_TIMEOUT`）、首帧校验、读回/裁切/编码与「失败不改任何东西」的语义全部不变。
- app 侧的 GPUI 任务在两次 `step` 之间 `Timer::after(frame_interval()).await`，让出点落在
  foreground executor 上：主循环照常泵事件与重绘，设置窗口的 Spinner 与 overlay 帧循环在截取期间
  正常运行。
- `capture_model_cover` 保留为阻塞形式（内部改为按会话逐步执行后 `thread::sleep`），仅供
  `capture-cover` 子命令等独占线程的调用方使用；其输出与修订前逐字节一致（keyboard：
  186,091 bytes / 640×352）。

### 修订（2026-09-23，截取失败改为终止导入）

**决策 4 由「截取失败不改变任何东西」改为「截取失败即放弃这次导入」**，决策 2 末句与残余风险 2 随之作废。

- 触发：用户实测两个社区 MVer 模型（`芙宁娜(按键气泡）`、`芙宁娜(无气泡)`）导入后启用报
  「无法启用所选模型」。根因不在截取本身，而在两个后端共用的渲染快照 preflight 把「有顶点、没有
  三角形」的 drawable 当成致命错误——`芙宁娜(无气泡)` 的 drawable `#115` 是 3 顶点 / 0 索引 /
  `visible = false`。Cubism 允许这种 drawable，官方渲染器只是不画它，而本项目的 preflight 让整个
  模型在 GPU prepare 阶段失败，于是同一个模型既截不出封面、也激活不了。修正见 TODO 中
  「D3D11/Metal 对相同 snapshot 行为一致」一项的 2026-09-23 状态。
- 这条 bug 同时证明了一个原决策 4 没考虑的事实：**截取与 overlay 激活走的是同一条 GPU 准备路径**
  （`GpuModel::prepare` 及其渲染快照 preflight）。截不出封面的模型，激活一定也失败。把「封面截不出来」
  当成纯粹的显示问题，就会给用户留下一张点了只报错的卡片。
- 因此新语义：截取失败时，app 主线程先 `SettingsCommand::DeleteModel` 撤销这次导入，再回调
  `finish_model_cover_capture(.., captured = false)`。模型自始至终没有进入网格（导入期间它一直在
  `SettingsView::pending_model_reveal` 里），用户看到的是一条「模型导入失败」通知，而不是一个坏卡片。
- 失败通知走新增的 `ModelImportFailedNotification` 与 `models.import.failed`（zh「模型导入失败」/
  en「Model import failed」），与既有的 `models.import.success` 对称。它在成功通知**之前**推送：
  一次导入装了多个模型而其中一个失败时，读起来是「这个失败了」在前、「其余已就绪」在后。
- 一次导入里只有部分模型的截取失败时，只有失败的那些被删除，成功的不受影响，两条通知都会出现。
- `capture-cover` 子命令、`ModelCoverCaptureSession` 与「帧 → 封面」的接口都不变；变的是 app 主线程
  对失败结果的处理，以及设置窗口的发布逻辑。

### 明确不做

- **不加偏好开关**：用户要的是「导入后就是模型自己的样子」，而不是多一个开关。此前考虑过的「偏好设置
  增加一个封面字段」被否决。
- **不放松 ADR-0047 决策 4 的 PNG 契约**：仍然只校验签名与大小，不重编码，不为 JPEG/WebP 打开 `image`
  feature。
- **不为已存在的模型回填封面**：本次只覆盖导入路径（含 MVer 三模式各自的导入），不扫描历史模型，
  也不新增「重新生成封面」入口。
- **不让 `bongocat-ui` 依赖 runtime / render / Live2D**：协议只携带 `Vec<u8>`，交接走 app 侧共享队列。
- **不改预置模型的包**：预置包仍是 product files，永远不被写入。用户给预置换的封面存在用户侧
  （见 ADR-0047 决策 2 修订），本决策的截取路径只服务导入模型，不参与预置封面。

### ADR-0060 · Model package and model-store boundary

**状态：** —

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0036, ADR-0037, ADR-0038, ADR-0041, ADR-0047, ADR-0050, ADR-0055

### Decision

Create `bongocat-model-store` as the filesystem-facing persistence and import
layer. It owns:

- `ModelStore`, its lock, recovery, staging, copy/validate/commit/delete flow,
  and installed catalog scan;
- `ModelStoreDiagnostic`, `ModelStoreError`, import stage/progress, and
  user-side preset cover storage;
- Mver detection/conversion and PNG composition;
- normalization of legacy key-image names inside the store's own staging copy.

`bongocat-model` remains the read-only model domain layer. It continues to own
model3/sidecar parsing, package limits and path safety, `ModelId`, immutable
model metadata, `PreparedModel`, `CommittedModel`, `ModelSnapshot`, and the
read-only `PresetModelCatalog`/`ModelCatalogEntry` projection. The preset
catalog stays here so overlay, Live2D, and runtime consumers do not acquire the
mutable store's image/UUID dependencies just to inspect bundled models.

The production dependency direction is:

```text
app ───────────────> model-store ───────> model
runtime/live2d/overlay ──────────────────> model
```

`bongocat-model-store` receives roots and lock paths from the application; it
does not read `StorageLayout`, select Development/Production, or depend on
config, platform, runtime, renderer, Live2D, or GPUI.

#### Commit ownership

Rust cannot express “only this sibling crate may call this constructor” without
adding another lower-level crate. The split therefore uses a narrow, documented
product seam instead of claiming compile-time sealing: `PreparedModel::relocate`
rebinds an already validated package after the store's atomic rename, and
`InstalledModel::from_prepared` is called by the store transaction before the
existing `CommittedModel` conversion. The model-store/app wiring is the only
supported caller. A future requirement for hostile third-party callers would
need a separate sealed commit type; this ADR does not pretend the current Rust
visibility provides that stronger guarantee.

### Consequences

- parser-only crates no longer compile the Mver image/PNG/UUID/storage-write
  dependency set;
- model-store tests cover permissions, locks, staging, cancellation, recovery,
  Mver conversion, cover writes, and every registered shared fixture;
- source directories remain read-only inputs; only store staging and committed
  destinations are written;
- `CommittedModel` construction is a documented store/preset product invariant,
  not a compiler-enforced sealed type in this slice;
- Cubism Core lifecycle and Core-coupled parameter application remain in `bongocat-live2d`; pure motion/expression playback is owned by `bongocat-live2d-playback`.

### ADR-0061 · Live2D playback boundary

**状态：** —

- Status: accepted
- Date: 2026-09-24
- Amended: 2026-09-25
- Depends on: ADR-0060, ADR-0041, ADR-0050, ADR-0055

### Decision

Create `bongocat-live2d-playback` for the pure clip layer. It owns:

- motion3 metadata/curve/segment parsing, linear/Bezier/stepped/inverse-stepped
  evaluation, fade/loop time, PartOpacity samples, and UserData crossings;
- exp3 parsing, fade weights, blend modes, and pure expression-layer mixing;
- the playback-specific `PlaybackError`/`PlaybackErrorCode` for malformed
  motion or expression bytes.

The crate depends only on `serde`, `serde_json`, and the standard library. It
does not depend on `bongocat-model`, `bongocat-render`, `bongocat-live2d`,
runtime, GPUI, platform APIs, or the filesystem. `MotionClip::load` and
`ExpressionClip::load` are not part of this crate; `bongocat-live2d` reads the
committed model resources through the render-resource adapter and calls
`from_slice`.

`bongocat-live2d` remains responsible for:

- `CommittedModel` resource lookup and bounded file reads;
- Cubism Core loading, parameter ranges/defaults, Core writes, part-opacity
  sinks, dynamic flags, RenderResources, and model opacity;
- adapting `PlaybackError` into the existing stable `Live2dErrorCode` catalog;
- `MotionApplyStatus` and `ExpressionApplyStatus`, whose counts describe actual
  writes accepted by Core.

`bongocat-runtime` remains the owner of active playback state, priority,
single-loop product policy, stop/crossfade timestamps, command sequencing,
model-generation cleanup, and audio side effects. It may depend directly on the
playback value types, but playback never sees runtime commands or Core state.

The production dependency direction is:

```text
bongocat-live2d-playback -> serde / serde_json / std
bongocat-live2d ----------> bongocat-live2d-playback
bongocat-runtime --------> bongocat-live2d + bongocat-live2d-playback
```

### Consequences

- motion/expression parser and numeric tests no longer compile the vendor SDK;
- Core-coupled loading and application tests remain in `bongocat-live2d`;
- runtime's existing frame order and playback ownership do not change;
- no physics/pose crate or placeholder is introduced by this slice; those
  evaluators remain a future extension of the same pure boundary;
- the old `bongocat-live2d` playback re-export is removed after runtime imports
  the playback types directly.

### Amendment: completed motions hold their terminal pose

The original one-shot policy removed renderer playback and the public active-motion identity as
soon as a clip reached its declared duration. The following evaluation restored Core defaults, so
ordinary motion parameters disappeared one frame after the terminal sample. This solved accidental
looping but did not match the 2026-09-24 product requirement to stop on the action's final state.

A one-shot motion now transitions to an internal completed state at the clip duration. Its local
sample time is pinned to that duration, and the fully evaluated parameter, part-opacity, and
model-opacity samples remain a renderer-owned motion layer that is reapplied after each per-frame
default reset. "Terminal sample" includes the resource's natural model3/curve fade weights; completion
does not strip a natural fade-out to expose an earlier raw curve endpoint. Core part opacities are
restored to their fresh-model values before the active motion is applied, so removing or replacing a
motion cannot leave stale part visibility. The later reference-compatible amendment below updates
the automatic/product/physics sub-order; the completed-motion lifecycle itself is unchanged.

Completed playback is not advanced again and does not reserve motion priority. The worker derives
completion from the injected monotonic clock as well as the last delivered frame, so a hidden or
sleeping overlay cannot swallow a replay or lower-priority request sent after the duration. The public
active identity remains current so an explicit `StopMotion` can still fade and remove it; a
replacement, successful model commit, or shutdown removes the layer as before. `StopMotion` continues
to target the current identity: a different old ID cannot stop a newer motion, while a replayed run
with the same ID is intentionally the current target of a later same-ID stop.

Motion UserData crossing evaluation uses the runtime's effective playback mode. Playing a
`Meta.Loop = true` asset once therefore does not emit its time-zero event again at the duration
boundary. Expressions retain the already-correct stateful lifecycle: the newest expression remains
pinned at full weight after fade-in, including across a test-clock rollback, until another valid
expression replaces it, a model commit succeeds, or shutdown clears it. No expression clear,
duration, new crate, dependency, FFI call, or frozen whole-model snapshot is introduced.

#### Superseded amendment: automatic breath is an explicit weighted model-range group

> This intermediate rule is retained as history only; the reference-compatible
> amendment below supersedes it for the current runtime.

A parameter merely named `ParamBreath` is not sufficient intent: third-party models can use that
conventional ID for authored pose selection. A signed sine written through the generic `[-1, 1]`
normalized-parameter helper also turns a common one-sided `0..1` range into a long clamp at the
minimum followed by a full-range sweep. A model-authored drawable threshold can then switch large
groups of parts every cycle even when the package has no motion or expression.

Automatic breathing is therefore opt-in through the first model3 `Parameter` group named `Breath`,
with the same first-64-ID bound as EyeBlink and LipSync. A model without that group keeps its
authored parameter defaults. For declared IDs, the runtime emits a deterministic four-second
unit-interval sine phase; the Core-coupled adapter maps it across each parameter's declared range
and applies it with `0.5` weight after motion/expression. Eye blink keeps its existing normalized
open/closed input. This is a parameter-evaluation correction, not a motion or expression trigger,
and it does not add renderer-side state.

#### Superseding amendment: restore the Mver reference breath and physics path

The explicit-group-only rule above was sufficient to stop the imported model's full-pose threshold
crossing, but it also removed the model's authored idle hair movement. The fixed Mver source shows
that this is not an optional model-specific effect: `myUserModel.cpp` always creates a
`CubismBreath` instance with the conventional `ParamAngleX`, `ParamAngleY`, `ParamAngleZ`,
`ParamBodyAngleX`, and `ParamBreath` IDs, then loads and evaluates the declared `physics3` resource.
The update order is motion/expression, product drag, breath, physics, and Core update.

BongoCat therefore applies the same five fixed breath targets with their reference offsets, peaks,
cycles, and `0.5` contribution weight after typed product input. The contribution is additive
(`current + value * 0.5`, followed by the Core range clamp), matching the reference
`AddParameterValue` call; it is not a blend toward the breath target, which would weaken a
mouse-driven angle before physics reads it. A model3 `Breath` group remains an optional source of up
to 64 additional IDs and retains its existing model-range blend behavior; it is no longer required
for the conventional targets, and fixed IDs are not applied twice. For the reported model, the
`ParamBreath` range is `0..1` with default `0`; the reference contribution stays at or below `0.5`
and does not cross its authored visibility threshold,
while the fixed angle targets feed the declared physics rig and restore the hair's idle motion.

Validated physics3 v3 definitions are parsed by the model contract and evaluated by a bounded Rust
runtime with fixed-step interpolation, inertia, delay, and typed parameter IDs. Legacy exported
resources may omit `Meta.Fps`; that representation uses the incoming monotonic frame delta rather
than inventing a fixed rate, while an explicitly supplied FPS must still be finite and positive.
Unknown input or output IDs are skipped rather than reaching Core. This is a compatibility path for
the declared physics resource, not a claim that every Cubism feature or every platform has
completed the full R5 black-box gate; pose evaluation remains separate and unfinished.

### ADR-0062 · Live2D render-resource preparation boundary

**状态：** —

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0042, ADR-0060, ADR-0061, ADR-0003

### Decision

Create `bongocat-live2d-render` for the platform-neutral model-to-render
resource preparation layer. It owns:

- construction of `RenderResources` from a `CommittedModel` index;
- background and key-image discovery, regular-file filtering, and PNG
  dimension validation;
- `KeyImageInventory`, the ordered HID-to-artwork candidate vocabulary, and
  `resolve_key_overlays`;
- a small resource-preparation error that is mapped to the existing
  `Live2dErrorCode::ResourceIo` by the Core adapter.

The crate depends on `bongocat-model`, `bongocat-render`, `image`, and the
standard library. It does not depend on Cubism Core, `bongocat-live2d`,
runtime, GPUI, platform APIs, or a GPU backend. It does not decide input
bindings or mutate runtime state; the app still applies the inventory when
building bindings, and runtime still consumes only the resulting immutable
`RenderSnapshot`.

`bongocat-live2d` now asks `bongocat-live2d-render` to prepare resources during
model loading, then retains Core loading, parameter/part writes, Core error
mapping, and snapshot production. `bongocat-app` and `bongocat-runtime` import
the inventory and overlay resolver from the new crate directly.

`bongocat-render` remains the immutable snapshot/resource/transport contract.
The actual Metal and D3D11 GPU owners, window surfaces, and present loops stay
in `bongocat-overlay`; this slice does not move platform GPU code or make the
renderer read configuration or input services.

### Consequences

- model resource preparation and key-image contracts can be tested without the
  Cubism SDK;
- `bongocat-live2d` no longer owns the image decoder or key-image directory
  scan;
- app/runtime no longer acquire those contracts indirectly through the Core
  crate;
- the existing "missing artwork means no binding" behavior and side-specific
  overlay rules remain unchanged;
- no new renderer backend, window owner, or GPU lifecycle is introduced.

### ADR-0063 · 模型渲染采用 encoded-space 兼容颜色管线

**状态：** Accepted

取代：ADR-0046
修订（2026-09-24）：根据多层模型在逐 drawable 透明度下出现重影的实机反馈，presentation opacity
改为在内部 drawable 合成完成后由平台最终 surface 统一施加。

### 决策

1. **两端统一采用 encoded-space compatibility contract。**
   - Windows：模型、背景和按键纹理使用 `DXGI_FORMAT_R8G8B8A8_UNORM`；composition back
     buffer 与 render target view 使用 `DXGI_FORMAT_B8G8R8A8_UNORM`。
   - macOS：模型、背景和按键纹理使用 `MTLPixelFormat::RGBA8Unorm`；CAMetalLayer 和
     color attachment 使用 `MTLPixelFormat::BGRA8Unorm`。
   - alpha 仍按 premultiplied alpha 输出；clipping mask 仍使用 alpha-only UNORM target。
2. **不在任一平台的 shader 中插入 sRGB decode/encode，也不对 blend color 做隐式转换。**
   两端的 shader、filter、预乘和 blend factor 由同一平台无关 contract 驱动，避免只修一端。
   模型、背景和按键先按模型/资源 alpha 完成内部合成；窗口 presentation opacity 不进入逐
   drawable 的 fragment alpha，而是在最终 surface 合成后由 Windows DirectComposition visual
   effect 或 macOS `NSPanel` 统一施加一次。这样多层 Live2D 不会各自衰减并产生重影。
3. **消费 Core 的 `double_sided` 状态。** D3D11 和 Metal 都对非 double-sided drawable（包括
   mask source）执行背面剔除；两端明确以 counter-clockwise 为正面。水平镜像会反转三角形
   winding，因此镜像时改用相反的剔除面；double-sided drawable、背景和按键 overlay 不剔除。
4. **这是兼容优先的产品决策，不是 encoded-space 线性光度正确性的声明。** 将来若要切换到
   linear-light，必须新增独立决策，同时修改两个 backend、pixel contract、截图基线和技术
   设计，不能只改 Windows 的 RTV 或只改 Metal 的 drawable。
5. **不改变模型资源、schema 或用户配置。** 这是 renderer-only 行为修复，不需要迁移、版本
   分支或旧配置转换。

### 影响

- 在来源链尚未完全核实的前提下，优先选择能解释用户色差、且不改变模型资源的产品兼容语义；
  最终是否与实际 Mver 像素一致仍由硬件 readback 决定。
- 两端使用相同的 encoded RGBA 输入和 blend state；presentation opacity 在内部 drawable 合成
  完成后由各自平台的最终 surface 机制统一施加，避免自动 gamma 转换或逐层衰减造成单边漂移。
- macOS Core Animation 和 Windows DirectComposition 仍可能在广色域显示器上对最终 surface
  做不同的系统级显示转换；renderer 不承诺跨系统色彩管理绝对一致。
- 不把 Cubism Framework 的 C++ 源码、shader 文本或厂商二进制复制进产品；实现只使用项目
  自有 contract，并遵守 Cubism 许可边界。

### 当前验证与待完成门禁

已完成的自动化验证：

- `cargo fmt --all -- --check`；
- `cargo clippy -p bongocat-overlay --all-targets --all-features --locked -- -D warnings`；
- `cargo test -p bongocat-overlay --locked`（当前 72 项通过）；
- `cargo test -p bongocat-render --locked`（16 项通过）与
  `cargo test -p bongocat-live2d-render --locked`（3 项通过）；
- `cargo check -p bongocat-overlay --target x86_64-apple-darwin --locked`；
- `cargo check --workspace --release --locked`；
- Windows `bongocat-overlay` model-switch preview（包含 D3D11/DirectComposition visual
  opacity effect 的创建、提交和资源切换）通过；这只是 renderer lifecycle smoke，不是 Mver
  像素验收；
- 共享 culling decision、blend-factor contract 和两端 format contract 单元测试。

完整 `cargo test --workspace --locked` 当前在 `bongocat-app` 测试二进制中报告 136 passed、1 failed：
`settings::tests::service_renames_and_covers_a_model_of_either_origin` 的 Windows 路径分隔/前缀断言；
命令随后停止，不能把它写成整个 workspace 已通过。`cargo clippy --workspace --all-targets
--all-features` 还会触发仓库现有的 `storage-test-injection` 与 Production 互斥 feature
`compile_error!`，因此 workspace 静态检查使用不含 `--all-features` 的命令并单独记录该限制。

尚未完成、因而不能宣称兼容验收通过：

- Mver 实际 build 的 Cubism/SFML 版本、texture manager 和 shader provenance；
- Windows/macOS 目标机首帧、模型截图与同 snapshot readback；
- encoded RGB、premultiplied alpha、mask、blend、presentation opacity 和颜色 tolerance 的
  跨平台对照；
- mipmap 生成/缩小过滤、背景与按键层顺序及复杂 mask channel parity；
- 真实 GPU/驱动、显示器色彩管理和 mirror/culling 的硬件验证。

在这些证据完成前，`P3-MVER-COLOR-COMPATIBILITY` 与相关 TODO 保持未勾选。

### ADR-0065 · 模型模式元数据与卡片 Badge

**状态：** 已接受（2026-09-25）

依赖：ADR-0037（在应用内导入 BongoCatMver 模型）、ADR-0047（模型元数据编辑边界）、ADR-0059（UI protocol 边界）

### 决策

#### 1. installed 模型元数据持久化 `input_mode`

当前 v1 的 `model.imported_models[]` 每条记录包含：

```json
{
  "id": "model-storage-id",
  "title": "User-facing title",
  "input_mode": "standard"
}
```

`input_mode` 是必填的 `standard`、`keyboard` 或 `gamepad`。它与标题一起在导入事务完成后写入 `config.json`，重启时从配置读取。模型改名只修改 `title`，必须保留原 `input_mode`；删除模型随整条元数据删除。

`model.built_in_models[]` 继续只保存 `id` 与 `title`。预置模式由构建拥有的稳定 id 派生，不在用户配置里重复保存。

`input_mode` 不加入 `{ id, source }` 模型身份，不新增模式编辑 command，也不从显示标题反解析。

#### 2. 导入时判定普通包模式

`bongocat-model-store` 在普通包复制、键名归一化和第二次 `PreparedModel` 校验之后扫描 staging 树，使用与 runtime 键图读取相同的直接 `.png`/文件名语义：

1. 任一手存在手柄专用键名 → `gamepad`；
2. 否则任一手存在 `right-keys` → `keyboard`；
3. 否则存在 `left-keys` → `standard`；
4. 两者都不存在 → `InvalidPackage`，导入事务失败。

判定结果和模型一起从 store API 返回，Application 在同一导入循环中写入 metadata。模式判定失败时，用户源目录、store 目的地、staging 和 config 都保持不变。`right-keys` 单独存在即可判定 keyboard；手柄判定不只依赖 `East`，还检查完整的预置/转换手柄词汇子集。

Mver 转换不重新用上述启发式覆盖 source section，而是把用户选择的 legacy mode 传入同一提交尾部。预置模型也不从资源目录反推，直接由稳定 preset id 映射。

#### 3. settings snapshot 投影强类型模式

`bongocat-ui-protocol` 独立拥有 `SettingsModelMode`。Application 把 config/model 值穷尽映射为该协议类型，并把它放进 `SettingsModelEntry.input_mode`。正常导入的模型一定有该值；只有手工放入 store 且无法通过包校验的异常目录才会没有模式 Badge。

GPUI 页面只消费 snapshot，不读取 config、不扫描模型目录，也不自行解释 id 或标题。

配置、model-store、UI protocol 各自保留自己的稳定枚举，避免跨层依赖或第三方类型泄漏。Application 的映射必须覆盖全部枚举值。

#### 4. 卡片右上角使用无图标、中性色的 GPUI Kit 标记

固定 GPUI Kit revision 的 `Badge` 原生支持数字、圆点和图标，不是文字标签 primitive。Models 页面保留 `Badge` 作为封面覆盖层容器，但不再使用它的 icon/number overlay；可见内容改为与原实现一致的 compact `secondary` `Tag`：

- 标准、键盘、手柄使用相同的中性主题色；
- 模式差异由本地化文字表达，不增加彩色背景噪声。

标记定位在封面右上角，不新增卡片行，也不改变封面、标题编辑面或卡片高度。模式文字始终可见，标记是只读展示，不加入 Tab 顺序。

#### 5. 本 ADR 不改变 runtime 输入绑定

`input_mode` 在本阶段是导入来源/模型类别元数据，用于配置完整性、快照和模型卡片展示。它不在本 ADR 中切换 runtime 的 key/gamepad binding；现有 installed 模型默认键盘绑定及预置 gamepad 特例仍按其独立任务维护。

因此“卡片显示 Gamepad”表示该模型来自或被构建标识为 gamepad 模式，不等于本 ADR 已完成 installed Mver gamepad 的 runtime 输入接线。未来若让该字段驱动输入行为，必须单独修改 runtime/overlay contract、迁移既有绑定测试并完成双平台手柄验证，不能在纯 UI 改动中静默发生。

#### 6. 当前 v1 直接增加必填字段

完整 v1 尚未发布，因此不增加 schema 版本、migration、alias 或旧字段 fallback。缺少 `input_mode` 的 installed metadata、未知枚举值或其它旧开发结构都不是当前完整 v1，按现有严格配置恢复边界处理。Development 与 Production 使用相同结构、不同存储根。

### 结果

- 普通模型不会以“未分类”进入 store 或 config；无法判定就是导入失败。
- Mver 三种转换模式在配置、快照与卡片之间有强类型闭环。
- 普通包的标题、目录名和具体美术文件不会改变已保存的模式；改名、删图、目录变化和重启不会重新推断。
- 预置模式不与用户可编辑标题或封面元数据混在一起。
- UI 使用官方 GPUI Kit 组件、主题色和本地化可见文案，没有自绘状态；模式标记无图标、位于封面右上，并保持中性 `secondary` 颜色。

### 未验证 / 后续

- Windows/macOS 设置窗口中右上角模式标记的真实观感、Retina/DPI 和长本地化文本仍需平台 smoke 与截图检查。
- installed Mver gamepad 的 runtime 输入接线不属于本 ADR；如产品要让模式 Badge 同时代表实际输入行为，需要独立任务。

### ADR-0065 · Runtime-Owned Random Model Behavior Scheduler

**状态：** 已接受（2026-09-25）

> 部分被 ADR-0072 取代：`model.random_behavior.enabled` 已改为
> `model.random_behavior.mode`（`off` 为其中一个取值），候选集合不再是固定的
> motion 与 expression 合并列表。间隔字段、`RandomBehaviorSettings`、
> `Idle` priority、seed 注入与 admission gate 的决策不变，仍然有效。

### 决策

- `model.random_behavior.enabled` 是唯一的启用门禁，默认 `false`；
  `model.random_behavior.interval_seconds` 默认 `30`，持久化范围为 `1..=3600`。
- `bongocat-runtime::RandomBehaviorSettings` 是 runtime 的唯一设置类型。
  `SetRandomBehaviorSettings` 携带 expected config revision 对应的强类型 command，并在
  `RuntimeSnapshot` 中回显；非法间隔在 runtime 和 config 边界都拒绝。
- runtime 从当前已成功提交的 model index 声明的 motion 与 expression 中均匀选择一个；
  这是合并后的行为列表等权，不是先以 50% 选择 motion 再以 50% 选择 expression。第一次选择等待
  一个完整间隔。成功模型切换、设置变更和重新启用都会重锚定时器；长暂停只选择一次，不追赶补发，
  时钟回退不会制造过去的触发。
- 自动 motion 使用 `Idle` priority，不替换正在进行的 `Normal` 或 `Force` 产品 motion。
  expression 仍遵守现有 expression 淡入、替换和模型 commit 清理语义。空行为列表是 no-op。
- 选择器是 runtime 内部的固定 seed PRNG；生产 seed 只用于进程内变化，测试可以注入 seed
  并用 `MonotonicClock` 得到可重复序列。renderer、GPUI Entity 和平台 callback 不参与选择。
- 自动行为的 runtime event sequence 与 audio command sequence 分离；所有生产 audio command
  使用 `MotionAudioClient` 的独立序号分配器，并在同一 publish lock 下完成分配与入队，避免自动
  行为的高位 event sequence 让产品音频 waiter 误判低序号 command 已完成。
- 自动行为和 shutdown request 通过同一 admission gate 排序：shutdown request 立即记录状态并
  开始有界等待，已接纳的自动副作用在 worker 进入 shutdown drain 前完成，后续请求不会插入新的
  随机行为。显式停止的非零 motion fade 即使 overlay 隐藏、没有下一帧，也在注入单调时钟达到
  fade duration 后视为 settled。
- settings 页面把两个字段放在独立的 Model behavior 页面中（ADR-0066）；间隔控件在
  开关关闭时置灰但保留已保存的值。配置写入仍使用原子、revision-checked 的 settings service。

### 后果

随机行为不再由第三方 `easy-live2d` helper 或旧版配置隐式继承；v1 明确拥有该产品
行为。配置初始关闭不会改变现有启动观感，用户打开后需要等待一个完整间隔才看到第一次选择。

### ADR-0072 · Random Behavior Becomes a Mode Enumeration

**状态：** 已接受（2026-09-27）

### 决策

- `model.random_behavior.enabled` 被 `model.random_behavior.mode` 取代。mode 是单一枚举：
  `off`（默认）、`expressions`、`motions`、`motions_and_expressions`，以
  `snake_case` 持久化。关闭是它的一个取值，不再是独立布尔字段。
- 关闭是枚举而不是保留布尔门禁，是因为「开关 + 模式下拉」要用户先回答两个问题才能
  读懂一行设置，而且它允许「打开但没有可播内容」这种没人主动要求的状态。单一枚举
  让每个配置只有一个答案，也就是设置页那一行下拉显示的答案。
- 默认值是 `off`：全新配置从未被要求自己动起来，这也是 v1 文档原本描述的行为，
  因此本决策不改变任何现有配置的实际效果。
- mode 先收窄候选集合再抽取，而不是先抽取再丢弃。这样选中项一定属于用户允许的类别，
  且 `motions_and_expressions` 是一个等权池子，不是两类各占 50%。
- 模型没有声明所选类别的行为时保持无操作，不退回另一类。mode 是用户对「播什么」的
  决定，不是两个都可用集合之间的优先级。
- 间隔在 mode 为 `off` 时仍须合法（`1..=3600`，默认 `30`），因此一个把越界值寄存
  在关闭状态下的配置会被拒绝，而不是等到用户打开时才发现。
- runtime 与 settings protocol 各自拥有自己的 mode 枚举，与 log level 一样由
  `bongocat-app` 的 `config_projection` 负责四处边界转换。runtime 的枚举紧挨着它过滤的
  `ModelBehaviorSnapshot`，protocol 的枚举是窗口渲染下拉所需的完整目录。
- 设置页用统一门禁规则（ADR-0053）：mode 下拉本身只被结构性阻塞禁用，两个下拉在开关关闭
  时置灰但保留已选值 —— 这里 mode 下拉扮演门禁控件的角色，所以它走 `disables_switch()`
  语义（永远不因自身取值而禁用），间隔行走 `disables_controls()`。否则用户一旦关掉就
  再也打不开。
- 间隔与 mode 仍是同一个持久化对象、同一个 typed command。两次独立命令会让「先改 mode
  再改间隔」的快速点击用第二个命令的旧快照覆盖第一次的结果，用户的第一次选择被静默撤销；
  因此两个控件都从仍在途中的值出发，debouncer 把其余变化收敛成一次后续提交。
- 该字段直接进入当前 v1，不提供迁移、旧字段探测或 fallback。按仓库既定策略，
  仍持有 `enabled` 的旧文档按损坏处理，走「最新有效备份 → 默认配置」。

### 后果

用户可以在「表情」「动作」「两者」之间三选一，也可以整体关闭，且默认行为与之前一致。
配置少一个布尔字段，多一个枚举字段；两者都是 v1 直接变更。`enabled` 消失后，
`RandomBehaviorSettings` 只剩 mode 与间隔，runtime 不再需要「门禁」与「模式」两个概念
去描述同一件事。

### ADR-0077 · 接受没有输出项的物理设置

**状态：** 已接受（2026-09-29）

补充：ADR-0060（模型包 store 边界）、ADR-0061（Live2D playback 边界）

### 决策

#### 1. 输出项可以为空，模拟照做但不写回

删除 `setting.outputs.is_empty()` 这一项要求，保留输入项非空和至少两个顶点。

理由不是"放宽一点试试"，而是求值器本来就按这个语义实现。`bongocat-live2d` 的
`PhysicsRuntime` 用 `setting.outputs.len()` 分配每个设置的输出缓冲区
（`crates/bongocat-live2d/src/physics.rs` 的 `reset`），`step` 与 `interpolate` 都按输出
列表迭代：列表为空时循环体不执行，粒子照常模拟，结果无处可写。这正是官方 Framework 的行为
——`csmUpdatePhysics` 为每个设置计算粒子，再逐个 `Output` 写回，没有输出就没有回写。

因此空输出是**惰性**而不是**错误**：它描述的是"这个设置驱动不了任何参数"，而不是"这个
设置坏了"。

#### 2. 保留的两条要求仍然有牙齿

- **输入项非空**：没有任何输入的设置无从反应，等于没有刚体。
- **至少两个顶点**：一个顶点挂不出摆锤，`reset` 也要靠前一个顶点递推初始位置。

两者都继续拒绝，且各自的反例被测试固定。

#### 3. Meta 计数仍然照实校验

`TotalOutputCount` 为 0 是合法的，计数一致性检查（`Meta` 声明值必须等于数组实际长度）
不变。这条检查防的是元数据自相矛盾，与输出项是否为空无关。

### 后果

- 报告的模型与另一个同病模型（`魈二代 · 标准模式`）现在都能导入，行为为
  `Ready`，物理照常求值。
- 与 Cubism Framework 的一致性提高：两条路径现在对同一个文件给出同一个答案，而校验层不再
  比被参照的实现更严格。
- 只放宽了输出项。字段级规则（权重范围、`VertexIndex` 边界、Meta 计数、Id 唯一性等）
  一条未动，因此这不是一次泛化的"宽松化"，而是一处定向修正。
- 回归测试分两层：`bongocat-model` 固定"空输出合法、缺输入/缺摆锤仍拒绝、混合 rig 正常"，
  `bongocat-live2d` 固定"空输出设置被跳过而旁边的有效设置照常驱动参数"——后者防止跳过逻辑
  退化成"整个 rig 变成 no-op"而测试仍然通过。

### ADR-0085 · 动作和表情支持自定义名称

**状态：** 已接受（2026-10-06）

补充：ADR-0067（配置域名与模型身份）、ADR-0079（新增配置字段的可选读取约定）
需求：Issue #1111

### 决策

#### 1. `model.behavior_names` 是每模型每行为一行，默认空

`Vec<ModelBehaviorName>`，每行是 `{ model: ModelIdentity, behavior_id: String, name: String }`。默认
`[]`，字段带 `#[serde(default)]`。

默认值就是 issue 要的「未命名保持编号」，而必填新字段会让该字段加入之前写下的每一份 `config.json`
解析失败、走「最新有效备份 → 默认配置」并丢掉用户设置——与 ADR-0079 §1 同一理由。

行身份用完整的 `ModelIdentity` 加 `behavior_id`，与快捷键绑定、随机播放勾选共用**同一种拼法**
（`bongocat_config::ModelBehaviorAction::behavior_id`）。这一层是刻意的：这个字符串在配置里出现于三个
地方，任何一处另写一个格式化函数就是「同一声明顺序下指两个行为」或「模型更新后指错行为」两种故障的来源，
而且两者都不会编译失败。窗口侧与应用侧都走 `bongocat-config` 那个唯一的实现。

校验：模型 id 合法、`behavior_id` 可解析、一个模型的一个行为最多一行、名字 trim 后非空、限长
`MODEL_BEHAVIOR_NAME_MAXIMUM_CHARS`（64）、不含控制字符。名字是行自己的显示文本，超长或不可打印都会变成
页面自己的问题而不是模型的问题。

#### 2. 空白名字是删除，不是名字

`set_model_behavior_name` 先删掉该行，名字 trim 后非空才写回。清空一个文本字段的含义是「回到编号名称」，
所以它是删除请求；把空字符串存下来会让页面自己判断空名字算不算名字。窗口与配置两侧各做一次 trim 与
长度限制，用的是**同一个常量**：窗口不能送出配置会拒绝的值。

#### 3. 标签优先用名字，行的其余部分一字不改

`ShortcutRow` 多一个 `custom_name: Option<String>`，`name()` 先用它再回落到编号标签。`Option` 而不是
`String`：未命名是常态，`None` 让页面不需要为它编造占位文本。

改名不动 capture target、播放控件、清除控件和已录制的组合键——它只换标签文字。应用 command 因此
不需要碰 shortcut table，改名期间正在录制的组合键也不会被取消。

#### 4. 改名在行内完成：名称后面一个编辑图标，只有图标开启编辑

这是本 ADR 唯一的非显然取舍。

最初的决定是独立弹框，理由是行的 chord frame 在框内任意点击都开始录制、Enter 也是，把一个文本框放进
这一行似乎就要与「按 Enter 开始录制」的控件共享焦点与 tab 顺序。但那个共享其实不存在：行的标签和
chord frame 是**兄弟**而不是父子，键盘事件只沿焦点路径冒泡，字段获得焦点时 frame 的按键处理器根本
不在路径上。为一个不存在的冲突付出弹框的代价不划算——弹框把「要改的名字」和「正在看的名字」拆到两个
表面上，用户改完还要自己确认改的是哪一行。

因此改为组件库里可编辑文本的标准形状，但入口只留一个：

- **名称后面一个编辑图标**（与模型卡片改名同一枚 `SquarePen`），点击它把名称就地换成文本框。字段预填
  该行当前的标签，用户改的是看得见的东西而不是要凭记忆重打一遍。
- **名称本身保持纯文本，不可点**。读过一遍标题就见过这种陷阱：想选中一行文字复制，落点稍偏就开始改
  名。入口只有图标一个，「看名字」和「改名字」两种意图在指针下就分开了。
- **字段自身不带任何控件**：Enter 保存，Escape 放弃，字段焦点被别处拿走视为保存——用户敲了一半的
  名字不因为看了一眼别处就被丢掉。字段仍显示该行当前标签时，保存是空操作，不发送请求。
- **一次只有一个字段打开**；打开另一行的编辑会先提交已打开的那份编辑，而不是丢弃它。
- 字段宽度、字号与行高复用模型改名输入框的常量：两个改名输入框长得不一样会让同一段文字读起来像
  两种不同的值。

行的三个控件（录制、播放、清除）位置、tab 序号与 `shortcut_page` 的既有测试全部保持不变——改名面
是**从名称长出来的**，不是这一行的第四个控件；编辑图标也不是 tab stop。编辑图标只带一个词的提示
（「重命名」），改名面没有别的说明文案：空字段恢复编号名称这条规则写在配置契约里，而不是写进界面。

代价要说清楚：**这一行没有第四个 tab stop**，所以纯键盘用户无法只靠 Tab 到达改名。本项目按 ADR-0054 的
visual-first 约定不维护辅助技术树与隐藏动作，可见标签就是入口；这是一处有意识的取舍，不是遗漏。

#### 5. 名字随模型走，删除模型时一并删除

snapshot 只带**当前模型**的已命名行，由配置的 catalog entry 过滤掉该模型已不再声明的行为——是否还存在是
模型包的属性，一个指向已不存在的名字的行在页面上无处安放。

删除导入模型时清掉它的名字，与 `last_expressions`、`gamepad_auto_switch` 的清理走**同一次 commit**：
分开写会留下一个「配置里还记着一个 store 已经没有了」的窗口。内置模型与导入模型同名时保留内置那一行，
因为那指的是另一个模型（与记忆表情的既有规则一致）。

### 后果

- 用户可以为每个动作和表情起自己的名字；未命名的行显示与升级前完全相同的「动作 N / 表情 N」。
- 快捷键页面的三个控件、tab 序号、录制语义、播放与清除都不变。
- 名字是纯显示文本：runtime 不感知它，改名不触碰 shortcut table，也不取消正在进行的录制。
- 自动化契约全部落在真实实现上：
  - `bongocat-config`（`a_configuration_written_before_behaviors_could_be_named_still_loads`、
    `a_behavior_name_is_parsed_bounded_and_held_once_per_behavior`）从序列化后的当前默认里删掉该字段，
    模拟旧文档的字节，并钉住解析、长度、可打印性与「一个行为一行」；
  - `bongocat-app`（`a_behavior_name_is_stored_and_read_back_for_its_own_model`、
    `renaming_the_same_behavior_replaces_its_row`、
    `clearing_a_behavior_name_removes_its_row`、
    `the_same_behavior_of_two_models_is_two_rows`、`a_name_is_trimmed_before_it_is_stored`）覆盖写入、
    覆盖写、清除、跨模型与 trim；
  - `bongocat-ui`（`a_named_behavior_shows_its_name_instead_of_its_number`、
    `an_unnamed_behavior_keeps_its_numbered_label`、
    `a_name_only_lands_on_the_behavior_it_belongs_to`、
    `naming_a_behavior_leaves_its_binding_and_play_control_alone`、
    `an_application_command_is_never_named`、
    `a_stored_name_is_bounded_and_carries_no_control_characters`、
    `a_confirmed_rename_sends_the_row_identity_it_was_opened_from`、
    `the_pencil_opens_the_field_the_label_opens`、
    `escape_leaves_the_field_without_writing`、
    `saving_an_unchanged_label_writes_nothing`、
    `the_field_replaces_the_label_and_the_row_keeps_its_controls`、
    `opening_another_rows_editor_commits_the_one_that_was_open`）覆盖标签解析、名字不落在别的行为上、
    「改名不动绑定与播放控件」、命令自身的身份与 revision、窗口与配置共用同一个长度上限，以及渲染出来的
    行内编辑：点标签、点图标、Enter、Esc、空操作与提交已打开的那份编辑；
  - `shared/config/fixtures` 新增 1 份 accept 与 5 份 reject 用例（无法解析的 `behavior_id`、空白名字、
    含控制字符、同一行为两行、非法模型 id）。
- 共享 fixture 只有 `default.json` 增加该键：新增字段可选，其余 48 份保持没有该键的样子，正是它们要验证的
  「旧文档仍能读取」。
- `spikes/config-store` 保留自己的 `NativeConfig` 副本，并断言它序列化后与 `shared/config/fixtures/
  default.json` 逐字相同，所以产品默认值新增字段时这里也必须同步。`just check` 不覆盖 `spikes/`，
  漏掉它只有一个 CI job 会报，所以它是单独写的。
- 未运行实机验证：只在本机（macOS）跑了 `just check` 与 `tools/` 下的校验脚本，没有做 macOS 或 Windows
  实机 smoke。行内字段在 800×600 与 Windows 125/150/200% 下的观感、以及「标签可点击」这一入口的可发现性
  需要实机确认。

### ADR-0086 · 帧 smoke 校验不再要求「采到一个半透明像素」

**状态：** 已接受（2026-10-07）

补充：ADR-0055（导入模型的封面由渲染截取）、ADR-0030（实现决策阶梯）
需求：用户报告——两个 Mver 模型同时导入后，后导入的那个启用时频繁提示「无法启用所选模型」，连续点击多次才偶尔成功；单独导入任一个则正常。

### 决策

#### 1. 删掉 `translucent_pixels > 0` 这一条，保留其余三条

`validate_frame_smoke` 现在只要求：

- 至少一个透明像素——模型之外确实还没被画过，能区分「一片不透明的垃圾表面」；
- 至少一个模型像素（不透明 + 半透明 > 0）——确实画了东西；
- 至少两种可见颜色——是一张图，不是一块纯色填充。

这三条已经完整覆盖这条校验真正要回答的问题（「这块表面是不是一张图」）。删掉的那一条既不能补充这个
答案（未初始化的垃圾表面照样能满足它，纯色填充则被第三条拦住），又会让合法模型失败。`FramePixelStatistics`
仍统计 `translucent_pixels`，它只作诊断数据，不再作判据。

#### 2. 保留「校验失败即拒绝这次模型提交」的语义

没有把失败改成「当作临时不可用去重试」。真正的空表面（无模型像素）在无合成器环境里是稳定状态，
ADR-0055 残余风险 5 已经把「失败」当作正确信号；无限重试只会把它变成卡住的窗口。本次只消除误判。

### 后果

- 与该模型同类的模型（单张大纹理、硬 alpha 边缘、画布留白多）现在第一次启用即可成功，不再需要连点。
- 预置模型与既有行为不变：仍要通过同三条判据，`just` 的预览与封面截取路径不受影响。
- 自动化契约：
  - `bongocat-overlay` 新增 `frame_smoke_accepts_a_frame_without_a_semi_transparent_sample`
    （只有透明 + 不透明采样时通过，钉住本 ADR），原拒绝用例改为钉住三条判据各自的消息；
  - `frame_smoke_requires_transparent_and_antialiased_model_coverage` 保留，半透明采样仍然算作合法采样。
- `Renderer readback` 的读回与统计口径不变，双后端（Metal / D3D11）共用同一个函数，Windows 侧同步生效。

### 明确不做

- **不调网格密度或改成自适应采样**：网格是 smoke 的固定口径，加密只会把「有时采不到」变成「更晚才采不到」，
  不改变判据本身的错误。
- **不给这条校验加重试次数**：issue 要求的是找出冲突原因，加重试是把误判藏起来。
- **不改 `reject_model_commit` 的语义**，也不动 `RUNTIME_TIMEOUT`（app 侧 2 s / overlay 侧 250 ms）。
- **不在本次处理 8192² 纹理与 106 张按 drawable 尺寸分配的 mask 纹理带来的显存占用**：实测单次 overlay
  已占约 620 MiB，切模型时新旧两份并存。这是独立的资源问题，见残余风险 1。

### ADR-0087 · 嵌套模型文件夹发现与顺序导入

**状态：** Accepted（issue #1105）

依赖：ADR-0037、ADR-0047、ADR-0055、ADR-0059

### 决策

- settings worker 调用 model store 只读扫描所选文件夹。普通包仍由 `PreparedModel` 与既有输入模式分类验证，Mver 源仍由现有内容检测识别。识别到模型根后停止向其内部寻找其他模型，避免把 Mver 的模式资源当成独立模型。
- 扫描复用包的目录深度和文件数量限制（目录也计入扫描条目预算），按相对路径排序。拒绝符号链接、非普通文件/目录、路径逃逸和包含目标 store 的来源；不写来源、不创建 staging。没有候选时保留原有普通包错误路径；直接模型根保留原有导入行为。
- 嵌套单模型自动进入原流程；多个模型用 GPUI Kit Dialog + Checkbox 选择，显示所选文件夹名与候选相对路径，默认全选，空选禁止确认。模型选择与模式选择共用弹框构建器，列表高度受窗口约束并支持滚动。
- 所选模型按显示顺序排队，每个模型重新检查来源并依次完成模式选择、导入和封面截取，再处理下一个。取消当前导入或关闭选择弹框终止剩余队列；失败报告通知并继续后续独立模型，成功结果不回滚。取消模式选择终止剩余队列。
- 队列仅是 UI 临时草稿，不持久化。所有复制、验证、转换、提交仍在 settings worker，使用已有单模型 command 与随机 UUID，不引入配置字段、依赖、平台 API 或第二种导入来源。

### ADR-0088 · Concurrent motion playback

**状态：** —

- Status: accepted
- Date: 2026-10-08
- Depends on: ADR-0061, ADR-0012 (ordered motion audio service)
- Issue: https://github.com/ayangweb/BongoCat/issues/1107

### Decision

Add `model.allow_motion_overlap`, a boolean defaulting to false and using
`#[serde(default)]` for existing v1 documents. The Model behavior page exposes
the setting on both supported platforms through a typed, revision-checked command.

When enabled, distinct motion identities coexist. Every layer owns its clip,
start time, natural completion, explicit fade-out and UserData cursor. Evaluate
layers in accepted start order, oldest first. Restarting a matching identity moves
it to the newest position; idempotent repeats and stops do not reorder layers.
Use insertion order rather than timestamps so starts at the same clock sample
remain ordered. model3 has groups rather than a dedicated stacking priority, and
the standard preset's groups reference the same files. Sorting by group name
would let a completed later-sorted layer hide every replay from an earlier group.
Later layers blend over earlier layers using the existing Cubism curve weights.
Distinct parameters remain independent. This does not make conflicting writes
additive or introduce a new model format.

A repeated product trigger for the same unfinished motion at the same priority
remains a no-op. A preview restarts only its matching layer. A completed one-shot
keeps its naturally faded terminal pose, as required by ADR-0061, and can replay.
Stops match one identity and remove only that layer after its own fade. Priority
arbitration applies to that identity in overlap mode, and to the current motion
in replacement mode. Automatic idle motions still yield to any unfinished manual
motion. Turning overlap off keeps the most recently started layer immediately;
successful model commits and shutdown clear every layer. Failed model preparation
preserves all current layers.

The runtime publishes all active motion identities. Its existing `active_motion`
view is derived from the most recently started surviving identity for consumers
that only display one motion. Renderer UserData carries its originating identity.
Audio retains the single ordered worker: each accepted start uses the existing
replace-audio rule, and an accepted stop uses the existing stop-audio rule.

### Validation And Remaining Work

Phase 3 playback and Phase 4 settings scope: verify old-document loading,
typed settings persistence, independent evaluation/fades/stops, deterministic
precedence in both start orders, standard-preset cross-group replays over held
terminal poses, repeated-trigger audio suppression, model cleanup and opt-out behavior.
Run workspace quality gates and macOS product smoke. Windows product smoke and
real-model visual comparison remain platform validation tasks until recorded;
this change does not close the broader Cubism compatibility or release gates.

---

# 四、输入 / 交互 / overlay

> 键位图、快捷键、手柄、overlay 放置与交互契约。

### ADR-0012 · Ordered Motion Audio Service

**状态：** Accepted

> 后续修订（2026-10-12）：`rodio` 由 `=0.22.2` 升级到 crates.io 精确 pin `=0.23.0`。
> 该 release 把 `cpal` 由 `0.17.3` 升到 `0.18.2`，并把此前分散的 cpal 错误类型
> （`StreamError`、`BuildStreamError`、`PlayStreamError`、`DevicesError`、
> `DefaultStreamConfigError`、`SupportedStreamConfigsError`）统一为 `cpal::Error`，以
> `CpalError`/`CpalErrorKind` 重导出，`rodio::DevicesError` 被移除。项目只使用 `Decoder`、
> `SamplesBuffer`、`DeviceSinkBuilder`、`Player` 与 `rodio::cpal::BufferSize`，不匹配这些
> 错误类型，源码零改动。仍然只启用 `playback` 与 `flac`，不改变本 ADR 的替换边界。

### Decision

- 使用 crates.io 当日最新稳定版 `rodio 0.23.0`，精确 pin，只启用 `playback` 与 `flac`。
  不启用录音、MP3、MP4、Vorbis、WAV、dither 或其它未使用 feature。
- `bongocat-audio` 是独立 Rust owner。它持有固定容量 16 的有序 command queue、专用
  worker、decoder、output stream 和唯一 voice；runtime 只非阻塞发布强类型
  `Play`/`Stop`，不执行文件、解码或设备工作。
- motion request 只有在 runtime priority 与 Live2D resource 校验均接受后才触发音效。
  新 motion 替换当前 voice；无声音的新 motion 也停止旧 voice。显式 motion stop、关闭
  配置、成功 model commit 和 shutdown 立即停止 voice，不等待 motion fade 结束。
- 模型 activation 在 renderer prepare 成功后，把该模型的去重 sound paths 作为 `Prepare` 发送给
  audio worker。worker 在这里解码 FLAC 为 immutable PCM cache；renderer prepare 与 PCM prepare 都完成后
  才提交该模型。预热失败只形成稳定 audio 诊断，仍允许模型提交；随后 play 会按既有降级语义失败，不能阻塞
  动作或渲染。成功 commit 后 `ActivatePrepared` 只保留新活动模型的 cache。
- 已活动模型的 motion 在同一 runtime command 中先向有序队列发布缓存 `Play`，再以同一单调 tick
  启动 motion。动作路径不执行文件 I/O、解码、设备创建、`get_pos()` 等待或 1 ms 轮询。audio owner
  优先以 512-frame buffer 打开默认设备，设备拒绝时回退 rodio 默认 sink。
- 音量使用经过校验的 `[0, 1]` 强类型值，当前产品触发值为 `1.0`。同一时刻最多一个
  motion voice，不混音、不排队延后播放。
- 队列满载不阻塞 runtime：返回原 command、增加匿名 overflow 计数、丢弃无法证明
  顺序的 backlog 并强制停止 voice。文件 I/O、解码和输出故障进入稳定 error code 与
  聚合诊断，后续 command 可以恢复服务；诊断不包含用户路径或音频内容。
- 应用先停止输入与 runtime，再停止并 join 音频 worker；worker 在返回 `Stopped` 前
  停止 voice、丢弃 pending command 并释放 rodio/CPAL output owner。

### Dependency Boundary

`rodio 0.23.0` 声明 `MIT OR Apache-2.0`，Rust 1.87+，仓库在审计日仍活跃；底层 CPAL
支持 Windows/macOS 系统输出。rodio 0.23.0 把 `cpal` 升到 `0.18.2`，lockfile 因此解析为
`0.18.2`；项目不直接依赖或 fork CPAL 来伪造版本升级。

rodio/CPAL 类型不离开 `bongocat-audio` 私有 backend。替换其它音频库时只需保持
`MotionAudioClient`、强类型 command、诊断和 shutdown contract，不修改 runtime、模型
或 UI 公共协议。Linux 首发后评估时再启用对应 output target；当前 Linux contract build
不链接 ALSA，也不把 Linux 音频变成 Windows/macOS 首发阻塞。

### Consequences

音频解码成本转移到模型 activation；设备打开继续由 audio owner 惰性执行，但 runtime 不等待它。
`rodio::Player::get_pos()` 只反映 mixer 位置，不能作为扬声器起播确认，因此不再用作动画时钟。当前实现支持现有预置模型实际使用的 FLAC；
新增格式必须先形成模型兼容需求，再单独启用 decoder feature 和测试。默认设备热切换、运行中 stream
error callback、端到端音频/画面时序测量、100 次 model/audio 资源测量与 8 小时 soak 仍属于后续
稳定性验收，不由本 ADR 的单元测试替代。

### ADR-0018 · Typed Model Interaction Settings

**状态：** 已接受（2026-09-01；2026-09-25 增加键盘/手柄模型输入门禁；2026-09-27 指针跟随按轴拆分）

### 决策

- `bongocat-runtime::ModelSettings` 是六项模型交互设置的唯一 runtime 类型，并通过
  `RuntimeCommand::SetModelSettings` 写入、`RuntimeSnapshot::model_settings` 读回。
- `mirror` 由 runtime 写入 `RenderSnapshot::mirror_horizontal`；Windows D3D11 和 macOS
  Metal 只依据该不可变字段对模型中心执行水平变换，不读取配置或 UI 状态。
- 指针跟随按轴拆成 `mirror_pointer_tracking_horizontal` 和
  `mirror_pointer_tracking_vertical`，两项互相独立、默认均为 `false`：水平项在产品参数覆盖阶段
  反转指针的 X/Z 值（Z 与 X 是同一水平扫视的两个方向），垂直项只反转 Y 值。拆成两个布尔值而不是
  一个方向枚举，是因为「左右相反」和「上下相反」可以只出现其一、也可以同时出现，枚举无法在不增加
  第三种取值的前提下表达这四种组合；`ignore_pointer` 仍然跳过所有指针参数覆盖，使 Core 默认值或
  前序动画值保持有效。
- `ignore_keyboard` 和 `ignore_gamepad` 是模型输入投影门禁：分别屏蔽键盘的键图/手部贡献，
  以及手柄的按钮/手部/摇杆/扳机贡献。它们不停止平台采集、pressed-state 校正与复位、
  匿名诊断或独立的快捷键注册；原始 pressed state 始终保留，解除门禁后仍能正确处理释放。
  三个模型输入忽略开关各自有一个可选的 application shortcut target，触发后仍经 settings
  service 的 typed command 和配置 revision 原子持久化。
- 应用启动在配置验证后发送一次 typed settings command。动态设置页编辑、revision-checked
  持久化 command 和完整多显示器/实机输入证据由后续 TODO 继续跟踪。

### 后续边界

本 ADR 不改变 physics/pose 的授权 fixture 和行为求值门禁，不声称完整 Live2D 兼容；
也不定义快捷键冲突、用户编辑或平台全局 hotkey 注册协议。

### ADR-0031 · Tray Icon and Menu Library Boundary

**状态：** 已接受（2026-09-13）；2026-09-14 补充 Windows `set_tooltip` 上游缺陷与 tooltip 创建期不变量；2026-09-25 接受托盘与 overlay 右键菜单复用（ADR-0068）

> 后续修订（2026-09-23）：ADR-0033 已退役 Windows 原生 ARM64；本文提到的 Windows x64/ARM64 cross-check 只是历史证据，当前 Windows 发布 target 只有 x64。
>
> 后续修订（2026-10-12）：`tray-icon` 由 `=0.25.0` 升级到 crates.io 精确 pin `=0.26.1`，
> `muda` 由 `=0.20.0` 升级到 `=0.21.2`。两者仍关闭默认 features、仍只在 macOS/Windows target
> 声明，并仍解析到同一份 `muda` package；本 ADR 的替换边界与安全不变量不变。
>
> - v0.26 弃用了 `TrayIconBuilder::with_icon_as_template(bool)`（`#[deprecated(since = "0.26.0")]`），
>   改为 macOS 专属的 `with_icon_templated(icon)`，它在一次调用里同时设置图标并标记 template。
>   adapter 相应改为在 macOS 上调用 `with_icon_templated`、其它平台调用 `with_icon`；template
>   语义与原先「先 `with_icon` 再 `with_icon_as_template(true)`」一致。
> - 源码复核确认「Windows `set_tooltip` 对以固定 GUID 注册的图标必然失败」的上游缺陷在 `0.26.1`
>   **仍未修复**：`set_tooltip` 仍只设 `NIF_TIP` 后直接 `NIM_MODIFY`，而 `set_icon`、
>   `set_tray_visible`、`register_tray_icon` 与 `remove_tray_icon` 都经 `apply_guid` 补
>   `NIF_GUID`。因此本文「`SystemMenuPresentation::tooltip` 是创建期输入」的不变量继续成立，
>   实现未改动，Windows 实机验收仍是发布门禁。
> - v0.26 把 macOS 图标高度从硬编码的 18pt 改成 `MAX_ICON_HEIGHT = 22.0` 上限，且
>   `RgbaIcon::to_nsimage` 只缩小不放大；`TrayIconBuilder` 与 `Icon` 都没有尺寸 API，像素数
>   直接当点数用，因此任何大于 22px 的画布都会被画成 22pt。项目原来的 256×256 蒙版占满画布，
>   菜单栏图标随之从 18pt 变成 22pt。为保持更新前的视觉尺寸，`tray-macos.png` 重绘为
>   「图形按 18/22 缩到 210×210 并居中、四周透明留白」的 256×256 画布，渲染回 18.05pt。
>   代价是状态栏占位仍为 22pt 宽，图形左右各多约 2pt 空隙；PNG 容器契约（256×256、
>   8-bit RGBA、非隔行、deflate）不变。
> - 本文「调研结论」与「安全与生命周期不变量」中仍写有 `0.25.0`/`0.20.0` 的段落均为
>   2026-09-13/2026-09-14 的升级前复核记录，保留原样；除上述 `set_tooltip` 缺陷外，本次未在
>   Windows 实机复核 `set_visible` 等 shell 行为，这些不变量在 `0.26.1` 上仍待实机确认。

### 决策

- macOS 与 Windows 状态图标统一由 `bongocat-platform` 私有 `system_menu_native` adapter 管理：
  `tray-icon 0.26.1` 是托盘/菜单栏图标的 native owner，直接依赖的 `muda 0.21.2` 是共用 popup 根
  和菜单事件的 owner；旧 `system_menu_macos.rs`、`system_menu_windows.rs` 与 `tray-windows.ico` 退役。
- adapter 是 `SystemMenu` 的唯一 owner，长期持有 `TrayIcon`、一个托盘/模型窗口共用菜单根、模型窗口
  子菜单、全部可变菜单项和 `muda` 菜单事件 receiver。`set_visible(false)` 只改变平台表示（macOS 移除
  `NSStatusItem`，Windows 保留注册并设置隐藏），菜单 owner 与事件通道不销毁；重新显示不创建第二套
  业务状态。
- Windows 使用固定 GUID 与 `tray-windows.png`；macOS 使用 `tray-macos.png` 并标记为 template
  image。两平台在菜单构建时共享相同 action id 到项目自有 `SystemMenuAction` 的映射。
- Windows 左键抬起映射为 `OpenSettings`，右键由 `tray-icon` 弹出共用菜单。macOS 由 `tray-icon`
  在左右键时弹出同一菜单；模型窗口右键也调用同一个菜单根。菜单按应用任务组织为「设置 → 模型窗口
  子菜单（隐藏模型窗口、穿透、置顶、鼠标移入隐藏）→ 可用的检查更新 → 退出」；模型窗口显隐的勾选
  表示当前已隐藏，不再提供单独的“显示模型窗口”项；其它模型窗口属性直接复用偏好设置已有文案。
  不可用的更新入口在构造期不创建，两个入口都使用同一组 action id 和同一个事件队列。
- overlay 右键由 adapter 的 `show_context_menu_for_window` 从 overlay session 获取真实 Windows
  HWND 或 macOS content `NSView`，再调用对应的 `muda::ContextMenu`；不得继续借用托盘隐藏 HWND，
  也不得由 overlay 自己创建第二个 owner。
- 第三方 `tray_icon`/`muda` 类型、platform handle、错误和回调不得进入 `bongocat-runtime`、
  `bongocat-ui` 或公共协议；adapter 只暴露项目自有的 `SystemMenu`、`SystemMenuAction`、
  `SystemMenuError` 和 `SystemMenuPresentation`。
- 新依赖精确固定为 `tray-icon = "=0.26.1"` 与 `muda = "=0.21.2"`，两者都关闭默认 features，
  只在 macOS/Windows target 声明。直接 `muda` 必须与 `tray-icon` 实际依赖的版本解析为同一
  package，避免菜单类型来自两个版本；升级时以 `cargo tree` 复核。
- PNG 格式契约由 `bongocat-app` build script 与 `product_icon_contract` 测试固定：PNG 签名、
  IHDR、尺寸、通道位深、RGBA、压缩/过滤方法和隔行字段，不把 `image` 类型暴露为产品 API。ADR
  记录字节 hash 作为来源与变更审计证据，不把 hash 写成运行时业务校验。
- `SystemMenuPresentation::title` 保留为兼容输入，但迁移后不映射到 native 菜单标题；菜单中的
  可见本地化文本、状态和可用性仍逐项同步。若未来产品需要可变菜单标题，必须先确认 `muda` 的
  上游 API，不得绕过安全边界直接修改库内部 `NSMenu`。

### 安全与生命周期不变量

- macOS 创建、更新、弹出菜单和销毁均要求 `MainThreadMarker`；`set_presentation` 也执行主线程
  检查，避免在错误线程触碰 AppKit。
- Windows `show_context_menu_for_hwnd` 需要在 `unsafe` block 中传入有效 HWND。应用窗口右键传入
  由 overlay session 持有、覆盖整个同步 `TrackPopupMenu` 生命周期的真实 overlay HWND；不得使用
  托盘隐藏窗口作为替代 owner。菜单 tracking 是同步调用，调用方不得在回调内销毁 overlay 或 adapter。
- macOS `show_context_menu_for_nsview` 要求 main thread 和有效 `NSView`。应用窗口右键传入
  overlay panel 的 content view；panel 保留该 view，调用期间也由 overlay session 保持存活，
  同步弹出结束后才返回。
- `SystemMenu` 的字段顺序保证 `tray_icon` 先于菜单项与根菜单析构；显式 shutdown 先隐藏图标，
  再按既定产品顺序停止输入、runtime、配置、GPU 和 overlay。禁止在多个线程或模块创建第二个
  `SystemMenu`。
- Windows adapter 捕获 `TrayIconEvent` 时按自有的稳定 `TrayIconId` 过滤，不消费其他 owner 的
  事件；菜单事件 receiver 也只在同一个 app owner 线程轮询并转换为强类型队列。
- `tray-icon 0.25.0` 的 Windows `set_visible` 会发送 `WM_USER_SHOW_TRAYICON` 并在库内部忽略
  `Shell_NotifyIconW(NIM_MODIFY)` 的失败结果，可能对已失效的 shell registration 返回 `Ok`；
  同样，初次 `NIM_ADD` 失败会等待 `TaskbarCreated` 恢复。adapter 的 `Ok` 不能替代真实托盘
  可见性验收，发布前必须在 Windows 10 1903+ 实机验证隐藏、恢复、Explorer 重启和右键菜单。
- `show_taskbar_icon` 同时出现在三个位置：runtime 发布的 settings snapshot、产品记录的已应用
  值，以及 shell 对两个产品窗口 taskbar button 的**实时**读数。设置写入按"先原生面、再配置、
  最后重新发布 snapshot"三步到达，因此写入与携带它的 snapshot 之间三者**本应**不一致；shell
  对 `WS_EX_TOOLWINDOW` 的可见性读数也会滞后于产品已施加的窗口样式。任何断言这三者一致的
  检查都必须是有界轮询等待收敛，而不是采样一次——采样一次正是 system menu smoke 的启动断言
  在 Windows runner 上偶发失败的原因（`startup taskbar visibility diverged`）。轮询预算只界定
  等待时长，不豁免结果：预算内不收敛仍然是失败，且必须报告具体是哪一处未收敛。判定逻辑放在
  平台无关的 `taskbar_settle` 里，以便在任意平台单测。
- `tray-icon 0.25.0` 的 Windows `set_tooltip` 对以固定 GUID 注册的图标必然失败：它发出的
  `NIM_MODIFY` 未带 `NIF_GUID`，而 shell 对以 `guidItem` 标识的图标忽略 `uID`，并要求后续每次
  `Shell_NotifyIcon` 调用都携带同一 GUID
  （<https://learn.microsoft.com/windows/win32/api/shellapi/ns-shellapi-notifyicondataw#troubleshooting>）。
  库内 `set_icon` 与内部 `set_tray_visible` 都调用了 `apply_guid`，只有 `set_tooltip` 漏掉，属上游
  缺陷；上游 `dev` 分支同样如此，`0.25.0` 已是 crates.io 最新稳定版，没有可升级的修复版本。
  因此 `SystemMenuPresentation::tooltip` 是**创建期输入**：由 `start_with_presentation` 经
  `with_tooltip` 一次性写入（该路径的 `NIM_ADD` 携带 `NIF_GUID`，可正常工作），
  `set_presentation` 不得再调用 `set_tooltip`。当前产品文案 `system_menu.title` 在中英目录中
  恒为 `BongoCat`，运行期不发生变化；若将来需要运行期更换 tooltip 文案，必须连同 GUID 一起
  重建托盘 owner，或先确认上游已修复该调用。

### 替换边界

替换点只有 `bongocat-platform` 私有 `system_menu_native` adapter 和 overlay 的
`HasWindowHandle` 实现。升级或替换 `tray-icon`/`muda` 时必须复验：两者仍解析到同一 `muda`
package、托盘与 overlay 共用 popup 根的 action 投影与层级、菜单项/根的析构顺序、Windows 固定 GUID、
GUID 注册下的 `set_tooltip` 是否已修复、左键/右键行为、macOS template image 与主线程约束、overlay
HWND/`NSView` 生命周期、cursor 定位、隐藏后的恢复、shutdown 清理、依赖 target/features、Windows shell
failure 语义和 PNG 资源格式。adapter 之外不得依赖第三方 tray/menu 类型。

### ADR-0038 · 左右 Alt 的键位图命名与旧名兼容

**状态：** 已接受（2026-09-17）

依赖：ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0036（模型导入来源识别与 staging 边界；
**该 ADR 已于 2026-09-22 撤回**）、ADR-0004（可校正输入状态）

修订（2026-09-22）：ADR-0036 撤回后模型来源只剩目录，本文「两种来源得到同一结果」「用户选中的
源目录/归档始终只读」里的归档一半不再适用；调用点也从「目录复制与归档解压之后」变成「目录复制
之后」，`commit_installed_staging` 之前的位置不变。**命名决策本身未变。**

### 决策

#### 1. 预置模型按 canonical 名重命名

`resources/models/{standard,keyboard}/resources/left-keys/` 下：

```text
Alt.png   → AltLeft.png
AltGr.png → AltRight.png
```

改名用 `git mv`，字节不变；`shared/fixtures/model-fixtures/preset-model3-index.json` 是
`spikes/model-package` 解析器产出的冻结快照，同步更新对应 `id` 与 `file`（重跑该解析器逐字段
比对通过）。文件名顺序在 ASCII 下不变（`.` < `L`），因此 `left_keys` 数组次序无需调整。

不新增通用的 `Alt.png`：重命名后两侧各有专属图，家族图对预置模型已无意义。

#### 2. 运行时优先精确名，`AltGr` 是唯一的旧名遗留

`bongocat-live2d::key_name_candidates` 的候选顺序：

```text
0xe2 (左 Alt) → AltLeft, Alt
0xe6 (右 Alt) → AltRight, AltGr, Alt
```

`AltGr` 只出现在右 Alt 的候选里，且排在 `AltRight` 之后、家族名 `Alt` 之前，所以：

- 已按新名导出的模型命中自己的图；
- 未经归一化的旧模型右 Alt 命中 `AltGr.png`（正确的右侧图），左 Alt 仍走 `Alt.png`；
- 只有一张共用 `Alt.png` 的模型两侧都还能画出来。

保留 `AltGr` 而不是让未归一化模型"降级为共用图"，是为了让**已经安装过**的模型不出现回退：
导入归一化只作用于新导入，不会回头改写用户数据根里已存在的模型目录，而这些目录
没有任何迁移路径可用（AGENTS §4.1）。

`Alt`/`Control`/`Shift`/`Meta` 家族回退保持原样；本 ADR 不改变 `ControlLeft`/`ShiftLeft` 等
既有行为。

#### 3. 包导入时归一化，且在 store 自己的 staging 上做

`bongocat-model-store` 持有 `key_names` 模块：把 staging 树里 `resources/{left-,right-}keys/` 下的
`Alt.png`、`AltGr.png` 改名为 canonical 名。调用点在目录复制**之后**、共用的
`commit_installed_staging` **之前**，因此：

- 用户选中的源目录始终只读，与 ADR-0036 的边界一致（该 ADR 的归档一半已于 2026-09-22 撤回）；
- 改名不改变文件数与字节数，`ModelImportProgress` 的计数仍然真实。

冲突不猜：canonical 文件已存在时保留它，旧名文件原样留下——运行时优先 canonical，模型照常
可用。只处理 `resources/*-keys` 下的一层常规文件；目录同名或其它位置出现 `Alt.png` 交给既有的
包校验按它自己的规则拒绝。

Mver 转换路径**不做**这层归一化：转换输出已经是产品词汇表（ADR-0037 §7），它写出的每个名字都是
从 HID usage 反推出来的，没有"旧名"需要翻译。

#### 4. Mver 的 Alt 映射：有侧信息就用侧，没有就两侧都写

`mver::legacy_key_names` 取代原来的单值 `legacy_key_name`：

| 控制码 | 输出 | 依据 |
| --- | --- | --- |
| `0x12` `VK_MENU`（教程图的 `18`） | `AltLeft` + `AltRight` | 旧表把两侧都记为该码，转换把同一张合成图同时装到两侧，既不丢行为也不再用共用的 `Alt` 名 |
| `0xA4` `VK_LMENU` | `AltLeft` | 手写键位表可以点名一侧；与该码在产品 Windows 输入层（`bongocat-platform`）的含义一致 |
| `0xA5` `VK_RMENU` | `AltRight` | 同上 |

一个绑定因此可以产出多于一个目标名；重复目标仍按"先到者胜"去重，且同一次绑定的合成图只做一次
编码（`MverSlotImage` 复用）。

`0x10`（Shift）、`0x11`（Control）与 `0x12` 属于同一类歧义，但**本次不改**：它们保持
`Shift`/`Control` 家族名，运行时会为两侧解析该家族图，且真实样本（`bongo_cat_mver_0.1.6_64`）
的转换结果 `left-keys{Control, KeyR, Shift}` 已被 ADR-0037 记录为验证证据，改动它会同时作废那份
证据。若将来要统一，应作为一次单独改动连同该证据一起更新。

### 明确不做

- **不新增 `Alt` 家族图的兼容 alias 以外的键名**：不引入 `LeftAlt`/`RightAlt` 之类第三套拼写。
- **不就地升级已安装模型**：本产品不做迁移；既有安装靠运行时的 `AltGr` alias 保持正确。
- **不改 `Control`/`Shift`/`Meta` 的候选顺序与家族回退**（理由见决策 4）。
- **不把归一化做成模型加载期的动态映射**：磁盘上的包应当是产品自己的词汇表，否则每个消费方都要
  再实现一遍旧名识别。

### ADR-0039 · 主 Enter 与小键盘 Enter 的键位图命名与旧名兼容

**状态：** 已接受（2026-09-17）

依赖：ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0036（模型导入来源识别与 staging 边界；**该 ADR 已于 2026-09-22 撤回**）、ADR-0004（可校正输入状态）

修订（2026-09-22）：ADR-0036 撤回后模型来源只剩目录，本文决策 4 里「目录与 `.zip` 两种来源」的
归档一半不再适用。**命名决策本身未变。**

### 决策

#### 1. 命名方案

| 物理键 | HID usage | canonical 名 | 说明 |
| --- | --- | --- | --- |
| 主 Enter | `0x28` | `Enter` | 取代 `Return`，与 HID 语义一致 |
| 小键盘 Enter | `0x58` | `KpEnter` | 沿用 Mver 转换词汇表已有的 `Kp` 前缀（`Kp0..Kp9`、`KpMultiply`、`KpPlus`、`KpMinus`、`KpDecimal`、`KpDivide`） |

两个名字分属不同 HID usage，候选列表互不重叠，不存在解析冲突。

#### 2. 运行时：精确名优先，`Return` 是第二个旧名遗留

`key_name_candidates` 的候选顺序：

```text
0x28 (主 Enter)   → Enter, Return
0x58 (小键盘 Enter) → KpEnter, Enter
```

- `Enter` 排在 `Return` 之前：新导入与预置模型命中 canonical 名；未经归一化的已安装模型
  落到 `Return.png`，主 Enter 照常画图（与 ADR-0038 保留 `AltGr` 的理由相同——本产品没有
  就地迁移路径，旧安装只能靠运行时 alias 保持可用）。
- `KpEnter` 排在 `Enter` 之前：模型画了专用图就用专用图；没画就回退主 Enter 的图。回退在
  `key_name_candidates`（资源加载层）实现，不要求模型额外提供图片；模型补了 `KpEnter.png`
  后自动优先。`Return` 不出现在 `0x58` 的候选里——它从来不是小键盘 Enter 的名字。

#### 3. 预置模型重命名，冻结快照逐字段同步

`resources/models/{standard,keyboard}/resources/left-keys/` 下 `Return.png → Enter.png`
（`git mv`，字节不变）。`shared/fixtures/model-fixtures/preset-model3-index.json` 同步更新
`id` 与 `file` 两处；条目按 id 字母序移到 `Delete` 与 `Escape` 之间。`spikes/model-package`
解析器的 `preset_model_indices_match_frozen_snapshot` 重跑通过即逐字段相等（顺带清掉了
工作树里未跟踪的 `resources/models/standard/resources/.DS_Store`，与 ADR-0038 记录的是
同一个干扰项）。预置模型不新增 `KpEnter.png`：没有专属 artwork，回退到 `Enter.png` 就是
期望行为。

#### 4. 包导入归一化扩展到 `Return`

`key_names::LEGACY_KEY_IMAGE_NAMES` 增加第三项 `("Return", "Enter")`，沿用既有规则：只处理
`resources/{left-,right-}keys` 一层常规文件，canonical 文件已存在时保留它、旧名原样留下，
用户选中的源永远是只读输入。

#### 5. Mver 转换：`VK_RETURN` 展开成两个名字

`legacy_key_names` 对键盘模式的 `0x0D` 返回 `["Enter", "KpEnter"]`（`legacy_key_name` 单值
表从 `Return` 改为 `Enter`，供测试与文档固定词形）；同一张合成图装到两个名字下，运行时按
实际按下的键各自解析。这与 `VK_MENU → AltLeft + AltRight` 的展开完全同类，gamepad 模式
不受影响（`13` 仍是 `DPadDown`）。

`0x10`（Shift）、`0x11`（Control）仍保持家族名不变——理由与 ADR-0038 决策 4 相同：改动会
移动真实样本的转换产物证据，留给单独的改动。

### 明确不做

- **不引入 `NumEnter` 之类第三套拼写**：`Num` 已被数字行占用。
- **不就地升级已安装模型**：既有安装靠运行时的 `Return` alias 保持可用（同 ADR-0038）。
- **不在本次把 `Kp0..Kp9`、`KpMultiply` 等接入运行时候选**：Mver 转换早已输出这些名字，但
  `key_name_candidates` 至今不解析它们（按下的 `0x54..0x63` 无候选、不画图）。这是 ADR-0037
  记录过的同类既有缺口（功能键 `F1..F12` 逐键图不可达同款），仓库内没有模型画过这些键，
  改动无真实数据可验证，记录为后续项。本次只接入小键盘 Enter，因为它有明确的产品需求。
- **不做模型加载期的动态旧名映射**：磁盘上的包应当是产品自己的词汇表（同 ADR-0038）。

### ADR-0040 · 小键盘键位图的 `Kp*` 命名与主键盘键位图回退

**状态：** 已接受（2026-09-17）

依赖：ADR-0039（主 Enter 与小键盘 Enter 的键位图命名与旧名兼容）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0004（可校正输入状态）

### 决策

#### 1. 小键盘与主键盘的对应清单

小键盘中凡是与主键盘产生**同一个字符**的键，都回退到主键盘那一张键位图；其余键没有对应关系。
（ADR-0041 补齐主键盘词表后，`KpMinus`/`KpDecimal` 也拿到了对应键，见下表。）

| 小键盘键 | HID | 主键盘对应键 | 主键盘 HID | 可复用键位图 | 回退候选 |
| --- | --- | --- | --- | --- | --- |
| NumLock | `0x53` | — | — | — | 无 |
| KpDivide `/` | `0x54` | `/` Slash | `0x38` | `Slash.png` | `Slash` |
| KpMultiply `*` | `0x55` | `Shift`+`8`，无独立键 | — | — | 无 |
| KpMinus `-` | `0x56` | `-` Minus | `0x2d` | 尚无模型提供 `Minus.png` | `Minus` |
| KpPlus `+` | `0x57` | `Shift`+`=`，无独立键 | — | — | 无 |
| KpEnter | `0x58` | Enter | `0x28` | `Enter.png` | `Enter` |
| Kp1 … Kp9 | `0x59..=0x61` | 1 … 9 | `0x1e..=0x26` | `Num1.png` … `Num9.png` | `Num1` … `Num9` |
| Kp0 | `0x62` | 0 | `0x27` | `Num0.png` | `Num0` |
| KpDecimal `.` | `0x63` | `.` Dot | `0x37` | 尚无模型提供 `Dot.png` | `Dot` |

HID `0x64`（`Keyboard Non-US \ and |`）不是小键盘键，而是 ISO/ABNT2 的额外键，本 ADR 不处理。

#### 2. 候选顺序：精确名在前，主键盘对应键在后

`key_name_candidates` 的候选顺序：

```text
0x53         → NumLock
0x54         → KpDivide, Slash
0x55         → KpMultiply
0x56         → KpMinus, Minus
0x57         → KpPlus
0x58         → KpEnter, Enter
0x59..=0x62  → Kp1 … Kp9, Kp0，再补 Num1 … Num9, Num0
0x63         → KpDecimal, Dot
```

- 精确名在前：模型画了专用 `Kp*.png` 就用专用图，与 ADR-0039 对 `KpEnter` 的规则完全一致。
- 主键盘对应键在后：没有任何已知模型画过小键盘图，所以这条回退就是用户实际看到的东西。
  回退仍在 `key_name_candidates`（资源加载层）实现，不要求模型额外提供图片。模型补了 `Kp*.png`
  后自动优先，但**必须放在 `left-keys`**——`resolve_key_overlays` 只按按下的 side 查同目录资源
  （见决策 3）。
- 没有对应键的三个键只保留精确名。`*` 和 `+` 在主键盘上只能通过 `Shift` 得到（`NumLock` 则完全
  没有对应键），主键区本身没有这两个键的图，因此它们缺图时照旧不绘制。不引入"借用 `Num8` 当
  `*`"这类会误导用户的映射。
- 数字表复用：新增的 `KEYPAD_DIGIT_NAMES`（`Kp1…Kp9, Kp0`）与既有 `KEY_NUMBERS`（`Num1…Num9,
  Num0`）逐下标对齐，一个 `hid_usage - 0x59` 同时索引两张表，两张表不可能漂移。

#### 3. 小键盘整块绑到左手

`bongocat-app::input_bindings_for_model` 为键盘模型（`Installed` 或预置 `standard`/`keyboard`）
新增 `0x53..=0x63` → `HandSide::Left`。

- **这一步是必要前提，不是可选优化**：没有 hand 归属的按键在 `InputState::model_snapshot` 就被
  丢弃，回退永远不可达（事实 4）。只加候选不改绑定会得到一段在实机上跑不到的死代码。
- **为什么是左手**：`resolve_key_overlays` 只按按下的 side 查同目录资源，而可复用的
  `Num*.png`/`Enter.png`/`Slash.png` 只存在于 `left-keys`；`standard` 预置甚至没有 `right-keys`
  目录，`keyboard` 的 `right-keys` 只有四张方向键图。绑到右手会让本 ADR 的回退对每一个已知模型
  都不可达。代价是按下小键盘时动的是左爪——这是"显示降级"可接受的近似。
  同一条 side 严格性也意味着**模型为小键盘补图时必须放进 `left-keys`**：放在 `right-keys` 的
  `Kp*.png` 在左手绑定下不可达（见残余风险 2）。
- 不碰 `0x4f..=0x52`：方向键保持右手，`standard` 照旧不绑方向键（与既有契约测试一致）。
- `gamepad` 预置保持按钮专用映射，不绑键盘小键盘。

### 明确不做

- **不给预置模型新增任何 `Kp*.png`**：没有专属 artwork 时回退就是期望行为（同 ADR-0039 决策 3）。
- **不引入文字/字形渲染管线**：渲染器（`bongocat-render`、`bongocat-overlay`）目前只有带纹理的
  四边形，没有字体资源、字形图集或文字 shader。"显示其可读名称"因此落地为"按可读名解析到已有
  键位图"，与 ADR-0039 的 `KpEnter → Enter` 完全同构；真正的屏幕文字需要字体资源加 D3D11/Metal
  两套文字管线，属于独立改动。
- **不改 `resolve_key_overlays` 的 side 严格性**：缺同侧资源时不绘制是既有契约
  （Technical Design §11），不为小键盘放宽成跨侧查找。
- **不处理 HID `0x64`**：ISO/ABNT2 额外键，不是小键盘键。

### ADR-0041 · 完整键位词表与绑定覆盖

**状态：** 已接受（2026-09-17）

依赖：ADR-0040（小键盘键位图的 `Kp*` 命名与主键盘键位图回退）、ADR-0039（主 Enter 与小键盘 Enter 的键位图命名与旧名兼容）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0004（可校正输入状态）

### 决策

#### 1. 命名标准布局 + 小键盘的全部按键

`key_name_candidates` 补齐上述所有缺失 usage 的精确名：

```text
0x2d Minus   0x2e Equal   0x2f LeftBracket   0x30 RightBracket   0x31 BackSlash
0x32 IntlHash   0x33 SemiColon   0x34 Quote   0x36 Comma   0x37 Dot
0x46 PrintScreen   0x47 ScrollLock   0x48 Pause
0x49 Insert   0x4a Home   0x4b PageUp   0x4c Delete   0x4d End   0x4e PageDown
0x64 IntlBackslash   0x65 Apps   0x67 KpEqual
```

- `0x32`/`0x64` 是 ISO/ABNT2 的额外键，旧图未命名。沿用 W3C UI Events `KeyboardEvent.code`
  的既有拼写 `IntlHash`/`IntlBackslash`，避免自创词汇；`0x64` 是 Windows `map_scan_code`
  真会产出的 usage，不命名就等于又留一个不可达的键。
- `0x67`（小键盘 `=`）只有 macOS 产出（keycode `81`），按既有 `Kp*` 方案命名 `KpEqual`。
- **命名不以美术是否存在为前提。** 这些键今天都没有图，这是资源事实，不是命名依据。

#### 2. 绑定覆盖同一集合

`bongocat-app::input_bindings_for_model` 的键盘模型分支收敛为一条循环：

```text
0x04..=0x65（方向键 0x4f..=0x52 除外）→ 左手
F13..F24（0x68..=0x73）              → 左手
0x67                                 → 左手
0xe0..=0xe7（八个修饰键）             → 左手   ← 2026-09-17 修订补回，见事实 4
```

方向键仍归右手，且仍只对 `Installed`/`keyboard`/`gamepad` 生效（`standard` 预置照旧不绑方向键，
与既有契约测试一致）；`gamepad` 预置仍是按钮专用映射。左爪画键盘区、右爪画方向键，这条分工不变。

> **修订（2026-09-17，同日，见 ADR-0042）：** 本节描述的是**候选** hand 表，它仍然覆盖整块布局。
> 真正写入 runtime 的每模型绑定是该表与"该模型确实有的键位图"的交集：模型没有图的按键不再进入
> `InputState::model_snapshot`，因此既不驱动 `CatParamLeftHandDown`/`CatParamRightHandDown`，
> 也不产生按键图层。残余风险 1 记录的"缺图也有爪部反馈"由 ADR-0042 消除；命名不以美术存在为
> 前提这一条不变——模型补图后绑定自动恢复。

#### 3. `PrintScreen` 绑定，但明确不是功能键

`FUNCTION_KEY_USAGES` 的边界（`0x3a..=0x45` 与 `0x68..=0x73`）**不变**，`PrintScreen` 不享受
`Fn` 回退。它只是作为一张普通按键获得名字和 hand——这与"不享受功能键语义"并不矛盾，
原来的 `must not bind PrintScreen` 断言混淆了这两件事，本次改成断言
`function_key_name(0x46) == None`（真正的边界保证）+ `hand_for(0x46) == Some(Left)`（新的绑定事实）。

#### 4. 已出厂的 `Delete.png` 恢复可达

`0x4c => Some("Delete")` 让两个预置模型已经出厂的 `Delete.png` 立即生效。这是本次唯一的
"修复既有不可达资源"，不是新增能力。

### 明确不做

- **不为这些键新增任何预置美术**：名字先就位，图由模型决定；预置模型保持现状。
- **不命名平台 adapter 产不出的 usage**：`0x66`（`Power`）两边都不映射，保持无名（有测试断言）。

> **修订（2026-09-19）：** 上一条的原则是"不为纯理论 usage 造名字"，但它当时的表述过宽，而且
> 没有被完全执行。`Apps`（`0x65`）被命名并绑定，却**两个平台都产不出**：Windows `map_scan_code`
> 的 E0 分支没有 `0x5d`（菜单键的扩展扫描码），macOS `map_key_code` 没有 `110`
> （`kVK_ContextualMenu`）。产不出的键永远进不了绑定表，所以 `Apps.png` 一直画不出来——名字、
> 绑定、文档三者都"到位"，只有平台层没有。同日补上两处映射（Windows `0x5d => 0x65`、macOS
> `110 => 0x65`），并给**两个 adapter 各加一条穷举断言**把"本 adapter 实际能产出哪些 usage"逐项
> 固定：`bongocat-platform::macos::tests::this_adapter_reports_exactly_the_keycodes_the_platform_defines`
> 与 `bongocat-platform::windows::tests::this_adapter_reports_exactly_the_scan_codes_the_platform_defines`
> （见 TODO 第 90 项）。
>
> 修订后的原则分两层，不要再混为一谈：
>
> - **词表可以命名不可达的键。** 名字是给模型作者的契约：模型提供美术就必须生效，与今天有没有
>   硬件能按无关。`IntlHash`（`0x32`，macOS 把 ISO `#` 与 ANSI `\` 给了同一个键码）与 F21–F24
>   （`0x70..=0x73`，Carbon 无键码、Windows 无可依据的扫描码）属于这一类，维持现状。
> - **"某个 adapter 能产出它"这句话必须有该 adapter 自己的断言支撑**，不得从并集推断。本 ADR
>   事实 4 的并集、以及两处 `adapter_keyboard_usages()` 的注释都曾经比事实宽。
- **不改 `resolve_key_overlays` 的 side 严格性**：缺同侧资源时不绘制是既有契约。
- **不改 `FUNCTION_KEY_USAGES` 的边界**：`PrintScreen` 依旧不是功能键。

### ADR-0042 · 键位图存在才触发按键动作

**状态：** 已接受（2026-09-17）

依赖：ADR-0041（完整键位词表与绑定覆盖）、ADR-0040（小键盘键位图的 `Kp*` 命名与主键盘键位图回退）、ADR-0004（可校正输入状态）

### 决策

#### 1. `bongocat-live2d-render` 提供键位图清单

新增 `KeyImageInventory`：

- `read(root)` 扫描 `resources/left-keys` 与 `resources/right-keys`，只取目录下的常规 `.png`，
  文件名主干即资源名，**不解码任何图片**。
- `provides(side, name)` 回答某一手是否提供该名字的图。
- `can_draw(side, hid_usage)` 用 `key_name_candidates` 的候选顺序回答"这个键在这一手能不能画出来"
  ——精确名、家族图（`Fn`、`Control`/`Shift`/`Alt`/`Meta`）、旧名别名（`AltGr`、`Return`）、
  小键盘回退（`Kp1..Kp9`→`Num1..Num9` 等）都算。
- `load_key_assets` 与 `KeyImageInventory::read` 共用同一个私有目录扫描（`key_image_files`），
  所以"清单说有"与"渲染加载到"不可能漂移；这也是唯一的文件规则来源（目录、扩展名大小写不敏感、
  只认目录下的常规文件）。

#### 2. 绑定按模型实际资源收窄

`bongocat-app::input_bindings_for_model` 保留 ADR-0041 的静态 hand 表作为**候选集合**（命名与
绑定仍覆盖同一集合，词表仍不以美术存在为前提），但每个键只有在 `can_draw` 为真时才写进
`InputBindings`。也就是说：

```text
写入 runtime 的每模型绑定 = 静态 hand 表 ∩ 该模型的键位图
```

- 模型补上 `Dot.png` 后，`. ` 键在下次激活该模型时自动恢复绑定——仍然不需要改产品代码。
- 判断在 `bongocat-app` 完成，`prepare_model` / `select_model` 两条激活路径都传入
  `CommittedModel::root()` 的清单，因此启动恢复、设置页切换和 overlay 预览走的是同一条规则。
- 判断放在绑定层而不是渲染层：renderer 不得决定动作，runtime 也不应为了解图片资源而依赖
  Cubism/`image` 相关 crate。

#### 3. 缺图即无动作

模型没有图的按键不写入绑定，于是 `InputState::model_snapshot` 在 `hand_for == None` 处丢弃它：
既不置 `left_hand_down`/`right_hand_down`（因此不驱动 `CatParamLeftHandDown`/
`CatParamRightHandDown`），也不产生按键图层。**规则对所有缺少图片资源的按键一致**，不区分
"词表新补的名字"和"出厂就有的名字"。

仍然生效的既有行为：

- `Delete.png`（ADR-0041 恢复的可达资源）继续绑定并绘制。
- 小键盘数字继续回退到 `Num1..Num0`，小键盘 Enter 继续回退到 `Enter`，`KpDivide` 继续回退到
  `Slash`。
- 功能键继续优先专属 `F1.png`…`F24.png`，缺失时回退 `Fn.png`。
- 左右修饰键继续使用各自精确名，回退家族图。

#### 4. 作用范围是键位图

- **鼠标**不属于本规则：`mouse_left_down`/`mouse_right_down` 驱动的是
  `ParamMouseLeftDown`/`ParamMouseRightDown`，是指针状态而不是键位图，产品词表里也没有它们的
  图片名。
- **手柄按钮**不属于本规则：它们经由 `hand_for_gamepad` 驱动爪子，但键位图解析器没有手柄按钮
  的词表，手柄美术今天本来就不进入按键图层（预置 `gamepad` 的 `South.png` 等只被加载、不被绘制）。

### 明确不做

- **不给预置模型补任何键位图**：缺图就是缺图，本 ADR 只让缺图时不再假装有反馈。
- **不改 `key_name_candidates` 的词表与 `resolve_key_overlays` 的 side 严格性**：命名仍覆盖
  平台 adapter 能产出的全部按键，同侧资源缺失仍不绘制。
- **不把判断搬进 runtime 或 renderer**：runtime 需要图片资源信息就得依赖 Cubism/图片解码；
  renderer 决定动作会破坏"renderer 只消费 `RenderSnapshot`"的边界。
- **不改鼠标与手柄输入**。

### ADR-0044 · 全局快捷键改用 global-hotkey 注册制后端

**状态：** Accepted

取代：无（首次为快捷键子系统建立 ADR）

### Decision

- 引入 `global-hotkey = "=0.8.0"`，仅作为 `bongocat-platform` 的
  macOS/Windows 目标依赖。组合键解析（`ShortcutChord`）留在
  `bongocat-config`，平台 token → `keyboard_types::Code` 映射与注册管理
  在 `bongocat-platform/src/shortcut.rs`。
- 新增 `GlobalShortcutService`：单一 owner 线程持有
  `GlobalHotKeyManager`，以 50ms 轮询共享 `ShortcutTable` 并做增量
  register/unregister，消费 `GlobalHotKeyEvent::receiver()` 后把目标交给
  精简后的 `ShortcutDispatcher::execute`（runtime 触发 + application
  sink）。`ShortcutTable::replace` 的全部发布点（`set_shortcuts`、录制
  suspend/resume、行为快捷键开关、恢复默认）无需新增通知通道即被覆盖。
- 平台线程模型：
  - Windows：`WM_HOTKEY` 投递到创建 manager 隐藏窗口的线程队列，owner
    线程运行 `MsgWaitForMultipleObjectsEx` + `PeekMessageW` 消息泵。
  - macOS：热键事件经 Carbon handler 在主事件循环投递（应用 runloop
    泵送）；在 owner 后台线程上创建/注册/注销
    `RegisterEventHotKey` 已通过本机反向控制实验验证可行。
- 删除 `ShortcutMatcher` 与输入管线中的全部快捷键喂点
  （`apply`/`reset`/`reconcile`），`WindowsInputService` /
  `MacInputService` 移除 `_and_shortcuts` 启动入口，
  `OverlayInteractionSinks` 移除 `shortcut_dispatcher`。app 生命周期：
  服务在产品 run 启动，`begin_product_shutdown` 中先于
  `overlay.stop_input()` 停止并注销全部注册。
- 注册失败不阻塞其他绑定：被其他应用占用或平台无 scancode（macOS 的
  ScrollLock/Pause 没有 Carbon scancode）的组合键记入
  `registration_failures` 诊断，其余正常注册。
- 配置 schema、行为动作与 UI 录制流程均不变。`ShortcutCommand` 的 application target
  在原有集合上增加 `toggle_ignore_mouse_input`、`toggle_ignore_keyboard_input` 和
  `toggle_ignore_gamepad_input`；三个 target 都经同一 settings-service typed handoff
  切换并持久化对应的模型输入门禁，不直接修改 runtime 或平台输入状态。

### Consequences

- 行为变化：注册的组合键会被操作系统消费，不再同时传递给前台应用
  （与历史 Tauri 版本一致，属于预期语义）。
- macOS 上快捷键注册本身不要求辅助功能权限（Carbon 注册），但猫爪动画
  的全键盘监听仍依赖 CGEventTap，TCC 授权要求不变。
- 快捷键不再随输入服务失败而失效；输入服务的恢复/重置路径与快捷键状态
  完全解耦，Issue #47 的 pressed-state 校正只服务动画输入。
- 服务按 50ms 轮询感知改绑，录制 suspend/resume 的生效延迟 ≤ 100ms。
- 三个输入门禁快捷键与普通应用快捷键共享同一录制、冲突和 `commands_enabled` 门禁；它们只切换模型输入投影，键盘/鼠标/手柄的可靠采集与 pressed-state 恢复不受影响。

### ADR-0045 · Overlay 放置约束改为屏幕范围并延迟收敛

**状态：** Accepted

取代：无（取代 `P6-KEEP-OVERLAY-IN-WORK-AREA` 确立的工作区约束行为，该行为未单独留下 ADR）

> 后续修订（2026-09-23）：ADR-0054 后项目不再维护 AccessKit 标签同步；字段改名的当前收口是中英文案、共享 schema、fixture 与可见控件。
>
> 后续修订（2026-10-13）：下面「`always_on_top` 开启时 overlay 可以覆盖 macOS 菜单栏」当时并不成立。
> `NSMainMenuWindowLevel`（24）与菜单栏自身同层，同层级由菜单栏压住应用窗口，因此窗口被拖进菜单栏
> 区域后那部分不可见（issue #1155 报告的「无法移动到屏幕最顶」）。`always_on_top` 的层级改为
> `NSStatusWindowLevel`（25）。实测依据（`CGWindowListCopyWindowInfo` 与直接调用
> `NSWindow.constrainFrameRect(_:to:)`）：菜单栏在 24、程序坞在 20、AppKit 的 `NSMenu` 弹出菜单在 101；
> `constrainFrameRect` 只在层级低于 24 时把窗口顶边压到 `visibleFrame` 顶部，层级 ≥ 24 时原样返回，
> 所以当时挡住窗口的是层级本身，而不是拖拽约束。

### Decision

- 约束区域从“某一块显示器的工作区”改为**所有已连接显示器矩形的并集**：Windows 取
  `EnumDisplayMonitors` + `MONITORINFO.rcMonitor`，macOS 取 `NSScreen.frame`。允许窗口覆盖
  任务栏/Dock/菜单栏，负坐标保持有效。
- 判定“窗口完整显示”的口径是**并集覆盖**而不是“落在单块显示器内”：一个横跨两块显示器、
  但每一部分都落在某块显示器上的窗口是合法摆放，不纠正。只有窗口真正越过桌面边界时才纠正。
- 纠正目标是“与窗口交叠面积最大的显示器，无交叠时取中心距离最近者”，保持窗口尺寸不变；
  窗口大于显示器时把原点贴到显示器原点（与旧行为一致）。
- 纠正**延迟**执行：窗口连续静止 `1s`（`PLACEMENT_SETTLE_DELAY`）后才移回显示器内，期间
  任何被观测到的位移都重新计时。创建、缩放/资源设置重建和模型重建这些没有拖拽在途的路径仍然
  立即收敛。
- 放置检查最多缓存 `500ms`（`PLACEMENT_INSPECTION_INTERVAL`）后重新评估。这既让静止窗口不必
  每帧枚举显示器（macOS 会分配、Windows 会进窗口管理器），也让显示器拓扑变化——包括拔掉
  外接屏后窗口留在桌面外——在没有平台通知通道的情况下仍能被纠正。
- 逻辑落在平台无关的 `bongocat-overlay/src/placement` 模块，平台适配器只负责枚举显示器与
  读写窗口矩形；判定与倒计时都可以在没有 GPU 和原生窗口的情况下测试。
- 字段随语义改名：`overlay.keep_inside_work_area` → `overlay.keep_inside_screen`。当前 v1
  尚未发布，按 §4.1 直接改名，不引入 alias、迁移或旧键兼容。中英文案、共享 schema 与
  fixture、AccessKit 标签同步更新。

### Consequences

- 行为变化：任务栏、Dock 和菜单栏所在区域不再是禁区，窗口可以停留在其上。
- 行为变化：窗口被拖出桌面后会有一段可见的“停留”，随后才自动回到显示器内；这是延迟的
  有意结果，也是跨显示器拖拽可用的前提。
- 多显示器选择逻辑从此只有一份（原先 macOS 在 `bongocat-overlay` 里自算交叠，Windows 交给
  `MonitorFromRect(MONITOR_DEFAULTTONEAREST)`）；两者语义一致，行为差异只剩平台坐标原点
  方向（AppKit 底部原点、Win32 顶部原点），分类计算对两者等价。
- 失去的能力：不再保证窗口避开系统栏；`always_on_top` 开启时 overlay 可以覆盖 macOS 菜单栏。
  需要“让开任务栏”时改由用户自己摆放。
- 一个大于所有显示器的窗口永远无法“完整显示”，此时约束只会把它贴到显示器原点且不再产生
  新的移动（纠正结果与当前矩形相同即视为无事可做），不会每帧重复 `SetWindowPos`。
- 纠正发生在 frame tick 内：overlay 不可见或 frame source 已停止时不会纠正，隐藏的窗口
  保留原位置。
- 显示器热插拔靠 `500ms` 定期重评估自愈，没有监听 `WM_DISPLAYCHANGE` /
  `NSApplicationDidChangeScreenParametersNotification`；因此拓扑变化到窗口纠正之间最多有
  “重评估间隔 + 静止延迟”的延迟。若后续需要更快的响应，应加平台通知而不是缩短这两个常量。
- 未验证项：两平台真实鼠标拖拽观感、多显示器实机摆放与显示器热插拔均未实机核验（本机只有
  macOS 单显示器环境，Windows 交叉工具链在本机无法构建 overlay 的 C 依赖）。

### ADR-0046 · Overlay 颜色契约以两端都执行 linear -> sRGB 编码为准

**状态：** Superseded by ADR-0063

取代日期：2026-09-24
取代：无（修正 Technical Design §Renderer 颜色段落与两条 Phase 3/5 验收证据的结论）

> 本文记录的是曾经用于补齐 Windows/macOS 线性编码不对称的修复决策与预期实现路径；
> 当时没有完成目标硬件的像素验证。它讨论了“两端是否都执行 linear -> sRGB 编码”，
> 但没有解决“是否要匹配 Bongo-Cat-Mver 的历史 encoded-space 观感”问题；当前产品目标由
> ADR-0063 取代。

### Decision

1. **两端都必须执行最终 linear -> sRGB 编码，语义以 macOS 现有的 linear 合成路径为基准。**
   Windows 保留 `B8G8R8A8_UNORM` swapchain，改由 back buffer 的 render target view 使用
   `DXGI_FORMAT_B8G8R8A8_UNORM_SRGB`（`COMPOSITION_RENDER_TARGET_FORMAT`）承担编码。
2. **alpha-only clipping mask target 保持 linear UNORM。** mask 只携带 coverage，对它做编码
   等于二次施加 gamma。
3. **不为对齐旧版观感而退回 gamma 空间。** 当时审查的 BongoCatMver/OpenGL 参考路径没有显示
   sRGB texture view 或 framebuffer encode，因此本条选择继续采用 linear 合成（decode + linear
   运算 + encode）；该来源链与最终像素当时均未完成验证，本条修复只补齐 Windows 缺失的编码。
4. **本机不可验证的 Windows 侧改动必须留下可重复的形态证据。** 由于 `bongocat-overlay` 在
   非 Windows 主机上无法为 Windows 目标构建，涉及 D3D11 调用形态的改动应以独立探针 crate 在
   `--target x86_64-pc-windows-msvc` 下通过 check/clippy，并在合并前删除探针。

### Consequences

- 按当时的设计，同一组 sRGB 贴图应在两平台产出相同的 surface 字节，并预期消除 Windows
  以往“整体偏暗、对比更强”的观感；该结果当时没有目标硬件 readback 证据。
- 修复是格式层的，不改变 drawable 拓扑、遮罩算法、frame 调度或任何产品配置，因此不需要
  数据迁移，也不影响 `schema_version: 1`。
- 未收敛的残差是系统级显示色彩管理：macOS 由 Core Animation 把 sRGB 内容转换到显示器色彩空间，
  Windows DirectComposition 不做这一步。广色域显示器上同一组 sRGB 值仍可能观感不同，这属于
  合成器行为，不属于 renderer 契约。
- 旧版观感对齐（是否改用 gamma 空间合成）仍是独立问题。若将来决定对齐旧版，需要同时修改两端
  的纹理视图与附件格式，并同步本文第 3 条与 Technical Design。

### ADR-0049 · 地球键与 `Fn` 的语义分离

**状态：** 已接受（2026-09-19）

依赖：ADR-0041（完整键位词表与绑定覆盖）、ADR-0042（键位图存在才触发按键动作）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0037（应用内导入 BongoCatMver 模型）

### 决策

#### 1. 地球键 = `Globe`

`bongocat-render` 新增 `GLOBE_KEY_USAGE: u16 = 0xff03`（`0xff00 | 0x03` 的折叠值）作为六处共用的
唯一来源；`bongocat-runtime::PhysicalKey::GLOBE` 就是它，`bongocat-platform` 的 keycode 表也引用它
而不是重写数值。

不叫 `Fn`：见事实 4。不叫 `GlobeKey`：词表里没有 `*Key` 后缀，而 `KeyA`…`KeyZ` 是 W3C
`KeyboardEvent.code` 的既有拼写，加后缀会被读成同一族。`Function` 是旧名，但它与功能键回退名 `Fn`
只差两个字符而意思不同——这个仓库已经被 `Backslash`/`BackSlash` 这类近似名坑过一次（ADR-0050），
词表里不该再放一对。

#### 2. `Fn` 保持原义，`Fn.png` 不改名、不迁移

`Fn` 是 F1–F24 的共享回退图名，语义本来就正确。改名会把每个现存模型的功能键美术挪到地球键上，
是对全部现存模型的行为回归，也与"导入要兼容旧图片资源"直接冲突。

地球键与功能键的候选列表**完全不相交**：

```text
F1 … F24 (0x3a..=0x45, 0x68..=0x73) → F1 … F24, Fn
0xff03   (Globe)                    → Globe, Function
```

模型只提供 `Fn.png` 时地球键画不出来；只提供 `Globe.png` 时功能键画不出来。

#### 3. 地球键的候选与旧名兼容

`Function` 是旧版对同一物理键的可达名，排在最具体名之后。导入归一化把
`resources/{left-,right-}keys/Function.png` 改名为 `Globe.png`；运行时的 `Function` 候选保证
**已经安装过**的包（不会被重新归一化）仍然能画。这与 ADR-0038 对 `AltGr` 的处理同构。

#### 4. macOS 映射 keycode 63

`map_key_code` 加 `63 => PhysicalKey::GLOBE.hid_usage()`。Windows 不加：Fn 键由键盘固件处理，
Raw Input 从不报告它，没有可映射的码。

#### 5. Mver 转换补齐 F13–F24

`legacy_virtual_key_name` 加 `0x7C..=0x87` → `F13`…`F24`。`VK_F13`…`VK_F24` = `124`…`135` 是
`windows` crate `Win32/UI/Input/KeyboardAndMouse` 里的连续常量（可离线交叉验证），不是猜的硬件表。
地球键在旧版码空间里不存在，转换永不产出 `Globe.png`。

### 明确不做

- **不重命名 `Fn.png`**，理由见决策 2。
- **不为地球键新增任何预置美术**：两个预置键盘模型与 `gamepad` 都保持现状，按下地球键不产生任何动作
  （ADR-0042 的缺图规则）。
- **不加 Windows 映射**：Fn 由固件处理，Raw Input 不报告。
- **不猜 Windows 的 F13–F24 扫描码**：仓库此前已在 TODO `P4-MODEL-LEGACY-SOURCE` 判定"缺可依据的
  扫描码，不猜值"，本次沿用；macOS 侧 Carbon 没有 `kVK_F21`…`kVK_F24`。
- **不改 `FUNCTION_KEY_USAGES` 的边界**：`Fn` 回退仍只覆盖 `0x3a..=0x45` 与 `0x68..=0x73`。

### ADR-0050 · 转换输出的键位图名必须用产品拼写

**状态：** 已接受（2026-09-19）

依赖：ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0039（主 Enter 与小键盘 Enter 的键位图命名与旧名兼容）、ADR-0049（地球键与 `Fn` 的语义分离）

### 决策

#### 1. 转换输出改产品拼写

`legacy_virtual_key_name` 的 `0xDC` 输出 `BackSlash`。这与 ADR-0038 的 `Alt`/`AltGr` → 产品名、
ADR-0039 的 `Return` → `Enter` 是同一套做法：转换写产品词汇表，旧拼写由运行时候选兜底。

#### 2. 运行时候选把旧拼写加成末位别名

```text
0x31 (BackSlash) → BackSlash, Backslash
```

canonical 在前。已经安装过、不会被重新归一化的包（无迁移路径）因此仍然能画。

#### 3. 导入归一化登记该拼写

`LEGACY_KEY_IMAGE_NAMES` 加 `("Backslash", "BackSlash")`。按事实 3，它在两个首发平台上通常是空操作；
登记它的价值是让"旧拼写 → canonical"这件事只有一个来源，而不是散落在运行时别名里。

#### 4. 把"转换能安装的名字"做成可遍历契约，并同时断言两件事

- `bongocat_model::legacy_keyboard_key_image_names()`：遍历两种键盘模式的全部虚拟键码，返回去重排序后的
  输出名集合。手柄按钮名**不在**其中（它们是 gamepad 模型的美术名，不进入键位图，ADR-0037）。
- `bongocat-live2d::every_key_image_name_the_conversion_can_install_resolves_to_a_key`：对每个输出名断言
  **既能解析、又是 canonical 名**（即某个可达 usage 的候选列表第一项）。只查前者不够（事实 4）。
  `Shift` 与 `Control` 是**有意**的非 canonical 名（旧表给两侧同一个码，ADR-0038 决策 4），用显式例外
  列表记录，并断言这两个例外仍然必要，避免列表变成死代码。

这样下次漂移会被测试发现，而不是被用户发现。

### 明确不做

- **不把游戏按钮名纳入该契约**：它们不进入键位图解析器（ADR-0037）。
- **不新增第三套拼写**（如 `BackSlashKey`）：ADR-0038 已经明确不做同类事。
- **不把 `Shift`/`Control` 展开成两侧**：那会同时作废 ADR-0037 记录的真实样本转换证据。

### ADR-0052 · 快捷键门禁各自跟随所属分组，并新增窗口快捷键开关

**状态：** 已接受（2026-09-21）

依赖：ADR-0002（GPUI 设置界面）、ADR-0044（全局热键注册后端）

### 决策

#### 1. 每个作用域的第一行是它自己的"启用"开关

`shortcuts_page::ShortcutScope` 新增两个方法，把开关需要的知识全部收进作用域本身：

- `gate_label(language) -> &'static str`：该作用域的开关标题。可见开关与辅助功能节点读同一个
  返回值，页面上不再出现第二份措辞。
- `is_enabled(snapshot) -> bool`：该作用域在配置里的真值，直接来自 `commands_enabled` /
  `shortcuts.model_behaviors_enabled`，不做任何取反。

`group()` 把 `gate_item()` 作为该分组的第一个 item，行列表仍是原来的自定义元素。开关标题是
`启用窗口快捷键` / `启用模型行为快捷键`，**只带标题、不带描述**：第 97 项已经把"标题已能表达清楚的
设置项不再重复写一遍描述"定成规则，"启用×××"正属于这一类（维护者原话：标题就已经表达了足够的
意思）。

#### 2. 配置新增正向字段 `shortcuts.commands_enabled`，默认 `true`

- 字段与 `shortcuts.command_bindings` 同段，命名与数组对齐；`ShortcutConfig` 改为手写 `Default`，使全新
  v1 配置是"窗口快捷键生效、模型行为快捷键不生效"——与改动前逐字段一致。
- `canonicalized()` 原样透传该字段：设置页面提交绑定表时不得顺手把它改回默认。
- 应用层 `shortcut_config_from_settings` 多接一个 `commands_enabled` 参数，由调用方（`set_shortcuts`、
  `suspend_shortcut_capture`）从当前配置传入。录制或清空快捷键不会把窗口快捷键在用户背后重新打开。

#### 3. 门禁的唯一实现点是 `ShortcutConfig::active_bindings`

`active_bindings` 是"此刻生效的绑定"的唯一投影（模型侧的活动模型过滤也在同一处），命令侧门禁加
在这里，应用层与平台层不再判第二次。关闭时 `commands` 投影为空，`model_behaviors` 与配置里的绑定
一律不动。

#### 4. 开关直接显示配置真值，任何一层都不取反

配置、`SettingsSnapshot`、settings command、`SettingValue`、`PendingOperation` 全是正向字段
（`commands_enabled` / `command_shortcuts_enabled` / `SetCommandShortcutsEnabled`），开关也按"启用"
方向渲染，因此可见开关、辅助功能节点的 `toggled`、开关发出的命令读的是同一个布尔值——不存在"某个
地方取反了没有"这类问题。开门时按"禁用"方向渲染的初版方案（取反收在 `is_disabled` 一处）已被替换：
两个开关各自只带标题，正向直出既更短也更好读。

#### 5. 节点顺序跟着页面走

两个门禁节点从「交互」页的节点块移到「快捷键」页的节点块，并按页面顺序插在各自作用域的行之前：
窗口门禁、窗口行、模型门禁、模型行。行的切分点用 `window_shortcut_rows(...).len()`，与页面的
`row_index_offset`、tab 序号、无障碍节点编号同一条行序。

### 明确不做

- **不合并成一个总开关。** 维护者要的是两个模块各自管理；一个总开关会让"只想关掉模型行为快捷键"
  变成"连显示/隐藏窗口的快捷键也没了"。
- **不改变任何默认值。** 窗口快捷键默认生效（关掉它等于让产品失去唯一的快捷入口），模型行为快捷键默认
  关闭（第 91 项记下的刻意收窄，本 ADR 不反转）。两个开关因此默认状态不同（窗口开、模型关），这是
  有意的，也是"启用"方向比"禁用"方向更符合直觉的地方。
- **不给两个开关写描述行。** 第 97 项的规则：标题已能表达清楚的设置项不再重复一遍。
- **不在关闭时清空或删除绑定。** 与模型侧一致：门禁只决定是否进入平台匹配表。
- ~~**不给行列表加禁用态样式。**~~ **已被 ADR-0053 取代（2026-09-21 决策变更）**：门禁关闭时行
  现在置灰且不可交互，遵循统一的「开关门禁」禁用绑定规则（见 ADR-0053）；当初"先把组合键排好再
  打开开关"的理由不再是产品方向。
- **不为"开关状态与行状态不一致"再加提示。** 开关现在就在行上方，它就是那个提示。

### 后果

- 快捷键页面每个分组的第一行是开关（纯标题行），其后是该作用域的行；「交互」页不再出现任何快捷键开关。
- 用户可以在不丢失绑定的前提下把窗口快捷键整体关掉。**这是新增能力**：此前应用级快捷键只能在
  逐个清除绑定（不可逆，且模型侧还会被下次激活补回来）之后才失效。
- `shortcuts.commands_enabled: false` 会随配置落盘并跨重启保持；重启、切换模型、重新录制绑定都不会
  把它改回 `true`。
- 无障碍树里「交互」页少一个 Switch、「快捷键」页多一个，两个门禁节点的标题与可见开关同源，
  且节点没有 value（行没有描述，读屏也不会把描述当值念出来）。
- `spikes/config-store` 的配置副本需要同步该字段，否则它那条与共享 fixture 对拍的测试会失败
  （spike 是独立 workspace，不在 `just check` 范围内）。

### ADR-0053 · 「开关门禁」与被控设置项的统一禁用绑定规则

**状态：** 已接受（2026-09-21；项目 AccessKit tree 部分由 ADR-0054 取代）

依赖：ADR-0052（快捷键门禁各自跟随所属分组）、ADR-0054（visual-first UI）

### 决策

#### 1. 统一机制：`bongocat-ui` 的 `window::setting_gate::SettingGate`

每个"开关门禁"场景构造一个 `SettingGate::new(editing_blocked, enabled)`，其中 `enabled` 是该开关的
配置真值（正向、不取反），`editing_blocked` 只在结构性编辑不可能时为真（不含 `pending`）。规则由
两个方法固定：

- `disables_switch()` = `editing_blocked`——**开关自身永不被自己的状态禁用**，否则关掉的门禁永远
  无法再打开；
- `disables_controls()` = `editing_blocked || !enabled`——被门禁控件的禁用条件。

三条固有性质写进模块文档，对所有场景生效：

- **门禁只投影可用性，从不改写被门禁的值**：重开开关即恢复用户原值，无需重录（与 ADR-0052 的
  "只过滤、不改写"一致）。
- **`pending` 不进门禁**：头部状态栏是保存指示器。
- **三层读同一个 gate 值**：可见行、无障碍树（disabled 且不可点击/聚焦）、变更方法守卫必须同源，
  以保证"在开关翻转前渲染的树上操作"也不生效。

#### 2. 新场景的接入方式（标准流程）

1. 定义一个从 snapshot 读开关真值的正向谓词（如 `ShortcutScope::is_enabled`、
   `hover_hide_delay_applies`），全链路复用，不取反；
2. 渲染时为每个被门禁分组构造一个 `SettingGate`；
3. 开关行走 `disables_switch()`，被控行走 `disables_controls()`：打包字段用
   `SettingItem::disabled(...)`；自定义行**整行置灰（标签与控件一起，与 `SettingItem::disabled`
   的整行置灰一致——禁用的是"这条设置"，不是仅控件）**并停止注册交互 handler；
4. 无障碍节点对被控行报 disabled 且不加 `clickable().focusable()`；
5. 变更方法以同一谓词做守卫。

#### 3. 快捷键页的落地

- `ShortcutScope::for_target(target)` 是"目标 → 门禁它的作用域"的唯一映射（Command → Window，
  ModelBehavior → Model），渲染层、无障碍层与变更方法都经它取谓词，不再各自 match 目标类型。
- 每个分组渲染时构造自己的 `SettingGate`（`editing_blocked` 共享、`enabled` 各自取
  `is_enabled(snapshot)`）；行**整行置灰（标签与控件一起）**，捕获按钮、清除按钮不再注册
  `on_click` / `on_key_down`。整行置灰与 `SettingItem::disabled` 对打包字段的处理一致
  （标题、描述、控件一起 0.5 透明度），两类行在外观上不可区分。
- `sync_shortcut_row_focus` 按同一谓词决定 tab stop，并在门禁翻转后取消仍挂在该作用域目标上的
  捕获（与"目标离开行序即取消捕获"同一机制）；`finish_shortcut_capture_if_valid` 再次守卫，
  门禁在捕获中途关闭时结束捕获而不是写入一条界面上不可见的绑定。
- `begin_shortcut_capture`、`begin_shortcut_capture_from_accessibility`、`clear_shortcut` 增加同一
  守卫：结构性检查（`shortcut_commands_available`）之外再判目标作用域的门禁真值。
- 鼠标悬停时隐藏延迟行改读同一规则：`SettingGate::new(editing_blocked, hover_hide_delay_applies(..))
  .disables_controls()`，与其 mutator 守卫、无障碍节点同源。**行为差异**：结构性编辑阻塞期间
  （导入进行中等）该行现在也会置灰——此前它只随开关禁用；这是统一规则的直接结果，且与其变更守卫
  `set_overlay_hover_hide_delay_value` 的既有早退对齐。

#### 4. ADR-0052 的对应修订

ADR-0052「明确不做」中"不给行列表加禁用态样式"一条由本 ADR 取代（维护者 2026-09-21 决策变更），
该条内注明的理由（"先把组合键排好再打开开关"）不再是产品方向；该 ADR 其余决策不受影响。

### 明确不做

- **不改 runtime 投影语义。** 门禁的唯一实现点仍是 `ShortcutConfig::active_bindings`；本 ADR 只把
  UI 层的可用性投影对齐到同一开关真值，配置、命令通道、平台表均不变。
- **不在门禁关闭时清空或改写绑定。** 置灰只是 UI 投影；落盘配置不动。
- **不把 `pending` 计入门禁。** 防整页明暗刷新；保存指示器在头部状态栏。
- **不为打包字段重写禁用样式。** `SettingItem::disabled` 已同时处理置灰与字段不可交互，统一机制
  直接复用，不再自绘。
- **不为开关关闭加提示行。** 开关就在被控行上方，置灰即提示（延续 ADR-0052 的立场）。

### 后果

- 快捷键页两个门禁关闭时，各自作用域的行**整行置灰**（标签与控件一起）且不可交互；无障碍节点报
  disabled、不可聚焦；键盘 tab 跳过这些行；辅助技术触发的捕获/清除请求被守卫拒绝。
- 门禁关闭期间无法再通过 UI 录制或清除该作用域的绑定（ADR-0052 曾刻意保留这条路，本 ADR 收窄它）。
  绕过 UI 的路径（直接编辑配置文件）不受影响，配置层从不过滤或改写绑定。
- 鼠标悬停时隐藏延迟行与两个快捷键分组共用同一条规则与同一份实现，新增门禁场景按第 2 节流程接入。
- 捕获中途翻转门禁会取消捕获并恢复平台表（`resume_shortcut_capture`），不会留下悬空的录制会话。

### ADR-0057 · 模型窗口用右键拖动缩放

**状态：** Accepted

修订（2026-09-24）：缩放写回配置不再重建 HWND/NSPanel；统一改为在现有原生窗口上更新几何，
并在尺寸变化后立即填充新的 swap-chain/drawable，避免设置更新暴露未初始化透明帧。
取代：无（恢复旧版 `pre-refactor` 分支存在、当前产品尚未实现的窗口缩放交互）

### Decision

- **触发**：右键按下后指针位移（欧氏距离）超过 `3px` 才算缩放；未越阈值的右键仍然是上下文菜单。
  不要求按住 Shift——旧版的修饰键要求是当时用 `mousemove` 无法区分单击与拖动的权宜做法，而
  位移阈值直接解决了同一个问题。越阈值之后即使缩放没有实际变化，这次右键也不再弹菜单。
- **映射**：`scale = clamp(按下时的缩放 + (dx + dy) * 0.5, 25, 400)`。系数沿用旧版（向右下拖动
  放大、向左上拖动缩小，右上/左下相互抵消），范围收敛到当前配置契约
  （`OverlayConfig::scale_percent`、`OverlaySettings::is_valid`）而不是旧版的 `10–500`：拖动结果
  必须总能被配置接受，否则一次拖动会产生一个存不进去的值。位移始终相对按下点测量，因此缓慢
  拖动与手抖都不会改变判定。
- **起点**：按下时的缩放由窗口**当前宽度**反算（`width / base * 100`，同样钳制到 `25–400`），
  而不是读配置里的 `overlay.scale_percent`。窗口几何与配置并不总是同步——用户拖动过一次、
  显示器 DPI 变了、或者手工编辑过配置——用配置值当起点会让第一次指针移动就把窗口跳到另一个
  尺寸。反算让拖动始终从用户看到的大小继续。
- **锚点**：窗口左上角保持不动，与旧版 `setSize` 的观感一致。Windows 用
  `SetWindowPos(..., SWP_NOMOVE)`；macOS 的 `NSWindow` frame 原点在左下角，所以按“顶边不动”
  修正 origin.y，两者在屏幕上等价。
- **实时跟随**：拖动过程中窗口逐帧改尺寸，渲染器**就地** resize，不重建窗口、不重载纹理。
  Windows 走 `IDXGISwapChain1::ResizeBuffers` + 重建 RTV/staging/每个 mesh 的 mask target；
  macOS 走 `setFrame:display:` + 重设 drawable size + 重建 mask 纹理。其余与尺寸无关的资源
  （device、pipelines、composition graph、模型纹理、顶点/索引缓冲）全程保留。
- **尺寸来源**：`100%` 的基准尺寸是 `default_overlay_window_dimensions(canvas)`，与窗口创建时
  使用的同一个值。Windows 的拖动状态机工作在物理像素（`SetWindowPos` 的单位），基准按窗口 DPI
  换算；macOS 工作在点。尺寸由基准与缩放重新算出，而不是在旧尺寸上累加像素，因此连续拖动不累积
  舍入误差。
- **持久化**：拖动结束时把最终缩放通过新的 `OverlayInteractionSinks::resize_sender` 报给应用，
  应用提交 `SettingsCommand::SetOverlaySettings` 写回 `overlay.scale_percent`；窗口几何仍由既有
  的 placement 通路（`OverlayWindowPlacementDebouncer` → `OverlayWindowPlacementChanged`）落到
  `window-state.json`。overlay 只提出请求，配置的写入方仍然是应用。
- **幂等**：缩放写回配置后，runtime snapshot 的变化会让 frame tick 走原地尺寸更新分支。该分支先
  判断窗口尺寸是否已经等于新缩放对应的尺寸（`bounds_match_scale`，容差 `1px`），相等时不再按
  比例重算——否则拖动刚设好的尺寸会被乘第二次；需要改变尺寸时只更新现有 HWND/NSPanel 与
  swap-chain/drawable，并在返回 frame loop 前立即绘制一帧，不替换原生窗口。
- **click-through**：穿透模式下窗口返回 `HTTRANSPARENT`（Windows）或 `ignoresMouseEvents`
  （macOS），收不到指针消息，因此不可拖动，也没有右键菜单。这与既有的“非穿透模式才可拖动”
  一致，不是本次引入的限制。
- **与 hover 隐藏的互斥**：拖动进行期间 `hide_on_pointer_hover` 不再生效。该功能会把窗口淡出并
  让指针事件穿透，正好会中断窗口自己正在执行的拖动；两个平台都在计算 hover 状态时把“拖动中”
  当作“指针不在窗口内”，因此拖动期间窗口保持可见、拖动结束后恢复原有的 hover 行为。
- **平台无关部分**：判定、映射、钳制与尺寸计算落在 `bongocat-overlay/src/resize_drag.rs`，
  可以在没有 GPU 和原生窗口的情况下测试；平台适配器只负责读取指针、改原生窗口尺寸、以及把
  最终缩放报出去。

### Consequences

- 行为变化：右键不再无条件弹菜单。按住右键移动超过 `3px` 会开始缩放，松手后不弹菜单；原地
  按下松开仍然是原来的菜单。
- 行为变化：缩放范围是 `25–400%`（旧版为 `10–500%`）。这是配置契约的范围，放宽它需要同时改
  schema、fixture、设置页滑块与校验，属于另一次契约变更。
- 拖动期间每个 frame tick 都会做一次 GPU resize：Windows 会重建 render target、staging 纹理与
  全部 mask target，macOS 会重建全部 mask 纹理。这些资源都很小，但拖动时的帧率低于静止时是
  预期结果，尤其是在有 mask 的模型上。
- 拖动结束后的写回不再重建 overlay 窗口；它只让 runtime snapshot 与已经完成的窗口几何对齐。
  若设置页直接改变缩放，现有窗口会在原地调整尺寸，并立即绘制新尺寸的第一帧，避免显示未初始化
  的透明 back buffer/drawable。
- 缩放值从此有两个来源（设置页与右键拖动），两者都写入同一个 `overlay.scale_percent`，因此
  设置页滑块在拖动结束后会显示拖动结果。
- 窗口位置与尺寸仍然分离：`scale_percent` 在 `config.json`，实际几何在 `window-state.json`。若两者
  不一致（例如手工编辑配置），下一次拖动会以当前窗口宽度反推的缩放为起点，并在结束时把两者
  对齐。
- 未验证项：两平台的实机拖动观感、Windows 的 `ResizeBuffers` 路径、macOS 的 drawable resize 与
  mask 重建都未经人眼核验；当前代码已通过 Windows 构建检查，但真实拖动、显示器/DPI 切换
  与 compositor 透明帧仍需目标硬件 smoke。

### ADR-0066 · gilrs 手柄后端与平台窄适配边界

**状态：** 已接受（2026-09-25）；fork 已补齐 callback lifetime、bounded queue/epoch、authoritative reset、bounded shutdown、compile guard 与 xinput 回归修复；2026-09-26 追加 macOS IOHID worker run-loop 空转修复（空闲 CPU 98% → 0%）；2026-10-02 追加 WGI 焦点门控读取修复与 WGI raw analog axis 取值域修复；2026-10-05 追加 WGI raw element 顺序修复与 SDL 轴槽位翻译的实测回退（见 Issue #1099）；双平台物理设备与长期证据仍阻塞完成声明

### 决策

- 根 workspace 精确固定 `https://github.com/ayangweb/gilrs` 的 commit
  `9a5ee3d6c36db25871f6ea67eafaf09c637a0c50`，package 版本为 `gilrs 0.11.2` /
  `gilrs-core 0.6.8`。该 commit 包含 callback context ownership、bounded queue/epoch、authoritative
  reset、WGI/XInput bounded shutdown、macOS IOHID stop/join、target-scoped compile guard 与 xinput
  extreme-axis regression 修复；再追加 macOS IOHID worker 的 run-loop 空转修复：worker 曾以 null
  mode 调用 `CFRunLoopRunInMode`，CoreFoundation 会立即返回而不等待，使无条件创建的空闲
  backend 持续占满一个 CPU 核；修复传入真实 run-loop mode，空闲 CPU 由 98% 降到 0%，
  `reset` 仍在约 8ms 内 ack、`shutdown` 约 105ms join，并新增 `idle_backend_does_not_spin`
  回归测试；再追加 WGI 焦点门控读取修复：Windows 只把 mapped 的
  `Windows.Gaming.Input.Gamepad` 读取投递给拥有前台窗口的进程，而 gilrs 在
  `Gamepad.FromGameController` 成功（即 XInput 设备）时固定选它，使这类设备只在 BongoCat
  窗口获得焦点时才有输入，HID 设备不受影响因为它们走 `RawGameController`。修复改为设备只要
  暴露任何 raw report 就优先用 `RawGameController`，mapped 读取仅作为完全无 raw report 时的
  回退；该选择同时决定 element 列表、`AxisInfo` 取值域和 SDL mapping 查询键，因为三者都描述
  raw 布局；再追加 WGI raw analog axis 的取值域修复：backend 曾用 SDL 的预居中换算
  `(value * 65535.0) - 32768.0` 生成 raw sample，同时把 `EvCodeKind::Axis` 声明为 `i16`
  取值域，而 gilrs 会再归一化一次，导致静止摇杆落在 `i16::MIN` 并被读成 `-1.0`、
  负方向一半行程被钳制为最大偏移，且映射到按键的模拟扳机按有符号域读取时静止值正好是半程、
  压在 axis-to-button 阈值上。最后追加 WGI raw element 顺序修复（Issue #1099）：
  `native_ev_codes` 沿用上游 gilrs 的 evdev 下标，而 `RawGameController` 按 Windows Gaming
  Input 自己的顺序报告元素，`Mapping::default` 又按 element code 身份匹配，于是四个面键整体错位
  一位、把 D-pad 报告成按键的设备十字键四个方向全部落到表外而失效、两个模拟扳机分别落到右摇杆
  Y 轴和左扳机上——静止摇杆因此正好压在 axis-to-button 阈值上而让被它驱动的按键逐采样抖动，
  这就是 Switch 模式下"模型鬼畜地疯狂按下所有按键"。修复是让 `native_ev_codes` 改带 WGI 原始下标
  （面键 `SOUTH/EAST/WEST/NORTH` = 0..3，D-pad 落在 10..13，Guide 之后的元素排在 15 之后以免借用
  别的按键下标），这一半只影响没有 SDL mapping 的设备与 mapped 回退路径。曾同时尝试把
  `Gamepad::axes` 按 SDL 映射的槽位顺序重排（六轴时 `[0, 1, 4, 2, 3, 5]`），使 `lefttrigger:a2`
  解析到左扳机；用整库实测后回退，见阻塞 0b。后续修复仍必须形成可审计的 patch series、
  推送到 fork `master` 并再次精确固定 commit。
  `deny.toml` 只放行该精确 git source；fork 的 SDL mapping submodule 由该 commit 固定为
  `15b5e9f4abfb1c5c691c468799816755a91a2e11`。`deny.toml` 通过 `required-git-spec = "rev"` 拒绝
  branch/tag git source。workspace dependency 不启用平台 feature；`bongocat-platform` 的 Windows target
  table 启用 `wgi`，macOS target table 保持 featureless 并由 gilrs-core 选择 IOHID。
- Windows 使用 gilrs 的 `wgi` backend；macOS 使用 gilrs 的 IOHID backend。BongoCat 不再声明
  `objc2-game-controller`，不直接加载 XInput DLL，也不自行维护 SDL mapping、hat/D-pad 解码或设备
  backend。
- `GilrsGamepad` 是 `bongocat-platform` 私有且双平台共用的窄 adapter。`gilrs::GamepadId`、
  `Button`、`Axis`、`Event` 和错误只在该文件内出现；runtime/UI 只接收项目自有的
  `GamepadConnection`、`GamepadButton`、`GamepadAxis`、`InputEvent` 和
  `GamepadAxisSample`。
- builder 关闭 gilrs 默认 jitter/dead-zone filter、force feedback 和
  `SDL_GAMECONTROLLERCONFIG` 环境映射，保留 fork 内置 SDL mapping，并关闭自动 state update 后由
  adapter 显式更新。D-pad axis 使用 gilrs 自带 `axis_dpad_to_button`，BongoCat 不复制 hat 解码。
  产品 dead-zone 仍只由 runtime 的 `GamepadAxisSettings` 决定。
- 一次 drain 最多消费 `256` 个 gilrs event，剩余 backlog 留在 gilrs 的 bounded backend/high-level pending
  queue，不能让手柄洪峰阻塞 Raw Input/CGEventTap。最大活动设备数仍为 `4`，与现有 24-key axis transport
  容量一致；gilrs
  backend id 不进入项目 API，adapter 分配 `0..3` 项目 slot，每次连接只通过
  `GamepadAxisProducer::connect` 分配新 generation。
- gilrs 的位置名与项目词表并非逐字相同：left/right trigger 逻辑按钮映射为项目 shoulder，
  `LeftTrigger2`/`RightTrigger2` 的连续值同时映射为项目 trigger button 与 trigger axis。连续值采用
  产品阈值 `value >= 0.5` press、`< 0.5` release；stick 使用 `[-1, 1]`，trigger 使用 `[0, 1]`。
  `C`、`Z`、`Mode`、`Unknown` 和未知 axis 不伪造项目 identity，只进入匿名拒绝/解码计数。
- 连接成功和任何已提交的全局 input Reset 后，adapter 调用 gilrs reset epoch，清空 fork queue 与
  cached state，要求 backend 重播 authoritative held button 与六轴 latest value；不分配新 generation。
  fork overflow 通过强类型 `BackendOverflow { dropped }` 事件可见，adapter 计数并触发同一 reset/reseed
  恢复。断开先淘汰该 generation 的 pending axis，再可靠发布 `GamepadDisconnected`。shutdown 清理项目
  connection/axis state并消费 gilrs 的 bounded shutdown acknowledgement，最终 Reset 仍由 input owner
  发布。
- gilrs 构造返回错误或构造阶段 panic 只禁用 gamepad adapter并增加一次匿名 backend failure；键鼠
  service 继续运行。fork worker 的 bounded reset/stop acknowledgement、overflow marker 和 callback
  quiescence 通过强类型 gilrs API/事件进入 adapter；旧 XInput 轮询次数/查询错误、GameController
  background policy 和 callback 临时 snapshot 等 backend 专用诊断从项目 contract 删除，只保留连接、
  断连、容量拒绝、按钮、axis、拒绝、backend failure、backend event discard 与通用 input service 指标。
- 后续手柄兼容、驱动差异、backend queue、callback 和 shutdown 修复全部在该 fork 完成并提升精确
  commit；BongoCat 不加入平行 XInput/GameController workaround。升级 fork 后必须重跑双平台 adapter
  contract、物理手柄矩阵、100-cycle restart 与长时间压力。

### 依赖审查

- `gilrs 0.11.2` 的许可证为 `Apache-2.0 OR MIT`，MSRV `1.84`，低于 workspace 的 Rust `1.97`。
- Windows WGI 和 macOS IOHID 的系统绑定及 unsafe 面积被限制在第三方包内；项目 adapter 保持
  safe Rust，第三方对象和错误不进入项目公共 API。替换边界是私有 `gilrs_gamepad.rs`，不会改变
  runtime/config/UI contract。
- 内置 SDL mapping 来自 fork 固定 submodule；该 database 有独立于 gilrs dual-license 的许可，
  构建和分发前必须把 `gilrs/SDL_GameControllerDB/LICENSE`、revision 和 attribution 纳入第三方
  合规清单。

### 已知阻塞

0. **Windows WGI 焦点矩阵（已在 fork 修复，实机证据仍缺）：** gilrs 文档提示 Windows Gaming Input
   可能需要关联且获得焦点的窗口。Issue #1082 证实了这个门控真实存在：BongoCat Raw Input window 为
   hidden，overlay 默认为 click-through，设置窗口也不保证聚焦，因此 mapped 读取让 XInput 模式手柄
   只在 BongoCat 窗口获得焦点时才有输入。fork 已改为优先使用不受焦点门控的 `RawGameController`，
   但仍必须在 Windows 10/11 实机验证 settings 开关、焦点/失焦、click-through、启动时已连接、重连和
   多手柄；在取得该证据前不得把焦点矩阵写成已验证，若 raw 路径在这些状态仍不可靠，应继续修复 fork
   backend，而不是改回 BongoCat 内 XInput workaround。
0b. **Windows WGI raw element 顺序（已在 fork 修复，实机证据与映射库缺口仍存）：** Issue #1099 报告
   Xbox 兼容模式下面键错位、十字键无响应、左爪默认姿态错误，Switch 模式下模型疯狂按下所有按键，而
   DS4 模式正常。该症状与 `native_ev_codes` 的 evdev 下标和 WGI raw 顺序不一致完全吻合：没有 SDL
   mapping 的设备走 `Mapping::default` 时面键错位、十字键落到表外，两个模拟扳机则落到右摇杆 Y 轴
   和左扳机上；Switch Pro 只有四个轴，其第三轴（正是右摇杆 Y）因此被当成左扳机，静止时按有符号
   域归一化后正好是 `0.5`，压在 axis-to-button 阈值上逐采样抖动，这就是"鬼畜"。fork 已把
   `native_ev_codes` 改为 WGI 原始下标；轴顺序依据是 Microsoft 的 `Raw game controller` 文档，
   按键顺序依据是 Windows 手柄的实际报告顺序（Microsoft 不跨设备固定它，设备在注册表
   `Labels\Buttons` 自报），因此仍必须在 Windows 10/11 实机逐一确认面键、十字键、摇杆与扳机。

   **SDL mapping 数据库描述的不是 WGI。** 该库由本后端的 GUID 查询键命中（2058 条中 1592 条形如
   `bus/vendor/product/version` 后全零；138 条 Bluetooth 条目因查询键固定用 USB bustype 而不可达），
   而这些条目是给 DirectInput/evdev 写的：DirectInput 把游戏柄的六个轴排成左摇杆 X、���摇杆 Y、
   左扳机、右摇杆 X、右摇杆 Y、右扳机，WGI 则把两个扳机排在两个摇杆之后。一个列表无法同时满足
   两种顺序。实测整库后可命中的条目里，582 条把四个摇杆排在 0、1、2、3（期望设备报告顺序），
   393 条把扳机插在槽位 2 与 5（需要交换）；按前者交付会破坏更大的一组，且分组顺序正是微软文档
   给出的顺序，因此 `Gamepad::axes` 维持设备报告顺序。由此产生的已知残留：命中"交错扳机"映射的设备
   （主要是真机 Xbox 360/One 一类 XInput 手柄）在右摇杆与扳机上仍然错位，表现为拉左扳机时左右扳机
   图可能同时出现。要彻底解决需要一份针对 WGI 写的映射数据库，或把平台知识放进共享的映射解析
   层；两者都超出本轮可验证范围，不在 fork 里加启发式。

   切换 `crates/bongocat-platform/Cargo.toml` 的 Windows feature 到 `xinput` 可让 Xbox 兼容模式正常
   但让 Switch/DS4 完全无反应，这是 `xinput` backend 只认 XInput 设备的预期行为，不作为回退方案。
1. **双平台物理设备与系统生命周期：** 仍需在真实 macOS IOHID 和 Windows WGI 设备上验证 held-at-startup、
   held-at-reconnect、lost-release、overflow/reset epoch、100-cycle restart、锁屏/睡眠/快速用户切换和
   长时间无增长。cross-check、纯函数测试和无设备 smoke 不能替代这些证据。
2. **macOS 物理 callback 静止证明：** fork 已拥有持久 context、run-loop stop、close 和 bounded join，但
   TCC deny/grant/revoke、自然 timeout、设备移除及 callback in-flight 的静止/恢复仍需目标系统实测。
3. **WGI 默认 mapping 的轴位置错位（fork 待修，本仓库不兜底）：** 没有 SDL mapping 的
   Windows 控制器走 `gilrs::mapping::Mapping::default`，它按 `gilrs-core` 的
   `windows_wgi::native_ev_codes` 表把位置名绑定到事件码，而该表的 axis 下标写的是
   `LSTICKX=0, LSTICKY=1, RSTICKX=2, LT2=3, RT2=4, RSTICKY=5`，与 Windows
   Gaming Input 的 raw 报告顺序不符。Microsoft 的 `Raw game controller` 文档逐字写明
   Xbox 手柄的六个 raw 轴是 `LeftThumbstickX=0, LeftThumbstickY=1, RightThumbstickX=2,
   RightThumbstickY=3, LeftTrigger=4, RightTrigger=5`。raw reading 的 element 列表来自
   `collect_axes_and_buttons`（设备自己的 `AxisCount`），`Mapping::default` 按「下标在列表里
   吗」过滤，因此 LT/RT 与右摇杆 Y 轴互相错位：`LT2=3` 实际读到右摇杆 Y，`RT2=4` 实际读到
   左手扳机（两个扳机对调），`RSTICKY=5` 实际读到右手扳机。同一张表的 button 下标也按
   evdev 排列（`BTN_WEST=0, BTN_SOUTH=1, …`），而 raw button 顺序由设备的 registry
   `Labels\Buttons` 决定、没有跨设备固定顺序，所以 button 侧无法在没有证据的情况下改。
   影响面：只覆盖 SDL database 里没有 GUID 的 Windows 控制器；有 mapping 的设备按元素下标
   解析（`parse_sdl_mapping` 用 `axes.get(from)`/`buttons.get(from)`），因此不受影响。
   需要的修法：只把 axis 六个下标改成上表的 raw 顺序，并加一条 fork 回归测试断言
   `Mapping::default` 在一个六轴十五键的 raw 设备上把 `LeftStickX/Y`、`RightStickX/Y` 绑到
   `0/1/2/3`、把两个模拟扳机绑到 `4/5`。改下标不影响 mapped reading——那条路径按事件码
   身份（`nec::AXIS_LSTICKX` 等）收发，从不读下标。
   本仓库**不**在 `gilrs_gamepad.rs` 里补偿：补偿就要在 adapter 复制一份第三方位置表，并在
   上游修好之后静默变成双重修正。fork 修复、推送 `master` 与再次精确 pin 之前，该缺陷只在
   Windows 无 mapping 的控制器上可见。

这些阻塞不授权在 BongoCat 内复制 backend，但禁止把当前状态写成双平台手柄完成或 stable 可发布。

### 被拒绝方案

- **继续维护平台 XInput/GameController backend**：拒绝，因为它重复设备发现、mapping 和生命周期
  修复面，并继续把驱动兼容问题锁在 BongoCat。
- **让 runtime/UI 直接消费 gilrs 类型**：拒绝，因为它破坏强类型项目 contract、第三方替换边界和
  runtime 单一状态所有权。
- **在 BongoCat 内修复 gilrs 的 macOS 生命周期、队列或 WGI 焦点问题**：拒绝；这些是 fork/backend
  问题，应在 fork 修复、测试并精确升级 commit。
- **用 crates.io `gilrs 0.11.2` 代替 fork**：拒绝；当前需要 fork 中与 D-pad/平台行为相关的修复，且
  后续兼容修复统一在 fork 维护。

### ADR-0068 · 托盘与模型窗口右键菜单复用

**状态：** 已接受（2026-09-25）

修订：2026-09-25 根据产品反馈调整为托盘与模型窗口右键共用同一套菜单；取代本文早期“拆分两棵菜单树”的方案。

### 决策

#### 一棵 popup 根，一个 owner

`bongocat-platform::SystemMenu` 是唯一的托盘/菜单 owner，持有 `TrayIcon`、一个 `muda::Menu` 根、
一个模型窗口 `Submenu`、一套菜单项、`MenuEvent` receiver 和强类型事件队列。托盘图标和模型窗口右键
都展示这同一棵菜单树：

```text
设置
────────
模型窗口
  ├─ □ 隐藏模型窗口
  ├─ □ 鼠标穿透
  ├─ □ 始终置顶
  └─ □ 鼠标悬停时隐藏
────────
检查更新（仅在更新能力可用时创建）
────────
退出 BongoCat
```

菜单不再提供源码、重启、版本、缩放或透明度行。源码与版本已经在设置的 About 页面提供；缩放由设置页
和右键拖动提供，透明度留在设置页。检查更新仍由构建/channel 事实决定是否创建，不显示永久禁用的空行。

#### 动作和状态边界

两个入口共用同一批 native item 实例和同一组稳定 action id，由同一个 `action_for_menu_id` 映射到
`SystemMenuAction`。应用 action 分发和 revisioned settings command 不增加菜单专用弱类型协议。

菜单 presentation 仍由 settings snapshot 驱动：

- 显隐使用偏好设置已有的 `settings.overlay.hide_model_window.label`，开关和菜单 check item 都以
  “已隐藏”为选中值；启动默认未选中，因此模型窗口默认可见。
- 鼠标穿透、置顶和鼠标悬停隐藏分别复用偏好设置已有的
  `settings.overlay.click_through.label`、`settings.overlay.always_on_top.label` 和
  `settings.overlay.hide_on_mouse_hover.label`。
- 显隐是 runtime 会话状态，不写入 `config.json`；菜单与设置页共享同一个 runtime snapshot 投影。
- 若 check item 的 command 失败，应用重新读取当前 snapshot 并回写 presentation，避免 native menu
  保留用户点击产生的乐观勾选状态。

#### 生命周期不变量

- `TrayIcon` 字段必须先于菜单根、`Submenu` 和菜单项声明；显式 shutdown 先隐藏托盘，再按既定顺序停止
  input/runtime/frame/renderer/overlay。
- overlay 右键只使用真实 Windows HWND 或 macOS content `NSView`，不借用托盘隐藏窗口，也不在 overlay
  内创建第二个 owner。
- 菜单根、模型窗口子菜单和所有更新/文字/勾选操作都在平台 UI 主线程执行。菜单 tracking 期间只允许把
  事件送入既有有界队列，不能在 callback 内销毁 `SystemMenu` 或 overlay。
- `muda` 和 `tray-icon` 的版本、features 与替换边界继续由 ADR-0031 固定；第三方类型不进入公共业务 API。

### 替换边界

替换点只有 `crates/bongocat-platform/src/system_menu_native.rs` 与 overlay 的 `HasWindowHandle` 实现。
升级 `muda`/`tray-icon` 时必须重新验证共用菜单根、菜单项父级关系、析构顺序、同一个 `MenuEvent`
receiver、Windows GUID/tooltip 缺陷、主线程约束、macOS `NSView` 生命周期和两个平台的实机菜单层级；
不能退回由 overlay 持有菜单或把第三方类型泄漏到 runtime/UI。

### ADR-0070 · 手柄按键图片词表与按键层解析通道

**状态：** 已接受（2026-09-26）

### 决策

- **`KeyPress` 改为携带带标签的身份。** `bongocat_render::KeyIdentity` 是
  `Keyboard(u16)` | `Gamepad(GamepadButton)`，`KeyPress` 是 `{ key, side }`。两族共用同一个按键层
  和同样两个手目录，但不是同一类控件，不折叠进一个数字空间：手柄按钮没有 HID usage，任何"给
  手柄按钮分配一段假 usage"的做法都会让一类控件被当成另一类解析，也不会有第三种输入族能安全
  扩展。`KeyPress::keyboard` / `KeyPress::gamepad` 是唯一构造入口。
- **图片名等于 `GamepadButton` 变体名。** `GamepadButton::key_image_name()` 是 16 个名字的唯一
  来源，名字与变体逐字相同。这条不变量让类型和美术词表无法漂移：改一个变体名会同时打穿所有
  装了旧主干的模型，新增一个变体在没有名字之前画不出任何东西。契约测试遍历 16 个按钮，检查名字
  两两不同且与变体名逐字相同。
- **词表是产品的，不是 backend 的。** `bongocat-platform` 把每个 backend 控制映射到项目枚举，
  gilrs 类型不越过 adapter 边界（ADR-0066），因此没有任何环节需要 backend 名字。
- **预置 `gamepad` 模型按产品词表重命名。** `DPad*`→`Dpad*`、
  `LeftTrigger`→`LeftShoulder`、`LeftTrigger2`→`LeftTrigger`、`RightTrigger`→`RightShoulder`、
  `RightTrigger2`→`RightTrigger`。目录归属不变（仍是真实手柄布局）。这是对用户已有模型的
  可见变化，因此记入 CHANGELOG 的用户可见修复。
- **导入时把旧主干改写为产品名。** `bongocat-model-store::key_names` 在 staging 副本上重写，
  与键盘侧既有的 `AltGr`→`AltRight`、`Return`→`Enter`、`Function`→`Globe`、`Backslash`→`BackSlash`
  是同一套机制、同一个唯一合法的位置（用户的源目录是只读输入）。
- **手柄词表不提供运行时旧名候选。** 键盘旧名可以作为末位候选，是因为 `AltLeft`/`AltRight`
  这样的 canonical 名和旧名 `Alt` 不冲突。手柄不是这样：旧词表里 `LeftTrigger` 已经是肩键的
  名字。给 `LeftTrigger`（扳机键）加 `LeftTrigger2` 旧名候选，会让一个装过归一化的旧包在按
  扳机时画出肩键的图；反过来给 `LeftShoulder` 加 `LeftTrigger` 候选，则会让一个已经用产品名
  写 `LeftTrigger.png` 表示扳机的包在按肩键时画出扳机的图。两种方向都会产生"看起来能用但按错键"
  的静默错误，比缺图更糟。归一化是唯一安全的答案。手工拷进 store、绕过归一化的目录只保留
  词表本身不冲突的那部分（四个面部键、两个菜单键）可达。
- **一个包整体判定为旧词表或新词表。** 归一化按目录判定：目录里出现一个只在旧词表里存在的主干
  （`LeftTrigger2`、`LeftTrigger2` 的右侧、`LeftThumb`、`RightThumb`、`DPad*`）就整体按旧词表
  重写。判定必须读目录真实条目名而不是探测路径——Windows 和 macOS 的路径不区分大小写，探测
  `DPadUp.png` 在装着 `DpadUp.png` 的目录里也会成功，那样每个包都会被判成旧词表。
- **只改大小写的条目真的改名。** `Backslash`→`BackSlash` 和 `DPadUp`→`DpadUp` 在大小写不敏感的
  文件系统上源和目标是同一个文件，直接 `rename` 是空操作；这两条经由同目录临时名完成两步改名，
  于是归一化在所有平台都生效，而不只在大小写敏感的文件系统上。临时名只存在于 store 自己的
  staging 副本里，两次 rename 之间被中断的导入由 store 的 abandoned-import 恢复删除。
- **Mver 的 XInput 序号表按真实按钮顺序重写。** 0–3 面部键、4/5 肩键、6/7 模拟扳机、8/9 菜单键、
  10/11 摇杆键、12–15 方向键（上/下/左/右），输出产品自己的 16 个按钮名。逐条断言这张表，并断言
  每一个输出名都是某个 `GamepadButton` 的图片名。
- **左右手由模型自己的目录决定。** `gamepad_hands_for_model` 遍历 `GamepadButton::ALL`，把按钮
  绑定到提供它图片的那一侧，`left-keys` 是左爪、`right-keys` 是右爪。这与旧实现一致，也让 Mver
  转换按 `lefthand`/`righthand` 落盘的结果直接可用，不再需要为每个转换模型单独配一张手表。
  同一按钮在两个目录都有图时归左手（键盘侧整块主键区也归左手）；产品每个控件只投影一只手，
  而没有预置模型会这样做。
- **缺图按钮保持惰性，摇杆键保留自己的参数。** 没有图片的按钮不绑定、不产生爪子动作、不产生
  按键层（ADR-0042）。`LeftStick`/`RightStick` 另外驱动 `StickLeftDown`/`StickRightDown`，
  与爪子状态互相独立；模型若提供 `LeftStick.png`，L3 依然同时得到按键层。
- **输入来源门禁对两族同样生效。** `ignore_keyboard` 让键盘连同它的按键层一起消失，
  `ignore_gamepad` 让手柄连同它的按键层一起消失；`model.ignore_gamepad` 打开时手柄按钮不贡献
  按键层。共享 fixture 现在逐 checkpoint 声明 `activeKeyOverlays`，因此省略声明等于声明空列表，
  漏写会被测试抓到。

### 后果

- 手柄模式下按下面板键、肩键、扳机和方向键都会显示对应图片，并抬起该手爪子；不画图的是模型
  没有图片的按钮（预置模型缺 `Select`、`Start`、两个摇杆键）。
- 已安装的旧版游戏手柄模型：只要经过一次导入，归一化会把旧主干改成产品名并从按 F 失效变为可用。
  手工拷入 store 的旧目录按上面"不冲突的那部分可达"的规则降级。
- 预置模型文件名变化会影响手工放置同名文件的用户，CHANGELOG 记录为用户可见修复。
- `KeyPress` 的字段从 `hid_usage` 变为 `key`，`bongocat-render` 的公共 API 有一处不兼容变更；
  它不进入配置或持久化格式，`schema_version` 仍是 1。
- 剩余门禁不变：Windows WGI 焦点矩阵与双平台物理设备、profile、热插拔和生命周期矩阵仍是
  ADR-0066 的完成门禁，本次修复不提供任何实机证据。

### ADR-0071 · Gamepad-Connection Model Auto Switch

**状态：** 已接受（2026-09-26）

### 决策

- 配置是 `model.gamepad_auto_switch`：`enabled`（默认 `false`）、
  `connected_model` 和 `disconnected_model`。两个目标都是完整 `ModelIdentity` 或
  `null`，`null` 是默认值，含义是「上次在该输入族上使用过的模型」。
- 页面位置是 Input & interaction 的 Gamepad 分组末尾：门禁开关紧接在它控制的两个
  下拉之上，分组内已有的忽略手柄输入、摇杆与扳机死区保持原顺序。
- 两个下拉的第一个选项恒为「上次使用的手柄模型 / 上次使用的非手柄模型」，其余选项
  只包含该状态真正可用的模型：连接方向只列 `gamepad` 模式，断开方向只列其它模式，
  且只列 `Ready` 条目。已配置但不再可选的目标（模型被删或失效）以模型自己的显示名
  追加在末尾，避免控件显示为空而用户看不出设置指向哪里。
- 「上次使用」是 Application 的会话状态，不是配置：每次成功激活模型后按其输入模式
  记入对应族。手工选择与自动切换产生的激活一视同仁，因为这条记忆描述的是「屏幕上曾
  经是什么」，不是「谁要求的」。用户没有用过某一族时该方向无目标，切换不发生。
- 观察者是产品 frame source：它每帧已经读取 runtime snapshot，因此在
  `connected_gamepad_count` 的「0 ↔ >0」发生跨帧变化时向 settings client 发送一次
  `GamepadConnectionChanged`。第一帧只建立基线；已被系统上报的连接会在这之后形成真实
  跳变。发送失败（队列满）保留待发状态，下一帧重试。
- `GamepadConnectionChanged` 不携带状态。settings service 处理时重新读取 runtime 的
  `connected_gamepad_count`，因此重复通知、迟到通知和被补发的通知都落在同一结果上，
  不存在按过期观察切换的路径。
- 决定者是 settings service worker：门禁关闭、该方向无目标、目标已是当前模型时直接
  不做动作；否则复用既有 `Application::select_model`，因此配置提交、行为快捷键分配、
  失败回滚、`selected_model` 持久化和两阶段提交语义与用户手动选择完全一致，没有第二条
  切换路径。
- 自动切换是普通模型选择，所以它的结果进入 `model.selected_model`，重启后恢复用户最后
  看到的模型。删除导入模型时，与该模型同族的会话记忆和两个配置目标都在同一次提交中
  一起清除，不留下指向已删除文件的引用。
- 设置页用统一门禁规则（ADR-0053）：开关本身只被结构性阻塞禁用，两个下拉在开关关闭
  时置灰但保留已选值。

### 后果

手柄成为「换一个模型」的显式动作，而不需要用户在模型库里找当前模式。默认配置保持关闭
且不预设任何模型，因此新装应用不会因为插上手柄而突然换模型；用户打开开关后，第一次
连接若该族还没有使用记录，切换不发生，直到用户自己用过一次该族模型或在下拉里显式
指定。设置文件因此仍然只有一个模型选择事实来源：自动切换与手工选择写同一处。

### ADR-0079 · 按键层按按下顺序叠放全部按住的键

**状态：** 已接受（2026-10-02）

补充：ADR-0042（按键必须有自己的键位图）、ADR-0060（模型包 store 边界）

### 决策

#### 1. `model.show_all_pressed_keys` 是唯一的开关，默认关闭

放在 `ModelConfig`，`bool`、默认 `false`、正向语义（`show_…` 而不是 `single_…`），设置页呈现为
「同时显示所有按下的键」开关。

默认关闭是刻意的：叠放会让一个和弦在画面上明显变"挤"，而现有用户已经习惯了单图。把可见变化交给
一次显式选择，也保证这个字段不改变任何未选择它的用户的画面。

它是 `ModelConfig` 里**唯一带 `#[serde(default)]` 的字段**。v1 解析入口是严格的（`deny_unknown_fields`
且字段缺失即失败），所以一个必填新字段会让该字段加入之前写下的每一份 `config.json` 都解析失败，
进而走「最新有效备份 → 默认配置」的恢复流程并丢掉用户设置。缺少该字段的旧文档按兼容模式加载，
不视为一次新的开启。

#### 2. 按下顺序用 `pressed_at` + 可靠输入队列的 sequence 构成全序

`PressedRecord` 增加 `pressed_sequence: u64`，取自按下该控件那条边沿的 envelope sequence，并随
`apply_event` 的参数一起传入。比较键变成 `(pressed_at, pressed_sequence)`。

sequence 是进程内单调计数器，与输入队列给事件排序用的是同一个值，所以它和队列的顺序永远一致，
跨过毫秒边界也继续有效。**这同时修掉了兼容模式里的平局仲裁**：同一毫秒按下 `A` 再 `D` 时，
现在显示的是 `D`（后按下的），而不是 usage 更大的那个恰好也是后按到的巧合结果。行为文档
`input-semantics` 原本就写的是「最后按下且仍有效」，实现此前并没有保证这一点。

#### 3. 折叠只保留在 runtime 的投影里，renderer 不再按手折叠

`resolve_key_overlays` 改为对每个 press 解析出一个 overlay 并**保持顺序**。兼容模式的投影本来就是
「左手先、右手后」，所以逐个解析的结果与旧的 `[None, None]` 折叠**逐项相同**，兼容模式的画面不变。

两个按住的键可能解析到同一张图（`Fn` 家族回退、小键盘复用主键区图）。这种情况只画一次，保留先
解析到的那个在底层：重复绘制看起来一模一样，只是多一次 draw call，而先按下的那个本来就该在下面。

#### 4. 手部参数与显示模式无关

只要有任意一个绑定到该手的键仍按住，`CatParamLeftHandDown`/`CatParamRightHandDown` 就是 true，
两种模式都一样。叠放是显示选择：它改变画几张图和叠放顺序，不改变爪子对单个按键的反应，也不触碰
pressed state、释放路径、reconciliation 或 Reset。

#### 5. 溢出丢最早按下的那几个

`KeyPressSet` 的 64 槽上限原本就存在（防止设备把内存撑大），叠放模式让每个槽位都可能真的画出一张图。
投影在排序后只取最后 `KeyPressSet::CAPACITY` 个：**堆叠的底部是历史，顶层是用户此刻正在看的**，
所以宁可丢最早按下的。上限以 `KeyPressSet::CAPACITY` 命名暴露，而不是让投影硬编码数组长度。

### 后果

- 打开开关后，一个和弦在按键层里逐键可辨认，且最后按下的键在最上层；关闭时画面与升级前一致。
- 兼容模式的同一毫秒平局改为按 sequence 仲裁，符合行为文档既有措辞。这是一处可观察的行为修正，
  只在两次按下落在同一毫秒时出现。
- 渲染器没有新增任何状态：叠放完全由 `RenderSnapshot::active_keys` 的顺序表达。
- 自动化契约分四处固定，且都在真实实现上：
  - `bongocat-runtime`（`the_stacked_mode_keeps_every_held_key_oldest_first`、
    `the_stacked_mode_does_not_change_which_hands_are_down`、
    `the_stacked_mode_drops_the_oldest_presses_past_the_layer_capacity`）钉住全序、平局与溢出；
  - `bongocat-live2d-render`（`every_press_gets_its_own_overlay_in_the_order_the_runtime_supplied`、
    `two_presses_that_share_one_image_draw_it_once`）钉住不重排序与同图只画一次；
  - `bongocat-app`（`a_chord_draws_one_overlay_per_key_with_the_newest_on_top`）跑通
    「按键 → 投影 → overlay 解析 → render frame」的完整链路，并在同一测试里对比开关两态；
  - `bongocat-config`（`a_configuration_written_before_the_key_layer_could_stack_still_loads`）
    从序列化后的当前默认里删掉该字段，模拟旧文档的字节，钉住旧数据仍按兼容模式加载。
- 共享 fixture 没有新增用例。`shared/fixtures/expected-state` 的 `activeKeyOverlays` 在
  `crates/bongocat-runtime/tests/shared_input_fixtures.rs` 里按 `BTreeSet` 比较，是**成员集合**而不是
  绘制顺序；`spikes/fixture-runner` 也独立实现了一遍「一手一图」。为了一个只有开启后才存在的模式去
  改 fixture 的 `context` 语法并同步两个 oracle，会把「输入状态机的边沿语义」这份 fixture 契约扩成
  「渲染顺序契约」。叠放顺序改由上面四处直接覆盖真实实现的测试固定，其中 app 那一处覆盖的正是
  fixture 声明为成员的同一份 `active_keys`。
- 未运行实机验证：本次只在本机（Windows）跑了 `just check` 与 `tools/` 下的校验脚本，没有做
  Windows 实机 smoke 或 macOS 实机 smoke，叠放的实际观感需要实机确认。

### ADR-0080 · 按住一个物理修饰键临时恢复 overlay 交互

**状态：** 已接受（2026-10-04）

补充：ADR-0053（「开关门禁」统一禁用绑定规则）、ADR-0057（右键拖动期间悬停隐藏不生效）、ADR-0079（按键层叠放）
实现：Issue #970

### 决策

#### 1. `ModifierKey` 是八个修饰键的闭合词汇表，落在 `bongocat-input`

`bongocat_input::ModifierKey` 是 `LeftControl | LeftShift | LeftAlt | LeftMeta` 与对应 `Right*`，
由一张 `(ModifierKey, u16)` 表同时给出 `ALL` 与 `hid_usage()`，因此「顺序」与「usage」不可能各走
各的。`PressedModifiers` 是 pressed set 在这张表上的位投影。

放在 `bongocat-input` 而不是配置 crate，是因为四层必须说同一件事：落盘文档、runtime 的
`OverlaySettings`、设置协议与 overlay 自己的 `OverlaySessionOptions`。任何一层再写一份拼写，它就
可以与平台上报的 usage 悄悄分叉。这也让 `bongocat-config` 与 `bongocat-ui-protocol` 各自多了一条
指向 `bongocat-input` 的依赖边——这是本决策显式接受的代价。

因此 `bongocat-input` 新增 `serde` 与 `schemars` 两个依赖。这是序列化格式而不是子系统，替代方案是
在配置 crate 里为同八个键再写一份 `snake_case` 枚举，而它与 usage 表之间没有任何编译期约束。
`ModifierKey::name()` 的测试同时固定「拼写即变体的 snake_case」，所以重命名变体会让该测试失败，
而不是留下一份指向旧键的旧文档。

#### 2. `input.pressed_modifiers` 是这个设置唯一的读法

`InputSnapshot` 新增 `pressed_modifiers: PressedModifiers`，由 `InputState::snapshot` 在同一次遍历
pressed map 的过程中算出（`InputSnapshot` 的四个计数原本就是四次 `keys().filter()`，合并成一次是
顺带的收敛，不是额外开销）。

不复用 `model_input.key_presses`：`snapshot_of.rs` 会丢掉任何当前模型画不出来的键，而修饰键恰好是
模型不会提供的键，因此这个面上它们多半根本不存在。也不复用四个计数：计数答不出「哪一个」。

overlay 侧只在 `update_hover_presentation` 里读一次，两个平台共用
`OverlaySessionOptions::hold_modifier_pressed`。呈现结果是
`enabled: hide_on_pointer_hover && input_running && !held` 与 `(click_through && !held) || hover.hidden() || idle.hidden()`。
这与 ADR-0057 的右键拖动是同一条既有规则：拖动进行期间把「指针在窗口内」当作不成立。

不把它做成新的状态机，也不新增第三个 alpha 乘数：暂停悬停隐藏恰好就是「该状态机关闭」，而
`PointerHoverHide` 在 `enabled == false` 时已经显式复位 pending 与 completed 两种隐藏。代价是松开键
后指针通常仍在窗口内，于是延迟**重新开始计时**而不是恢复一个已过期的截止时间——这是 ADR-0057
已经接受过的同一形状，并有测试固定。

`requires_window_recreation` 不含这个字段：它必须像悬停隐藏一样在窗口运行期间生效，因此改设置
不得替换原生窗口资源。

#### 3. 设置页的录入控件读 runtime 的 pressed set，因此是「按住」语义

新增 `SettingsCommand::ReadPressedModifiers`，设置服务读 `runtime.client().snapshot().input.pressed_modifiers`
并直接应答。它必须是独立命令而不是 `ReadSnapshot` 的一层：控件每 50ms 问一次，而构建 snapshot 会
扫描磁盘上的模型库。它是 runtime 已有的原子读，不新增输入消费者，也不新增服务到 UI 的推送通道——
设置协议至今仍是请求-应答，UI 靠轮询，加一条推送会是远更大的改动。

「按住」语义由事实决定而不是由偏好决定：读的是**按下集合**而不是边沿，快捷的点击可能整个落在两次
轮询之间，而按住不会。设置本身要求用户做的事也正是按住不放，所以录入动作与使用动作是同一个手势。

录入提示的措辞跟着快捷键录入框而不是跟着这个机制：`按下组合键以录制` / `点击以录制快捷键`
对应 `按下一个修饰键以录制` / `点击以录制修饰键`。这一行因此在同一页面上与快捷键页读作同一个手势，
而机制上的差别（必须在下一次轮询时仍按着）由描述里的「按住时」承担，不再由提示语重复一遍——一个
只在录入控件里出现的「长按」警告会让两个录入框看起来像两种不同的东西。

控件的其余部分尽量复用现有逻辑：帧的外观、tab 位置、清除按钮与 `Escape` 取消都照快捷键录入框写，
显示格式读同一个 `macos_shortcut_token` 符号表，并在符号**前面**加一个表示左右的词（`左⇧` 而非
`⇧L`：字母跟在符号后面会被读成符号的一部分）；非 macOS 平台拼出 `左 Shift` 这类名称，与快捷键页的
`Control+Shift+A` 用同一批词。

侧标记是**界面语言里的词**，随已解析语言切换（`side_left` / `side_right`），不是固定字母。修饰键
符号本身在任何语言里都不翻译，但侧标记是这一行里唯一跟着界面语言走的部分——它和这一行的标题一样
属于产品文案；写成 `L` / `R` 会得到一个没人能读的键帽，也是唯一一段任何语言里都不像人话的标签。

七种语言必须说同一件事，因此有两条由测试固定的约束：

- **一个名词。** 标题与两条录入提示在每种语言里用同一个词指称修饰键。同一行的三句话若从不同术语表
  翻译，会让同一个键有两个名字。
- **没有任何语言暗示时长门槛。** 这个功能没有门槛：按住多久算多久，因此标题一律用「按住」类动词
  （`按住` / `Hold` / `Segure` / `Giữ` / `누르고` / `استمر في الضغط`），不用「长按」类
  （`길게` / `مطولًا`）——后者在韩语和阿拉伯语 UI 里同样读作「按够时长」，而实现里没有这个时长。
  录入提示按快捷键录入框的写法说「按下」，两者分工见上。

这一行是 Model window 页唯一带描述的设置项，按的是 ADR-0066 的判据而不是新增惯例：**描述只在标题与
控件说不清时出现**。它逐字写出被暂停的两项设置在页面上的标题，**且按页面顺序**：先「鼠标穿透」，
后「鼠标悬停时隐藏」。引用标题是把句子接到屏幕上某个东西上的唯一方式；改成描述效果，则要用户自己
再把「接收鼠标点击」对回「鼠标穿透」。而按页面顺序念下来，读的人不必在脑子里重排一次。缺了它，用户
面对一个「会让窗口暂时看不见也摸不着」的开关却看不到它影响哪两项、也不知道松开会不会有事，宁可
留着不开。描述同时进入搜索索引，因此搜索「穿透」或「click-through」能从设置窗口任何位置找到这一行。

这一行排在 Window behavior 分组的**最后**，而不是紧挨着它影响的两项。原因是布局而不是内容：鼠标穿透
在第三行、鼠标悬停时隐藏在第六行，中间隔着「保持在屏幕内」，因此**没有任何线性位置能让它同时挨着两者**。
放在整组之后，两个被暂停的开关都在它上方，它读作对上方设置的一条附注；撤销型修饰出现在被修饰项之后
而不是之前（之前会读成「这个开关启用了它们」）。紧贴着的「无操作时隐藏」两项确实不受它影响，这一处
邻接误导由描述本身承担——文字就在行内，且逐字点名了受影响的另外两项。

放在快捷键页被明确否掉：那一页是两个受门禁管辖的绑定作用域（窗口快捷键 / 模型行为快捷键），带冲突
检测规则（ADR-0052、ADR-0066），而这一行既不是应用 command 也不是模型行为，没有门禁也没有冲突域；
放进去要么逃出两个门禁，要么被错误地卷进某一个。为它单开分组也被否掉：ADR-0066 把这一页定义成恰好
三个分组，而分组可以成为二级导航，为一行新增分组等于换一个侧边栏条目。

测试反过来把这两条都固定下来：`the_recorder_description_names_both_settings_it_suspends` 拿描述去和
catalog 里那两个标题比对**并检查先后顺序**，所以改设置名、改了名却忘了改描述、或把顺序写反，都会在这里
失败，而不是留下一行指向页面上已不存在的开关。

#### 4. 这一行不受开关门禁，只受结构性编辑阻塞

ADR-0053 把「开关门禁」定义为**一个**开关绑定它下方的设置项。这一行被两个开关绑定：
`click_through` 与 `hide_on_pointer_hover` 都在它上方，但两者本身不相邻，任选其一作为门禁来源都是
任意的。一个因为某个不相干的开关没开而置灰的行，比一个「存下取值、等任一开关打开即生效」的行更糟。

因此它只接 `editing_blocked`，与 ADR-0053 第 2 节标准流程的第 5 步（变更方法守卫）保持一致：
录制与清除都在 `editing_blocked` 时拒绝。这是显式偏离该 ADR 的绑定前提，不是遗漏。

#### 5. 只作用于穿透与悬停隐藏，不作用于无操作隐藏

issue 与维护者的确认都只点了这两项。无操作隐藏期间用户本来就在操作输入（否则计时不会重置），
把它一起暂停没有可观察的收益，却会让「暂时失效」覆盖三个行为而不是两个。范围因此留在两项。

### 明确不做

- **不做「按住任意修饰键」的模式**。需求明确要求录入一个具体的物理键。
- **不把 Caps Lock 纳入词汇表**。它没有左右之分，与「必须区分左右」冲突。
- **不在 UI 侧新增输入观察者或推送通道**。录入读 runtime，与运行判定同一来源。
- **不缓存按住状态**。任何缓存都会让松开键后 overlay 继续可交互。
- **不在 `Schema` 版本上做迁移**。新字段 `#[serde(default)]`，缺失即 `null`。

### 后果

- 开启穿透或悬停隐藏后，可以按住配置的修饰键直接拖动窗口，不必来回切换设置。
- 左右两侧是两个独立设置，拖动时惯用哪只手就用哪只。
- 配置、runtime 设置与 overlay options 之间只有一份拼写；平台上报的 usage 与落盘 token 由同一张表
  绑定。
- 录入机制是「按住」语义：一次快速点击可能被漏掉，这正是它读取按下集合而非边沿的直接后果。提示语
  仍按快捷键录入框的写法说「按下」，因此这个差别对用户不可见——实际使用中按住即可，与设置本身要求的
  动作是同一个手势。
- 该行不随开关置灰；在两个开关都关闭时设置它不会报错，取值保留到任一开关打开时生效。
- 该设置不参与无操作隐藏。

### ADR-0081 · 指针被捕获时按相对位移累计光标位置

**状态：** 已接受（2026-10-04）；2026-10-13 采样改为同时携带指针自身位置、悬停命中测试改读它，见「决策 8」

补充：ADR-0011（模块契约）、ADR-0054（配置读取失败恢复）
实现：Issue #785；指针位置分离见 Issue #1140

### 决策

#### 1. 累加在平台 adapter 内完成，不进入 runtime

`CursorProducer` 是 latest-value 通道（`CursorSlot`），只保留最后一个待消费样本。相对位移如果在
平台侧只作为「这一帧动了」的布尔量传给 runtime，再由 runtime 累加，那么两次发布之间被合并掉的采样
会连同位移一起丢失——快速移动会比设备实际移动的距离短。因此位移必须在**发布之前**合并，也就是在
平台线程内完成。

结果是 runtime、renderer 和 `ModelSettings` 都不知道这个开关存在：平台发布的仍是同一个
`CursorSample`，只是位置来源不同；平滑、归一化和模型投影完全不变。

#### 2. `CursorForceMoveState` 是平台无关的状态机，落在 `bongocat-input`

`bongocat_input::CursorForceMoveState` 持有三件事：模式本身、`CursorMotionAccumulator`、上一个采样
的绝对位置。它对外只有三个操作：

- `sync(enabled) -> bool`：读模式，模式变化时丢弃累加位置（旧位置描述的是另一个读法，不能继续推进），
  返回当前是否累加。
- `reset()`：只丢累加位置，不改模式。输入 Reset、以及平台识别出「报告位置而非位移的设备」时调用。
- `advance(absolute, delta, viewport) -> CursorPosition`：推进一个采样并给出要发布的位置。

放在 `bongocat-input` 而不是各自平台，是因为这段策略对两个平台是同一件事，而平台文件在对方平台上
根本不参与编译。放在共享 crate 让它有单元测试，也避免两份实现分叉。

`CursorMotionAccumulator` 单独拆出来，是因为「以绝对位置为种子、按位移推进、夹在 viewport 内」这段
几何与「模式与上一个位置」是两件事，前者可以脱离后者测试。

#### 3. 第一个采样与 viewport 变化重新对齐，且不叠加该采样的位移

累加器的第一个采样和任何 viewport 变化都直接从绝对位置开始，**不**再加上这一次的 `delta`：那个位移
已经体现在绝对位置里，再加一次会多走一个采样。这与 `CursorSmoother` 对首个样本和跨显示器样本的处理
是同一条规则（直接对齐目标），因此两者在 `force_move` 打开与关闭之间切换时不会产生位置跳变。

跨 viewport 必须重新对齐的理由和平滑器一致：两个显示器的坐标系原点不同，把旧显示器的位移加到新
显示器上会得到一个无意义的位置。

#### 4. 累加位置夹在当前 viewport 内，饱和而不是记账

`advance` 把位置夹在 `viewport` 内。到达边缘后继续朝同方向推，位置**饱和**在边缘，而不是把多出来的
位移记在账上——后者会让用户先「还清」反向位移才能看到指针往回动。viewerport 没有可用面积（宽或高
为 0、非有限）时不夹取也不 panic：一个退化的读数不能让输入 worker 挂掉。

#### 5. 报告位置的设备被识别出来并重新对齐，而不是冻结

不是所有设备都报告相对位移。Windows 的 `RAWMOUSE::usFlags` 有 `MOUSE_MOVE_ABSOLUTE` 位（远程桌面、
绝对定位设备会置位），这类包里的 `lLastX/lLastY` 是坐标而不是位移。macOS 没有对应的标志位。

- Windows：绝对包到达时调用 `reset()`，于是下一个采样从绝对光标重新对齐。跳过绝对包而不重置会让
  累加位置停在被钉住的那一点，而绝对设备的光标其实是正常移动的。
- macOS：没有标志位，于是从事实推断——**位置移动了但位移为零**说明这是报告位置的设备；被捕获的
  指针恰好相反（位置不动、位移持续到达）。这个推断只会导致重新对齐，而重新对齐的结果永远是「指针
  真正所在的位置」，所以它最多是无收益，不会引入错误位置。反过来，位置移动**且**位移非零时不移位
  （有些游戏会把光标重新居中，此时按位移累加才是对的，按位置对齐会让模型跳到屏幕中心）。

#### 6. `input.mouse.force_move` 是 `InputConfig` 下的独立命名空间，runtime 只负责转发

配置落在 `input.mouse.force_move`，与 `input.gamepad` 并列：两者都是「输入在被模型看到之前怎么读」。
runtime 侧是 `CursorSettings { force_move: bool }`，通过 `SetCursorSettings` 命令进入
`RuntimeSnapshot::cursor_settings`。

runtime **不消费**这个值。消费者是 overlay session——它持有平台输入服务，而输入服务是唯一能看到设备
位移的组件。session 每帧从 snapshot 读出该值并写进服务的 `Arc<AtomicBool>`（两个平台各一个
`set_force_move`）。用原子量而不是命令通道，是因为这是每帧重放的幂等状态：丢一帧只损失一个采样，
下一帧就补回来，而加一条命令通道会让输入服务多一个必须排序的消息来源。

不放进 `OverlaySettings`：那个类型的文档把它定义成「单个 overlay 窗口的属性」，而这个字段是输入
采集策略，与窗口无关。不放进 `ModelSettings`：那个类型会交给 renderer，而这个字段与渲染无关。
`CursorSettings` 与 `GamepadAxisSettings` 是同一个形状——runtime 拥有的强类型输入设置——所以两者
放在同一个 crate、走同一条 revision-checked 设置命令。

字段带 `#[serde(default)]`（命名空间与内层字段都是），因此缺少 `input.mouse` 的旧配置按关闭读取，
不进入 ADR-0054 的「最新有效备份 → 默认配置」恢复流程。

#### 7. 设置行放在 Input & interaction → Mouse 的最后一行

页面已有的三行是「忽略鼠标输入」「水平翻转鼠标跟随」「垂直翻转鼠标跟随」，都描述**模型拿到指针之后
怎么用**。这一行描述的是**指针位置从哪来**，放在三行之后读作对上面三行的一条附注：如果指针根本不动，
上面三项都没有作用对象。放在最前会读成「下面几项的前提条件」，而它并不是——关闭它时上面三行照常工作。

#### 8. 采样同时携带指针自身位置，悬停命中测试只读它

`force_move` 把模型跟随的位置换成累加位置，但累加位置**不是指针位置**。它由设备的相对位移
（Windows 的 `RAWMOUSE::lLastX/lLastY`、macOS 的 event delta）累加得到，而设备位移与光标位移
不是同一个量：Windows 把鼠标计数换算成光标像素时要经过「指针速度」与「提高指针精确度」两条曲线。
因此累加值是对指针位置的**估计**，并且与鼠标**走到**当前位置的路径有关：同一个终点，快慢不同会
得到不同的累加值。

原实现里 `CursorSample` 只有一个 `position`，于是 overlay 的悬停命中测试
（`overlay.hide_on_pointer_hover`）也拿累加位置判断「指针是否在窗口矩形内」。窗口只有几百像素宽，
路径相关的误差足以让同一个手势有时命中、有时不命中。这就是 Issue #1140：打开该设置后模型窗口
「有时隐藏、有时不隐藏」，把 `input.mouse.force_move` 关掉再打开（累加器重新对齐到绝对光标）
之后短暂恢复正常，重启后又复现。

决策：`CursorSample` 同时携带两个位置——`pointer`（指针自身位置，平台直接读到的绝对光标）与
`position`（模型跟随的位置，`force_move` 打开时是累加值）。两个平台在同一条采样里都发布这两个值。
runtime 的平滑、归一化与模型投影继续只读 `position`，因此本决策不改变决策 1 的边界：runtime 仍然
不知道这个开关存在。悬停命中测试改读 `pointer`——它问的是「指针是否在窗口上」，而累加值回答的是
「模型该看哪里」。

命中测试必须在同一坐标空间里比较，因此 macOS 仍先把指针位置从 CoreGraphics 空间换算到 AppKit
空间（换算对象由「整个采样」收窄为「采样的 `pointer`」）；Windows 两个值本来就是虚拟屏幕像素，
不需要换算。

不采用的替代方案：

- **让累加位置在绝对光标移动时重新对齐**（把决策 5 的规则改成「位置一动就重对齐」）。这能一并消除
  桌面上的累加漂移，但会把「游戏每帧把光标重新居中」误判成普通移动，让模型跳到屏幕中心，正是决策 5
  要避免的情况；而且它改的是模型跟随行为，与 Issue #1140 无关。
- **让 overlay 自己调用 `GetCursorPos` / `NSEvent::mouseLocation`。** 这会绕过「采样缺失或输入服务
  不在 `Running` 时视为不在窗口内」的降级规则（见 Technical Design），也让平台光标读取出现第二份实现。
- **只在 `force_move` 打开时才发布 `pointer`**（其余情况用 `Option`）。指针位置在两种模式下都存在，
  用 `Option` 会让每个读者都必须处理一个不可能出现的分支。

### 明确不做

- **不默认打开。** 打开期间由设备以外的方式移动的指针（程序化 warp、绝对定位设备、触摸板手势）不会
  被跟随；这是模式的代价，因此由用户显式选择。默认值与参考实现一致，都是关闭。
- **不用低级鼠标钩子。** `WH_MOUSE_LL` 提供的是同一个被钉住的坐标，解决不了问题，却会引入一个必须
  常驻的钩子。
- **不引入 DirectInput。** 参考实现用 DirectInput 读位移；本项目的 Windows 输入路径已经是 Raw Input，
  同一个 `RAWMOUSE` 里就有 `lLastX/lLastY`，没有理由再引入第二套设备 API（AGENTS §2.1）。
- **不在 runtime 侧累加。** 理由见决策 1。
- **不改变平滑、归一化或模型投影。** 这个开关只改位置来源。
- **不做「仅特定进程生效」的探测。** 是否捕获指针是前台应用的行为，逐进程探测既不必要也不可靠。

### 后果

- 在捕获指针的全屏游戏里，模型继续跟随鼠标移动；按键跟随不受影响，因为它本来就走另一条路径。
- 桌面上的正常使用不受影响：位移与绝对光标同步移动，累加位置跟着走；绝对设备与重新居中的游戏由
  决策 5 的规则处理。**（2026-10-13 修正）** 「跟着走」不等于「逐像素相等」：系统把设备位移换算成
  光标位移时会应用指针速度与加速度曲线，因此累加位置是对指针位置的估计，误差随鼠标移动路径累积。
  模型跟随用的就是这个估计值（本决策不变），但任何需要回答「指针在哪」的判断必须读采样里的
  `pointer`，见决策 8。
- 配置、runtime 快照、设置协议与 overlay options 之间只有一份拼写；`CursorSettings` 与
  `GamepadAxisSettings` 共用同一套命令与快照通道。
- 该开关在 Windows 与 macOS 上都生效，且两平台共用同一段累加策略与同一批单元测试。
- 采样类型（`CursorSample`）多一个位置字段；runtime 的消费面不变，仍然只读模型跟随的位置。

### ADR-0082 · 手柄摇杆显示参数与多手柄轴投影

**状态：** 已接受（2026-10-04）

### 决策

- **`CatParamStickShowLeftHand` / `CatParamStickShowRightHand` 由 runtime 写入，条件是
  `displaced || pressed`。** 两个值由 `ModelInputSnapshot` 已经带的 `stick_*_down` 与
  `stick_*_x`/`stick_*_y` 现算，不新增快照字段：多一份副本就多一处可能与轴值不一致的
  地方。条件取的是**死区之后**的轴值，和重构前一致（旧版读的是 `gilrs` 自带 jitter/deadzone
  过滤之后的值，当前版把死区放在 runtime 投影阶段，语义位置相同），所以静止的摇杆不会因为
  硬件噪声而闪现。
- **爪子跟随摇杆，但只在模型自己声明了摇杆时跟随。** 门禁是模型参数表里查
  `StickShowLeftHand`/`StickShowRightHand` 是否存在，即 `set_normalized_parameter` 报
  `Unsupported` 的同一个答案。这把 ADR-0042「缺图不动爪」从按键图片推广到摇杆美术：一个
  没有摇杆的模型上，推动摇杆不会让爪子为一件永远不出现的东西落下。预置 `standard` /
  `keyboard` 都没有这两个参数，因此和重构前「只在 gamepad 模式下监听手柄」的结果一致。
- **摇杆的三个事实仍然互相独立。** `StickLeftDown`/`StickRightDown` 只表示摇杆键被按下，
  `Stick*L*`/`Stick*R*` 只表示位置，可见性参数只表示「这一侧的手柄正在被使用」。旧版
  `CatParamStickLeftDown = value !== 0` 也只跟着摇杆键走。
- **每个轴由最新的样本获胜，而不是由某一个 connection。** 投影遍历所有仍被 runtime 接受
  的 connection 的样本，逐轴取 `at` 最大的那个，再对它应用死区。这与重构前「同名轴最新
  一次写入获胜」逐字一致，也让「摇杆回到中心」读到 0 而不是停在最后一个非零值。先选
  connection 再取值的旧写法在单手柄下不变（只有一个 connection 能赢）。
- **不在本仓库为 WGI 默认 mapping 兜底。** 那个映射表属于 `ayangweb/gilrs`，本仓库补偿
  它就要在 adapter 里复制一份第三方位置表，并在上游修好之后静默变成双重修正。ADR-0066
  记录了证据与需要的修法。

### 后果

- 手柄模式下推动任一摇杆或按下 L3/R3，该侧的摇杆美术出现，并且该侧的爪子落下；松开或回中
  后同一帧消失并抬起。预置 `gamepad` 模型与 Mver 转换出的手柄模型立刻可见地正确了。
- 键盘/鼠标模型不受影响：它们没有摇杆参数，摇杆既不画也不动爪。
- 两个手柄交替使用时，后用的那个手柄的摇杆和扳机立刻生效；同一轴上最后一次移动的那只手
  柄获胜，这与重构前合并所有手柄的行为一致。多手柄仍然只有一副爪子，这是模型的限制。
- `ModelInputSnapshot` 的形状不变，`schema_version` 仍是 1，没有配置或持久化格式变化。
- 未变的行为：缺图的按钮保持惰性；`model.ignore_gamepad` 把六个轴投影为零并清掉摇杆与
  手柄的爪子贡献；扳机仍然在 `>= 0.5` 产生按钮边沿，其连续值仍然只作为轴运输。
- **仍然存在的空白，且不由本 ADR 关闭**：预置 `gamepad` 模型没有 `Select.png`、`Start.png`
  和两个摇杆键的图，因此这四个按钮不画图也不落爪（L3/R3 仍驱动 `*Stick*Down`）。这是美术
  缺失而不是映射缺失——ADR-0042 规定缺图按钮保持惰性，凭空生成美术会越过模型作者的
  contract。Mver 转换出的手柄模型不受此限：legacy config 的 XInput 序号表覆盖 0–15，
  转换会为全部十六个按钮生成图并按 `lefthand`/`righthand` 落盘。预置模型的美术补齐是独立的
  资源任务。

### ADR-0083 · 重复触发同一表情可以把它关掉

**状态：** 已接受（2026-10-06）

补充：ADR-0079（新增配置字段的可选读取约定）、ADR-0030（实现决策阶梯）
需求：Issue #1109

### 决策

#### 1. `model.toggle_repeated_expression` 是唯一的开关，默认关闭

放在 `ModelConfig`，`bool`、默认 `false`、正向语义，设置页呈现为「重复触发同一表情时关闭它」，
位置紧邻「记住每个模型上次使用的表情」。

默认 `false` 是为了不改变任何现有用户的画面：这是一个可观察的行为差异，只有一次显式选择才引入。

字段带 `#[serde(default)]`，与 `model.show_all_pressed_keys` 同一理由（ADR-0079 §1）：v1 解析入口是
严格的，一个必填新字段会让该字段加入之前写下的每一份 `config.json` 解析失败，走「最新有效备份 →
默认配置」的恢复流程并丢掉用户设置。缺少该字段的旧文档读出 `false`，即升级前既有的行为。

它是**独立开关**而不是 `remember_last_expression` 的一个取值或第二个字段：两者回答不同问题——一个
是模型从哪里开始（记住上次那张脸），另一个是重复触发时做什么。把它们合成一个枚举会让「记住但不
切换」「切换但不记住」两种组合都不可表达。

#### 2. 判定在 runtime，因为那里才知道当前生效的是哪个表情

两个触发源都收敛为同一条 `RuntimeCommand::SetExpression`：设置窗口里每个表情行的播放按钮走
`Application::set_expression`，快捷键走 `ShortcutAction::SetExpression` 后的 `trigger_shortcut`。
只有 runtime 同时持有 `active_expression` 和 `renderer`。放在 Application 需要把「当前生效的表情」
复制一份到 Application 会话状态里，那是把 runtime 的事实复制到它外面。

`ModelSettings` 增加 `toggle_repeated_expression`，由 `config_projection::model_settings_from_config`
从配置投影，因此这个字段和其它模型设置走同一条 `SetModelSettings` 路径。设置页的
`SettingsCommand::SetModelSettings` 分支在构造 runtime 设置时从配置里带上这个值，避免用户动其它
模型设置时把它重置。

命令语义：请求的表达式等于当前 `active_expression` 时，命令变成「关闭」——`renderer.clear_expression`
让所有图层开始淡出、不压入新图层，`active_expression` 置空。不等于是普通路径，逐字不变。

#### 3. 关闭动作不写「记住上次使用的表情」

`user_expression_memory` 只在「应用」分支写。用户正在撤销一个选择，把刚被关掉的表情记下来会让它
在下次启动、或切回该模型时以「用户选过」的名义回来。因此关闭之后 `last_expressions` 保持不变，
记住的仍然是用户最后选择穿上的那张脸。

这也是它作为独立开关的另一个理由：记忆的语义（「用户选过什么」）和开关的行为（「重复触发做什么」）
在关闭这条路径上必须解耦。

#### 4. 随机播放不参与

随机选择器通过 `renderer.set_expression` 直接播放，不走 command 队列——这与「自动选择不能进入
记忆集合」是同一条既有边界（`SetExpression` 是「用户主动」的唯一判据）。把开关的判定放在
command 分支里，随机播放天然不在其中，不需要额外门禁。

#### 5. 「恢复上次表情」不会被读成重复触发

`restore_remembered_expression` 在 `prepare_model` / `select_model` 之后发送 `SetExpression`。worker
在 `pending_model` 非空时把除 `Shutdown` 与 `ApplyInput` 以外的所有命令推迟
（`crates/bongocat-runtime/src/worker/mod.rs`），而 `active_expression` 在模型 commit 时被清空，所以
恢复命令执行时 `active_expression` 一定是 `None`，不可能与请求的表达式相等。

这条不是新加的保证，而是**必须被钉住的不变量**：一旦有人让模型切换不再清空 `active_expression`，
或者不再推迟命令，打开开关的用户就会看到「刚恢复的表情立刻自己关掉了」。

### 后果

- 打开开关后，再触发一次当前生效的表情会淡出回到模型默认表情；关闭时行为与升级前逐字一致。
- 渲染器没有新增模型状态：`clear_expression` 复用「替换」已经在用的同一条淡出路径，只是不压入新
  图层，剩余图层由既有的 `evaluate` retain 按 clip 自己声明的淡出时长丢弃。
- 设置页新增一行开关、一个 revision-checked command、一条 client 方法和一份 snapshot 字段；
  `SettingsModelSettings` 不变，因此这个开关不会被其它模型设置覆盖。
- 自动化契约全部落在真实实现上：
  - `bongocat-runtime` 渲染层（`clearing_the_expression_stack_returns_the_model_to_its_default_face`、
    `clearing_twice_is_the_same_as_clearing_once`、
    `clearing_without_an_active_model_reports_nothing_to_close`）读模型参数而不是 render snapshot
    ——呼吸和眨眼每帧都动大部分模型，只有被表情独占写入的参数能说明表情是否还在生效；
  - `bongocat-runtime` worker（`a_repeated_expression_is_applied_again_while_the_toggle_is_off`、
    `a_repeated_expression_turns_itself_off_while_the_toggle_is_on`、
    `the_toggle_does_not_reach_the_automatic_expression_picker`、
    `a_restored_expression_is_never_read_as_a_repeat`）钉住两态、关闭不写记忆、随机播放不受影响、
    以及恢复不被读成重复；
  - `bongocat-app`（`the_expression_toggle_is_one_switch_that_reaches_the_runtime_and_the_document`）
    跑通「开关 → 配置 + runtime 设置 → 表情触发」并确认记忆集合不变；
  - `bongocat-ui`（`the_expression_toggle_switch_sends_its_own_command`）钉住它是自己的行、自己的
    command，并且不会连带移动相邻的记忆开关；
  - `bongocat-config`（`a_configuration_written_before_the_expression_toggle_still_loads`、
    `the_expression_toggle_defaults_to_off_and_is_independent_of_the_memory_switch`）从序列化后的
    当前默认里删掉该字段，模拟旧文档的字节。
- 共享 fixture 只更新 `shared/config/fixtures/default.json`：新增字段是可选的，其余 48 份 fixture
  保持没有该键的样子，正是它们要验证的「旧文档仍能读取」。
- 未运行实机验证：只在本机（macOS）跑了 `just check` 与 `tools/` 下的校验脚本，没有做 macOS 或
  Windows 实机 smoke。淡出回到默认表情的观感需要实机确认。

---

# 五、番茄钟 / 修仙

> 番茄钟领域边界与修仙体系全部数值/规则不变量。

### ADR-0090 · 番茄钟的领域边界、持久化与倒计时不持久化

**状态：** 已接受（2026-10-15）

### 决策

#### 独立的领域 crate，平行于音频服务

新增 `bongocat-pomodoro`，与 `bongocat-audio` 平行：不进 `bongocat-platform`（它没有任何 OS 能力需求），
也不进 `bongocat-runtime`（不动渲染）。它自带 worker 线程、有界命令队列、共享快照与三段式 shutdown，
由 `bongocat-app` 的 startup 接线。四个 crate 之间的方向是单向的：`app → ui/overlay/platform → pomodoro`，
领域 crate 不认识任何展示面。

实现决策阶梯上它落在「复用仓库已有模式」一级：worker 线程形状照抄 `MotionAudioService`，
持久化照抄 `interaction_stats.rs` 的严格版本 + 原子写 + 私有权限。

#### 纯值状态机，时钟由调用方注入

`PomodoroState` 是没有时钟也没有 IO 的纯值：`apply(command, now_unix)` 处理命令，
`advance(elapsed: Duration, now_unix)` 折叠时间。worker 线程每秒把「距上次唤醒过去了多久」交给它，
测试则直接交一个选定的时长。

这是 ADR-0030「时间逻辑使用可注入的单调时钟，不得用墙上时间驱动」的落地：倒计时由单调时间测量，
墙上时间只用于把一次完成记进月历。附带的好处是二十五分钟的会话在测试里以微秒完成，
而「唤醒晚了五秒」这个真实故障模式也能被直接覆盖。

worker 的等待时长取「本次会话剩余时间」与一秒上界的较小值，因此运行中的会话恰好在到点时被唤醒；
非运行状态只等命令，空闲服务不消耗任何周期。

#### 倒计时不进文档

`pomodoro-tasks.json`（`schema_version = 1`）只保存任务列表（名称、预计/已完成番茄数、状态、
创建与完成时间戳、每次完成的时间戳数组）与焦点时长字段，不保存倒计时。
月历数据从完成时间戳推导，不另存日历文件。

理由是倒计时不是一个值得保存的事实：用户可以放弃它、暂停它或者直接离开机器，
这些结局都会让存下来的剩余秒数变成某个无人问询的时刻的快照。下次启动恢复它，
等于重启一个截止时间早已过去的倒计时。用户想要的是已完成的工作，所以文档记录它，
然后由用户从任务重新开始一整段会话。

`expected_pomodoros` 与 `completed_pomodoros` 是文档里两个独立的计数：预计值是用户的预期，
已完成值是记录的事实，改一个不能顺带改另一个。

#### 每一条完成都带时间戳，因为月历要数「任务数」

月历要显示每日「完成番茄数 + 任务数」。两个数不能互相推导，所以每次完成都往任务的
`completion_timestamps` 里追加一个时间戳，聚合时按本地日把时间戳分桶，桶内先数会话数、
再去重任务 id 得到任务数。按日去重意味着「一天跑十段同一个任务」显示为 10 与 1，
这是一对能解释当天情况的数字，而不是两个看起来应该相等的数字。

时区处理：本地日 = Unix 时间戳加上调用方给的 UTC 偏移后再整除 86400，偏移只在时间戳进入时施加一次。
本 crate 因此不依赖时区库；读本机偏移是平台调用，而拥有它的展示面已经知道偏移。

#### 表演不进模型动画系统

完成表演是纯代码实现：猫窗口左右摇摆、红色渐变、头顶番茄图标缩小消失。
现有模型动画系统只能播放模型包声明的 motion group，没有「吃番茄」「摇晃」也没有染色 API，
所以表演在 AppKit 浮层层实现，不调用 Live2D motion。

摇摆必须与窗口的自动纠偏和 hover hit test 解耦：session 的 `window_bounds()` 返回不含表演偏移的
逻辑原点，placement 纠偏与命中测试都读逻辑原点，表演偏移只叠加到真实窗口 frame 上。
否则摇摆会被当成「窗口被拖出屏幕」而拉回，并让 hover hide 逐帧抖动。

#### 暂停是冻结，放弃是作废

暂停冻结剩余时间且可恢复，不写入文档；放弃结束会话且不计数。这两者必须能区分：
一个暂停的倒计时仍然有剩余时间，把它渲染成空会让暂停与放弃无法分辨。

「完成」与「取消完成」是同一个命令的一个方向（`SetTaskDone { done }`）：
勾选框表达的是一个位置，不是两个按钮。取消完成不抹掉已经完成的番茄数。

#### 入口只在 macOS 编译

菜单项、菜单 id 常量、`SystemMenuAction::StartPomodoro` 变体与 presentation 字段
全部用 `#[cfg(target_os = "macos")]` 编译掉，Windows 的菜单树保持原样且不留塌缩的分隔符。
i18n 键仍然写进全部 8 个语言文件：locale 键集一致性由 `tools/validate-locales.py` 与
catalog 测试强制，它们不接受平台豁免；覆盖测试是纯文本扫描，不解析 cfg，所以放在条件块里的键在两个平台上都能被看见。

### 后果

- 退出应用会丢失进行中的倒计时，用户从任务重计一整段。这是明确的取舍，写在任务界面上。
- 新增一个消费面（例如喝水提醒）不需要新的持久化文件，只要接上 `PomodoroSnapshot` 的读路径。
- 浮层与表演代码只在 macOS 编译，因此无法用 macOS 编译推断 Windows 的任何行为；Windows 侧只验证菜单 cfg 分支。
- 领域 crate 无 OS 依赖，可以在任何平台上跑完整状态机测试。

### ADR-0091 · 番茄钟在设置中的入口是顶级页，任务列表只读 + 勾选

**状态：** 已接受（2026-10-15）

### 决策

#### 顶级页，不嵌套在「模型行为」下

「番茄时钟」是 `SettingsNavigationPage` 的一个新变体（index 3，位于「模型行为」与
「模型窗口」之间，About 仍最后）。理由是它与「模型行为」不同类：模型行为是**每个模型**
的配置，而番茄计时是用户在会话之间使用的、**模型无关**的活动。把它塞进模型行为页会让
一个全局功能看起来像是某个模型的属性。顶级页还让它在侧栏直接可达，不需要先选一个模型。

#### 任务列表只读 + 勾选完成，不做增删改

设置页的任务列表只展示任务，并提供勾选/取消勾选（`PomodoroCommand::SetTaskDone`）。
新建、编辑、开始仍走第 4 步的独立任务弹窗。理由：任务的创建/删除是写操作，
若设置页也能增删改，就会出现两个写入口对同一份 worker 状态双写而分歧的风险。
只读 + 勾选让设置页成为任务状态的第二个**读面**加一个**同源勾选写面**，
与弹窗共用同一条命令通道，不会双写。

#### 用 `SettingItem::render` 自绘整页

因为 `SettingPage` 无子页，任务列表 + 月历这种复杂管理视图唯一的落地方式就是一个
自绘的 `SettingItem::Element`：一个 `SettingItem::render` 闭包返回整页内容，
列表/日历用页内一个 `PomodoroSettingsView` 开关切换（切换只换 body，不清空任务数据）。

#### 设置页靠 revision 轮询跟随 worker

`PomodoroClient` 不提供订阅 API，因此 `SettingsView` 起一个 500ms 的后台轮询
`poll_pomodoro`：比对 `client.revision()`，仅在 revision 变动时 `cx.notify()`。
这复用了第 4 步 `pomodoro_window` 的 `poll_state` 模式，空闲窗口零快照克隆。

### 后果

- 侧栏多一个顶级项「番茄时钟」；`SettingsNavigationPage` 的顺序、`index()`、
  `.pages(vec![...])` 三处必须同步，由 `navigation_uses_the_agreed_task_order_with_about_last`
  等测试强制。
- `SettingsView` 需要持有一个 `PomodoroClient`：它从 `open_settings_window` 一路透传
  （app 侧 `ensure_settings_window` 已持有 `coordinator.pomodoro_client`）。
  该字段**不加平台 cfg**——Windows 上菜单入口隐藏但 worker 照常起（ADR-0090）。
- 任务的创建/编辑只存在于弹窗；设置页是第二读面 + 勾选写面。
- edition 2024 下自绘函数若返回 `impl IntoElement` 会捕获 `&mut Window/App` 生命周期，
  与 `SettingItem::render` 要求的 `'static` 闭包冲突，故统一返回 `gpui_kit::AnyElement`。

### ADR-0092 · 修仙阶梯取代交互次数等级，破境必须靠装备

**状态：** 已接受（2026-10-19）

### 决策

#### 阶梯是一个纯值状态机，挂在番茄钟领域里

`bongocat-pomodoro/src/cultivation/` 新增子模块：`mod.rs`（境界、属性、战力、
SplitMix64）、`curve.rs`（成长值阶梯）、`items.rs`（装备）、`garden.rs`（仙草园）、
`battle.rs`（守卫）、`document.rs`（持久化形状）、`state.rs`（状态机与视图）。
`Cultivation` 不持有文件句柄、不读时钟、不碰线程；所有时间由 `PomodoroState` 传入。
`CultivationDocument` 作为 `Document` 的一个 `#[serde(default)]` 字段落在同一份文件里，
因此旧文档读进来就是「一个刚入门的新修士」。

#### 成长值是唯一货币，境界是硬上限

当天第一个完成的番茄钟固定 +500 点（`SESSION_BASE_GROWTH`）；同一天第 N 个（N≥2）按
`500 × 1.1^(N-1) × 境界序号`（`session_growth`），键鼠每次输入各 +1（合批下发，
避免逐键落盘）。`level_for_growth` 把成长值换回等级，`STAGE_FLOORS` 定义每境第一层的
门槛（炼气 0 → 仙人 600,000，Lv.1000 需 1,095,000）。等级一律 `min(realm.cap_level())`：
**成长值攒得再多也停在 100 层**，必须破境才看得见后面的层。这一条把「境界」做成
真正的门槛而不是显示分组。

#### 战力两条分系、每系配对联动，守卫按裸装的 120% 取值

属性四项（根骨/力量/仙力/身法）线性增长并在每次破境整段跃升。战力分两系：

- **法术** = `(根骨×15 + 仙力×20 + min(根骨, 仙力)×10) / 10`
- **武术** = `(力量×20 + 身法×12 + min(力量, 身法)×10) / 10`

两系的最后一项是**联动**：同系两个属性互为表里（根骨是法术的根基、仙力是法力；
力量是武技的劲、身法是发劲的准头），所以取较低者再计一份默契加成。结果是「只堆同系
里权重更高的那个」会白费加成，20 点均分（法术 45）胜过 20 点全押仙力（40）——这条由
测试 `a_balanced_pair_out_powers_a_lopsided_one` 钉住。

总战力 `power()` = 法术 + 武术，等级、装备、仙草三处加成全部先落到 `Attributes`
再走同一条公式。守卫战力 `base_attributes(cap).power() × 120%`，即同境界满层、**不带
任何装备**的修士的 1.2 倍。于是「裸装刚好差一点、补装备就能赢」是算出来的而不是调参
出来的，测试直接断言两条。

#### 掉落互斥且是概率，RNG 可种子化

每次完成专注各掷一次：装备 60‰、种子 50‰，**不共存**（一次只掉一样）。随机源是模块内
实现的 SplitMix64，`Rng::seeded` 让测试用固定种子复现同一串掉落，测试因此能断言
「1000 次会话掉落数落在 20..=250」而不是断言某个具体结果。

#### 扣分幂等、翻倍、有下限、碰不到已破的境界

`apply_absence(today)` 只结算到**昨天**，并用 `charged_through_day` 记住结算到哪一天，
所以同一天多次 tick、多次重启都不会重复扣。一天不修炼扣 `800 × 境界序号`（起点境界
恰为 800），连续未修炼再按 `1.2^missed_streak` 放大（`missed_streak` 自 0 起，首日即
指数 0）。扣完再 `max(stage_floor(realm))`：成长值可以被扣掉，境界不能。

#### 时间从启动处注入一次

`PomodoroService::start_at(path, utc_offset_seconds)` 取代只传路径的入口，app 启动时传
`bongocat_platform::local_utc_offset_seconds()`。旧的 `start(path)` 保留为 offset 0 的
委托，仅测试与 Windows 使用。跨夜结算发生在 worker 的 tick 里：每个 tick 先结算缺勤，
有扣分才发布新快照并落盘。

#### 每升一级发 1 点自由属性，按历史峰值结算

等级本身已经抬升四项属性，但那是固定曲线，用户没有选择。因此每升一级额外发
`FREE_POINTS_PER_LEVEL`（1 点）由用户手工分配：`Cultivation::allocate` 把 1 点加到某
一项上，落进 `CultivationDocument.allocated`，再与等级/装备/仙草一起进同一条战力公式。

点数按**历史最高等级**（`peak_level`）结算，不是按当前等级：缺勤扣分会把等级压下来，
但已经赚到并花掉的点不能被收回。`normalized()` 里从成长值反推峰值并夹住已分配点数，
所以手改文档既不能凭空多也不会有负数的点。`free_points = granted - allocated` 用饱和减法，
多分的非法状态只会暂停发点，不会变成负余额。

#### 收获按低概率返种，让仙草园能自己续上

`harvest` 先付作物属性点（`5 + 3 × 阶`），再掷一次 `SEED_RETURN_PERMILLE`（150‰，约七分之
一）：中了就给同属性的种子，其中 `SEED_RETURN_UPGRADE_PERCENT`（30%）升一阶，其余同阶。
返种概率远低于掉落（装备 60‰ + 种子 50‰），所以「种一轮只回一次种子」是常态，仙草园
不会变成自给自足的循环经济，但也不会因为种子用完而彻底停摆。返回的种子与掉落走同一个
`push_seed`，因此自动并进已有的种子堆。

#### 展示面继续分层：overlay 不读 catalog

额头徽章与悬浮 HUD 仍是 AppKit 面板，但它们只画被传进来的字符串。app 侧
`cultivation_copy(&CultivationView, locale, total, remaining)` 用 `bongocat_i18n` 合成
`OverlayCultivationText { badge, hud, hud_progress_permille }` 后交给 overlay，与番茄钟
菜单的分工一致：读 catalog 是产品的职责，画文字是面板的职责。

悬浮 HUD 两行：第一行境界·层·成长值进度，第二行累计番茄数与当前任务剩余数。这两个数
来自 `PomodoroSnapshot` 的 `total_pomodoros` / `remaining_pomodoros`，不是修仙状态机算的
东西——它们关于「喂养阶梯的会话」，而不是关于「阶梯」。累计数单独记在
`Document.total_completions` 而不是把任务计数器相加：删掉一条任务不该把已经干过的活
从历史里抹掉；剩余数在倒计时运行时答当前任务，空闲时答所有未完成任务之和。

#### 删掉交互计数里的等级曲线

`bongocat-platform/src/interaction_stats.rs` 只保留计数，等级相关的
`LEVEL_ANCHORS`/`threshold_for`/`level_for`/升级音效/观察等级全部删除，模块文档改写为
「等级来自番茄钟修仙体系」。同一份成长值不再有两个来源。

### 后果

- 一千个 `stats.level.name.*` 键从全部 catalog 消失，换成 `stats.stage.name.1..10` 与
  `cultivation.*` 一组键；`tools/validate-locales.py` 与
  `catalog_keys_are_referenced_by_source` 双向覆盖仍然强制每键都有调用点。
- 旧用户升级后从炼气一层重新开始：交互次数不再是等级，成长值从 0 起算。这是需求的
  直接结果，changelog 的「移除」段明写。
- 时间依赖从「只有番茄钟倒计时需要时钟」扩展到「缺勤结算需要本地日历日」，因此
  `PomodoroState` 持有 offset；`CalendarDay` 仍是唯一的日历换算入口。
- 破境的失败次数被记录（`breakthrough_failures`）但暂不参与任何公式，为后续
  「连败保底」留下字段而不必再改文档形状。
- 仙草成熟按真实时间（每阶半小时），页面上的剩余分钟是展示层自己的减法，领域不轮询。
- 自由属性点与返种都是「多一个可以慢慢丢掉的字段」：`peak_level`、`allocated` 都有
  `#[serde(default)]`，旧文档读进来按已有成长值补算峰值，不会凭空少点也不会凭空发点。

### ADR-0094 · 成长曲线按真实节奏重设计，等级只对之后生效

**状态：** 已接受（2026-10-20）

修订：ADR-0092 中 `STAGE_FLOORS` 的具体数值（阶梯形状、境界硬上限、战力公式、守卫比例均不变）

### 决策

#### 新阶梯：单层成本抬 7–18 倍，境界间仍约 2.1 倍递进

`STAGE_FLOORS` 从 `[0, 1_200, …, 600_000]`（`TOP_GROWTH` 1,100,000）改为
`[0, 10_000, 30_000, 80_000, 200_000, 450_000, 1_000_000, 2_100_000, 4_400_000, 9_000_000]`
（`TOP_GROWTH` 18,000,000），满级总成长 1,095,000 → 17,910,000（16.4 倍）。

| 境界 | 单层成本 v1 → v2 | 该境耗时（每天 3 场） | 累计天数 | 累计场次 |
| ---- | ---- | ---- | ---- | ---- |
| 炼气 | 12 → 100（8.3×） | 1 → 6 天 | 6 | 18 |
| 筑基 | 28 → 200（7.1×） | 1 → 8 天 | 14 | 42 |
| 金丹 | 60 → 500（8.3×） | 2 → 12 天 | 26 | 78 |
| 元婴 | 120 → 1,200（10×） | 2 → 23 天 | 49 | 147 |
| 化神 | 230 → 2,500（10.9×） | 4 → 40 天 | 89 | 267 |
| 炼虚 | 450 → 5,500（12.2×） | 6 → 74 天 | 163 | 489 |
| 合体 | 800 → 11,000（13.8×） | 9 → 128 天 | 291 | 873 |
| 大乘 | 1,500 → 23,000（15.3×） | 16 → 235 天 | 526 | 1,578 |
| 渡劫 | 2,800 → 46,000（16.4×） | 25 → 420 天 | 946 | 2,838 |
| 真仙 | 5,000 → 90,000（18×） | 42 → 744 天 | 1,690 | 5,070 |

每天一场的轻用户节奏按同一张表整体拉长三倍（炼气满 20 天、全程以年计）。目标是让
"一个境界"重新等于一段有记忆的坚持，而不是一天的余量。

不变的部分：每段跨度仍是 `LAYERS_PER_STAGE`（一百）的整数倍，`threshold_for` 与
`level_for_growth` 仍精确互逆；境界仍是硬上限，等级一律 `min(realm.cap_level())`。

#### 只有"之后"变难：`curve_rev` 哨兵加保级重标定

重设计不能拿走已经付过的等级，因此 `CultivationDocument` 增加 `curve_rev`：

- **字段缺失读作 1**（v1 曲线）。`normalized()` 首次遇到它时把 `growth` 重标定到
  **同一等级**在当前曲线的开口处，再加上同一层内进度比例：
  `growth' = threshold_v2(level) + (growth - threshold_v1(level)) × step_v2 / step_v1`。
  已得的层、已得的自由属性点（`peak_level`）、已得的装备与功法全部原样保留，之后的层
  按新门槛计算。生产存档 4,986（v1 的 217 级 26/60 处）迁移到 38,216，仍是 217 级。
- **新鲜默认值读作 0**，含义是"还没上过任何曲线"，只盖章不重标定。这条区分是必需的：
  `Default` 派生出的文档 curve_rev 为 0，若与"字段缺失"共用一个读法，新用户第一次
  落盘后攒的成长值会被当成 v1 再解释一次，等级凭空暴涨。
- 已带 `curve_rev = 2` 的文档原样通过，迁移因此是幂等的，`normalized()` 跑两次结果相同。

`curve.rs` 保留一份 v1 阶梯的只读镜像（`mod v1`），唯一用途是这次重标定的读侧；没有
第二处调用它。

#### 刻意不动的三个量

本次只放曲线这一个变量：

- **`session_growth`（500 基数、1.1 递增、境界序号倍率）不动。** 它是"越爬越快"的正
  反馈来源，也是玩家每天都能看见的数字；改它会同时动到缺勤扣分的对称性。
- **`absence_penalty`（800 基数、1.2 递增）不动。** 门槛抬高后缺勤扣分的相对重量自动变
  轻（炼气一天从"抹掉 67% 的境界"变成 8 层），这正是难度提升后应有的方向。
- **战力两系公式与守卫 120% 比例不动。** 它们与成长值曲线无关。

### 不变量

1. `level_for_growth(threshold_for(L)).level == L` 对全部 1..=1000 成立（测试钉住）。
2. 迁移不降低等级、不降低 `peak_level`、不触碰装备、种子、功法（测试钉住）。
3. `curve_rev` 为 0 的文档永远不重标定；为 2 的文档永远不被改动（测试钉住）。

### 回滚条件

若迁移后出现任一等级下降、`peak_level` 下降、或同一天内重复迁移的痕迹（成长值跳变两
次），说明哨兵语义被破坏，立即回滚到 v1 阶梯并把 `curve_rev` 置 2（保留字段，v1 与 v2 的
重标定是恒等的常数变换，回滚不需要二次迁移）。

### ADR-0095 · 试炼塔——层即等级，在家才可爬

**状态：** 已接受（2026-10-21）

### 决策

#### 一层 = 一个等级，守关人是那一层的裸装修士打九折

`trial_floor(f)` 映射到等级 `f`（`floor_level`），守关人取 `base_attributes(f)` 的
**属性**再乘 `TRIAL_OPPONENT_PERMILLE`（900‰），走与玩家完全相同的
`Attributes → Derived → Fighter` 换算。于是：

- 首层就是炼气 1 层 1 阶（需求原文），第 101 层起自动进筑基，因为等级跨境界时
  `base_attributes` 有整段跃升。
- 塔的难度曲线不需要新表：它**就是**等级曲线。层数成为战力的另一种读法——
  「你打到第 37 层」等于「你的战力相当于 37 级的裸装修士」，一句可验证的话。
- 面板显示的战力 `warden_power(f)` 与 `guardian` 同口径（裸装战力 × 千分比），
  显示的数和交手的数是同一条公式。

九折是**入门缓冲**而非难度斜坡：早期境界里 `攻击 10 / 生命 100` 这类常数占比重，
0.9 倍属性在低层几乎等于白送（第 1 层胜率约 85%），高层才是真门槛。测试
`a_naked_save_holds_its_own_level_and_not_much_past_it` 把这条钉住：裸装修士在自己的
层上胜率 ≥85%，在 1.25 倍层上 ≤15%。**换任何属性公式或战斗公式都必须重新过这条测试**,
它就是这一层的「守卫」。

#### 每天五次，且只在「家中」开塔

`TRIAL_ATTEMPTS_PER_DAY = 5`，按本地日记数（`trial_day` + `trial_attempts_today`），
与集市货架同一天界，所以「昨天没爬」不会累积成 10 次。

「外出」在现有实现里就是探索的 12 小时冷却（文案本来就写「上次外出尚未回返」）。
`trial_blocked` 因此有三个理由，页面按其一显示：`Out`（探索未回）、`Drained`
（今日次数尽）、`Top`（已通关 1000 层）。三者都是 `Refusal`，走既有的 toast 通道。

这条门把探索与试炼做成**二选一**：走一趟换灵石与随机产出，留在家换五次可控的闯关。
不做成「都能做」是因为两者都在发放同一批物品，同时开等于把每日产出翻倍。

#### 胜负用同一套战斗引擎，赢才掷奖

`trial()` 复用 `duel()`：玩家的 `fighter()`（含最强的三个外功之一）对
`warden_fighter(floor)`，战斗回放与破境、遭遇战完全同一套弹窗。赢→
`document.trial_floor = floor`（推进一层）→ 掷一次奖励；输→ 层数不动，只费一次机会。

奖励是**一次互斥掷奖**，比例照探索的静走轮（种子多、装备少、功法极少）压低：

| 结果 | 千分比 |
| --- | --- |
| 种子 | 300 |
| 装备 | 60 |
| 功法 | 10 |
| 一无所获 | 630 |

品阶不随概率变，随**层所属的境界**变（`floor_realm(floor)`）：越往上打，出的东西档次
越高，这是「爬得越高越值钱」的唯一实现方式，不需要给概率再加一条深度曲线。按五次全胜
估算期望约 1.8 件/天，低于「探索 + 掉落」同时做满时的水平，但试炼不出灵石——那仍归
专注与探索。

#### 每日额度、层数与已通关层都落盘

`CultivationDocument` 新增三个 `#[serde(default)]` 字段：`trial_floor`（已通关层）、
`trial_day`（额度所属的本地日）、`trial_attempts_today`（当日已用次数），外加
`trials`（累计尝试次数，供 UI 判断「这是新战报」）。`normalized()` 把层数夹到
`TRIAL_MAX_FLOOR`、次数夹到 5，手改存档不能凭空多爬一层或多打一次。

#### 常驻提示不再复述事件日志

需求同时指出「最近事件」上方与日志内容重复。原先常驻提示读的是 `last_training`、
`last_breakthrough`、`last_drop`、`last_seed_return`、`last_absence`，其中前两者与日志
逐字重复。修法是**结构性的**，不是去重比较：日志能装的两件事（修习、破境）从提示里
删除，日志装不下的三件事（缺勤扣分、败北掉装备、收获返种）留下。破境日志条目顺带补上
`stage: Option<u8>`（`#[serde(default)]`，旧事件读作 `None` 走裸行），于是提示删掉之后
「破境成功，晋入元婴」这句并没有丢。

### 后果

- 试炼塔与探索共用「外出」这一个状态，因此**同时只能做一件**。想两头都要的存档做不到，
  这是刻意的：两个口子发同一批物品。
- `FightKind::Trial` 进入战报记录；试炼的胜场同样计入战斗维度，进而抬升成长值倍率——
  塔不只是发物品，也是练战的地方。
- 第 1000 层之后 `trial_blocked` 恒为 `Top`，塔顶不可再爬；这是阶梯的长度的直接结果，
  不是额外的封顶开关。
- 事件日志新增 `EventDetail::Trial { floor, won, art, gear, seed }`；试炼因此和散步一样
  留下「第几层、得了什么」的一行，弹窗关掉也还在。
- 删掉 `cultivation.notice.art_failed / art_ranked / art_mastered` 与
  `cultivation.breakthrough.lost` 四个键（`catalog_keys_are_referenced_by_source` 会抓
  孤儿键），常驻提示组只剩三条。

### ADR-0096 · 每一处都有所得，四种来源各有侧重；每天开一次晨间事件

**状态：** 已接受（2026-10-21）

### 决策

#### 灵石是每一处的保底，四种来源各有一只钱包

灵石不是「发现」，是钱：它和发现并列掷，而不是替代发现。于是每个来源有自己的钱包
（`StoneRoll { permille, min, max }`），钱包的大小就是该来源的侧重之一：

| 来源 | 灵石（必得） | 种子 | 装备 | 功法 | 侧重 |
| --- | --- | --- | --- | --- | --- |
| 静走探索 | 3~8 | 450‰ | 120‰ | 30‰ | 采药 |
| 遭遇战胜利 | 5~14 | 220‰ | 300‰ | 80‰ | 劫财 + 装备 |
| 破境胜利 | 10~24 | 45% | 必得 1 件 + 40‰ 追加 | 必得 1 部 | 传承 |
| 试炼通关 | 2~6 | 300‰ | 60‰ | 10‰ | 练兵 |

四只钱包的大小顺序（试炼 < 静走 < 遭遇战 < 破境）写成了模块级 `const` 断言：这顺序
就是「侧重」本身，调参调坏了会直接编译失败，而不是等到玩家发现四处产出一样多。

> 本文的固定钱包已被 **ADR-0097** 修定：钱包改为「第一境区间 × 当前境界」，顺序也改为
> 静走 < 试炼 < 遭遇战 < 破境（试炼是战斗，必须高于非战斗）。晨间事件的概率与「直接获得
> < 遇袭」不变。

遭遇战的发现权重从「种子 450 / 装备 250」改成「装备 300 / 种子 220」：旅人身上带的
是他带的东西，路边长的才是路边长的——这是四处来源里唯一按**种类**而不是按数量区分的
一处。

「一无所获」因此只存在于旧存档的事件里：现在任何一次回家都至少有一只钱包。

#### 晨间事件挂在 worker 的 tick 上，靠本地日去重

`Cultivation::daily_event(now, day)` 由 worker 每 tick 调用，`daily_event_day` 记住已经
开过哪一天，所以「开机第一次」和「跨 0 点」是同一段代码：留在跑跨夜时由第一次 tick
开新日，中午启动的机器由第一次 tick 补开当天。重复调用返回 `None`，调用方不需要记任何
状态。

三种结果，一次掷定：

| 结果 | 概率 | 内容 |
| --- | --- | --- |
| 遇袭 | 280‰ | 与游荡者一战（同一套战斗引擎与战报弹窗），赢了照发奖励 + 遭遇战钱包 |
| 直接获得 | 120‰ | 灵石 6~18 / 种子 / 装备 三选一 |
| 无事 | 600‰ | 拾得 1~3 枚灵石 |

**直接获得 120‰ < 遇袭 280‰**，这是需求原话的落点，写成了 `const` 断言
（`DAILY_GIFT_PERMILLE < DAILY_AMBUSH_PERMILLE`）。理由不只是合规：直接获得是整条
阶梯里唯一不花力气就到手的东西，和战斗一样频繁的话，战斗就成了爱好、晨间事件就成了
老虎机。「无事」也不是空手：一两枚灵石让那条日志值得一看，但远不到能养活一个存档。

晨间事件**只增不减**：三种结果都不拿东西，输了也只是没赢。

#### 展示分工：日志记全部，弹窗只给战斗

三种结果都写进 `EventDetail::Daily`，所以页面的「最近事件」永远说得出今天发生了什么。
战报弹窗只给遇袭——礼物和拾遗各是一行字，为一行字弹一个需要手动关掉的窗口是打扰。

弹窗由 `poll_pomodoro` 看 `daily_events` 计数打开（该计数只在内存里、随 worker 重启
归零，视图侧对归零做了基线回退），并且只在番茄钟/修仙页可见时才会被看到：在别的设置页
时事件照样发生、照样落盘，回放等到用户回到修仙页才补上。

### 后果

- 灵石收入从「专注 + 探索的三成」扩展到每一处：按每天两次静走、五次试炼全胜、一次破境
  粗算，白天的灵石收入与专注收入同量级，但四只钱包的上限都刻意压着，商店仍然是长线目标
  而不是几次点击就能清空。
- `ExplorationOutcome::Nothing` 在新存档里不再可达，但枚举成员保留：旧存档与旧事件
  里它就是「那次真的什么都没有」，删掉会让旧数据读不出来。
- `EventDetail::Breakthrough` 补 `stones`、`EventDetail::Trial` 补 `stones`（都有
  `#[serde(default)]`），破境的事件推送从「先记事件再结算」改成「先结算再记事件」，
  于是一行事件能同时说出境界与钱包，而不是两行半句话。
- 已习得功法面板改为「最近修习的 3 部 + 展开全部」：`CultivationDocument.recent_arts`
  是一个 8 格的环，`train_selected` 每次尝试（含失败）把该 id 挪到最前，视图按环排序后
  再截前 3 部。没有这个环就只能按「什么时候买到这本书」排序，而购买顺序回答不了
  「我最近在练什么」。

### ADR-0097 · 灵石按难度计价——钱包随境界翻倍，战斗高于非战斗

**状态：** 已接受（2026-10-21）

### 决策

#### 一只钱包 = 一阶的区间 + 境界倍数

`StonePurse { min, max }` 存的是**第一境**的区间，实付 = 区间 × tier，tier 是这次支付
发生的境界（一趟探索的所在境界、一个守卫所守的境界、一层试炼所属的境界、一次晨间的
当前境界）。`tier²` 会把高境养肥（十境是百倍，而价格只有十七倍），`tier` 线性则让灵石
与货架的相对价值在整条阶梯上大致不变，所以选线性。

| 来源 | 一阶 | 十阶 | 性质 |
| --- | --- | --- | --- |
| 静走探索 | 6~10 | 60~100 | 非战斗 |
| 试炼通关 | 12~18 | 120~180 | 战斗 |
| 遭遇战 / 晨间遇袭 | 20~34 | 200~340 | 战斗 |
| 破境胜利 | 38~66 | 380~660 | 战斗 |
| 午间礼物（晨间事件的直接获得） | 25~50 | 250~500 | 事件 |
| 无事拾遗 | 3~6 | 30~60 | 事件 |
| 番茄钟 | 16 | 160 | 非战斗（按境界乘） |

顺序是设计本身：**静走 < 试炼 < 遭遇战 < 破境**，三条 `const` 断言把它钉成编译期
不变量（`WALK.max < TRIAL.min`、`TRIAL.max < WANDERER.min`、`WANDERER.max < GUARDIAN.min`）。
试炼是战斗，所以它的单次钱包必须高于静走；它比遭遇战低，因为一层只是同级的对手。

#### 番茄钟的「工资」也随境界

`SESSION_EARNINGS` 从 12 提到 16（一阶），并按 `realm.index()+1` 乘。理由与成长值同源：
成长值本身就按境界序号放大，一个金丹的下午本来就比炼气值钱；灵石若保持固定，专注这条
最稳定的产出会在后期归零。商店不卖成长，所以「专注同时给成长与灵石」仍然安全。

#### 一阶日收入对照

按一天 4 个番茄钟、2 趟静走、5 次试炼（假定 3 胜）、1 次晨间事件粗算：

| | 改前 | 改后 |
| --- | --- | --- |
| 番茄钟 | 48 | 64 |
| 静走 | 11 | 16 |
| 试炼 | 20 | 45 |
| 晨间 | 0~3 | 7~14 |
| 合计 | 约 80 | 约 140 |

一阶黄阶功法 302 灵石：改前约 3.8 天，改后约 2.2 天；十阶神阶功法 3137：改前约 40 天
（固定收入），改后约 1.6 天（收入 1600/天）——后期不再是纯磨。战斗单次最高（破境
38~66 × 境界），静走最低但仍是一只可读的钱包。

### 后果

- `StoneRoll { permille, min, max }` 被 `StonePurse { min, max }` 取代：所有来源现在都
  100% 支付（ADR-0096 已经这么定过），概率字段没有存在意义了。`roll_stones` 返回 `u32`
  而不是 `Option<u32>`，调用点的 `if let Some` 一并收敛。
- `session_earnings()` 改为 `session_earnings(realm)`；`SESSION_EARNINGS` 的含义变成
  「第一境的工资」。
- ADR-0096 的钱包表被本文修定，其余结论（晨间事件概率、直接获得 < 遇袭）不变。
- 后期灵石不再稀缺，商店的长期价值从「攒钱」转向「选择买哪件」；如果后续发现通胀，
  最省事的闸门是 `StonePurse` 的倍数（改成 `tier` 的 3/4）而不是逐源调参。

### ADR-0098 · 仙草园十五阶——种植成功率与升级

**状态：** 已定稿并落地（2026-10-27，**v3 修订**）

> **v3 修订说明**：两项开放项已拍板——品质罚法采用**阶偏移**（§2.4），灵石采用**中档 15715**
> （§2.10）。实现已进 `crates/bongocat-pomodoro/src/cultivation/garden.rs`（域函数）以及三个文档
> 字段、两个事件、一条命令、两语文案，并由单测把 15×10 矩阵与两个校准锚点逐格钉死。
>
> **v2 修订说明**（保留备查）：初版把种子的「品阶」误当成了品质（黄玄地天神），坐标错位，整套
> 数值作废。正确坐标见 §2.1：种子的**品阶是 1~10 阶**（`SeedStack.tier`），**品质是黄玄地天神**
> （`SeedGrade`）。

### 1. 背景

现有 `garden.rs` 里，「种下去」是无条件的：有空地块 + 有种子 = 一定有收成（`CropRecord::planted`
从不掷骰）。花园只有一个变量——等多久。需求要补的另一半，本轮明确了三条：

1. **品阶一一对比**：种子品阶 1~10，仙草园品阶 1~15，两者直接对比。相同品阶 → 基础 80%，
   园高于种子 → 增长，园低于种子 → 递减。
2. **品质在品阶的基础上做降低**：同一品阶下，品质越高成功率越低。校准锚点：
   **园 15 阶种 10 阶神级种子 → 90% 以上**，其余按此标准缩减。
3. **升级所需灵石要增加**（初版给的 3577 太少）。

### 2. 决策

#### 2.1 坐标：两个名字，两件事

| 名字 | 取值范围 | 现有字段 | 决定什么 |
| --- | --- | --- | --- |
| **种子品阶** | 1 ~ 10 | `SeedStack.tier` | 成熟时间（8h→10d）、收获点数基数、**成功率的一半坐标** |
| **品质** | 黄 / 玄 / 地 / 天 / 神 | `SeedGrade` | 收获点数倍率（250/600/1000/1600/2500‰）、**折算成 0 / 0.6 / 1.3 / 1.9 / 2.5 阶的深度** |
| **园阶** | 1 ~ 15 | 新增 `garden_rank` | **成功率的一半坐标**、升级门槛 |

分工结论：**园阶、种子品阶、品质三者合成一个数——「深度」——再走一条曲线得出成功率**（§2.4）；
**经验 = 收获点数**（品阶与品质都在里面）。园阶不进产出，种子品阶除了走既有曲线之外不产生别的——
每条规则只有一个读音。

#### 2.2 阶差 Δ：园阶与种子品阶的一一对比

```
Δ = 园阶 G − 种子品阶 T          G ∈ [1,15]，T ∈ [1,10]  ⇒  Δ ∈ [−9, +14]
```

这就是需求要的一一对比：**同阶（Δ=0）= 80% 基准，园高一阶 Δ+1，园低一阶 Δ−1**。
15 阶对 10 阶，满配时 Δ 最大到 +5（15 − 10），那正是 §2.3 的校准锚点所在。

#### 2.3 用 0.1 阶做步长：一条曲线、一个坐标

品质在 v2 里是从成功率的结果上减掉的（−25‰ 一档），在 v3 里它**并入横坐标**。于是已经没有
「裸成功率」这个中间量了，剩下的只有一条曲线；这一节说的是它自己的两条腿。

```rust
/// depth ≥ 0：正区，每 0.1 阶 +4‰（即每整阶 +40‰），封 1000‰
/// depth < 0：负区，每 0.1 阶 ×0.984，地板 80‰
const UP_PERMILLE_PER_TENTH: i32 = 4;
const DOWN_PERMILLE_PER_TENTH: i32 = 984;
```

| depth（单位 0.1 阶） | −90 | −70 | −50 | −30 | −20 | −10 | 0 | +10 | +25 | +50 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 成功率 | 18.8% | 25.9% | 35.7% | 49.3% | 57.9% | 68.1% | **80.0%** | 84.0% | 90.0% | 100.0% |

**负区为什么用 0.984 而不是 `0.85^(1/10)`**：前者走十步是 0.8503，与「每整阶 ×0.85」相差不到
1‰，换来的是**全程整数**——曲线上任何一点都是同一个 `odds = (odds * 984 + 500) / 1_000` 循环算
出来的。这条曲线同时被两个地方读取：页面上的百分比，和单测里逐格比对的表。浮点会让两者
在某一格相差 1‰，而 1‰ 的差在测试里就是一次随机失败。

**正区为什么是每整阶 +40‰ 而不是等比**：可心算。80 → 84 → 88 → 92 → 96 → 100 是玩家在按下
「种植」之前能在心里走完的路；等比会把第四阶写成 93.4% 这种没法念出来的数。

**那 Δ ≥ +5 一律 1000‰ 还成立吗**：成立，理由换了。v2 里它承载着 90% 锚点（那样做是被逼的，
见 §2.5）；v3 里锚点移到了 +2.5 阶，这条线现在承载的是另一个承诺——**园阶高出种子五阶以上时，
种植不再失败**。15 阶园对 1~9 阶种子必成，这是对「碾压」的预期，也让满阶之后玩家可以停止计算。

#### 2.4 品质折算：把「这株草有多金贵」读成深度

品质不再从成功率里减点数，而是**折算成额外深度**，与品阶差一起进同一条曲线：

```rust
/// 品质向园子索要的东西，单位是 0.1 阶：黄 / 玄 / 地 / 天 / 神。
const GRADE_STEP_TENTHS: [i32; 5] = [0, 6, 13, 19, 25];

pub fn plant_permille(rank: u8, tier: u8, grade: SeedGrade) -> u32 {
    let depth = (i32::from(rank) - i32::from(tier.clamp(1, MAX_SEED_TIER))) * 10
        - GRADE_STEP_TENTHS[grade.index()];
    if depth >= 0 {
        (800 + 4 * depth).min(1_000) as u32
    } else {
        let mut odds = 800;
        for _ in 0..-depth {
            odds = (odds * 984 + 500) / 1_000;
        }
        odds.max(80) as u32
    }
}
```

一句话的语义：**一株神阶种子和一株同阶黄阶种子是同一种草，只是它长了两个半阶那么深**，所以园子
得先把那两个半阶的本事长出来。

| Δ（园阶 − 种子品阶） | 黄 | 玄 | 地 | 天 | 神 |
| --- | --- | --- | --- | --- | --- |
| +5（满配） | 100.0% | 97.6% | 94.8% | 92.4% | **90.0%** ← 校准锚点 |
| +4 | 96.0% | 93.6% | 90.8% | 88.4% | 86.0% |
| +3 | 92.0% | 89.6% | 86.8% | 84.4% | 82.0% |
| +2 | 88.0% | 85.6% | 82.8% | 80.4% | 73.8% |
| +1 | 84.0% | 81.6% | 76.2% | 69.2% | 62.8% |
| **0（同品阶）** | **80.0%** | 72.6% | 64.8% | 58.8% | **53.4%** |
| −1 | 68.1% | 61.8% | 55.2% | 50.1% | 45.4% |
| −2 | 57.9% | 52.5% | 46.9% | 42.6% | 38.7% |
| −3 | 49.3% | 44.7% | 39.9% | 36.3% | 32.9% |
| −5 | 35.7% | 32.4% | 28.9% | 26.3% | 23.9% |
| −9（最低点） | 18.8% | 17.0% | 15.1% | 13.9% | 12.7% |

两个读数要顺着说：

1. **品质跨度现在有 26.6 个百分点**（同品阶时 80.0% → 53.4%），是固定扣减的 2.7 倍。
   「神阶种子是高攀」这句话这才有了牙齿：一颗神阶种子到手时的第一反应不再是「种」，而是
   「我配不配种」。
2. **Δ=+2 那一行的神阶有一个拐点**（73.8%，比 Δ=0 的天阶还低）。不是 bug，是这本书的语法：
   神阶要 2.5 阶，所以在 Δ=+2 处它其实还站在负区（2 − 2.5 = −0.5）。每条品质曲线有自己的零点，
   神阶要到 Δ=+3 才真正翻身。代价之一，§2.5 里说。

#### 2.5 为什么最后是阶偏移，代价是什么

先说固定扣减为什么走到头：

```
base(+5) ≤ 1000‰                        （成功率的上限）
神(+5)   = base(+5) − penalty(神) ≥ 900‰ （需求给的锚点）
⇒ penalty(神) ≤ 100‰ = 10 个百分点
```

只要「Δ=0 是 80%」和「满配神级 ≥ 90%」同时成立，扣减的总预算就是 10pp，没有第二个解。

阶偏移把这个死锁拆掉了：**神阶在满配点的横坐标不再是 +5，而是 +5 − 2.5 = +2.5**。

```
plant(+2.5 阶) = 800 + 4 × 25 = 900‰        （离上限还有 100‰ 的余量）
品质罚现在是「深度」而不是「成功率」，不再受成功率上限约束 ⇒ 想要多大的跨度都给得出
```

三个代价，都已确认可以接受：

| 代价 | 实际影响 | 为什么接受 |
| --- | --- | --- |
| 「相同品阶 80%」严格来说只对黄阶成立；神阶同阶是 53.4% | 需求原话里「基础 80%」的含义要收窄 | 80% 仍然是**地上最低一档**的坐标锚；品质的存在感正是要用这个斜坡换来，26.6pp 的相对跨度值这个价 |
| 品质不再是「横向减 25‰」，坐标变成 0.1 阶 | 页面和文档不能再靠心算打折 | 涉及到的只有一张表和一行五个常量 |
| 神阶在 Δ=+2 处出现拐点（低于 Δ=0 的天阶） | 曲线不是单一视觉斜率 | 它是这套模型的语法：每条品质曲线有自己的零点。愿意看的人看得懂，不愿看的人只读自己那一格的百分比 |

>（上一行的补充：代价 2 的实现成本实际为零——品质只在上表里出现一次，实现里是
> `GRADE_STEP_TENTHS[grade.index()]`，加一个品质不用动任何其它数字。）

**这不是两害相权**：固定扣减被算术排除了可能性（它给不出超过 10pp 的跨度），剩下的问题只是
「要不要为此把 `0.1 阶` 引进来」。引进来的成本是三个整数常量，收益是把「品质」从形容词变成
名词——它现在是同一个坐标系里的东西，而不是乘到最后的一个系数。

#### 2.6 完整矩阵：15 园阶 × 10 种子品阶（黄阶基准）

| 园阶＼种子品阶 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 80% | 68% | 58% | 49% | 42% | 36% | 30% | 26% | 22% | 19% |
| 2 | 84% | 80% | 68% | 58% | 49% | 42% | 36% | 30% | 26% | 22% |
| 3 | 88% | 84% | 80% | 68% | 58% | 49% | 42% | 36% | 30% | 26% |
| 4 | 92% | 88% | 84% | 80% | 68% | 58% | 49% | 42% | 36% | 30% |
| 5 | 96% | 92% | 88% | 84% | 80% | 68% | 58% | 49% | 42% | 36% |
| 6 | 100% | 96% | 92% | 88% | 84% | 80% | 68% | 58% | 49% | 42% |
| 7 | 100% | 100% | 96% | 92% | 88% | 84% | 80% | 68% | 58% | 49% |
| 8 | 100% | 100% | 100% | 96% | 92% | 88% | 84% | 80% | 68% | 58% |
| 9 | 100% | 100% | 100% | 100% | 96% | 92% | 88% | 84% | 80% | 68% |
| 10 | 100% | 100% | 100% | 100% | 100% | 96% | 92% | 88% | 84% | 80% |
| 11 | 100% | 100% | 100% | 100% | 100% | 100% | 96% | 92% | 88% | 84% |
| 12 | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 96% | 92% | 88% |
| 13 | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 96% | 92% |
| 14 | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 96% |
| 15 | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% | 100% |

黄阶是唯一「零偏移」的品质，所以这张表同时也是 §2.3 曲线的展开。对角线上的 80% 是「一一对比」
的锚；每一行往右走（种子比园深）就是负区，往左走到 T≤G−5 就是必成。

**其它品质怎么读这张表**：把**园阶往下挪** 0.6 / 1.3 / 1.9 / 2.5 阶。例如 15 阶园种 10 阶神阶
种子 → 读第 12.5 行的第 10 列 → 落在 90%~94% 之间（精确值 90.0%，因为位移是连续的）；而 1 阶园
种同一株神阶种子 → 只有 12.7%。品质不能横向减了，但它可以横向挪 —— 一句更准确的说法是：
**品质把整张表往右推**。

#### 2.7 判定时机：种下即掷，收获不再掷

需求没写这条，但它决定这套机制能不能活：

> **成功率在「种下」的那一刻结算，一次掷完。成长期只负责走路，苗长大了必然能收。**

成熟时判定是灾难：第十阶种子要等 **10 天**，10 天后才被告知失败，等于罚掉 10 天和一个地块，
熟手会直接弃掉这条机制不碰。

失败结算：

| 项 | 处置 |
| --- | --- |
| 种子 | **消耗**（不返还）。失败必须有代价，否则成功率是摆设 |
| 地块 | **立即空出**，不再占用时间，枯萎是一瞬间的事 |
| 收获 | 无 |
| 经验 | **给 `(XP + 4) / 5`，至少 1**——「枯苗亦有心得」，避免纯负反馈吓退玩家 |
| 日志 | 新增 `EventDetail::PlantFailed` |

#### 2.8 保底：只吸收方差，不改变期望

> **2026-10-30 已移除**。原实现用全局 `plant_fail_streak` 计数，导致一个种子种类的连败
> 让**所有**种子（包括低园阶下本应高失败率的高品阶/高阶级种子）统一显示并触发「必定成功」，
> 与「低等级花园种高品阶应高失败率」的直觉相悖。已改为完全按 `plant_permille` 真实成功率
> 判定，不再有保底；本节所述保底机制不再生效。

```rust
const PITY_MIN_PERMILLE: u32 = 500;   // 只有 ≥50% 的种植才进保底序列
const PITY_STREAK: u32 = 2;           // 连败 2 次后，下一次必成
```

全局一个 `plant_fail_streak` 计数器（不属地块，避免换地块规避），成功清零。**本次成功率 < 50%
时不计入连败、也不享受保底**——低园阶硬种高品阶种子就是纯赌博，这是它定价的一部分。

对长期期望的实测影响：Δ=+1 处 +0.1pp、Δ=0 处 +0.6pp、Δ=−1 处 +2.3pp、Δ=−2 处 +4.7pp，
Δ≤−3 处 **+0.0pp**。它把「99% 还连败」这种手感最差的区域磨平，但完全不动期望曲线。

#### 2.9 经验：直接复用 `harvest_points`

需求要「依据品质和品阶结算经验」。仓库里已有完全满足这个描述的函数，不必新造：

```rust
pub fn harvest_points(tier: u8, grade: SeedGrade) -> i32   // 品阶定时长，品质定倍率
```

| 品阶＼品质 | 黄 | 玄 | 地 | 天 | 神 |
| --- | --- | --- | --- | --- | --- |
| 1 | 1 | 2 | 3 | 5 | 8 |
| 2 | 1 | 3 | 5 | 8 | 12 |
| 3 | 2 | 4 | 7 | 12 | 18 |
| 4 | 2 | 6 | 10 | 15 | 24 |
| 5 | 4 | 9 | 14 | 23 | 36 |
| 6 | 5 | 12 | 19 | 31 | 48 |
| 7 | 7 | 17 | 29 | 46 | 72 |
| 8 | 10 | 23 | 38 | 61 | 96 |
| 9 | 14 | 35 | 58 | 92 | 144 |
| 10 | 24 | 58 | 96 | 154 | 240 |

品阶跨 10 档是 32 倍，品质跨 5 档是 10 倍。早期捡到一颗神级种子的兴奋感，一半来自这张表。

#### 2.10 升级门槛：经验不变，灵石大幅提高

```rust
pub const MAX_GARDEN_LEVEL: u8 = 15;

/// 由第 g 阶升到 g+1 阶所需经验：三版以来一直没动。
pub const fn rank_xp(g: u8) -> i64 { 5 * g² }
/// 累计：5(n−1)n(2n−1)/6，满阶 5075

/// 由第 g 阶升到 g+1 阶所需灵石：中档。
pub const fn rank_cost(g: u8) -> u32 { 50 + 30(g−1) + 15(g−1)² }   // 满阶 15715

/// 文档落地时的字段名：`garden_rank` / `garden_xp`（不是 `garden_level`——Level 这个词
/// 在这份仓库里已经指道行等级了，混用会在某次读代码时骗到一个人的半小时）。
```

| 目标园阶 | 本阶需 XP | 累计 XP | 本阶灵石 | 累计灵石 | 卖掉收成占比 |
| --- | --- | --- | --- | --- | --- |
| 2 | 5 | 5 | 50 | 50 | 71% |
| 3 | 20 | 25 | 95 | 145 | 41% |
| 4 | 45 | 70 | 170 | 315 | 32% |
| 5 | 80 | 150 | 275 | 590 | 28% |
| 6 | 125 | 275 | 410 | 1000 | 26% |
| 7 | 180 | 455 | 575 | 1575 | 25% |
| 8 | 245 | 700 | 770 | 2345 | 24% |
| 9 | 320 | 1020 | 995 | 3340 | 23% |
| 10 | 405 | 1425 | 1250 | 4590 | 23% |
| 11 | 500 | 1925 | 1535 | 6125 | 23% |
| 12 | 605 | 2530 | 1850 | 7975 | 23% |
| 13 | 720 | 3250 | 2195 | 10170 | 22% |
| 14 | 845 | 4095 | 2570 | 12740 | 22% |
| 15 | 980 | 5075 | 2975 | 15715 | 22% |

**「卖掉收成占比」这一列回答了「灵石到底该给多少」**：

```
仙草售价 = 点数 × 14（既有规则），而 XP = 收获点数
⇒ 一棵草要么留下换属性与经验，要么卖掉换 14×点数 灵石
⇒ 满阶灵石 ÷（14 × 满阶经验）= 15715 ÷ 71050 = 22%
```

读法：**只要把 22% 的收成卖掉，剩下的全喂给属性，就能把园阶一路顶到满。** 旧方案的 3577 只要求
卖掉 5%，难怪太便宜——它连一个「留还是卖」的犹豫都制造不出来。22% 正好让这个取舍天天发生，
又不至于逼玩家放弃属性成长。

拍板过程里摆过的三档，**中档入选**：

| 档 | 公式 | 满阶累计灵石 | 卖掉收成占比 | 结论 |
| --- | --- | --- | --- | --- |
| 轻（v1） | `15 + 10(g−1) + 3(g−1)²` | 3577 | 5% | 弃：连一次犹豫都制造不出来 |
| **中（选中）** | `50 + 30(g−1) + 15(g−1)²` | **15715** | **22%** | 每几茬就要做一次「留还是卖」 |
| 重 | `80 + 45(g−1) + 25(g−1)²` | 25690 | 36% | 弃：占比过高会挤掉属性成长本身 |

对照参考：买满九个地块是 4650 灵石（既有 `PLOT_PRICES`）。新方案让整条园阶线是地块树的 3.4 倍，
成为前期最大的单一灵石去处——这是「灵石要有分量」的直接结果。

**瓶颈落在哪**：XP 维持 5g² 不变、灵石涨 4.4 倍，卡的通常是钱而不是经验。这是刻意的——经验的
来源是时间，灵石的来源是「要不要卖」，让经济决策当守门员比让日历当守门员有意思。若希望双门槛
并重，把 `XP_UNIT` 从 5 提到 15 即可，形状不用改。

#### 2.11 叠加计算：一次函数调用走完

```
plant(plot, seed, now):
    depth = 10 × (garden_rank − seed.tier) − GRADE_STEP_TENTHS[seed.grade]   # §2.4
    p     = depth ≥ 0 ? min(1000, 800 + 4 × depth)                           # §2.3 正区
                      : max(80, loop(-depth){ p = (p × 984 + 500) / 1000 })  # §2.3 负区
    took  = (plant_fail_streak ≥ 2 且 p ≥ 500‰) 或 rng.below(1000) < p       # §2.8
    seed.count -= 1                                                          # 种子先消耗
    if took:
        plot.crop          = CropRecord::planted(..)     # 之后必然成功
        plant_fail_streak  = 0
    else:
        plot.crop          = None                        # 地块立即空出
        plant_fail_streak += 1                           # 仅当 p ≥ 500‰
        garden_xp        += max(1, (harvest_points(tier, grade) + 4) / 5)
        emit Withered { permille: p, xp }

harvest(plot, now):                       # 成熟即成功，不再掷
    points      = harvest_points(crop.tier, crop.grade)
    attributes[crop.attribute] += points
    garden_xp  += points

upgrade_garden(now):
    require garden_rank < 15                              → GardenCapped
    require garden_xp   ≥ rank_xp_total(garden_rank + 1)  → NotEnoughRankXp
    require stones      ≥ rank_cost(garden_rank)          → NotEnoughStones
    stones -= rank_cost(garden_rank); garden_rank += 1
    emit GardenRank { rank, stones }
```

**规则之间的叠加方式**（这是需求点名的部分）：

| 关系 | 叠加形态 | 说明 |
| --- | --- | --- |
| 园阶 ↑ | 同一种子：**每深一阶 ⇒ +40‰**（到超出五阶封 1000‰） | 唯一的正向控制杆 |
| 种子品阶 ↑ | 同一园阶：**深度进负区 ⇒ 每 0.1 阶 ×0.984** | 与园阶共用同一个「深度」，不是独立乘数 |
| 品质 ↑ | **折算成 +0.6 / 1.3 / 1.9 / 2.5 阶的深度**，进同一条曲线 | 与品阶差同质：三者合成一个数，不叠 multiplier |
| 种子品阶 ↑ 对产出 | 时长 8h→10d ⇒ 点数 ×32 | 走既有 `harvest_points`，不另设 |
| 品质 ↑ 对产出 | 点数 ×10（250‰→2500‰） | 同一张表，另一列 |
| 保底 | 分段修正，`p ≥ 500‰` 才生效 | 削方差，不削期望 |

#### 2.12 自洽性检验：低园阶不该「无脑硬种高阶种子」，高园阶不该有大一统最优

| 园阶 | T1 | T2 | T4 | T6 | T8 | T10 | 最优 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 5.83 | 5.52 | 3.99 | 2.75 | 1.99 | 1.45 | **T1** |
| 3 | 7.45 | 7.62 | 5.52 | 3.79 | 2.75 | 2.01 | **T2** |
| 5 | 8.17 | 8.68 | 7.62 | 5.24 | 3.79 | 2.77 | **T2** |
| 8 | 9.00 | 9.88 | 9.08 | 7.87 | 6.16 | 4.50 | **T2** |
| 10 | 9.00 | 10.00 | 9.88 | 8.63 | 7.87 | 6.22 | **T2** |
| 15 | 9.00 | 10.00 | 10.00 | 9.50 | 9.50 | 9.10 | **T2** |

尺子是**地块时间效率**——「成功率 × 收获点数 ÷ 成熟天数」（地阶基准，既有锚 1000‰）。只算绝对期望
会误判成「无脑种最高阶」，因为 T10 一棵 96 点但要占地块十天。

四条结论：

1. **越级必亏，而且亏得不轻**：1 阶园种 10 阶种子只得 1.45 点/天，种 1 阶是 5.83 点/天，差 4 倍。
   这条对所有品质都成立（黄差 5.3 倍、神差 4.2 倍），所以「拿到稀有种子就硬种」不是套路。
2. **最优种子跟着园阶走**：1 阶园最优 T1，3~10 阶园最优 T2，满阶才轮到 T2/T4/T6 并列。刚长上来的
   一阶修为立刻改变该种什么，这是这套系统唯一真正随玩家进度移动的刻度。
3. **高阶种子赚的是绝对点数，不是效率**：T10 一棵顶三十棵 T1，但要占十天地块。稀缺种子的正确用法
   是「等地块闲下来、园阶也够高时，才值得为它占十天」，而不是「拿到就种」。
4. **品质维度的差距是清晰的、且是单向的**：同品阶下神阶的效率是黄阶的 5~8 倍（因为产出 ×2.5 远大于
   成功率 ÷1.5）。这不该被误读为「神阶统治」——神阶掉率万分之一，稀缺性本身就是它的限制；而且这个
   差距在任何园阶都成立，玩家不需要为此做额外决策。

>| 补充：第 4 条是我最想让人先看的数字。**「有神阶种子就种神阶」是正确且有意为之的**——这套
>系统要制造的抉择不在「种哪棵」而在「要不要现在种」。

### 3. 为什么好玩

1. **每一次种植现在都回答一个问题**：「我的园配不配这株？」——这个问题第一次让稀有种子变成
   **事件**而不是**资源**。
2. **同阶 80%、高一阶 84%、低一阶 68%**：数字是整数、可心算，玩家在按下种植之前就能预判结果。
   预期感来自看得懂的规则，而不是藏在黑箱里的权重。
3. **每天一个真实的经济决策**：22% 的「留还是卖」服务于一条自洽的循环（更强的园 → 更好的种子
   → 更强的修为），而不是凭空长出来的消耗项。
4. **失败立即可见**：种下即掷，损失一颗种子，不损失十天。负反馈短、脆、可复盘。

### 4. 影响面

- `garden.rs`：新增 `MAX_GARDEN_RANK`、`GRADE_STEP_TENTHS`、`UP_/DOWN_PERMILLE_PER_TENTH`、
  `PLANT_FLOOR_PERMILLE`、`PITY_MIN_PERMILLE`、`PITY_STREAK`、域函数 `plant_permille`、`rank_xp`、
  `rank_xp_total`、`rank_cost`、`rank_for_xp`、`wither_points`。**成活率与门槛全部算在这里，UI 只负责把数字说出来**。
- `document.rs`：`CultivationDocument` 增 `garden_rank: u8`、`garden_xp: i64`、
  `plant_fail_streak: u32`，全部 `#[serde(default)]`；`normalized()` 加一次性迁移（§6）。
- `events.rs`：增 `EventDetail::Withered { attribute, tier, grade, name, permille, xp }` 与
  `EventDetail::GardenRank { rank, stones }`。
- `trade.rs`：`TradeRefusal` 增 `GardenCapped` 与 `NotEnoughRankXp`，各带一条 i18n key。
- `state.rs`（cultivation）：`plant` 的返回值从 `bool` 改为 `Option<PlantOutcome>`；新增
  `planting_is_certain` 与 `upgrade_garden`；`CultivationView` 增五个派生字段（`garden_rank`、
  `garden_xp`、`garden_xp_needed`、`garden_upgrade_cost`、`garden_pity_ready`）。
- `state.rs`（pomodoro）：新增 `PomodoroCommand::UpgradeGarden`，拒绝走既有 `Refusal → toast`。
- UI：花园面板顶部加园阶行（等级 / 造化进度 / 进阶按钮含灵石价）；**每颗种子名字下方直接打印本次
  成活率**，保底触发时改说「此番必成」。
- i18n：新增词条 zh-CN + en-US 各 12 条（9 条 garden + 2 条 event + 1 条复用）。
- 测试：`garden.rs` 单测 6 个（两个锚点、15×10 矩阵逐格比对、灵石占收成比重的推算、由经历反查园阶、枯苗心得）；
  `tests.rs` 行为测试 4 个（失败结算、保底、双门槛升级、旧档迁移）。

### 5. 与既有系统的关系

| 现有 | 是否改动 | 说明 |
| --- | --- | --- |
| `SeedStack.tier` | **不改** | 直接当种子品阶用 |
| `SeedGrade` / `multiplier_permille` | **不改** | 品质扣减是新增的独立项，产出倍率不动 |
| `harvest_points` / `GROW_SECONDS` | **不改** | 经验直接复用 |
| `roll_seed_grade` 掉率 | **不改** | 稀缺度不变，新的取舍发生在「要不要种」而非「能不能拿到」 |
| `PLOT_PRICES` | **不改** | 4650，与新灵石线是并列的 sink |
| 仙草售价 = 点数 × 14 | **不改** | 它是「22% 卖掉收成」这条论证的地基 |

新增 sink 15715 灵石。faucet 不动，整体偏回收；但种植本身就是 faucet（卖草），这条线自带血流
循环，不会吸干装备和功法的预算。

### 6. 兼容性（AGENTS §3.1）

新字段一律 `#[serde(default)]`，旧档读到 **1 阶 / 0 经验 / 0 连败**。

但 1 阶会让老玩家体感倒退（原本必成，现在 10 阶种子只剩 18.5%）。推荐用现成的 `harvests: u32`
做一次性迁移：**字段缺失时 `garden_xp = harvests × 3`**（3 ≈ 一次平均收割的地阶一阶产出），
再按累计表反查园阶。100 次收割的老存档直接落到 6 阶。

落地时必须补一条「旧档 → 新代码」读取回归测试，并同步 JSON schema（`just schema`）。

### 7. 采纳门槛

| 字段 | 内容 |
| --- | --- |
| `decision` | 成功率是**一条曲线上的一个点**：`depth = 10 × (园阶 − 种子品阶) − 品质折算的深度`（0/6/13/19/25 十分之一阶）；`depth ≥ 0 → min(1000, 800 + 4·depth)`，`depth < 0 →` 每十分之一阶 `×0.984`，地板 80‰。两个锚点：**品阶相同且为黄阶 = 800‰**、**满配（15 阶园、10 阶神级）= 900‰**。**种下即掷、成熟必收**；失败只损种子、地块立即空出、返 1/5 经验；保底仅在 `p ≥ 500‰` 时累计、连败 2 次后必成；经验复用 `harvest_points`；XP 门槛 `5g²`（5075）不动，灵石中档 `50+30(g−1)+15(g−1)²`（15715） |
| `evidence_level` | **L1+** —— v3 的矩阵、品质表、门槛表已**不是脚本的输出而是代码本身**，并由 10 个新增单测逐格钉死（`just check` 全绿：fmt / clippy×3 / workspace tests / release check）；剩余的不确定性不在数值而在手感，**仍未在任何真实存档或真实游玩中见过**，不能据此声称「好玩」 |
| `primary_bottleneck` | 不是数值问题，是**既有花园缺乏纵深感**：只有一个变量（等待时间），任何决策都不成立。先用 §2.12 证明「越级必亏、满阶趋平」，数值才有意义 |
| `released_variables` | 本轮实际改动 2 个：①品质罚法（固定扣减 `[0,25,50,75,100]‰` → 阶偏移 `[0,6,13,19,25]` 十分之一阶）；②灵石二次系数（15/10/3 → 50/30/15，即 3577 → 15715）。其余 `held_constant`：`GROW_SECONDS`、`multiplier_permille`、`roll_seed_grade`、`PLOT_PRICES`、仙草售价 ×14、`SEED_RETURN_*`、`FREE_PLOT_COUNT` 一律不动；XP 门槛也刻意没动，以便把瓶颈单独隔离在灵石上 |
| `success_criteria` | 已达成：(a) **15×10 矩阵逐格等于 §2.6**（`the_fifteen_ranks_against_the_ten_tiers`），800‰ 与 900‰ 双锚钉死；(b) 旧档按 §6 落到预期园阶且与 `harvests` 单调对应（`an_old_save_keeps_the_garden_it_earned`）；(c) 失败结算「损种子、空地块、返造化、写日志」四条同时成立（`a_planting_that_does_not_take_...`）；(d) 保底的双门槛行为有正反两例。待实跑：§2.12 的最优种子是否与实际种植习惯一致 |
| `rollback_trigger` | 任一：①出现统治性最优解（同一品质内，某园阶下单一品阶的时间效率领先次优 >15%）；②实跑中玩家因失败率而只种最低品阶（参与度下降）；③灵石回收过强，表现为灵石存量持续下降且装备/功法购买量趋零。**回滚的最省路径**：把 `GRADE_STEP_TENTHS` 改回 `[0,0,0,0,0]` 即回到「品质不影响成活」，其余数值无需改动 |

### 8. 开放项

- **A. 品质罚法** —— **已拍：阶偏移**（§2.4）。固定扣减被算术排除（见 §2.5）。
- **B. 灵石档位** —— **已拍：中档 15715**（卖掉 22% 收成）。见 §2.10。
- **C. 满配是否该 100%** —— 仍开放。当前 `depth ≥ +50`（即高出五阶）一律 1000‰，所以 15 阶园种
  1~9 阶种子必成。若希望终局仍留一丝不确定性，把 `plant_permille` 的上限从 1000 降到 990 即可；
  代价是满配神级跟着降到 89%，不再满足 ≥90% 锚点——所以真要动这条，锚点也得一起改。
