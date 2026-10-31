# 输入、交互与 overlay

> 本文件由 `docs/adr/` 下 **25 个 ADR** 合并而成（2026-10-31）。
> 每个 ADR 保留原始编号（`ADR-XXXX`）与完整正文，可继续按编号引用；原始单文件已归档到 `docs/adr/archive/`，仅作考古与回滚，勿直接编辑。

---
<a id="adr-0012"></a>
# ADR-0012: Ordered Motion Audio Service

状态：Accepted
日期：2026-08-31

> 后续修订（2026-10-12）：`rodio` 由 `=0.22.2` 升级到 crates.io 精确 pin `=0.23.0`。
> 该 release 把 `cpal` 由 `0.17.3` 升到 `0.18.2`，并把此前分散的 cpal 错误类型
> （`StreamError`、`BuildStreamError`、`PlayStreamError`、`DevicesError`、
> `DefaultStreamConfigError`、`SupportedStreamConfigsError`）统一为 `cpal::Error`，以
> `CpalError`/`CpalErrorKind` 重导出，`rodio::DevicesError` 被移除。项目只使用 `Decoder`、
> `SamplesBuffer`、`DeviceSinkBuilder`、`Player` 与 `rodio::cpal::BufferSize`，不匹配这些
> 错误类型，源码零改动。仍然只启用 `playback` 与 `flac`，不改变本 ADR 的替换边界。

## Context

预置 model3 的 motion 通过相对路径引用 FLAC。音效是动作被接受后的产品副作用，
不能进入 `RenderSnapshot`、阻塞 runtime/input，也不能因文件、解码或输出设备故障让
动作或画面失败。旧产品同一时刻只保留一个 motion voice；新动作、显式停止、禁用、
成功切模和退出都需要明确停止边界。

## Decision

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

## Dependency Boundary

`rodio 0.23.0` 声明 `MIT OR Apache-2.0`，Rust 1.87+，仓库在审计日仍活跃；底层 CPAL
支持 Windows/macOS 系统输出。rodio 0.23.0 把 `cpal` 升到 `0.18.2`，lockfile 因此解析为
`0.18.2`；项目不直接依赖或 fork CPAL 来伪造版本升级。

rodio/CPAL 类型不离开 `bongocat-audio` 私有 backend。替换其它音频库时只需保持
`MotionAudioClient`、强类型 command、诊断和 shutdown contract，不修改 runtime、模型
或 UI 公共协议。Linux 首发后评估时再启用对应 output target；当前 Linux contract build
不链接 ALSA，也不把 Linux 音频变成 Windows/macOS 首发阻塞。

## Consequences

音频解码成本转移到模型 activation；设备打开继续由 audio owner 惰性执行，但 runtime 不等待它。
`rodio::Player::get_pos()` 只反映 mixer 位置，不能作为扬声器起播确认，因此不再用作动画时钟。当前实现支持现有预置模型实际使用的 FLAC；
新增格式必须先形成模型兼容需求，再单独启用 decoder feature 和测试。默认设备热切换、运行中 stream
error callback、端到端音频/画面时序测量、100 次 model/audio 资源测量与 8 小时 soak 仍属于后续
稳定性验收，不由本 ADR 的单元测试替代。

---
<a id="adr-0018"></a>
# ADR-0018: Typed Model Interaction Settings

状态：已接受（2026-09-01；2026-09-25 增加键盘/手柄模型输入门禁；2026-09-27 指针跟随按轴拆分）

## 背景

BongoCat 的 配置已经包含水平镜像、镜像指针跟随和忽略指针字段，但此前
这些字段没有穿过 runtime 边界。直接让 renderer 读取配置会违反 renderer 只消费不可变
`RenderSnapshot` 的所有权规则，也会让模型求值与显示变换在不同线程拥有不一致的设置。

## 决策

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

## 验证

runtime 测试验证 settings command 的 revision、snapshot 和 shutdown 保留，分别验证键盘/手柄
输入门禁只改变模型投影而不丢失另一来源或原始 pressed state，并验证两个指针翻转开关各自只影响
所属轴：单开水平项时 X/Z 反号而 Y 不变，单开垂直项时只有 Y 反号，两项同开等于两次单开结果的组合。
app service 测试验证三个输入门禁快捷键会切换并持久化对应设置，并验证两个翻转字段经 typed command
写入配置文件后重启仍被恢复。macOS Cubism 测试验证指针镜像/忽略规则及 render snapshot 镜像
字段；overlay 变换测试验证普通与水平镜像在中心点保持相同且 X scale 符号相反。Windows 使用
相同纯 Rust 变换逻辑，其真实 GPU/DPI 矩阵仍属于平台门禁。

## 后续边界

本 ADR 不改变 physics/pose 的授权 fixture 和行为求值门禁，不声称完整 Live2D 兼容；
也不定义快捷键冲突、用户编辑或平台全局 hotkey 注册协议。

---
<a id="adr-0031"></a>
# ADR-0031: Tray Icon and Menu Library Boundary

状态：已接受（2026-09-13）；2026-09-14 补充 Windows `set_tooltip` 上游缺陷与 tooltip 创建期不变量；2026-09-25 接受托盘与 overlay 右键菜单复用（ADR-0068）

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

## 背景

Windows 的 `Shell_NotifyIcon`/`HMENU` 与 macOS 的 `NSStatusItem`/`NSMenu` 原本各自实现，约 690 行
平台代码需要分别维护隐藏窗口、菜单句柄、回调、主线程与生命周期。两边已经共享
`SystemMenuAction`、`SystemMenuError` 和 `SystemMenuPresentation`，但 native owner 的行为与
清理顺序仍容易漂移。项目需要在不改变 runtime/UI 强类型边界的前提下，减少重复平台代码并保留
Windows GUID、菜单顺序、左键打开设置和右键菜单等产品行为。overlay 右键最初复用托盘隐藏窗口，
这会让 Windows 菜单 owner 与实际窗口不一致，并绕过 macOS 的跨平台菜单 API；因此托盘与菜单的
第三方边界在本 ADR 中分别固定。

## 调研结论

已核实（2026-09-13，`cargo search`、`cargo info`、`cargo tree` 与源码阅读）：

- `tray-icon 0.25.0`（MIT OR Apache-2.0，Rust 1.90+）是 crates.io 当日最新稳定版，由 Tauri
  项目维护，同时支持 macOS `NSStatusItem` 与 Windows `Shell_NotifyIcon`，并提供图标、tooltip、
  菜单、固定 GUID 和显隐 API。当前 toolchain 为 Rust 1.97.1，满足其 MSRV。
- `muda 0.20.0`（Apache-2.0 OR MIT，Rust 1.90+）是 crates.io 当日最新稳定版，由 Tauri 项目维护；
  `Menu`/`MenuItem`/`CheckMenuItem` 提供菜单构建与动态文本、状态更新，`ContextMenu` 提供 Windows
  HWND 与 macOS `NSView` 的右键弹出 API。它没有可设置的菜单标题，因此既有
  `SystemMenuPresentation::title` 不再有对应可变标题 API。`tray-icon 0.25.0` 也依赖同一
  `muda 0.20.0` package 来挂载托盘菜单；项目直接声明 `muda`，确保共用 popup 根使用同一份第三方
  类型和同一个项目 owner，而不是依赖第三方 crate 的重导出。
- `tray-icon` 的 `TrayIcon`、`TrayIconEvent::receiver` 与 `muda::MenuEvent::receiver` 均以进程级
  状态和 `Rc<RefCell<_>>`/静态 channel 实现。一个进程只能有一个长期存活的 `SystemMenu` 业务
  owner；隐藏图标只切换平台表示，不能销毁并重建 owner，否则会丢失菜单事件消费者。
- Windows 固定 GUID 仍使用
  `123f3c6f-7d2a-4ca3-b8cb-9b1d1eaf2f10`。库以 GUID 作为托盘身份并在 Explorer
  `TaskbarCreated` 广播后恢复注册；应用不自行重试或创建平行图标。
- 两个依赖都使用 `default-features = false`：`tray-icon` 关闭 `libappindicator`，`muda` 关闭
  默认的 `gtk3` 与 `libxdo`。它们只在 macOS/Windows target 下声明，因此 Linux 不引入
  GTK/libappindicator 系统依赖。Linux 托盘仍按 ADR-0006 单独评估，不属于本决策。
- Windows PNG 资源来自维护者指定的
  `https://raw.githubusercontent.com/ayangweb/BongoCat/refs/heads/master/src-tauri/assets/tray.png`，
  已逐字节核验为 256×256、8-bit RGBA、非隔行 PNG，SHA-256 为
  `65f3a1e71f60417916c0511945131c7341edbd46d037fa9e703014e6155434dd`。macOS 现有
  `tray-macos.png` 也固定为 256×256、8-bit RGBA、非隔行，SHA-256 为
  `9342c5f8138b03e492434133041264147b759e2309f1ed3b7fab2e0e51b5a99d`（2026-10-12 重绘，
  见文首「后续修订」）。

## 决策

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

## 安全与生命周期不变量

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

## 验证

- `product_icon_contract` 覆盖两个 PNG 的容器/尺寸/RGBA 格式，build script 在编译前验证资源，
  Windows RC 只嵌入产品 ICO，不再把托盘 PNG/ICO 作为 executable icon group。
- macOS/Windows target 的 `cargo check`、release build、完整 workspace fmt/clippy/test/check 和
  native dependency policy 必须通过。
- macOS release system-menu smoke 需验证 `tray-icon` 托盘 owner、托盘与 overlay 右键共用的 `muda` popup
  根、强类型 action、显隐恢复与 shutdown；现有 smoke 注入 action 而不弹出真实菜单，因此不能替代真实菜单弹出验证。
- 发布前必须在 Windows 10 1903+ 与受支持 macOS 实机验证托盘与 overlay 右键展示同一菜单层级、子菜单展开、
  cursor 定位、缩放/DPI、窗口层级、点击外部关闭、action 派发和 shutdown；重点确认两个入口的菜单项集合、
  check 状态和 action 行为一致。Windows x64 cross-check 只能证明编译与边界，不能证明 tray registration、菜单层级、
  左键打开设置、Explorer 重启恢复或资源管理器交互。
- `workspace` 的 `Smoke Windows D3D11 product overlay` 在真实 Windows runner 上运行
  `--settings-window-open-smoke` 与 `--settings-window-smoke --models-page-smoke`，是上述
  `set_tooltip` 缺陷的唯一回归来源：2026-09-14 的 job 日志显示 `set_presentation` 每 50ms 重试一次，
  在 30 秒内累计 380 条完全相同的 `StatusItemUpdateFailed`。该 job 和其中每个 smoke step 都必须
  保持启用，不得以跳过、忽略错误或放宽断言的方式让 CI 通过。
- 当前环境未运行 Windows 实机 smoke；Windows 托盘与右键菜单行为仍是发布门禁，不得以
  cross-compile 结果宣称完成。

## 替换边界

替换点只有 `bongocat-platform` 私有 `system_menu_native` adapter 和 overlay 的
`HasWindowHandle` 实现。升级或替换 `tray-icon`/`muda` 时必须复验：两者仍解析到同一 `muda`
package、托盘与 overlay 共用 popup 根的 action 投影与层级、菜单项/根的析构顺序、Windows 固定 GUID、
GUID 注册下的 `set_tooltip` 是否已修复、左键/右键行为、macOS template image 与主线程约束、overlay
HWND/`NSView` 生命周期、cursor 定位、隐藏后的恢复、shutdown 清理、依赖 target/features、Windows shell
failure 语义和 PNG 资源格式。adapter 之外不得依赖第三方 tray/menu 类型。

---
<a id="adr-0038"></a>
# ADR-0038: 左右 Alt 的键位图命名与旧名兼容

状态：已接受（2026-09-17）
依赖：ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0036（模型导入来源识别与 staging 边界；
**该 ADR 已于 2026-09-22 撤回**）、ADR-0004（可校正输入状态）

修订（2026-09-22）：ADR-0036 撤回后模型来源只剩目录，本文「两种来源得到同一结果」「用户选中的
源目录/归档始终只读」里的归档一半不再适用；调用点也从「目录复制与归档解压之后」变成「目录复制
之后」，`commit_installed_staging` 之前的位置不变。**命名决策本身未变。**

## 背景

旧版 BongoCat 用 `rdev` 做输入。`rdev` 把两个 Alt 键分别命名为 `Alt`（左）和 `AltGr`（右），
因此按旧命名导出的模型把左右 Alt 画成了 `resources/left-keys/Alt.png` 与
`resources/left-keys/AltGr.png`，仓库内三个预置模型也是这个名字（`git log` 可见 141 张键位图
的既有布局）。

BongoCat 的输入层从 HID usage 出发，左右修饰键是可区分的 canonical 名
（`shared/behavior/input-semantics.md`：`AltLeft`/`AltRight`）。`bongocat-live2d` 因此按
`AltLeft`、`AltRight` 去查资源，而预置模型里既没有 `AltLeft.png`，`AltGr.png` 也不在任何候选
列表里，于是：

| 按键 | 修复前的候选 | 实际命中 | 结果 |
| --- | --- | --- | --- |
| 左 Alt（HID `0xe2`） | `AltLeft`、`Alt` | `Alt.png` | 正确 |
| 右 Alt（HID `0xe6`） | `AltRight`、`Alt` | `Alt.png` | **画成左 Alt 的图**，模型自己的 `AltGr.png` 不可达 |

两个真实事实决定了本 ADR 的形态：

1. **`Alt.png` 与 `AltGr.png` 是两张不同的图**（预置模型中两者 SHA-256 不同，尺寸同为
   612×354），所以右 Alt 命中 `Alt.png` 是可见错误，不是"反正一样"。
2. **BongoCatMver 的键位表不区分左右 Alt。** 上游固定 commit 的教程原图
   （`BongoCatMverUI/Resources/tutorial/img_tutorial_8.jpg`）把左右两个 Alt 都编号为 `18`，而
   C++ 侧用 `GetKeyState(pkeynotecurrent->key)` 查询该码——`VK_MENU` 对任一 Alt 都为真。旧格式
   本身没有"哪一侧"这一位信息。

## 决策

### 1. 预置模型按 canonical 名重命名

`resources/models/{standard,keyboard}/resources/left-keys/` 下：

```text
Alt.png   → AltLeft.png
AltGr.png → AltRight.png
```

改名用 `git mv`，字节不变；`shared/fixtures/model-fixtures/preset-model3-index.json` 是
`spikes/model-package` 解析器产出的冻结快照，同步更新对应 `id` 与 `file`（重跑该解析器逐字段
比对通过）。文件名顺序在 ASCII 下不变（`.` < `L`），因此 `left_keys` 数组次序无需调整。

不新增通用的 `Alt.png`：重命名后两侧各有专属图，家族图对预置模型已无意义。

### 2. 运行时优先精确名，`AltGr` 是唯一的旧名遗留

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
没有任何迁移路径可用（AGENTS.md §4.1）。

`Alt`/`Control`/`Shift`/`Meta` 家族回退保持原样；本 ADR 不改变 `ControlLeft`/`ShiftLeft` 等
既有行为。

### 3. 包导入时归一化，且在 store 自己的 staging 上做

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

### 4. Mver 的 Alt 映射：有侧信息就用侧，没有就两侧都写

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

## 明确不做

- **不新增 `Alt` 家族图的兼容 alias 以外的键名**：不引入 `LeftAlt`/`RightAlt` 之类第三套拼写。
- **不就地升级已安装模型**：本产品不做迁移；既有安装靠运行时的 `AltGr` alias 保持正确。
- **不改 `Control`/`Shift`/`Meta` 的候选顺序与家族回退**（理由见决策 4）。
- **不把归一化做成模型加载期的动态映射**：磁盘上的包应当是产品自己的词汇表，否则每个消费方都要
  再实现一遍旧名识别。

## 残余风险与待验证项（不得当作已确认）

1. **没有真实"绑定 Alt 的 Mver 模型"样本**：上游教程图标明两侧同为 `18`，但仓库内唯一真实样本
   （`bongo_cat_mver_0.1.6_64`）的键位表只有 `17`/`16`（Control/Shift）与字母、方向键，不含 Alt，
   所以 `0xA4`/`0xA5` 与 `0x12` 的展开都只有合成测试覆盖。
2. **只提供一张 `Alt.png` 的模型**：归一化后它变成 `AltLeft.png`，右 Alt 再无专属图，只能靠家族
   回退画同一张。这与旧版 `rdev` 行为不完全相同——旧版右 Alt 报 `AltGr`，模型没有 `AltGr.png`
   时**什么都不画**。选择"两侧都画同一张"而不是"右侧留空"，是因为后者在用户侧看起来是"某个键
   没反应"的新缺口，而前者只影响本来就无法区分左右的老模型。真实样本里存在这类模型
   （`Bongo Cat v0.16/BongoCat - 标准模式` 只有 `Alt.png`）。
3. **`0x12` 展开成两张图**在极端情况下会掩盖旧数据的分歧：若某个 Mver 模型真的为左右 Alt 各配了
   一个绑定（两条 `18`），转换结果只保留先出现的那张图覆盖两侧——旧实现同样只保留一张，但那时
   是共用名，现在两侧共用同一份字节。
4. **Windows 未实机验证**：本次改动全部在 macOS/aarch64 上验证；`0xe2`/`0xe6` 的实际事件来源
   （Raw Input 的 E0 扩展码路径）不在本次改动范围内，也未重新实机复核。
5. **`AltGr` alias 只在单元测试里验证**：没有在真实用户数据根里的旧模型上跑过整条链路。

## 验证

已完成（2026-09-17，本机 macOS / aarch64）：

- `bongocat-live2d` 45 测试（新增 2）。`alt_keys_resolve_their_own_image_and_keep_the_legacy_alias`
  固定三组候选与命中结果（新名、旧名、共用图）；
  `shipped_keyboard_models_draw_both_alt_keys_with_their_own_artwork` 用真实预置模型断言左 Alt 命中
  `resources/left-keys/AltLeft.png`、右 Alt 命中 `.../AltRight.png`、两者字节不同，且预置模型里
  不再存在 `Alt.png`/`AltGr.png`。
- `bongocat-model-store` 92 测试（新增 6）：`key_names` 3 个（两侧目录改名、冲突保留 canonical、缺目录/
  非 PNG 名不动）、`store` 3 个（目录来源不修改源、归档来源同样生效、两侧同存时 canonical 优先）、
  `mver` 2 个（`0x12`/`0xA4`/`0xA5` 的展开；一次绑定产出两侧同名同图且都落盘）。
- **真实社区模型验证**（新增 env 门禁用例 `imports_the_bongo_cat_sample_named_by_the_environment`，
  由 `BONGOCAT_PACKAGE_SAMPLE` 指定，未设置时直接返回）：逐个跑通本机 `~/Downloads` 与
  `~/Documents/BongoCat` 下的真实模型，目录与 `.zip` 两种来源各跑一遍：

  | 样本 | left-keys | 旧名 | 结果 |
  | --- | ---: | ---: | --- |
  | `送葬人 · 标准模式`（目录 / zip） | 56 | 2 | `Alt.png`→`AltLeft.png`、`AltGr.png`→`AltRight.png`，字节相同 |
  | `经典小键盘 · 标准模式`（目录 / zip） | 15 | 0 | 无改名，15 张图逐字节相同 |
  | `Bongo Cat v0.16/BongoCat - 标准模式` | 50 | 1 | `Alt.png`→`AltLeft.png` |

  用例同时断言：导入后 `resources/{left-,right}-keys` 的**完整文件集合与逐文件字节**等于"源集合
  按规则改名后的期望"，且**重新读取源得到的快照与导入前完全一致**（目录不被改写、归档条目不变）。
- **真实 Mver 样本回归**：`cargo run -p bongocat-model-store --example model_conversion_smoke -- --source
  /Users/ayang/Downloads/bongo_cat_mver_0.1.6_64` 逐模式输出与 ADR-0037 记录的完全一致
  （`standard` 15 张键位图 / 31 文件 / 1 081 672 字节；`keyboard` 23 文件 / 1 015 987 字节且
  `left-keys{Control, KeyR, Shift}`；`gamepad` 28 文件 / 1 082 753 字节），说明展开改动没有改变
  真实样本的产物。
- 冻结快照：用 `spikes/model-package` 的解析器对仓库预置模型重新生成索引，与更新后的
  `preset-model3-index.json` **逐字段相等**（工作树里未跟踪的 `resources/models/standard/resources/.DS_Store`
  需排除，否则会多出一个 `package_file_count` 与一个 unreferenced 文件）。
- `cargo fmt --all --check`、三组 clippy（workspace `--all-targets --all-features --exclude
  bongocat-app`、`bongocat-app` 的 `storage-test-injection` 与 `production`，均 `-D warnings`）、
  `cargo test --locked --workspace`（全部测试二进制全绿：`bongocat-app` 126、`bongocat-ui` 115、
  `bongocat-model-store` 92、`bongocat-runtime` 73+2、`bongocat-platform` 50、`bongocat-live2d` 45、
  `bongocat-update` 36+10、`bongocat-render` 15、`bongocat-overlay` 28、`bongocat-packaging` 23、
  `bongocat-config` 51、`bongocat-audio` 10、`bongocat-i18n` 4、`bongocat-log` 3）、
  `cargo check --locked --workspace --release` 均以退出码 0 通过（仅剩与本次无关的
  `block v0.1.6` future-incompatibility 警告）。

**未运行**：Windows 构建与实机按键、真实旧版安装数据根上已安装模型的 `AltGr` 兼容路径、任何 UI
实机点击。

---
<a id="adr-0039"></a>
# ADR-0039: 主 Enter 与小键盘 Enter 的键位图命名与旧名兼容

状态：已接受（2026-09-17）
依赖：ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0036（模型导入来源识别与 staging 边界；**该 ADR 已于 2026-09-22 撤回**）、ADR-0004（可校正输入状态）

修订（2026-09-22）：ADR-0036 撤回后模型来源只剩目录，本文决策 4 里「目录与 `.zip` 两种来源」的
归档一半不再适用。**命名决策本身未变。**

## 背景

ADR-0038 处理了左右 Alt 的 `rdev` 旧名，但同一套 `rdev` 词汇表还有第二个问题：主 Enter 键
（HID usage `0x28`）的键位图叫 `Return`。仓库预置模型
（`resources/models/{standard,keyboard}/resources/left-keys/Return.png`）与真实社区模型
（`送葬人 · 标准模式`、`Bongo Cat v0.16/BongoCat - 标准模式`）都带 `Return.png`，而运行时
`key_name_candidates(0x28)` 也查这个名字——两者自洽，但 `Return` 不是 HID 语义的名字。

更实际的缺口在小键盘：小键盘 Enter 是另一个物理键（HID `0x58`，Windows Raw Input 的
make code `0x1c` + E0 扩展位，macOS keycode `76`），`bongocat-platform` 在两个平台上都把它
正确映射为 `0x58`，但 `key_name_candidates` 对 `0x58` 没有任何候选——按下小键盘 Enter 什么都不
画。旧词汇表里也没有这个名字：翻遍本机全部真实样本（送葬人、经典小键盘、Bongo Cat v0.16），
没有任何模型为小键盘 Enter 配图，旧产品对它是完全无感的。

两个决定本 ADR 形态的事实：

1. **`Num` 前缀在产品词汇表里已被数字行占用。** `key_name_candidates(0x1e..=0x27)` 输出
   `Num1..Num0`（主键区数字行），预置模型与社区模型按这个约定画图。小键盘 Enter 不能叫
   `NumEnter`，否则和既有语义冲突。
2. **BongoCatMver 的键位表不区分两个 Enter。** legacy 表用 Windows 虚拟键码 + `GetKeyState`
   查询，`VK_RETURN`（`0x13` 之前的 `0x0D`，教程编号 `13`）对主 Enter 与小键盘 Enter 都为真，
   与 `VK_MENU` 之于左右 Alt 完全同类（ADR-0038 决策 4 的先例）。真实样本
   （`bongo_cat_mver_0.1.6_64`）的 keyboard/standard 绑定不含 `13`，gamepad 的 `13` 是十字键下，
   所以这次改动不影响真实样本的转换产物。

## 决策

### 1. 命名方案

| 物理键 | HID usage | canonical 名 | 说明 |
| --- | --- | --- | --- |
| 主 Enter | `0x28` | `Enter` | 取代 `Return`，与 HID 语义一致 |
| 小键盘 Enter | `0x58` | `KpEnter` | 沿用 Mver 转换词汇表已有的 `Kp` 前缀（`Kp0..Kp9`、`KpMultiply`、`KpPlus`、`KpMinus`、`KpDecimal`、`KpDivide`） |

两个名字分属不同 HID usage，候选列表互不重叠，不存在解析冲突。

### 2. 运行时：精确名优先，`Return` 是第二个旧名遗留

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

### 3. 预置模型重命名，冻结快照逐字段同步

`resources/models/{standard,keyboard}/resources/left-keys/` 下 `Return.png → Enter.png`
（`git mv`，字节不变）。`shared/fixtures/model-fixtures/preset-model3-index.json` 同步更新
`id` 与 `file` 两处；条目按 id 字母序移到 `Delete` 与 `Escape` 之间。`spikes/model-package`
解析器的 `preset_model_indices_match_frozen_snapshot` 重跑通过即逐字段相等（顺带清掉了
工作树里未跟踪的 `resources/models/standard/resources/.DS_Store`，与 ADR-0038 记录的是
同一个干扰项）。预置模型不新增 `KpEnter.png`：没有专属 artwork，回退到 `Enter.png` 就是
期望行为。

### 4. 包导入归一化扩展到 `Return`

`key_names::LEGACY_KEY_IMAGE_NAMES` 增加第三项 `("Return", "Enter")`，沿用既有规则：只处理
`resources/{left-,right-}keys` 一层常规文件，canonical 文件已存在时保留它、旧名原样留下，
用户选中的源永远是只读输入。

### 5. Mver 转换：`VK_RETURN` 展开成两个名字

`legacy_key_names` 对键盘模式的 `0x0D` 返回 `["Enter", "KpEnter"]`（`legacy_key_name` 单值
表从 `Return` 改为 `Enter`，供测试与文档固定词形）；同一张合成图装到两个名字下，运行时按
实际按下的键各自解析。这与 `VK_MENU → AltLeft + AltRight` 的展开完全同类，gamepad 模式
不受影响（`13` 仍是 `DPadDown`）。

`0x10`（Shift）、`0x11`（Control）仍保持家族名不变——理由与 ADR-0038 决策 4 相同：改动会
移动真实样本的转换产物证据，留给单独的改动。

## 明确不做

- **不引入 `NumEnter` 之类第三套拼写**：`Num` 已被数字行占用。
- **不就地升级已安装模型**：既有安装靠运行时的 `Return` alias 保持可用（同 ADR-0038）。
- **不在本次把 `Kp0..Kp9`、`KpMultiply` 等接入运行时候选**：Mver 转换早已输出这些名字，但
  `key_name_candidates` 至今不解析它们（按下的 `0x54..0x63` 无候选、不画图）。这是 ADR-0037
  记录过的同类既有缺口（功能键 `F1..F12` 逐键图不可达同款），仓库内没有模型画过这些键，
  改动无真实数据可验证，记录为后续项。本次只接入小键盘 Enter，因为它有明确的产品需求。
- **不做模型加载期的动态旧名映射**：磁盘上的包应当是产品自己的词汇表（同 ADR-0038）。

## 残余风险与待验证项（不得当作已确认）

1. **`Return` alias 只在单元测试里验证**：没有在真实用户数据根里已安装的 `Return.png` 模型上
   跑过整条链路。
2. **小键盘 Enter 的实机事件未验证**：`0x58` 在两个平台 adapter 上的映射是既有代码（Windows
   `(0x1c, E0)`、macOS keycode `76`，各自有 contract 测试），本次未新增实机按键验证；Windows
   侧同样受既有的交叉编译限制。
3. **社区模型里如果存在名为 `Enter.png` 的图**（理论上不可能，旧词汇表从未用过这个名字），
   归一化会保留它并留下 `Return.png`——运行时优先 `Enter`，行为仍然正确。
4. **Mver 展开依赖真实样本不含 `13`**：一旦将来出现键盘模式绑定了 `13` 的真实 Mver 模型，
   其转换产物会多出一张 `KpEnter.png`（与 `Enter.png` 同字节）。这是行为保持的设计结果，
   不是缺陷，但会改变该样本的产物证据。

## 验证

已完成（2026-09-17，本机 macOS / aarch64）：

- `bongocat-live2d` 47 测试（新增 2）。`enter_keys_resolve_distinct_names_with_a_keypad_fallback`
  固定两组候选（`0x28 → [Enter, Return]`、`0x58 → [KpEnter, Enter]`）与四种模型的命中结果
  （canonical、双图专用、纯旧名 `Return`、空资源）；
  `shipped_keyboard_models_draw_both_enter_keys_from_the_renamed_artwork` 用真实预置模型断言
  `0x28` 与 `0x58` 都命中 `resources/left-keys/Enter.png` 且不再存在 `Return.png`。
- `bongocat-model` 93 测试（新增 1，改写 2）：`key_names` 既有 3 个用例扩展到 `Return →
  Enter`（两侧目录改名、同存时 canonical 优先）；`mver` 的
  `alt_control_codes_never_collapse_left_and_right` 扩展到 `0x0D → [Enter, KpEnter]`（gamepad
  的 `13` 仍是 `DPadDown`）、新增
  `one_shared_enter_binding_installs_the_same_overlay_for_both_keys`（一次 `13` 绑定产出
  `Enter.png` + `KpEnter.png` 且字节相同）。
- `spikes/model-package` 15 测试全绿：冻结快照与重解析产物逐字段相等。
- **真实社区模型验证**（`BONGOCAT_PACKAGE_SAMPLE` 门禁用例，目录与 `.zip` 各跑一遍）：

  | 样本 | 旧名 | 结果 |
  | --- | ---: | --- |
  | `送葬人 · 标准模式`（目录 / zip） | 3 | `Alt.png`、`AltGr.png`、`Return.png` 改名，字节相同，源不被改写 |
  | `Bongo Cat v0.16/BongoCat - 标准模式` | 3 | 同上 |
  | `经典小键盘 · 标准模式.zip` | 0 | 无改名，15 张图逐字节相同 |

- **真实 Mver 样本回归**：`cargo run -p bongocat-model-store --example model_conversion_smoke --
  --source /Users/ayang/Downloads/bongo_cat_mver_0.1.6_64` 逐模式输出与 ADR-0037 记录完全一致
  （standard 31 文件 / 1 081 672 字节、keyboard 23 文件 / 1 015 987 字节、gamepad 28 文件 /
  1 082 753 字节），确认 `0x0D` 展开与 `Return → Enter` 没有改变真实样本的产物。

**未运行**：Windows 构建与实机按键、真实旧版安装数据根上已安装模型的 `Return` 兼容路径、
任何 UI 实机点击（详见残余风险）。

---
<a id="adr-0040"></a>
# ADR-0040: 小键盘键位图的 `Kp*` 命名与主键盘键位图回退

状态：已接受（2026-09-17）
依赖：ADR-0039（主 Enter 与小键盘 Enter 的键位图命名与旧名兼容）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0004（可校正输入状态）

## 背景

ADR-0039 把主 Enter（HID `0x28`）与小键盘 Enter（HID `0x58`）分开命名，并把"把 `Kp0..Kp9`、
`KpMultiply` 等接入运行时候选"明确记为后续项。本 ADR 就是那一步。

决定本 ADR 形态的事实：

1. **小键盘是 HID `0x53`…`0x63` 一整块，两个平台 adapter 早已把整块映射正确。**
   Windows 的 `map_scan_code`（非扩展 `0x37/0x47..0x53`、扩展 `(0x1c,E0)`、`(0x35,E0)`）与 macOS 的
   keycode 表（`65/67/69/71/75/76/78/82..92`）都产出这些 usage，各自有 contract 测试。
2. **没有任何模型为小键盘画过专属图。** 预置 `standard`/`keyboard` 的 `left-keys` 不含任何
   `Kp*.png`；ADR-0037/0038/0039 收集的真实样本（送葬人、经典小键盘、Bongo Cat v0.16）同样没有。
   小键盘 Enter 在 ADR-0039 之后靠回退到 `Enter.png` 才画得出来，其余小键盘键什么都不画。
3. **`key_name_candidates` 对小键盘几乎无候选。** 改动前只有 `0x58 → KpEnter, Enter` 一条，
   `0x53..=0x57` 与 `0x59..=0x63` 全部落到 `_ => function_key`（`None`），候选列表为空。
4. **`InputState::model_snapshot` 会丢弃没有 hand 归属的按键。**
   `bongocat-app::input_bindings_for_model` 的手表只有 `0x04..=0x27`、功能键两段、`0x28..0x2c`、
   `0x35/0x38/0x39/0x4c`、`0x4f..=0x52`（右手）和修饰键——小键盘整块一个都没有。因此
   **ADR-0039 给 `0x58` 加的回退在实机上是不可达的**：按下小键盘 Enter 的 `KeyPress` 在
   `hand_for == None` 的 `None => {}` 分支就被丢掉，`resolve_key_overlays` 从未被调用。
   这与功能键逐键图不可达是同一个缺口（已在功能键那一项里记录并闭合），也是本 ADR 必须
   同时改绑定的原因。
5. **`Num` 前缀属于主键区数字行，`Kp` 前缀属于小键盘。** `key_name_candidates(0x1e..=0x27)` 输出
   `Num1..Num0`；Mver 转换的 legacy 表（VK `0x60..0x6F`）输出 `Kp0..Kp9`、`KpMultiply`、`KpPlus`、
   `KpMinus`、`KpDecimal`、`KpDivide`，ADR-0039 决策 1 已把 `KpEnter` 并入这套词汇。

## 决策

### 1. 小键盘与主键盘的对应清单

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

### 2. 候选顺序：精确名在前，主键盘对应键在后

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

### 3. 小键盘整块绑到左手

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

## 明确不做

- **不给预置模型新增任何 `Kp*.png`**：没有专属 artwork 时回退就是期望行为（同 ADR-0039 决策 3）。
- **不引入文字/字形渲染管线**：渲染器（`bongocat-render`、`bongocat-overlay`）目前只有带纹理的
  四边形，没有字体资源、字形图集或文字 shader。"显示其可读名称"因此落地为"按可读名解析到已有
  键位图"，与 ADR-0039 的 `KpEnter → Enter` 完全同构；真正的屏幕文字需要字体资源加 D3D11/Metal
  两套文字管线，属于独立改动。
- **不改 `resolve_key_overlays` 的 side 严格性**：缺同侧资源时不绘制是既有契约
  （Technical Design §11），不为小键盘放宽成跨侧查找。
- **不处理 HID `0x64`**：ISO/ABNT2 额外键，不是小键盘键。

## 残余风险与待验证项（不得当作已确认）

1. **小键盘的实机事件仍未验证**：两个平台 adapter 的映射是既有代码且有 contract 测试，本次未新增
   实机按键验证；Windows 侧同样受既有的交叉编译限制。
2. **左手绑定是产品近似，并且决定了小键盘美术只能放 `left-keys`**：按下小键盘会让左爪下压；
   放在 `right-keys` 的 `Kp*.png` 在左手绑定下不可达。若后续为小键盘出右手美术，需要同时把绑定
   改到右手（届时回退会失去 `left-keys` 里的 `Num*`/`Enter`/`Slash`），决策 3 需要重写。
   注意 `bongocat-model-store` 的 Mver 转换是按 legacy 的 `lefthand`/`righthand` 列表决定输出目录的
   （`OUTPUT_LEFT_KEYS`/`OUTPUT_RIGHT_KEYS`），因此手写的 legacy 表若把小键盘码放进右手列表，
   产物会落到 `right-keys`；已知真实样本不含小键盘码，所以当前不存在这种产物。
3. **`NumLock`、`KpMultiply`、`KpPlus` 仍然不画任何东西**：主键盘上没有这三个键（`*`、`+` 只能靠
   `Shift` 得到），因此没有可借的图。这不是本 ADR 引入的缺陷，但用户可能期望它们也有反馈。
4. **`0x64` 仍然无候选**：Windows 的 `map_scan_code` 会把扫描码 `0x56` 映射为 `0x64`（ISO/ABNT2
   额外键），该键在本 ADR 之后依旧不绘制，行为与改动前一致。

## 验证

已完成（2026-09-17，本机 Windows / x86_64-pc-windows-msvc）：

- `bongocat-live2d` 48 测试（新增 2）：
  `keypad_keys_name_themselves_and_fall_back_to_their_main_keyboard_twin` 固定整块
  `0x53..=0x63` 的候选列表（含五个无回退键只列精确名）；
  `shipped_keyboard_models_draw_the_keypad_from_the_main_keyboard_artwork` 用真实预置
  `standard`/`keyboard` 断言十二个小键盘键命中 `resources/left-keys/` 的 `Num1..Num9`、`Num0`、
  `Enter`、`Slash`，并断言预置模型确实不含任何 `Kp*` 图、五个无对应键解析为空（不绘制）。
- `bongocat-app` `--lib` 124 测试（新增 1）：
  `keyboard_models_assign_the_whole_keypad_block_to_the_left_hand` 断言三种键盘模型的整块小键盘
  左手绑定，且方向键 `0x4f..=0x52` 在 `Installed`/`keyboard` 上仍是右手，`gamepad` 预置不绑小键盘。
- `just check` 的六道门全过：`cargo fmt --all -- --check`；
  `cargo clippy --locked --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings`；
  `cargo clippy --locked -p bongocat-app --all-targets --features storage-test-injection -- -D warnings`；
  `cargo clippy --locked -p bongocat-app --all-targets --features production -- -D warnings`；
  `cargo test --locked --workspace`；`cargo check --locked --workspace --release`。
  （注意 `cargo clippy --workspace --all-features` 不带 `--exclude bongocat-app` 会因为
  `storage-test-injection` 与 `production` 互斥而失败，这是既有约束，不是本 ADR 引入的。）

**未运行**：Windows 实机按键、macOS 构建与实机按键、UI 实机点击、真实社区模型上的小键盘回归
（无样本画过小键盘图，因此没有可对照的产物证据）。

---
<a id="adr-0041"></a>
# ADR-0041: 完整键位词表与绑定覆盖

状态：已接受（2026-09-17）
依赖：ADR-0040（小键盘键位图的 `Kp*` 命名与主键盘键位图回退）、ADR-0039（主 Enter 与小键盘 Enter 的键位图命名与旧名兼容）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0004（可校正输入状态）

## 背景

ADR-0040 把 `Kp0..Kp9`、`KpMultiply` 等接入了运行时候选，但当时的范围只覆盖小键盘，并且判断
依据里混进了"当前没有模型画这些键"这一事实。本 ADR 纠正这个思路：**键位词表是与模型作者的契约，
不是"预置模型当前画了哪些图"的清单。**

决定本 ADR 形态的事实：

1. **词表原本是不完整的。** 改动前 `key_name_candidates` 只命名了字母、数字行、`Enter`、`Escape`、
   `Backspace`、`Tab`、`Space`、`BackQuote`、`Slash`、`CapsLock`、四个方向键、小键盘、修饰键和
   `F1..F24`。以下 usage 没有 arm，落到 `_ => function_key`（`None`）：
   `0x2d` `Minus`、`0x2e` `Equal`、`0x2f` `LeftBracket`、`0x30` `RightBracket`、`0x31` `BackSlash`、
   `0x32`、`0x33` `SemiColon`、`0x34` `Quote`、`0x36` `Comma`、`0x37` `Dot`、`0x46` `PrintScreen`、
   `0x47` `ScrollLock`、`0x48` `Pause`、`0x49` `Insert`、`0x4a` `Home`、`0x4b` `PageUp`、
   `0x4c` `Delete`、`0x4d` `End`、`0x4e` `PageDown`、`0x64`、`0x65` `Apps`、`0x67`。
2. **缺名字的代价不是理论上的。** `0x4c`（`Delete`）在 `input_bindings_for_model` 的左手表里，
   并且 `resources/models/{standard,keyboard}/resources/left-keys/Delete.png` **已随预置模型出厂**——
   但因为没有名字 arm，`key_name_candidates(0x4c)` 返回空候选，**`Delete.png` 从出厂那天起就
   永远画不出来**。逐张核对两个预置模型的 55 张键位图，`Delete.png` 是唯一不可达的一张。
   这与功能键逐键图曾经的缺口（已在功能键那一项闭合）是同一类错误。
3. **名字和绑定必须覆盖同一个集合。** `InputState::model_snapshot` 对 `hand_for == None` 的按键走
   `None => {}` 直接丢弃，`resolve_key_overlays` 从未被调用。所以"有名字、没绑定"和
   "没名字"一样不可达——只补名字不补绑定，等于把词表做成摆设。ADR-0040 已经踩过这个坑
   （`KpEnter` 有了名字却没有 hand 归属）。
4. **范围由平台 adapter 的实际产出界定。** Windows 的 `map_scan_code` 产出
   `0x04..=0x31`、`0x33..=0x45`、`0x46..=0x4e`、`0x4f..=0x64` 与八个修饰键 usage
   `0xe0..=0xe7`（E0/E1 前缀与非扩展码）；macOS 的 keycode 表产出
   `0x04..=0x31`、`0x33..=0x45`、`0x4a..=0x63`、`0x67`、`0x68..=0x6f`、`0xe0..=0xe7`。
   两者并集 = `0x04..=0x65` ∪ `{0x67}` ∪ `0x68..=0x73` ∪ `0xe0..=0xe7`。`0x66`（`Power`）
   两边都不产出。

   > **修订（2026-09-17，同日）：** 本事实原先写的并集漏掉了修饰键 `0xe0..=0xe7`（它们不在
   > `0x04..=0x65`、`0x67`、`0x68..=0x73` 任何一段里），决策 2 的绑定循环照着这个不完整的并集
   > 重写，于是八个修饰键 usage 失去了 hand 归属：`Shift`/`Control`/`Alt`/`Meta` 的按键被
   > `InputState::model_snapshot` 丢弃，模型再也画不出 `ShiftLeft.png`、`AltLeft.png`、
   > `AltRight.png`、`Meta.png` 等修饰键美术，爪子也不再为它们下压——即 ADR-0038 专门为 Alt
   > 图片做过的事被同时作废。修正：绑定循环补回 `0xe0..=0xe7` → 左手，并把并集作为**规范**写进
   > 绑定契约测试（遍历 `0x04..=0x65` ∪ `{0x67}` ∪ `0x68..=0x73` ∪ `0xe0..=0xe7`，断言每个
   > usage 的绑定等于"该模型能画则绑"），不再只遍历实现恰好用到的那几段。测试盲区是本次回归能
   > 溜过 `just check` 的直接原因：命名测试与绑定测试当时都只遍历前三段。
5. **旧版键位映射图给出的名字是词表依据。** 它覆盖标准 104/105 布局 + 小键盘，包含
   `Minus`/`Equal`/`LeftBracket`/`RightBracket`/`BackSlash`/`SemiColon`/`Quote`/`Comma`/`Dot`/
   `PrintScreen`/`ScrollLock`/`Pause`/`Insert`/`Home`/`PageUp`/`Delete`/`End`/`PageDown`/`Apps`
   这些当前代码没有 arm 的名字。图中的空格只是排版（`Back Quote` → `BackQuote`、
   `Kp Divide` → `KpDivide`），实际资源名无空格。

   > **读图约定（2026-09-19，与作者确认）。** 参考图上有两类**不承载实现语义**的记号，另有一条
   > 关于范围的说明，核对时都不要当成缺口：
   >
   > - **多词标签的折行是排版，不是名字的一部分。** `Back Quote`、`Control Left`、`Kp Divide`
   >   都是换行显示的单段名字。
   > - **右上角那三个绿点（Num Lock / Caps Lock / Scroll Lock）是纯装饰**，只用来把三个锁定键标成
   >   一类。**它不表示应用会显示锁定状态**——当前实现没有任何锁定状态指示，也不打算加（首版
   >   范围之外）。核对时若把它读成"应显示 Caps Lock 灯"，那是对图的过度解读。
   > - **图是物理布局参考，不是"每个平台都画得出来"的承诺。** 图上有几个键只在单个平台可达：
   >   `PrintScreen`/`ScrollLock`/`Pause`/`Insert` 仅 Windows，`Globe` 仅 macOS。
   >   照图做美术仍然正确——平台产不出的键只是那个平台画不出来，不是命名错了。平台边界见事实 4。
   >
   > 三个锁定键本身在词表与绑定里都是正常成员：`0x39` `CapsLock`、`0x47` `ScrollLock`、
   > `0x53` `NumLock` 各有名字（`bongocat-live2d::key_name_candidates`），都被
   > `input_bindings_for_model` 的 `0x04..=0x65` 循环绑到左手，是否真的画得出来只取决于模型有没有
   > 对应美术（ADR-0042；预置模型没出这三张图，所以当前画不出，这是设计如此）。
   > 可达性按两个 adapter 的实际产出：`CapsLock`（macOS keycode `57`）与 `NumLock`（macOS keycode
   > `71`）双平台可达，`ScrollLock` **仅 Windows**——macOS 没有对应 keycode，见 macOS 侧那条
   > 穷举测试的期望集合里没有 `0x47`。

## 决策

### 1. 命名标准布局 + 小键盘的全部按键

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

### 2. 绑定覆盖同一集合

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

### 3. `PrintScreen` 绑定，但明确不是功能键

`FUNCTION_KEY_USAGES` 的边界（`0x3a..=0x45` 与 `0x68..=0x73`）**不变**，`PrintScreen` 不享受
`Fn` 回退。它只是作为一张普通按键获得名字和 hand——这与"不享受功能键语义"并不矛盾，
原来的 `must not bind PrintScreen` 断言混淆了这两件事，本次改成断言
`function_key_name(0x46) == None`（真正的边界保证）+ `hand_for(0x46) == Some(Left)`（新的绑定事实）。

### 4. 已出厂的 `Delete.png` 恢复可达

`0x4c => Some("Delete")` 让两个预置模型已经出厂的 `Delete.png` 立即生效。这是本次唯一的
"修复既有不可达资源"，不是新增能力。

## 明确不做

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

## 残余风险与待验证项（不得当作已确认）

1. **绑定集合扩大是行为变化**：标点键、`PrintScreen`、导航键现在都会让左爪下压（此前完全无反应）。
   这是"名字可用"的必要代价，也让这些键在没有美术时仍有爪部反馈；但它确实改变了既有行为，
   尚未在实机上确认观感。
2. **这些键位图仍然全部不存在**：除 `Delete.png` 外，本次补的名字都不会画出任何东西，
   需要模型补图才有视觉效果。
3. **`IntlHash`/`IntlBackslash` 是本次新增词汇**：旧图未命名，两个平台 adapter 也只有 Windows
   产出 `0x64`、`0x32` 两边都不产出。若将来确认了更合适的旧名，需要按旧名兼容流程处理。
4. **实机按键仍未验证**：两个平台 adapter 的映射是既有代码且有 contract 测试，本次未新增实机验证。

## 验证

已完成（2026-09-17，本机 Windows / x86_64-pc-windows-msvc）：

- `bongocat-live2d` 50 测试（新增 2）：
  `every_key_the_platform_adapters_can_report_has_a_name` 断言 `0x04..=0x65`、`0x67`、
  `0x68..=0x73` 每个 usage 的候选列表非空，并断言 `0x66`（`Power`）保持无名；
  `a_model_providing_a_named_key_image_draws_it` 用合成资源断言 27 个新命名键在模型提供
  对应 PNG 时全部命中该图，并断言只提供出厂词汇的模型对这些键仍然解析为空。
- `bongocat-app` `--lib` 124 测试：
  `keyboard_models_bind_every_named_key_of_the_standard_layout`（改写自原小键盘用例）
  断言三种键盘模型绑定 `0x04..=0x65`（方向键除外）、`0x68..=0x73`、`0x67` 到左手，方向键
  对 `Installed`/`keyboard` 仍是右手，`gamepad` 仍不绑键盘小键盘；功能键那条用例改为断言
  `function_key_name(0x46) == None`（真正的"不是功能键"边界）与 `hand_for(0x46) == Some(Left)`
  （新的绑定事实）。
- `just check` 六道门（fmt、三组 clippy、workspace test、release check）。

**未运行**：Windows/macOS 实机按键、UI 实机点击。

---
<a id="adr-0042"></a>
# ADR-0042: 键位图存在才触发按键动作

状态：已接受（2026-09-17）
依赖：ADR-0041（完整键位词表与绑定覆盖）、ADR-0040（小键盘键位图的 `Kp*` 命名与主键盘键位图回退）、ADR-0004（可校正输入状态）

## 背景

ADR-0041 把标点键、`PrintScreen`、导航键和小键盘整块都绑到了左手，并在残余风险 1 里记下了代价：
这些键现在都会让左爪下压，而预置模型对其中大多数键并没有图。用户把它报成了缺陷：
**模型没有对应图片资源时，按下该键不应该产生任何动作。**

决定本 ADR 形态的事实：

1. **一次按键的"动作"是两件事，但只有一个开关。** 按键会驱动
   `ProductParameter::LeftHandDown`/`RightHandDown`（即 Cubism 的 `CatParamLeftHandDown`/
   `CatParamRightHandDown`，见 `bongocat-runtime::apply_model_input`），以及按键图层
   （`resolve_key_overlays`）。两者都由 `InputState::model_snapshot` 里同一个 `hand_for`
   归属决定：有归属的按键才置 hand 标志并成为该侧的候选 press，`None => {}` 的按键直接被丢弃。
   **所以"绑定"就是那个唯一开关**，不需要第二套门禁。
2. **图是模型包自带的，只有渲染侧知道。** `bongocat-live2d-render::load_key_assets` 扫描
   `resources/left-keys` / `resources/right-keys`，`resolve_key_overlays` 用
   `key_name_candidates` 的候选顺序在已加载的资产里找图；找不到就不画。
3. **预置 `standard` 的 55 张键位图里没有 `Dot.png`、`Minus.png`、`PrintScreen.png`、
   `NumLock.png`、小键盘 `*`/`+`/`-`/`.`**，也没有 `right-keys` 目录。按下 `.`、`,`、`-`
   或 `PrintScreen` 时，用户看到的是"爪子按下去但什么都没出现"——动作指向一个永远画不出来的东西。
4. **判断必须发生在模型与绑定相遇的地方。** runtime 是平台无关 crate，只在 Windows/macOS
   依赖 `bongocat-live2d` 与 `bongocat-live2d-render`，且它的 `model_snapshot` 只接收 `InputBindings`；renderer 只消费不可变
   `RenderSnapshot`，不得决定动作（Technical Design §5.2/§5.3）。而 `bongocat-app` 在模型激活时
   本来就为每个模型计算绑定（`input_bindings_for_model`），手上正好有 `CommittedModel`。
5. **"可绘制"必须与实际绘制同源。** 如果判断用的目录扫描或候选顺序与渲染侧不一致，就会出现
   "以为画得出来却没画"或"画得出来却不响应"。两者必须由同一段代码回答。

## 决策

### 1. `bongocat-live2d-render` 提供键位图清单

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

### 2. 绑定按模型实际资源收窄

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

### 3. 缺图即无动作

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

### 4. 作用范围是键位图

- **鼠标**不属于本规则：`mouse_left_down`/`mouse_right_down` 驱动的是
  `ParamMouseLeftDown`/`ParamMouseRightDown`，是指针状态而不是键位图，产品词表里也没有它们的
  图片名。
- **手柄按钮**不属于本规则：它们经由 `hand_for_gamepad` 驱动爪子，但键位图解析器没有手柄按钮
  的词表，手柄美术今天本来就不进入按键图层（预置 `gamepad` 的 `South.png` 等只被加载、不被绘制）。

## 明确不做

- **不给预置模型补任何键位图**：缺图就是缺图，本 ADR 只让缺图时不再假装有反馈。
- **不改 `key_name_candidates` 的词表与 `resolve_key_overlays` 的 side 严格性**：命名仍覆盖
  平台 adapter 能产出的全部按键，同侧资源缺失仍不绘制。
- **不把判断搬进 runtime 或 renderer**：runtime 需要图片资源信息就得依赖 Cubism/图片解码；
  renderer 决定动作会破坏"renderer 只消费 `RenderSnapshot`"的边界。
- **不改鼠标与手柄输入**。

## 残余风险与待验证项（不得当作已确认）

1. **只有另一手有图时该键无动作**：side 严格性是既有契约，现在"绑定"与"绘制"在这一点上也一致
   ——模型把小键盘美术放进 `right-keys` 时左手绑定下不可达（同 ADR-0040 决策 3）。
2. **预置 `gamepad` 不再响应任何键盘按键**：它没有键盘键位图（此前也不绘制任何键盘图层），
   键盘输入现在完全不驱动它；手柄按钮映射不受影响。
3. **完全没有键位图的导入模型不再响应键盘**：这类模型此前会让爪子下压但画不出图；现在键盘输入
   对它完全无动作，模型仍由指针、呼吸和眨眼驱动。这是本 ADR 的直接意图，但改变了观感。
4. **`bongocat-overlay` 的预览工具仍持有自己的静态绑定表**（`preview_input_bindings`，停留在
   ADR-0041 之前的形态），不感知键位图：`just preview gamepad` 仍会绑定 `A` 并让左爪下压，
   与产品行为不同。它是人工诊断工具，本次未改；后续如要收敛，应与产品共用同一套绑定构造。
5. **实机观感未验证**：本次只跑自动化契约，未在 Windows/macOS 实机上逐键确认。

## 验证

已完成（2026-09-17，本机 Windows / x86_64-pc-windows-msvc）：

- `bongocat-live2d-render` 资源 contract 与 `bongocat-live2d` 集成测试：
  `key_image_inventory_lists_exactly_the_assets_the_renderer_loads` 对三个预置模型断言清单与
  `load_key_assets` 加载到的 (side, name) 集合逐侧相等（"报告"与"绘制"同源）；
  `a_shipped_model_can_draw_only_the_keys_it_ships_artwork_for` 断言 `standard` 能画 `KeyA`、
  `Delete`、功能键（`Fn`）、小键盘数字（回退 `Num*`）、小键盘 Enter（回退 `Enter`），不能画
  `.`、`-`、`PrintScreen`、`NumLock`、小键盘 `.`、小键盘 `=` 与方向键（含 `right-keys` 不存在），
  并断言 `keyboard` 的右手方向键可画、`gamepad` 连 `KeyA` 都不可画。
- `bongocat-app` `--lib` 125 测试（新增 1，改写 3）：
  `a_key_the_active_model_cannot_draw_never_moves_the_paw` 启动真实渲染应用、激活预置
  `standard`，断言按下 `A`（有 `KeyA.png`）、左 `Shift`（`ShiftLeft.png`）与右 `Meta`
  （无 `MetaRight.png`，回退共享 `Meta.png`）都置 `left_hand_down` 并进入 `key_presses`，而按下
  `.`（无 `Dot.png`）既不置爪也不产生按键 press；
  `keyboard_models_bind_every_drawable_key_of_the_standard_layout` 断言绑定等于"静态表 ∩ 模型
  键位图"，**遍历两个平台 adapter 的产出并集**（`0x04..=0x65` ∪ `{0x67}` ∪ `0x68..=0x73` ∪
  `0xe0..=0xe7`，见 ADR-0041 事实 4 修订），并对 `KeyA`/`Num1`/`Delete`/小键盘 1/小键盘 Enter/
  左右 `Shift`/`Alt`/`Control`/`Meta` 为真、`.`/`PrintScreen`/`NumLock`/小键盘 `.` 为假做逐条
  抽查；功能键用例改为断言 `PrintScreen` 不再绑定（无图），
  `installed_models_get_default_keyboard_bindings` 断言 `gamepad` 预置不绑任何键盘键、手柄按钮
  映射不变。
- `just check` 的六道门全过：`cargo fmt --all -- --check`；
  `cargo clippy --locked --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings`；
  `cargo clippy --locked -p bongocat-app --all-targets --features storage-test-injection -- -D warnings`；
  `cargo clippy --locked -p bongocat-app --all-targets --features production -- -D warnings`；
  `cargo test --locked --workspace`；`cargo check --locked --workspace --release`。

**未运行**：Windows/macOS 实机按键与观感确认、UI 实机点击、真实社区模型回归。

---
<a id="adr-0044"></a>
# ADR-0044: 全局快捷键改用 global-hotkey 注册制后端

状态：Accepted
日期：2026-09-17；2026-09-25 修订应用级输入门禁快捷键
取代：无（首次为快捷键子系统建立 ADR）

## Context

BongoCat 的快捷键（应用命令与模型行为）此前由输入管线内匹配实现：
Windows Raw Input / macOS CGEventTap 把按键边沿映射为 HID usage 后喂给
`ShortcutMatcher`，`ShortcutDispatcher` 在进程内解析目标并分发。该匹配是
被动的：注册过的组合键仍会继续传递给前台应用，同时快捷键依赖输入服务的
生命周期（输入服务因权限或恢复失败退出时快捷键一并失效）。

产品需要对齐历史 Tauri 版本的全局快捷键语义：组合键由操作系统注册并
消费，应用失焦或处于后台时必须可靠触发。经评审选择 tauri-apps 维护的
`global-hotkey` crate（crates.io 最新稳定版 0.8.0，Apache-2.0 OR MIT，
活跃维护）：Windows 走 `RegisterHotKey`，macOS 走 Carbon
`RegisterEventHotKey`，与历史实现的注册模型一致。

## Decision

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

## Consequences

- 行为变化：注册的组合键会被操作系统消费，不再同时传递给前台应用
  （与历史 Tauri 版本一致，属于预期语义）。
- macOS 上快捷键注册本身不要求辅助功能权限（Carbon 注册），但猫爪动画
  的全键盘监听仍依赖 CGEventTap，TCC 授权要求不变。
- 快捷键不再随输入服务失败而失效；输入服务的恢复/重置路径与快捷键状态
  完全解耦，Issue #47 的 pressed-state 校正只服务动画输入。
- 服务按 50ms 轮询感知改绑，录制 suspend/resume 的生效延迟 ≤ 100ms。
- 三个输入门禁快捷键与普通应用快捷键共享同一录制、冲突和 `commands_enabled` 门禁；它们只切换模型输入投影，键盘/鼠标/手柄的可靠采集与 pressed-state 恢复不受影响。

---
<a id="adr-0045"></a>
# ADR-0045: Overlay 放置约束改为屏幕范围并延迟收敛

状态：Accepted
日期：2026-09-18
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

## Context

当前 v1 的 `overlay.keep_inside_work_area` 约束 overlay 窗口避开工作区之外的一切区域：
Windows 用 `MONITORINFO.rcWork`、macOS 用 `NSScreen.visibleFrame`，并在创建、需要资源重建的
设置/模型重建和每一次 frame tick 里收敛窗口原点。由此产生两类不符合预期的行为：

- 模型窗口无法覆盖任务栏、程序坞和菜单栏所在的那一条区域。用户把猫放在任务栏上方这类明显
  合理的摆放会被立刻推回工作区内。
- 收敛在每一个 frame tick 上执行，因此拖拽一结束（两个平台的原生拖动循环都会阻塞 frame
  source，所以“结束”就是在下一帧）窗口就被拉回最近的显示器。当窗口在相邻两块显示器接缝处、
  或交叠面积更大的其实是对面的显示器时，这一拉回会把用户刚拖到的位置直接抢走，跨显示器摆放
  因此难以完成。

## Decision

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

## Consequences

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

---
<a id="adr-0046"></a>
# ADR-0046: Overlay 颜色契约以两端都执行 linear -> sRGB 编码为准

状态：Superseded by ADR-0063
日期：2026-09-18
取代日期：2026-09-24
取代：无（修正 Technical Design §Renderer 颜色段落与两条 Phase 3/5 验收证据的结论）

> 本文记录的是曾经用于补齐 Windows/macOS 线性编码不对称的修复决策与预期实现路径；
> 当时没有完成目标硬件的像素验证。它讨论了“两端是否都执行 linear -> sRGB 编码”，
> 但没有解决“是否要匹配 Bongo-Cat-Mver 的历史 encoded-space 观感”问题；当前产品目标由
> ADR-0063 取代。

## Context

Windows 与 macOS 上同一个模型出现明显色差：中间调整体不同，纯黑与纯白相同。排查后排除了
模型资源、纹理上传和 blend 模式，差异只落在像素管道的最后一步。

两端在编码之前完全一致：模型、背景和按键 PNG 都以 sRGB texture view 采样，shader 在解码后的
linear RGB 上执行 multiply/screen、mask 与预乘，blend 因子表也相同（normal 为
`ONE / ONE_MINUS_SRC_ALPHA`，additive 为 `ONE / ONE`，multiplicative 为
`DST_COLOR / ONE_MINUS_SRC_ALPHA`）。

分叉点在写入目标：

- macOS 把 `BGRA8Unorm_sRGB` 同时设给 `CAMetalLayer` 和管线颜色附件，硬件在 store 时完成
  linear -> sRGB 编码。
- Windows 的 flip presentation model 只能创建非 sRGB 的 `B8G8R8A8_UNORM` back buffer，
  代码据此把 swapchain 固定为 UNORM —— 但 back buffer 的 render target view 也是用
  `CreateRenderTargetView(texture, None, …)` 创建的，即继承 UNORM。于是 linear 预乘值被
  直接写进一个被 DirectComposition 当作 sRGB 解释的 surface：中间调输出约等于 `v^2.2`，
  黑白不变，且 alpha 边沿按 linear 权重加权。

Windows 官方文档给出的组合正是本例缺的一半：flip model 不允许 sRGB back buffer，正确做法是
UNORM buffer + `_SRGB` render target view。仓库原先把“DirectComposition 不接受 sRGB
swapchain format”当作完整结论，导致编码步骤从未执行，而 Technical Design 与两条验收证据都
断言这组格式已经避免了两平台颜色语义漂移。

## Decision

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

## Consequences

- 按当时的设计，同一组 sRGB 贴图应在两平台产出相同的 surface 字节，并预期消除 Windows
  以往“整体偏暗、对比更强”的观感；该结果当时没有目标硬件 readback 证据。
- 修复是格式层的，不改变 drawable 拓扑、遮罩算法、frame 调度或任何产品配置，因此不需要
  数据迁移，也不影响 `schema_version: 1`。
- 未收敛的残差是系统级显示色彩管理：macOS 由 Core Animation 把 sRGB 内容转换到显示器色彩空间，
  Windows DirectComposition 不做这一步。广色域显示器上同一组 sRGB 值仍可能观感不同，这属于
  合成器行为，不属于 renderer 契约。
- 旧版观感对齐（是否改用 gamma 空间合成）仍是独立问题。若将来决定对齐旧版，需要同时修改两端
  的纹理视图与附件格式，并同步本文第 3 条与 Technical Design。

## Verification

以下记录的是 2026-09-18 当时的中间方案；其中测试名称和格式断言已被 ADR-0063 的
encoded-space contract 取代，不能作为当前实现的验证结果。

- `windows.rs` 的 `color_formats_decode_assets_and_encode_the_composited_frame_as_srgb` 增加
  `COMPOSITION_RENDER_TARGET_FORMAT` 断言；`macos.rs` 的对应测试继续固定
  `BGRA8Unorm_sRGB`，两侧注释互相指向，防止再次单边漂移。
- 本机（macOS 26.5.2 / Apple M1 Pro / Rust 1.97.1）通过：`cargo fmt -p bongocat-overlay --
  --check`、`cargo clippy -p bongocat-overlay --all-targets --all-features -- -D warnings`、
  `cargo test -p bongocat-overlay`，以及完整 workspace 门禁的其余各项（三组严格 Clippy、
  `cargo test --locked --workspace` 738 项、`cargo check --locked --workspace --release`）。
  仓库级 `cargo fmt --all -- --check` 未执行：它在 HEAD 上已因 `bongocat-platform` 的两个
  无关文件失败，属于既有问题。
- D3D11 调用形态（descriptor 字段、view 维度、`CreateRenderTargetView` 参数类型、DXGI 常量）
  以临时独立探针在 `--target x86_64-pc-windows-msvc` 下 check 与 clippy 通过。
- 尚未完成：Windows 实机首帧、Windows job 的单元测试执行结果、同 snapshot 的两平台 readback
  色值对照。该历史任务已由 `P3-MVER-COLOR-COMPATIBILITY` 取代；在新的跨平台硬件证据完成前，
  两条任务都不得作为当前观感验收依据。

---
<a id="adr-0049"></a>
# ADR-0049: 地球键与 `Fn` 的语义分离

状态：已接受（2026-09-19）
依赖：ADR-0041（完整键位词表与绑定覆盖）、ADR-0042（键位图存在才触发按键动作）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0037（应用内导入 BongoCatMver 模型）

## 背景

需求是"F1–F24 与 macOS 左下角地球键都要完整支持，且不能共用一张图"。考古后有一处前提需要修正：
**今天并不存在共用，`Fn` 也从来不是地球键的名字。**

决定本 ADR 形态的事实：

1. **`Fn` 是"F 编号通配符"，不是 Fn 键。** 唯一产生点是旧版
   `pre-refactor:src/composables/useDevice.ts:105-110`：

   ```js
   if (key.startsWith('F') && unsupportedKey) {
     nextKey = key.replace(/F(\d+)/, 'Fn')
   }
   ```

   只对"模型没逐键画图"的 `F<数字>` 生效。两个预置键盘模型出厂的
   `resources/models/{standard,keyboard}/resources/left-keys/Fn.png`（各自 55 张键位图之一）
   就是这张共享图。物理 Fn 键在 `rdev` 里叫 `Function`，`F` 后没有数字，命不中该正则。

2. **旧版地球键的名字是 `Function`，而且它是可达的。** `rdev` 的 macOS 表
   `const FUNCTION: CGKeyCode = 63;` → `Key::Function`（上游 `rdev/src/macos/keycodes.rs:36,274`），
   `pre-refactor:src-tauri/src/core/device.rs` 用 `format!("{:?}", key)` 取名；旧版 model store 的键集
   就是模型包自己的文件名主干（`pre-refactor:src/pages/main/index.vue:82`），所以一个提供
   `Function.png` 的模型在旧版里确实能画出地球键——只是已知的预置与社区模型都没有这张图。

3. **地球键在 BongoCat 里完全不可达。** `bongocat-platform/src/macos.rs` 的 `map_key_code`
   没有 keycode `63` 的 arm，而 `FlagsChanged` 分支先查表再解码（同文件 `1636-1641`），查不到就计入
   `unmapped_keys` 丢弃。缺的只是这一个 arm：`ModifierDecoder` 早就认识 63（`modifier_family_bit`
   里有 `63 => MaskSecondaryFn`）。

4. **`Fn` 不能同时是两个键的名字。** 若地球键取名 `Fn`，旧包里的 `Fn.png`（功能键共享图）与新地球键
   美术就同名，导入归一化无法区分二者；而本产品不做迁移（AGENTS.md §4.1），用户数据根里已安装的
   模型不会被重新归一化。结果是按 F3 画出地球图、按地球键画出功能键图——双向回归，且没有能同时满足
   两边的归一化规则。这不是取舍，是方案本身不成立。

5. **地球键不在 HID Keyboard/Keypad 页。** 它是 Apple 厂商页 `0xFF` 的 usage `0x03`（`KeyboardFn`）。
   任何 `0x04..=0xe7` 式区间都覆盖不到它，所以命名、绑定、两处契约并集都必须显式写出。

## 决策

### 1. 地球键 = `Globe`

`bongocat-render` 新增 `GLOBE_KEY_USAGE: u16 = 0xff03`（`0xff00 | 0x03` 的折叠值）作为六处共用的
唯一来源；`bongocat-runtime::PhysicalKey::GLOBE` 就是它，`bongocat-platform` 的 keycode 表也引用它
而不是重写数值。

不叫 `Fn`：见事实 4。不叫 `GlobeKey`：词表里没有 `*Key` 后缀，而 `KeyA`…`KeyZ` 是 W3C
`KeyboardEvent.code` 的既有拼写，加后缀会被读成同一族。`Function` 是旧名，但它与功能键回退名 `Fn`
只差两个字符而意思不同——这个仓库已经被 `Backslash`/`BackSlash` 这类近似名坑过一次（ADR-0050），
词表里不该再放一对。

### 2. `Fn` 保持原义，`Fn.png` 不改名、不迁移

`Fn` 是 F1–F24 的共享回退图名，语义本来就正确。改名会把每个现存模型的功能键美术挪到地球键上，
是对全部现存模型的行为回归，也与"导入要兼容旧图片资源"直接冲突。

地球键与功能键的候选列表**完全不相交**：

```text
F1 … F24 (0x3a..=0x45, 0x68..=0x73) → F1 … F24, Fn
0xff03   (Globe)                    → Globe, Function
```

模型只提供 `Fn.png` 时地球键画不出来；只提供 `Globe.png` 时功能键画不出来。

### 3. 地球键的候选与旧名兼容

`Function` 是旧版对同一物理键的可达名，排在最具体名之后。导入归一化把
`resources/{left-,right-}keys/Function.png` 改名为 `Globe.png`；运行时的 `Function` 候选保证
**已经安装过**的包（不会被重新归一化）仍然能画。这与 ADR-0038 对 `AltGr` 的处理同构。

### 4. macOS 映射 keycode 63

`map_key_code` 加 `63 => PhysicalKey::GLOBE.hid_usage()`。Windows 不加：Fn 键由键盘固件处理，
Raw Input 从不报告它，没有可映射的码。

### 5. Mver 转换补齐 F13–F24

`legacy_virtual_key_name` 加 `0x7C..=0x87` → `F13`…`F24`。`VK_F13`…`VK_F24` = `124`…`135` 是
`windows` crate `Win32/UI/Input/KeyboardAndMouse` 里的连续常量（可离线交叉验证），不是猜的硬件表。
地球键在旧版码空间里不存在，转换永不产出 `Globe.png`。

## 明确不做

- **不重命名 `Fn.png`**，理由见决策 2。
- **不为地球键新增任何预置美术**：两个预置键盘模型与 `gamepad` 都保持现状，按下地球键不产生任何动作
  （ADR-0042 的缺图规则）。
- **不加 Windows 映射**：Fn 由固件处理，Raw Input 不报告。
- **不猜 Windows 的 F13–F24 扫描码**：仓库此前已在 TODO `P4-MODEL-LEGACY-SOURCE` 判定"缺可依据的
  扫描码，不猜值"，本次沿用；macOS 侧 Carbon 没有 `kVK_F21`…`kVK_F24`。
- **不改 `FUNCTION_KEY_USAGES` 的边界**：`Fn` 回退仍只覆盖 `0x3a..=0x45` 与 `0x68..=0x73`。

## 残余风险与待验证项（不得当作已确认）

1. **F21–F24 是"命名到位但两个平台都不可达"**：Windows 扫描码不补（见上），macOS 没有对应 keycode。
   词表保留它们是为了让模型可以先画图，不是声称平台能报。
2. **macOS 实机地球键未验证**：`CGEventSourceKeyState(63)` 的周期校正是否会误释放地球键，只有合成事件
   证据（见验证），真实 HID 状态没测过。
3. **`Function.png` 兼容路径只有单元测试覆盖**：没有真实用户数据根里的旧模型样本，`Function.png` 在
   野外是否存在也不确定（它从未随任何已知模型出厂）。
4. **`Fn` 这个名字仍然容易误读**：它读起来像 Fn 键。本 ADR 选择不动它（改名会回归现存模型），代价是
   文档必须反复强调；`bongocat-live2d::key_name_candidates` 的文档注释已写明这一点。
5. **地球键归左手**：与其余键盘区一致，但爪部落点没有实机观感确认。
6. **`bongocat-overlay` 的预览工具不认地球键**（`preview_input_bindings`，
   `macos/switch_preview.rs` / `windows/switch_preview.rs`）。该表停留在 ADR-0041 之前的形态，
   连 F1–F12 都没有，也不感知键位图，
   ADR-0042 残余风险 4 已记录它是人工诊断工具、本次未收敛。`just preview` 因此不覆盖地球键。

## 验证

已完成（2026-09-19，本机 macOS / aarch64）：

- `bongocat-live2d` 55 测试（新增 2）：
  `the_globe_key_never_shares_an_image_with_the_function_row` 断言 `0xff03` 的候选恰为
  `["Globe", "Function"]`、F1–F24 的候选不含 `Globe`/`Function`，并用合成资源断言 `Fn.png` 画不出
  地球键、`Globe.png` 与 `Function.png` 都画不出功能键；
  `every_key_image_name_the_conversion_can_install_resolves_to_a_key`（见 ADR-0050）。
- `bongocat-platform` 62 测试（新增 1）：
  `this_adapter_reports_f1_through_f20_and_the_globe_key_only` 遍历全部 `u16` keycode，断言可达功能键
  恰为 `0x3a..=0x45` ∪ `0x68..=0x6f`（F1–F20），且 `0xff03` 是唯一不在 `0x07` 页的产出；
  `mac_key_codes_map_to_usb_hid_usages` 补 `map_key_code(63) == Some(PhysicalKey::GLOBE)`。
- `bongocat-runtime` 74 测试（新增 1）：
  `the_globe_key_projects_through_the_model_snapshot_like_any_other_key` 走完
  input → binding → `model_snapshot`，证明这条路径不按下标索引 usage、不窄化成 `u8`，且未绑定时完全惰性。
- `bongocat-app` `--lib`（新增 1）：`the_globe_key_binds_only_for_a_model_that_ships_its_image` 断言
  四个预置/已安装模型都不绑地球键，而提供 `Globe.png` 或 `Function.png` 的模型绑到左手。
- `bongocat-model`（新增 2）：`the_globe_key_image_is_renamed_from_its_pre_rename_stem` 断言
  `Function.png` → `Globe.png` 且 `Fn.png` 原样保留。
- **变异验证**：6 处改动逐个还原（删命名 arm、删 keycode arm、删归一化项、把 `BackSlash` 改回旧拼写、
  删 F13 arm、删绑定语句），对应测试全部变红（exit 101），随后逐个还原并复核文件内容一致。

**未运行**：macOS 实机地球键与爪部观感、Windows 实机、真实社区模型回归、UI 实机点击。

---
<a id="adr-0050"></a>
# ADR-0050: 转换输出的键位图名必须用产品拼写

状态：已接受（2026-09-19）
依赖：ADR-0037（应用内导入 BongoCatMver 模型）、ADR-0038（左右 Alt 的键位图命名与旧名兼容）、ADR-0039（主 Enter 与小键盘 Enter 的键位图命名与旧名兼容）、ADR-0049（地球键与 `Fn` 的语义分离）

## 背景

ADR-0049 要求"键位事件、键名、资源名称、图片回退、BongoCat 模型导入与 Mver 转换之间的映射完全一致"。
把这句话做成可检验的检查时，发现两张表已经各自演化出了一处拼写漂移。

决定本 ADR 形态的事实：

1. **漂移是程序化 diff 找出来的，不是肉眼看到的。** 正则抽出 `mver::legacy_virtual_key_name` 与
   `live2d::key_name_candidates`（含 `KEY_LETTERS`、`KEY_NUMBERS`、`KEYPAD_DIGIT_NAMES`、
   `bongocat-render::FUNCTION_KEY_NAMES`）里所有 `"名字"` 字面量求差集：**114 个转换输出名里恰好一个
   漏网——`Backslash`。**
2. **漏网的那个是死键。** `mver.rs` 的 `0xDC => Some("Backslash")` 照抄参考工具
   `BongoCat-Converter/src/utils/keyMap.ts` 的 `220: "Backslash"`，而产品词表是
   `0x31 => Some("BackSlash")`。转换安装的 `Backslash.png` 从功能上线起就永远选不中；按 ADR-0042，
   该键**完全不产生动作**——不画图，爪子也不下压。这与 ADR-0038 记录的 `AltGr` 是同一类错误。
3. **只差大小写的改名在大小写不敏感的文件系统上是空操作。** macOS 与 Windows 都不区分路径大小写，
   所以 `Backslash.png` → `BackSlash.png` 的"改名"里 `destination.exists()` 恒为真（目标其实就是同一个
   文件），归一化直接跳过，包保留旧拼写。真正让这类包能画的是**运行时候选别名**，归一化只在大小写
   敏感的文件系统（非首发平台）上才真正改名。
4. **只断言"能解析"的契约测试挡不住这类漂移。** 别名一旦存在，旧拼写也能解析，于是"转换输出能解析"
   这条断言在漂移回退时仍然通过。变异验证证实了这一点：先把 `BackSlash` 改回 `Backslash`，只查
   "能解析"的版本**没有变红**。

## 决策

### 1. 转换输出改产品拼写

`legacy_virtual_key_name` 的 `0xDC` 输出 `BackSlash`。这与 ADR-0038 的 `Alt`/`AltGr` → 产品名、
ADR-0039 的 `Return` → `Enter` 是同一套做法：转换写产品词汇表，旧拼写由运行时候选兜底。

### 2. 运行时候选把旧拼写加成末位别名

```text
0x31 (BackSlash) → BackSlash, Backslash
```

canonical 在前。已经安装过、不会被重新归一化的包（无迁移路径）因此仍然能画。

### 3. 导入归一化登记该拼写

`LEGACY_KEY_IMAGE_NAMES` 加 `("Backslash", "BackSlash")`。按事实 3，它在两个首发平台上通常是空操作；
登记它的价值是让"旧拼写 → canonical"这件事只有一个来源，而不是散落在运行时别名里。

### 4. 把"转换能安装的名字"做成可遍历契约，并同时断言两件事

- `bongocat_model::legacy_keyboard_key_image_names()`：遍历两种键盘模式的全部虚拟键码，返回去重排序后的
  输出名集合。手柄按钮名**不在**其中（它们是 gamepad 模型的美术名，不进入键位图，ADR-0037）。
- `bongocat-live2d::every_key_image_name_the_conversion_can_install_resolves_to_a_key`：对每个输出名断言
  **既能解析、又是 canonical 名**（即某个可达 usage 的候选列表第一项）。只查前者不够（事实 4）。
  `Shift` 与 `Control` 是**有意**的非 canonical 名（旧表给两侧同一个码，ADR-0038 决策 4），用显式例外
  列表记录，并断言这两个例外仍然必要，避免列表变成死代码。

这样下次漂移会被测试发现，而不是被用户发现。

## 明确不做

- **不把游戏按钮名纳入该契约**：它们不进入键位图解析器（ADR-0037）。
- **不新增第三套拼写**（如 `BackSlashKey`）：ADR-0038 已经明确不做同类事。
- **不把 `Shift`/`Control` 展开成两侧**：那会同时作废 ADR-0037 记录的真实样本转换证据。

## 残余风险与待验证项（不得当作已确认）

1. **已安装的旧包靠别名工作，磁盘上没有被重写**：`Backslash` 是运行时候选表的一部分，属于永久词汇。
2. **例外列表是硬编码的两个名字**：如果将来 `Shift`/`Control` 有了 canonical 拼写，例外断言会先变红
   提醒删除，但删除本身是人工动作。
3. **该漂移没有真实样本覆盖**：仓库里唯一真实 Mver 样本（`bongo_cat_mver_0.1.6_64`）的键位表不含
   `0xDC`，所以这条路径只有合成测试。
4. **大小写不敏感文件系统上的行为只在 macOS 上验证过**：Windows 侧同样是大小写不敏感，但没有实机跑过
   这条归一化。

## 验证

已完成（2026-09-19，本机 macOS / aarch64）：

- **先红后绿**：修复前 `every_key_image_name_the_conversion_can_install_resolves_to_a_key` 变红并精确
  报出 `no key resolves these conversion outputs: ["Backslash"]`；修复后绿。
- **变异验证（关键）**：把 `0xDC` 改回 `Backslash` 后该测试变红（exit 101），证明断言有牙齿；同时确认
  了"只断言能解析"的初版**不会**变红，因此把断言加强为"同时是 canonical 名"。
- `bongocat-model-store` 新增 `the_conversion_emits_the_product_spelling_for_every_key_image`：对
  `BackSlash`/`Backspace`/`Enter`/`AltLeft`/`AltRight` 成对断言"新拼写出现、旧拼写不出现"，并断言输出名
  都是合法资源主干。
- `bongocat-model-store` 新增 `a_case_only_legacy_stem_keeps_its_artwork_under_the_canonical_name`：断言
  canonical 名可读到该美术（**不断言旧名消失**——在大小写不敏感的文件系统上它本来就不会消失）。
- `cargo test --locked -p bongocat-model-store -p bongocat-live2d` 全绿。

**未运行**：真实 Mver 样本回归（样本键位表不含 `0xDC`）、Windows 实机归一化路径。

---
<a id="adr-0052"></a>
# ADR-0052: 快捷键门禁各自跟随所属分组，并新增窗口快捷键开关

状态：已接受（2026-09-21）
依赖：ADR-0002（GPUI 设置界面）、ADR-0044（全局热键注册后端）

## 背景

维护者反馈（2026-09-21）：模型行为快捷键的开关在「交互」页面，而它控制的绑定列表在「快捷键」页面，
两处隔着一整页，用户可能感知不到"这些组合键为什么按下去没有反应"；要求把快捷键相关的开关收敛到
「快捷键」页面，并让两个作用域各自管理自己。

决定本 ADR 形态的事实：

1. **开关与它控制的内容不在同一页。** `shortcuts.model_behaviors_enabled` 渲染在「交互」页「模型」
   分组的第一项，而 `shortcuts.model_behavior_bindings` 的行渲染在「快捷键」页「模型行为快捷键」分组里。模型
   默认关闭（`false`）加上自动分配会写满绑定，于是新安装的常态是"页面上一列组合键，按下去没有任何
   反应"，而解释这件事的开关在另一页。第 91 项当时把这一点记为"未决观察"，并说明"若维护者认为需要
   在快捷键页加一行提示，那属于独立的 UI 变更"——本 ADR 就是那次决策。
2. **两个作用域本来就是并列的。** 快捷键页面由两个带标题的 group 组成（窗口 / 模型），各自还是侧边栏
   的二级入口。每个 group 的第一行是"这一半的开关"的天然位置，不需要新增第三个分组（那会多出一个
   二级入口）。
3. **应用级快捷键此前没有门禁。** Technical Design 原文要求模型开关"不得禁用 `open_settings`、
   overlay 显隐、镜像、穿透或置顶等应用级快捷键"。维护者要求"两个模块分别单独自己去管理"，因此
   应用级快捷键需要它自己的开关，而不是被模型开关连坐——原约束因此只在模型开关上保留。
4. **门禁的既有语义是"只过滤、不改写"。** `shortcuts.model_behaviors_enabled` 关闭时不删除绑定，只把它们
   排除在平台匹配表之外，重新打开无需重录。新门禁沿用同一语义，这也是"关掉不等于清空"能被用户接受的
   前提。
5. **`ShortcutConfig` 的派生 `Default` 会成为陷阱。** 新字段若按"禁用"极性命名（`disable_commands`），
   派生 `Default` 恰好给出正确结果，但与 `shortcuts.model_behaviors_enabled` 的正向命名分叉；若按"启用"极性
   命名，派生 `Default` 会把全新配置判成"窗口快捷键已关闭"。两者都必须显式处理。

## 决策

### 1. 每个作用域的第一行是它自己的"启用"开关

`shortcuts_page::ShortcutScope` 新增两个方法，把开关需要的知识全部收进作用域本身：

- `gate_label(language) -> &'static str`：该作用域的开关标题。可见开关与辅助功能节点读同一个
  返回值，页面上不再出现第二份措辞。
- `is_enabled(snapshot) -> bool`：该作用域在配置里的真值，直接来自 `commands_enabled` /
  `shortcuts.model_behaviors_enabled`，不做任何取反。

`group()` 把 `gate_item()` 作为该分组的第一个 item，行列表仍是原来的自定义元素。开关标题是
`启用窗口快捷键` / `启用模型行为快捷键`，**只带标题、不带描述**：第 97 项已经把"标题已能表达清楚的
设置项不再重复写一遍描述"定成规则，"启用×××"正属于这一类（维护者原话：标题就已经表达了足够的
意思）。

### 2. 配置新增正向字段 `shortcuts.commands_enabled`，默认 `true`

- 字段与 `shortcuts.command_bindings` 同段，命名与数组对齐；`ShortcutConfig` 改为手写 `Default`，使全新
  v1 配置是"窗口快捷键生效、模型行为快捷键不生效"——与改动前逐字段一致。
- `canonicalized()` 原样透传该字段：设置页面提交绑定表时不得顺手把它改回默认。
- 应用层 `shortcut_config_from_settings` 多接一个 `commands_enabled` 参数，由调用方（`set_shortcuts`、
  `suspend_shortcut_capture`）从当前配置传入。录制或清空快捷键不会把窗口快捷键在用户背后重新打开。

### 3. 门禁的唯一实现点是 `ShortcutConfig::active_bindings`

`active_bindings` 是"此刻生效的绑定"的唯一投影（模型侧的活动模型过滤也在同一处），命令侧门禁加
在这里，应用层与平台层不再判第二次。关闭时 `commands` 投影为空，`model_behaviors` 与配置里的绑定
一律不动。

### 4. 开关直接显示配置真值，任何一层都不取反

配置、`SettingsSnapshot`、settings command、`SettingValue`、`PendingOperation` 全是正向字段
（`commands_enabled` / `command_shortcuts_enabled` / `SetCommandShortcutsEnabled`），开关也按"启用"
方向渲染，因此可见开关、辅助功能节点的 `toggled`、开关发出的命令读的是同一个布尔值——不存在"某个
地方取反了没有"这类问题。开门时按"禁用"方向渲染的初版方案（取反收在 `is_disabled` 一处）已被替换：
两个开关各自只带标题，正向直出既更短也更好读。

### 5. 节点顺序跟着页面走

两个门禁节点从「交互」页的节点块移到「快捷键」页的节点块，并按页面顺序插在各自作用域的行之前：
窗口门禁、窗口行、模型门禁、模型行。行的切分点用 `window_shortcut_rows(...).len()`，与页面的
`row_index_offset`、tab 序号、无障碍节点编号同一条行序。

## 明确不做

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

## 后果

- 快捷键页面每个分组的第一行是开关（纯标题行），其后是该作用域的行；「交互」页不再出现任何快捷键开关。
- 用户可以在不丢失绑定的前提下把窗口快捷键整体关掉。**这是新增能力**：此前应用级快捷键只能在
  逐个清除绑定（不可逆，且模型侧还会被下次激活补回来）之后才失效。
- `shortcuts.commands_enabled: false` 会随配置落盘并跨重启保持；重启、切换模型、重新录制绑定都不会
  把它改回 `true`。
- 无障碍树里「交互」页少一个 Switch、「快捷键」页多一个，两个门禁节点的标题与可见开关同源，
  且节点没有 value（行没有描述，读屏也不会把描述当值念出来）。
- `spikes/config-store` 的配置副本需要同步该字段，否则它那条与共享 fixture 对拍的测试会失败
  （spike 是独立 workspace，不在 `just check` 范围内）。

## 残余风险与待验证项（不得当作已确认）

1. **没有断言"侧边栏仍渲染出两个二级项、正文分组顺序不变"**——本仓库的 smoke 不做布局断言
   （与第 95 项同一限制），分组数量与顺序都没有变化，靠组件既有机制与人工查看。
2. **未做视觉确认**：开关在分组标题下方与行列表之间的间距、`Enable window shortcuts` 这类较长的
   英文标题在窄窗口下的换行都没有截图证据；125/150/200% DPI 同此。
3. **未在 Windows 实机验证**。改动落在配置、应用层与 UI 层，没有改平台代码；`cfg(windows)` 的
   无障碍与设置窗口路径本机无法执行。
4. **门禁关闭状态下再录制绑定**只由 `the_command_gate_keeps_the_recorded_bindings_and_only_leaves_the_table`
   间接覆盖（关闭 → 表为空 → 打开 → 恢复）。"关闭期间录制新组合键"这条组合路径没有专门用例。

## 验证

`the_command_gate_keeps_the_recorded_bindings_and_only_leaves_the_table`。

---
<a id="adr-0053"></a>
# ADR-0053: 「开关门禁」与被控设置项的统一禁用绑定规则

状态：已接受（2026-09-21；项目 AccessKit tree 部分由 ADR-0054 取代）
依赖：ADR-0052（快捷键门禁各自跟随所属分组）、ADR-0054（visual-first UI）

## 背景

维护者要求（2026-09-21）：快捷键页两个门禁开关（`启用窗口快捷键` / `启用模型行为快捷键`）中任意一个
关闭时，应自动禁用其作用域的全部设置项（置灰且不可交互），参照「鼠标悬停时隐藏延迟」开关关闭时禁用
延迟行的既有实现；并要求把这类"开关控制下属设置项"的场景收敛为一套统一标准，此后每遇到新开关不再
单独编写一次禁用逻辑。

决定本 ADR 形态的事实：

1. **参照实现本身是三层分散的逻辑。** 鼠标悬停时隐藏延迟的禁用规则由三处各自表达：渲染层的
   `.disabled(!hover_hide_delay_available)`、无障碍层的 `disabled || !hover_hide_delay_available`、
   变更方法 `adjust_overlay_hover_hide_delay` 的守卫。三处判的是同一件事，却没有任何一处命名它——
   第三个门禁出现时就会各自再写一遍并开始漂移。
2. **ADR-0052 明确决策过相反方向。** 其「明确不做」一节写明"不给行列表加禁用态样式"，理由是用户
   可能想先把组合键排好再打开开关。维护者现在的要求直接推翻这一条；按工作协议，本 ADR 显式取代该
   条目而非留下两份互相矛盾的文档，ADR-0052 的其余内容（每分组一个正向开关、只投影不清空、
   `active_bindings` 唯一实现点、全链路不取反）全部保持不变。
3. **自定义渲染行的"禁用"必须自己拼。** 打包字段（`SettingField::switch` 等）由
   `SettingItem::disabled` 一并处理置灰与不可交互；快捷键行是 `SettingItem::render` 的自定义元素，
   置灰只是 `opacity(0.5)`，`on_click` / `on_key_down` handler 仍会注册、命中测试仍有效（gpui-kit
   的 disabled 投影不吞事件）。"不可交互"必须通过不再注册 handler 来实现，这恰恰是最容易漏的一半。
4. **两种"禁用"来源是正交的。** 结构性编辑阻塞（无快照、配置不可用、模型导入进行中）与门禁关闭是
   两个独立布尔；渲染层与守卫层必须合成同一个值。若两层各判一次，任何一处漏加来源就会出现
   "看着灰但能改"或"能点但请求被拒"的不一致。
5. **`pending` 不能进门禁。** 保存往返期间的 in-flight 标志每次保存都会翻转，接入后整页会随每次
   控件变更明暗一轮，读作页面在刷新（ADR-0052 前身已踩过：d4f0155f）。

## 决策

### 1. 统一机制：`bongocat-ui` 的 `window::setting_gate::SettingGate`

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

### 2. 新场景的接入方式（标准流程）

1. 定义一个从 snapshot 读开关真值的正向谓词（如 `ShortcutScope::is_enabled`、
   `hover_hide_delay_applies`），全链路复用，不取反；
2. 渲染时为每个被门禁分组构造一个 `SettingGate`；
3. 开关行走 `disables_switch()`，被控行走 `disables_controls()`：打包字段用
   `SettingItem::disabled(...)`；自定义行**整行置灰（标签与控件一起，与 `SettingItem::disabled`
   的整行置灰一致——禁用的是"这条设置"，不是仅控件）**并停止注册交互 handler；
4. 无障碍节点对被控行报 disabled 且不加 `clickable().focusable()`；
5. 变更方法以同一谓词做守卫。

### 3. 快捷键页的落地

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

### 4. ADR-0052 的对应修订

ADR-0052「明确不做」中"不给行列表加禁用态样式"一条由本 ADR 取代（维护者 2026-09-21 决策变更），
该条内注明的理由（"先把组合键排好再打开开关"）不再是产品方向；该 ADR 其余决策不受影响。

## 明确不做

- **不改 runtime 投影语义。** 门禁的唯一实现点仍是 `ShortcutConfig::active_bindings`；本 ADR 只把
  UI 层的可用性投影对齐到同一开关真值，配置、命令通道、平台表均不变。
- **不在门禁关闭时清空或改写绑定。** 置灰只是 UI 投影；落盘配置不动。
- **不把 `pending` 计入门禁。** 防整页明暗刷新；保存指示器在头部状态栏。
- **不为打包字段重写禁用样式。** `SettingItem::disabled` 已同时处理置灰与字段不可交互，统一机制
  直接复用，不再自绘。
- **不为开关关闭加提示行。** 开关就在被控行上方，置灰即提示（延续 ADR-0052 的立场）。

## 后果

- 快捷键页两个门禁关闭时，各自作用域的行**整行置灰**（标签与控件一起）且不可交互；无障碍节点报
  disabled、不可聚焦；键盘 tab 跳过这些行；辅助技术触发的捕获/清除请求被守卫拒绝。
- 门禁关闭期间无法再通过 UI 录制或清除该作用域的绑定（ADR-0052 曾刻意保留这条路，本 ADR 收窄它）。
  绕过 UI 的路径（直接编辑配置文件）不受影响，配置层从不过滤或改写绑定。
- 鼠标悬停时隐藏延迟行与两个快捷键分组共用同一条规则与同一份实现，新增门禁场景按第 2 节流程接入。
- 捕获中途翻转门禁会取消捕获并恢复平台表（`resume_shortcut_capture`），不会留下悬空的录制会话。

## 残余风险与待验证项（不得当作已确认）

1. **未在 Windows 实机验证。** 改动全部在 `bongocat-ui`，无平台代码，但 `cfg(windows)` 的无障碍与
   设置窗口路径本机无法执行。
2. **无布局/视觉断言。** 置灰效果（`opacity(0.5)`，与 `editing_blocked` 既有禁用态一致）依赖组件
   机制与人工查看；125/150/200% DPI 无截图证据。
3. **无障碍树的门禁行为只有节点属性断言可覆盖**，读屏器实际表现（VoiceOver/Narrator 对 disabled
   按钮的播报）未实测。
4. **"关闭期间录制"被收窄**意味着喜欢"先排键再开闸"的用户失去这条路；若反馈需要恢复，可在
   `disables_controls()` 之外为捕获行单独放宽——那是独立决策，不预先实现。

## 验证

见 `disables_controls()` 相关用例与 ADR-0052 记录的验证状态。

---
<a id="adr-0057"></a>
# ADR-0057: 模型窗口用右键拖动缩放

状态：Accepted
日期：2026-09-23
修订（2026-09-24）：缩放写回配置不再重建 HWND/NSPanel；统一改为在现有原生窗口上更新几何，
并在尺寸变化后立即填充新的 swap-chain/drawable，避免设置更新暴露未初始化透明帧。
取代：无（恢复旧版 `pre-refactor` 分支存在、当前产品尚未实现的窗口缩放交互）

## Context

模型窗口尺寸只有一个来源：配置 `overlay.scale_percent`。在本 ADR 初始实现时，改变它意味着
**重建整个窗口**——`OverlaySessionOptions::requires_window_recreation` 把 `scale_percent` 列为重建条件，
重建会重新创建原生窗口与 GPU 资源，`GpuModel::prepare` 还会重新加载全部模型纹理。因此在这次改动之前，
调整窗口大小只有设置页滑块这一条路，而且每次都伴随一次可见的重建。

旧版（`pre-refactor` 分支 `src/pages/main/index.vue`）在主窗口上实现了这个交互：

```js
if (buttons !== 2 || !shiftKey) return          // 右键 + Shift
const delta = (movementX + movementY) * 0.5
catStore.window.scale = round(clamp(scale + delta, 10, 500))
```

即按住右键并移动指针即可改缩放，但要求同时按住 Shift，范围 `10–500`，并且直接改
`window.scale`（与设置页滑块同源）。本产品没有移植这个交互。

同时，右键已经被上下文菜单独占：Windows 拦截 `WM_CONTEXTMENU | WM_NCRBUTTONUP` 并转发
成 `OverlayContextMenuRequest`，macOS 用 `NSEvent` local monitor 监听 `RightMouseUp`，两者都交给
应用的 `muda` 菜单。任何新的右键交互都必须先解决这个冲突。

两个平台都没有窗口 resize 通路：渲染尺寸在窗口创建时固化进 swapchain/RTV/mask 纹理
（Windows）或 `CAMetalLayer` 的 drawable size（macOS），`overlay/src/windows/` 与
`overlay/src/macos.rs` 里此前不存在任何 resize 代码。

## Decision

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

## Consequences

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

---
<a id="adr-0066"></a>
# ADR-0066: gilrs 手柄后端与平台窄适配边界

状态：已接受（2026-09-25）；fork 已补齐 callback lifetime、bounded queue/epoch、authoritative reset、bounded shutdown、compile guard 与 xinput 回归修复；2026-09-26 追加 macOS IOHID worker run-loop 空转修复（空闲 CPU 98% → 0%）；2026-10-02 追加 WGI 焦点门控读取修复与 WGI raw analog axis 取值域修复；2026-10-05 追加 WGI raw element 顺序修复与 SDL 轴槽位翻译的实测回退（见 Issue #1099）；双平台物理设备与长期证据仍阻塞完成声明

## 背景

BongoCat 此前在 `bongocat-platform` 内分别维护 Windows XInput 轮询和 macOS
`GCExtendedGamepad` callback。两套代码重复承担设备发现、按钮映射、连接 generation、axis
归一化和 callback 生命周期，且产品兼容面被 XInput 0–3 slot 与 Apple extended profile 限制。手柄兼容、
驱动修复和平台 backend 问题应集中维护在第三方边界，而不是继续在 BongoCat 内扩展两套系统 API。

项目已有强类型 `InputEvent`、可靠 producer、generation-keyed gamepad-axis latest-value transport 和
runtime dead-zone；这些是产品语义，不能由第三方库类型替代。按 ADR-0030 的复用阶梯，应选择成熟的
跨平台 gamepad backend，再只保留 BongoCat 所需的窄适配。

## 决策

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

## 依赖审查

- `gilrs 0.11.2` 的许可证为 `Apache-2.0 OR MIT`，MSRV `1.84`，低于 workspace 的 Rust `1.97`。
- Windows WGI 和 macOS IOHID 的系统绑定及 unsafe 面积被限制在第三方包内；项目 adapter 保持
  safe Rust，第三方对象和错误不进入项目公共 API。替换边界是私有 `gilrs_gamepad.rs`，不会改变
  runtime/config/UI contract。
- 内置 SDL mapping 来自 fork 固定 submodule；该 database 有独立于 gilrs dual-license 的许可，
  构建和分发前必须把 `gilrs/SDL_GameControllerDB/LICENSE`、revision 和 attribution 纳入第三方
  合规清单。

## 已知阻塞

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

## 验证

- 共享 adapter 单元测试固定 16 个项目按钮、trigger 连续值与独立 axis、trigger 有限范围、四设备
  上限/容量拒绝、slot 复用 generation、runtime stop 错误、Reset 后同 generation 重播和 backend
  overflow recovery。
- fork 侧 `raw_axis_value_is_neutral_at_rest_and_spans_the_signed_range`、
  `raw_axis_value_stays_monotonic_across_the_whole_range`、
  `a_signed_raw_element_is_read_over_the_range_its_mapping_asks_for` 和
  `mapped_axis_info_leaves_an_already_matching_range_untouched` 固定 WGI raw analog axis 的取值域
  契约：静止读数为 0、两端到取值域端点、映射成按键时改按无符号域读取。`wgi` 与 `xinput` 两个
  feature 的 `cargo test` 均通过，`cargo fmt --all -- --check` 干净；`cargo clippy
  --no-default-features --features wgi --all-targets` 无新增告警（`gilrs/build.rs` 的
  `useless_borrows_in_formatting` 告警在 `master` 上已存在，不属本次改动）。
- 该修复只由纯函数回归测试覆盖。Issue #1083 与 #1099 报告的 Switch 模式手柄"模型鬼畜"是否由取值域
  与 element 顺序两处缺陷完全解释，仍需 Windows 10/11 实机加该手柄确认；未取得该证据前不得声称手柄
  输入完成。
- gilrs fork 的 queue/epoch、authoritative reset、WGI/XInput shutdown acknowledgement、macOS callback
  ownership/close 与 xinput extreme-axis test 已通过 fork 的 fmt/check/clippy/test；BongoCat Windows
  `cargo check/clippy/test`、WGI 无设备 context 初始化/关闭 smoke，以及 macOS
  `x86_64-apple-darwin` cross-check 已通过。macOS target 不再启用临时 `wgi` feature。
- fork 侧 `a_device_with_a_raw_report_is_polled_without_the_foreground_window` 与
  `a_device_without_a_raw_report_falls_back_to_the_mapped_reading` 固定读取来源的选择规则：设备只要
  暴露任何 raw report 就走不受焦点门控的 `RawGameController`，完全没有 raw report 才回退 mapped
  读取。焦点门控本身是 Windows 行为，无法用单元测试断言；该修复仍需 Windows 10/11 实机加 XInput
手柄确认背景投递。未取得该证据前不得声称焦点矩阵已验证。
- 共享 adapter 侧新增 `the_shoulder_and_the_analog_trigger_keep_their_own_buttons_and_artwork`
  与 `only_the_analog_trigger_reports_a_continuous_axis`，逐个名字钉住第三方
  `LeftTrigger`/`RightTrigger`（肩键）与 `LeftTrigger2`/`RightTrigger2`（模拟扳机）到项目
  shoulder/trigger 按钮与美术名的对应，以及只有模拟扳机同时是连续轴；这是 ADR-0070 修过的
  互换关系的防回归。
- fork 侧 `the_position_names_carry_the_windows_gaming_input_axis_order`、
  `the_face_buttons_carry_the_windows_gaming_input_button_order`、
  `a_device_that_reports_its_dpad_as_buttons_reaches_every_direction`、
`the_raw_elements_are_exposed_in_the_order_the_device_reports_them` 与
  `every_code_the_mapped_reading_emits_is_in_the_element_list` 固定 WGI raw element 顺序契约：
  位置名携带 WGI 原始下标、十字键落回真实下标、raw element 按设备报告顺序呈现且每个下标都可达、
  mapped 回退路径发出的每个 code 都在 `BUTTONS`/`AXES` 里。五处变异（把左扳机放回 evdev 下标、把面键
  放回 evdev 下标、把十字键推回表外、让轴下标整体偏移、把某个 code 从 `BUTTONS` 里换成重复项）分别
  让对应测试变红。fork 的 `cargo test --workspace --features wgi` 与 `cargo clippy --workspace
  --all-targets --features wgi` 通过；轴顺序依据 Microsoft `Raw game controller` 文档，按键顺序
  依据实机报告顺序，两者都仍需 Windows 10/11 实机逐键确认。
- 原 standalone XInput/GameController 手柄 probe、依赖、命令和 CI smoke 已删除；键鼠 Raw Input /
  CGEventTap spike 保留。物理 WGI/IOHID 矩阵、真实设备、物理 profile、热插拔和生命周期矩阵继续作为
  发布门禁。

## 被拒绝方案

- **继续维护平台 XInput/GameController backend**：拒绝，因为它重复设备发现、mapping 和生命周期
  修复面，并继续把驱动兼容问题锁在 BongoCat。
- **让 runtime/UI 直接消费 gilrs 类型**：拒绝，因为它破坏强类型项目 contract、第三方替换边界和
  runtime 单一状态所有权。
- **在 BongoCat 内修复 gilrs 的 macOS 生命周期、队列或 WGI 焦点问题**：拒绝；这些是 fork/backend
  问题，应在 fork 修复、测试并精确升级 commit。
- **用 crates.io `gilrs 0.11.2` 代替 fork**：拒绝；当前需要 fork 中与 D-pad/平台行为相关的修复，且
  后续兼容修复统一在 fork 维护。

---
<a id="adr-0068"></a>
# ADR-0068: 托盘与模型窗口右键菜单复用

状态：已接受（2026-09-25）

修订：2026-09-25 根据产品反馈调整为托盘与模型窗口右键共用同一套菜单；取代本文早期“拆分两棵菜单树”的方案。

## 背景

托盘菜单和模型窗口右键菜单都代表 BongoCat 的模型窗口控制面。此前曾尝试按入口拆成不同的菜单树，
但这会让同一组模型窗口操作在两个入口出现不同的层级和状态，增加维护与验收成本。

本次调整只改变菜单树的复用方式，不改变应用状态的所有权。平台仍只产生强类型 `SystemMenuAction`，
overlay 仍只发送右键请求，runtime、settings service 和 renderer 不感知菜单树。

## 决策

### 一棵 popup 根，一个 owner

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

### 动作和状态边界

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

### 生命周期不变量

- `TrayIcon` 字段必须先于菜单根、`Submenu` 和菜单项声明；显式 shutdown 先隐藏托盘，再按既定顺序停止
  input/runtime/frame/renderer/overlay。
- overlay 右键只使用真实 Windows HWND 或 macOS content `NSView`，不借用托盘隐藏窗口，也不在 overlay
  内创建第二个 owner。
- 菜单根、模型窗口子菜单和所有更新/文字/勾选操作都在平台 UI 主线程执行。菜单 tracking 期间只允许把
  事件送入既有有界队列，不能在 callback 内销毁 `SystemMenu` 或 overlay。
- `muda` 和 `tray-icon` 的版本、features 与替换边界继续由 ADR-0031 固定；第三方类型不进入公共业务 API。

## 验证

- 平台 crate 单测固定当前 action id 集合，并断言显隐、穿透、置顶、鼠标悬停隐藏、设置、更新和退出
  action 均存在；已删除的源码、重启和版本 id 不会产生 action。
- 菜单布局 contract 断言托盘和模型窗口右键共用同一组根项，并且模型窗口子菜单包含四个 check item。
- locale 双向 key 守门确保 `navigation.model_window.title`、偏好设置已有的显隐/穿透/置顶/鼠标悬停文案，
  以及 About 已有的 `update.about.label` 可解析；菜单不新增重复文案 key。
- macOS/Windows release system-menu smoke 继续覆盖托盘显隐、设置恢复、显隐 action、runtime snapshot
  变化和有序退出；它不替代真实 popup 展开与点击验证。
- 发布前仍需在两个目标平台实机确认：托盘与模型窗口右键展示同一层级、子菜单展开、cursor 定位、
  DPI/Retina、点击外部关闭、菜单项 action 派发、Explorer/菜单栏恢复和 shutdown 清理。

## 替换边界

替换点只有 `crates/bongocat-platform/src/system_menu_native.rs` 与 overlay 的 `HasWindowHandle` 实现。
升级 `muda`/`tray-icon` 时必须重新验证共用菜单根、菜单项父级关系、析构顺序、同一个 `MenuEvent`
receiver、Windows GUID/tooltip 缺陷、主线程约束、macOS `NSView` 生命周期和两个平台的实机菜单层级；
不能退回由 overlay 持有菜单或把第三方类型泄漏到 runtime/UI。

---
<a id="adr-0070"></a>
# ADR-0070: 手柄按键图片词表与按键层解析通道

状态：已接受（2026-09-26）

## 背景

用户报告：手柄模式下按下按键后模型没有显示对应按键图片。排查后发现的问题不是单点映射错误，
而是整条通道缺了一段，外加三处互相独立的命名缺陷。

**1. 手柄按键从来没有到达过按键层。** `bongocat_render::KeyPress` 只携带 `hid_usage: u16`，
`InputState::model_snapshot_with_filter` 遇到 `InputControl::Gamepad` 时只设置
`left_hand_down`/`right_hand_down` 或 `stick_*_down`，从不产生 `KeyPress`。手柄按钮没有 HID
Keyboard/Keypad usage，所以在这套类型里根本无法表达。后果是：手柄模式移动爪子，但**任何**按键
都不画图——与用户"部分按键不显示"的印象一致（实际是全部不显示，只是爪子动作让人以为按键生效了）。

**2. 预置模型的手柄图片名是第三方 backend 的词表。** 旧 Tauri 输入层用
`format!("{:?}", gilrs::Button)` 派生模型图片文件名，产物因此是 `South`/`East`/`West`/`North`、
`LeftTrigger`/`RightTrigger`（gilrs 语义是**肩键** LB/RB）、`LeftTrigger2`/`RightTrigger2`
（模拟扳机 LT/RT）、`LeftThumb`/`RightThumb`（摇杆键）、`DPad*`。项目自己的 `GamepadButton`
词表是 `South`/`East`/`West`/`North`/`LeftShoulder`/`RightShoulder`/`LeftTrigger`/`RightTrigger`/
`Select`/`Start`/`LeftStick`/`RightStick`/`Dpad*`。两套不一致，且旧词表里**一个主干承载两个语义**
（`LeftTrigger` 既是肩键名、又是产品扳机键名），任何"加个别名"的修法都会把其中一个按钮解析成
另一个按钮的图。

**3. 左右手归属是硬编码的，且与预置模型相反。** `bongocat-app::input_bindings_for_model` 对
`gamepad` 预置只写死两条：`South → Left`、`East → Right`。而预置模型把
`South`/`West`/`North`/`East` 和两个右扳机放在 `right-keys`，`DPad*` 和两个左扳机放在
`left-keys`——这是真实手柄布局，也是旧实现采用的判定方式（旧实现从图片所在目录推手）。旧实现的
`lefthand`/`righthand` 语义与此完全一致，Mver 转换也正是按它决定输出目录。

**4. Mver 的 XInput 按钮序号表有六个条目按错位置。** `legacy_gamepad_button_name` 写的是
8→`LeftThumb`、9→`RightThumb`、10→`DPadLeft`、11→`DPadRight`、14→`Start`、15→`Select`。
XInput（以及 gilrs 的 `Button` 顺序）是 8=Select、9=Start、10=LeftThumb、11=RightThumb、
12=DPadUp、13=DPadDown、14=DPadLeft、15=DPadRight。表是照着第三方词表的排列抄的，不是照着
按钮排列抄的，所以一个手柄按键转换出来的模型会把图装到错的按钮名下。

**5. 预置模型根本没有 `Select`/`Start`/两个摇杆键的图。** 即使通道修好，这四个按钮也只能保持惰性。

## 决策

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

## 后果

- 手柄模式下按下面板键、肩键、扳机和方向键都会显示对应图片，并抬起该手爪子；不画图的是模型
  没有图片的按钮（预置模型缺 `Select`、`Start`、两个摇杆键）。
- 已安装的旧版游戏手柄模型：只要经过一次导入，归一化会把旧主干改成产品名并从按 F 失效变为可用。
  手工拷入 store 的旧目录按上面"不冲突的那部分可达"的规则降级。
- 预置模型文件名变化会影响手工放置同名文件的用户，CHANGELOG 记录为用户可见修复。
- `KeyPress` 的字段从 `hid_usage` 变为 `key`，`bongocat-render` 的公共 API 有一处不兼容变更；
  它不进入配置或持久化格式，`schema_version` 仍是 1。
- 剩余门禁不变：Windows WGI 焦点矩阵与双平台物理设备、profile、热插拔和生命周期矩阵仍是
  ADR-0066 的完成门禁，本次修复不提供任何实机证据。

## 验证

- `bongocat-input`：16 个按钮的名字两两不同、与变体名逐字相同、都是可移植文件主干。
- `bongocat-render`：手柄身份与键盘身份永不相等；`KeyPressSet` 的去重与容量对两族一致。
- `bongocat-live2d-render`：预置 `gamepad` 模型的每一个有图按钮都能从它所在的那一侧解析到正确
  文件；没有图的按钮不解析到任何东西。
- `bongocat-runtime`：绑定的按钮同时产生爪子和自己的按键层；未绑定的按钮完全惰性；摇杆键的
  参数独立于爪子；两个来源门禁各自带走自己的按键层。
- `bongocat-app`：`gamepad_button_presses_reach_the_render_frame_as_key_overlays` 是整条链路的
  端到端断言——在真实激活的预置模型上按下面板键、肩键、扳机和方向键，逐个断言 runtime 快照里的
  按键层、爪子方向，以及 renderer 收到的帧确实从模型自己的文件解析出该按键图；没有图片的四个按钮
  断言为惰性。手绑定由模型目录得出，预置 `gamepad` 模型的四个面部键是右手。
- `bongocat-model-store`：旧 gamepad 主干归一化到产品名且两个扳机/肩键的图不互换；已经是产品名
  的包不被改写；只改大小写的条目在所有平台真的改名；XInput 序号表逐条等于真实按钮顺序。
- **Mver 转换与预置模型同名**：一个绑定全部 16 个 XInput 按钮的 legacy 源经真实转换后产出 16 个
  互不重名的产品名文件，每个都在自己的手列表指定的目录里、携带该按钮自己的合成图，且共享
  keyboard 图集的右手偏移落在正确的下标上。`bongocat-app` 的导入测试另外断言转换结果的键位图
  集合逐个等于产品按钮名，并按目录绑定到对应手——这正是"预置模型改名"与"转换输出命名"必须一致
  的那一条。
- 共享 fixture 逐 checkpoint 断言 `activeKeyOverlays`，`gamepad-reconnect-reset` 与
  `input-recovery-lifecycle` 覆盖手柄；`keyboard-modifiers-and-repeat` 顺带固定"同一只手只画最后
  按下的那个键"。
- `spikes/model-package` 的 `preset-model3-index.json` 同步更新并逐字段相等。
- 变异验证：把 `InputState` 的手柄手绑定改成永远 `None`，以及把
  `GamepadButton::key_image_name` 的 `LeftTrigger` 改成 `LeftTrigger2`，两层各自把
  `bongocat-app` 端到端、`bongocat-runtime`、`bongocat-live2d-render`、`bongocat-input`、
  `bongocat-model-store` 和共享 fixture 的对应测试全部变红。

---
<a id="adr-0071"></a>
# ADR-0071: Gamepad-Connection Model Auto Switch

状态：已接受（2026-09-26）

## 背景

模型目录按输入模式分为 `standard`、`keyboard` 和 `gamepad` 三类。用户在手柄和键鼠之间
切换时，需要手动到模型库重新选择模型；旧版没有对应能力，这是按维护者决定新增的
产品行为。

三条约束决定了它不能只做成 UI 行为：

- 模型加载是阻塞文件与 GPU 工作，UI executor 不执行；模型切换还必须走
  prepare → validate → commit 两阶段提交，只有 Application owner 能发起。
- 手柄连接状态的事实来源是 runtime 的 `InputState`：它按 connection generation 处理
  可靠 `GamepadConnected`/`GamepadDisconnected`，并在 Reset 后清空。上层只能读它的
  `connected_gamepad_count`，没有第二条判定链。
- 配置写入归 settings service worker 独占，platform 层不得直接改 overlay 或设置状态。

因此需要一个「谁观察、谁决定、谁写入」的完整通路，而不是在设置页轮询。

## 决策

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

## 验证

- `bongocat-config` fixture 覆盖默认关闭 + 两个 `null` 目标、可移植 id 的接受与拒绝，
  JSON Schema 与 Draft 2020-12 fixture validator 同步。
- `bongocat-app` 覆盖两条链：`the_gamepad_auto_switch_follows_the_last_used_model_of_each_family`
  验证门禁关闭不动模型、启用后按族记忆、显式目标优先、切换被持久化；
  `a_gamepad_connection_notice_switches_the_model_the_settings_service_owns` 走完
  「frame source 通知 → settings service 读取 runtime 答案 → 模型落定」并断言
  `selected_model` 与诊断计数。
- `bongocat-app` 单元测试固定 frame source 的过渡检测与重试，以及删除模型后两个配置
  目标被清除、内置同 id 模型不受影响。
- `bongocat-ui` 覆盖两个下拉的模式过滤、只列 `Ready`、`null` 恒为首项、悬空目标可见
  且不重复、空目录仍提供默认项；settings smoke 断言新增双语 key 非空。
- 变异验证：把 `connected_gamepad_count > 0` 改成 `>= 0`、把 `enabled` 门禁反向读取、
  或让下拉列出全部模式，都会让上述测试变红。
- 未运行：Windows 10 1903+ 与 macOS 12+ 实机手柄热插拔矩阵、长时间反复插拔、锁屏与
  睡眠中的连接变化，以及双平台高 DPI/Retina 目视检查。这些仍是 ADR-0066 的完成门禁，
  本 ADR 的自动化证据不替代它们。

## 后果

手柄成为「换一个模型」的显式动作，而不需要用户在模型库里找当前模式。默认配置保持关闭
且不预设任何模型，因此新装应用不会因为插上手柄而突然换模型；用户打开开关后，第一次
连接若该族还没有使用记录，切换不发生，直到用户自己用过一次该族模型或在下拉里显式
指定。设置文件因此仍然只有一个模型选择事实来源：自动切换与手工选择写同一处。

---
<a id="adr-0079"></a>
# ADR-0079: 按键层按按下顺序叠放全部按住的键

状态：已接受（2026-10-02）
补充：ADR-0042（按键必须有自己的键位图）、ADR-0060（模型包 store 边界）

## 背景

Issue #965 报告：同时按下 A 和 D 时只显示一张图片，「能不能怎么调试一下让一起显示呢」，并问设置里
为什么没有单键模式开关。issue 下的讨论把理由说清楚了——直播时手法很快，「一瞬间三四个键按出去」，
观众如果只看到一张图片就分不清是连击还是外挂。

现状是**两处各自独立地把多键折叠成单键**：

- `bongocat-runtime` 的 `InputState::model_snapshot_with_filter` 对每只手只保留一个
  `latest_left_key` / `latest_right_key`，其余按键直接丢弃，因此 `KeyPressSet` 最多两个元素。
- `bongocat-live2d-render` 的 `resolve_key_overlays` 又用 `let mut selected = [None, None]` 按手
  折叠一次，输出固定是 `[左手, 右手]`。

管道本身其实早就准备好了：`KeyPressSet` 是 64 槽的有序集合且 `push` 保留插入顺序，
`RenderSnapshot::active_keys` 是 `Vec<KeyOverlay>`，`GpuModel` 存的是 `Vec`，双平台渲染器都按
`active_keys` 顺序遍历。也就是说**只有内容被限制为 2，载体一直是可叠放的**。

z 序不需要新增任何机制：Windows 侧在画背景前把 `OMSetRenderTargets` 的 depth-stencil view 置为
`None`，整个渲染器也从不创建 `ID3D11DepthStencilState`，按键层循环因此继承那个空绑定；Metal 侧的
render pass 只挂 color attachment，从不创建 depth texture。两边都纯靠提交顺序。
**`active_keys` 的最后一项就是最上层**。

需要一并解决的还有顺序本身。`InputState::pressed` 是 `BTreeMap<InputControl, PressedRecord>`，
迭代顺序是 HID usage 升序，从不是按下顺序；唯一的顺序信号是
`PressedRecord::pressed_at`（`MonotonicMillis`，毫秒精度）。旧代码的比较是
`record.pressed_at >= at`，所以**同一毫秒内的两次按下由 usage 大小决出胜负**，与「最后按下」这句话
无关。快速连击正好落在这个情形里。

## 决策

### 1. `model.show_all_pressed_keys` 是唯一的开关，默认关闭

放在 `ModelConfig`，`bool`、默认 `false`、正向语义（`show_…` 而不是 `single_…`），设置页呈现为
「同时显示所有按下的键」开关。

默认关闭是刻意的：叠放会让一个和弦在画面上明显变"挤"，而现有用户已经习惯了单图。把可见变化交给
一次显式选择，也保证这个字段不改变任何未选择它的用户的画面。

它是 `ModelConfig` 里**唯一带 `#[serde(default)]` 的字段**。v1 解析入口是严格的（`deny_unknown_fields`
且字段缺失即失败），所以一个必填新字段会让该字段加入之前写下的每一份 `config.json` 都解析失败，
进而走「最新有效备份 → 默认配置」的恢复流程并丢掉用户设置。缺少该字段的旧文档按兼容模式加载，
不视为一次新的开启。

### 2. 按下顺序用 `pressed_at` + 可靠输入队列的 sequence 构成全序

`PressedRecord` 增加 `pressed_sequence: u64`，取自按下该控件那条边沿的 envelope sequence，并随
`apply_event` 的参数一起传入。比较键变成 `(pressed_at, pressed_sequence)`。

sequence 是进程内单调计数器，与输入队列给事件排序用的是同一个值，所以它和队列的顺序永远一致，
跨过毫秒边界也继续有效。**这同时修掉了兼容模式里的平局仲裁**：同一毫秒按下 `A` 再 `D` 时，
现在显示的是 `D`（后按下的），而不是 usage 更大的那个恰好也是后按到的巧合结果。行为文档
`shared/behavior/input-semantics.md` 原本就写的是「最后按下且仍有效」，实现此前并没有保证这一点。

### 3. 折叠只保留在 runtime 的投影里，renderer 不再按手折叠

`resolve_key_overlays` 改为对每个 press 解析出一个 overlay 并**保持顺序**。兼容模式的投影本来就是
「左手先、右手后」，所以逐个解析的结果与旧的 `[None, None]` 折叠**逐项相同**，兼容模式的画面不变。

两个按住的键可能解析到同一张图（`Fn` 家族回退、小键盘复用主键区图）。这种情况只画一次，保留先
解析到的那个在底层：重复绘制看起来一模一样，只是多一次 draw call，而先按下的那个本来就该在下面。

### 4. 手部参数与显示模式无关

只要有任意一个绑定到该手的键仍按住，`CatParamLeftHandDown`/`CatParamRightHandDown` 就是 true，
两种模式都一样。叠放是显示选择：它改变画几张图和叠放顺序，不改变爪子对单个按键的反应，也不触碰
pressed state、释放路径、reconciliation 或 Reset。

### 5. 溢出丢最早按下的那几个

`KeyPressSet` 的 64 槽上限原本就存在（防止设备把内存撑大），叠放模式让每个槽位都可能真的画出一张图。
投影在排序后只取最后 `KeyPressSet::CAPACITY` 个：**堆叠的底部是历史，顶层是用户此刻正在看的**，
所以宁可丢最早按下的。上限以 `KeyPressSet::CAPACITY` 命名暴露，而不是让投影硬编码数组长度。

## 后果

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

---
<a id="adr-0080"></a>
# ADR-0080: 按住一个物理修饰键临时恢复 overlay 交互

状态：已接受（2026-10-04）
补充：ADR-0053（「开关门禁」统一禁用绑定规则）、ADR-0057（右键拖动期间悬停隐藏不生效）、ADR-0079（按键层叠放）
实现：Issue #970

## 背景

Issue #970 报告：开启「鼠标穿透」和「鼠标悬停时隐藏」之后，窗口既抓不住也看不见，唯一能拖动它的
办法是先关掉这两项、拖过去、再打开，而报告者需要的是**一个能按住不放的键**。issue 原文同时提到
窗口穿透与鼠标悬停隐藏两项，维护者确认按 issue 处理：两者一起被这个键暂停。

范围里有两件互相独立、但都被同一条需求卡住的事：

1. **运行期**：一个已配置的修饰键按下时，overlay 暂停穿透与悬停隐藏；松开后立即恢复。
2. **设置期**：一个录入控件，只能录入**一个**修饰键，按下即完成，并且**必须区分左右**——
   左 Shift 与右 Shift 是两个不同的配置值。

第 2 项决定了整个方案的形状。把左右区分清楚这件事，GPUI 做不到：

- macOS 按下修饰键产生的是 `NSFlagsChanged`，`parse_keystroke` 只为 `NSKeyDown` / `NSKeyUp`
  构造 `Keystroke`，所以**按修饰键根本没有按键事件**；
- `ModifiersChangedEvent` 只有 `modifiers: Modifiers`（四个布尔）与 `capslock`，没有键身份；
- Windows 侧 `VK_SHIFT | VK_CONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN` 落在
  同一个分支里，同样只产生 `ModifiersChanged`。

因此现有快捷键录入框无法按需求复用：它本来就靠「修饰键先作为 `Modifiers` 布尔位、再由随后的非修饰
键收尾」工作（`shortcut_from_capture` 对只有修饰键的组合直接返回 `None`，测试固定了这一点），而这里
要的恰恰是「只有一个修饰键」。

唯一知道物理键的地方是应用自己的输入管线。`bongocat-platform` 的 scan code 表与 macOS
`modifier_device_bit` 早就把左右分开映射到 HID `0xE0..=0xE7`，`bongocat_input::PhysicalKey` 是那条
通道的类型，runtime 的 `InputState` 持有按下集合并由 release、reconcile、reset 三条路径清理。因此
**录入与运行判定都必须读 runtime 的 pressed set，不能读 UI 事件**。

## 决策

### 1. `ModifierKey` 是八个修饰键的闭合词汇表，落在 `bongocat-input`

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

### 2. `input.pressed_modifiers` 是这个设置唯一的读法

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

### 3. 设置页的录入控件读 runtime 的 pressed set，因此是「按住」语义

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

### 4. 这一行不受开关门禁，只受结构性编辑阻塞

ADR-0053 把「开关门禁」定义为**一个**开关绑定它下方的设置项。这一行被两个开关绑定：
`click_through` 与 `hide_on_pointer_hover` 都在它上方，但两者本身不相邻，任选其一作为门禁来源都是
任意的。一个因为某个不相干的开关没开而置灰的行，比一个「存下取值、等任一开关打开即生效」的行更糟。

因此它只接 `editing_blocked`，与 ADR-0053 第 2 节标准流程的第 5 步（变更方法守卫）保持一致：
录制与清除都在 `editing_blocked` 时拒绝。这是显式偏离该 ADR 的绑定前提，不是遗漏。

### 5. 只作用于穿透与悬停隐藏，不作用于无操作隐藏

issue 与维护者的确认都只点了这两项。无操作隐藏期间用户本来就在操作输入（否则计时不会重置），
把它一起暂停没有可观察的收益，却会让「暂时失效」覆盖三个行为而不是两个。范围因此留在两项。

## 明确不做

- **不做「按住任意修饰键」的模式**。需求明确要求录入一个具体的物理键。
- **不把 Caps Lock 纳入词汇表**。它没有左右之分，与「必须区分左右」冲突。
- **不在 UI 侧新增输入观察者或推送通道**。录入读 runtime，与运行判定同一来源。
- **不缓存按住状态**。任何缓存都会让松开键后 overlay 继续可交互。
- **不在 `Schema` 版本上做迁移**。新字段 `#[serde(default)]`，缺失即 `null`。

## 后果

- 开启穿透或悬停隐藏后，可以按住配置的修饰键直接拖动窗口，不必来回切换设置。
- 左右两侧是两个独立设置，拖动时惯用哪只手就用哪只。
- 配置、runtime 设置与 overlay options 之间只有一份拼写；平台上报的 usage 与落盘 token 由同一张表
  绑定。
- 录入机制是「按住」语义：一次快速点击可能被漏掉，这正是它读取按下集合而非边沿的直接后果。提示语
  仍按快捷键录入框的写法说「按下」，因此这个差别对用户不可见——实际使用中按住即可，与设置本身要求的
  动作是同一个手势。
- 该行不随开关置灰；在两个开关都关闭时设置它不会报错，取值保留到任一开关打开时生效。
- 该设置不参与无操作隐藏。

## 残余风险与待验证项（不得当作已确认）

1. **未在 Windows 实机验证。** 改动覆盖两个平台的 `update_hover_presentation`，但 Windows Raw Input
   与 D3D11 路径本机无法执行；只有 macOS 实机跑通。
2. **按住期间的实际拖动手感未实测。** 逻辑有测试固定（穿透收窄、悬停复位、延迟重新计时），但真机上
   从「按住」到「抓住窗口」之间是否有可感知的延迟没有测量证据。
3. **左右区分依赖平台上报的 usage。** 平台侧映射（Windows E0/E1 前缀、macOS `modifier_device_bit`）
   已有各自测试，本次未新增；两平台组合下每个物理键都映射到预期 usage 这一点未在本次实机核对。
4. **录入控件的键盘可达性只有单元级证据。** tab 位置、清除按钮与 `Escape` 有渲染测试点击覆盖，
   VoiceOver / Narrator 下的实际播报未实测。
5. **50ms 轮询的实机成本未测量。** 它只在控件武装期间运行且每次是一次原子读，但未在真实负载下确认
   对设置线程无可观察影响。

## 验证

- `bongocat-input`：`every_modifier_names_one_distinct_left_and_right_hid_usage`、
  `the_two_sides_of_a_modifier_are_separate_configuration_values`、
  `a_stored_token_round_trips_and_anything_else_is_refused`、
  `the_pressed_set_projection_answers_per_modifier_and_keeps_the_sides_apart`。
- `bongocat-config`：`the_hold_modifier_stores_one_physical_key_and_defaults_to_none`、
  `the_hold_modifier_defaults_when_missing_from_older_data`、
  `an_unknown_hold_modifier_is_refused_by_the_strict_parse`，以及
  `shared/config/fixtures` 的 `hold-modifier-to-interact` / `-unknown` 一对。
- `bongocat-runtime`：`the_pressed_modifier_projection_keeps_the_two_sides_apart`、
  `a_reset_clears_the_pressed_modifier_projection`、
  `a_reconciled_release_ends_the_modifier_hold`。
- `bongocat-overlay`：`the_hold_modifier_answers_for_the_configured_physical_key_only`、
  `recalling_the_hold_modifier_never_replaces_the_window`、
  `a_hold_restores_the_overlay_and_letting_go_starts_a_fresh_delay`。
- `bongocat-app`：`the_pressed_modifier_read_reports_the_physical_key_and_moves_no_revision`。
- `bongocat-ui`：`a_held_modifier_is_recorded_as_the_physical_key_it_is`、
  `two_held_modifiers_resolve_to_the_same_key_every_time`、
  `a_cancelled_recording_writes_nothing_even_if_a_key_is_held_afterwards`、
  `the_recorder_frame_arms_the_recording_and_the_clear_control_does_not`、
  `structural_editing_blocking_renders_the_recorder_inert`、
  `the_recorder_names_the_side_of_the_key_it_stores`、
  `the_side_word_follows_the_interface_language`、
  `every_prompt_in_the_row_names_the_modifier_the_same_way`、
  `the_recorder_description_names_both_settings_it_suspends`、
  `the_recorder_copy_is_named_in_every_shipped_language`。

---
<a id="adr-0081"></a>
# ADR-0081: 指针被捕获时按相对位移累计光标位置

状态：已接受（2026-10-04）；2026-10-13 采样改为同时携带指针自身位置、悬停命中测试改读它，见「决策 8」
补充：ADR-0011（模块契约）、ADR-0054（配置读取失败恢复）
实现：Issue #785；指针位置分离见 Issue #1140

## 背景

Issue #785 报告：打开桌宠后进入全屏游戏，键盘和鼠标**按键**都能跟随，但鼠标**移动**不再跟随。报告者
在附加信息里点名要「鼠标强制移动」功能，并提到参考实现 Bongo-Cat-Mver 里的「强制移动鼠标」开关。

根因不在采集，而在位置来源。当前 Windows 平台 adapter 通过 Raw Input（`RIDEV_INPUTSINK`）接收
`WM_INPUT`，因此前台应用独占输入时仍能拿到键鼠事件——这就是按键仍然工作的原因。光标位置则由
`forward_cursor` 在收到鼠标移动包后调用 `GetCursorPos` 读取。多数全屏游戏会捕获指针：调用
`ClipCursor` 把光标锁在窗口内、隐藏它、只消费设备报告的相对位移。此时 `GetCursorPos` 返回的是一个
不再变化的坐标，于是「有移动事件 → 读绝对位置 → 位置没变 → 模型不动」。

参考实现的做法印证了这一点：Bongo-Cat-Mver 用 DirectInput 的 `DIMOUSESTATE.lX/lY` 读相对位移，
累加进一个自维护的位置（`point.x += mouse_state_d7.lX`），而不是读绝对光标。它的
`mouse_force_move` 是 `decoration` 下的全局开关，默认关闭。

范围里有一件必须现在决定、且决定方案形状的事：**累加发生在哪一层**。

## 决策

### 1. 累加在平台 adapter 内完成，不进入 runtime

`CursorProducer` 是 latest-value 通道（`CursorSlot`），只保留最后一个待消费样本。相对位移如果在
平台侧只作为「这一帧动了」的布尔量传给 runtime，再由 runtime 累加，那么两次发布之间被合并掉的采样
会连同位移一起丢失——快速移动会比设备实际移动的距离短。因此位移必须在**发布之前**合并，也就是在
平台线程内完成。

结果是 runtime、renderer 和 `ModelSettings` 都不知道这个开关存在：平台发布的仍是同一个
`CursorSample`，只是位置来源不同；平滑、归一化和模型投影完全不变。

### 2. `CursorForceMoveState` 是平台无关的状态机，落在 `bongocat-input`

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

### 3. 第一个采样与 viewport 变化重新对齐，且不叠加该采样的位移

累加器的第一个采样和任何 viewport 变化都直接从绝对位置开始，**不**再加上这一次的 `delta`：那个位移
已经体现在绝对位置里，再加一次会多走一个采样。这与 `CursorSmoother` 对首个样本和跨显示器样本的处理
是同一条规则（直接对齐目标），因此两者在 `force_move` 打开与关闭之间切换时不会产生位置跳变。

跨 viewport 必须重新对齐的理由和平滑器一致：两个显示器的坐标系原点不同，把旧显示器的位移加到新
显示器上会得到一个无意义的位置。

### 4. 累加位置夹在当前 viewport 内，饱和而不是记账

`advance` 把位置夹在 `viewport` 内。到达边缘后继续朝同方向推，位置**饱和**在边缘，而不是把多出来的
位移记在账上——后者会让用户先「还清」反向位移才能看到指针往回动。viewerport 没有可用面积（宽或高
为 0、非有限）时不夹取也不 panic：一个退化的读数不能让输入 worker 挂掉。

### 5. 报告位置的设备被识别出来并重新对齐，而不是冻结

不是所有设备都报告相对位移。Windows 的 `RAWMOUSE::usFlags` 有 `MOUSE_MOVE_ABSOLUTE` 位（远程桌面、
绝对定位设备会置位），这类包里的 `lLastX/lLastY` 是坐标而不是位移。macOS 没有对应的标志位。

- Windows：绝对包到达时调用 `reset()`，于是下一个采样从绝对光标重新对齐。跳过绝对包而不重置会让
  累加位置停在被钉住的那一点，而绝对设备的光标其实是正常移动的。
- macOS：没有标志位，于是从事实推断——**位置移动了但位移为零**说明这是报告位置的设备；被捕获的
  指针恰好相反（位置不动、位移持续到达）。这个推断只会导致重新对齐，而重新对齐的结果永远是「指针
  真正所在的位置」，所以它最多是无收益，不会引入错误位置。反过来，位置移动**且**位移非零时不移位
  （有些游戏会把光标重新居中，此时按位移累加才是对的，按位置对齐会让模型跳到屏幕中心）。

### 6. `input.mouse.force_move` 是 `InputConfig` 下的独立命名空间，runtime 只负责转发

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

### 7. 设置行放在 Input & interaction → Mouse 的最后一行

页面已有的三行是「忽略鼠标输入」「水平翻转鼠标跟随」「垂直翻转鼠标跟随」，都描述**模型拿到指针之后
怎么用**。这一行描述的是**指针位置从哪来**，放在三行之后读作对上面三行的一条附注：如果指针根本不动，
上面三项都没有作用对象。放在最前会读成「下面几项的前提条件」，而它并不是——关闭它时上面三行照常工作。

### 8. 采样同时携带指针自身位置，悬停命中测试只读它

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

## 明确不做

- **不默认打开。** 打开期间由设备以外的方式移动的指针（程序化 warp、绝对定位设备、触摸板手势）不会
  被跟随；这是模式的代价，因此由用户显式选择。默认值与参考实现一致，都是关闭。
- **不用低级鼠标钩子。** `WH_MOUSE_LL` 提供的是同一个被钉住的坐标，解决不了问题，却会引入一个必须
  常驻的钩子。
- **不引入 DirectInput。** 参考实现用 DirectInput 读位移；本项目的 Windows 输入路径已经是 Raw Input，
  同一个 `RAWMOUSE` 里就有 `lLastX/lLastY`，没有理由再引入第二套设备 API（AGENTS.md §2.1）。
- **不在 runtime 侧累加。** 理由见决策 1。
- **不改变平滑、归一化或模型投影。** 这个开关只改位置来源。
- **不做「仅特定进程生效」的探测。** 是否捕获指针是前台应用的行为，逐进程探测既不必要也不可靠。

## 后果

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

## 残余风险与待验证项（不得当作已确认）

1. **未在真实全屏游戏中实测。** 本机无法运行受捕获指针影响的游戏；逻辑有单元测试与解码测试固定，
   但「游戏中猫确实跟随鼠标」这一点没有实机证据。
2. **macOS 实机未验证。** 改动覆盖 macOS 的 `forward_latest_cursor`，但本机是 Windows，交叉检查被
   `permission-flow` 的 Swift 构建脚本挡住，因此 macOS 侧只有人工审查，没有编译或运行证据。
3. **Windows 是否真的收到被捕获指针的移动包，只有间接证据。** 报告者的「按键跟随、鼠标不跟随」说明
   `WM_INPUT` 到达了本进程；移动包与按键包走同一条投递路径，但这一点未在本机核对。
4. **macOS 上「位置移动且位移为零」这个推断未在真机验证。** 它的失效方向是安全的（最多重新对齐），
   但绝对定位设备在 macOS 上是否真的产生零位移事件没有实测。
5. **`RAWMOUSE` 字节布局只在解码测试中固定。** `usFlags` 偏移（`header_size + 0`）与
   `MOUSE_MOVE_ABSOLUTE` 取值来自 `RAWMOUSE` 的定义，未在真实远程桌面或绘图板上核对。
6. **累加位置与光标位置的误差幅度未实测（2026-10-13 新增）。** 决策 8 的成立只依赖「累加值不等于
   指针位置」这一事实（设备位移与光标位移之间隔着指针速度与加速度换算），不依赖误差有多大；但误差
   在桌面上的实际幅度、以及它是否让模型跟随看起来明显滞后，只有推理没有实测。本机是 macOS，
   Windows 侧的相关测试（见「验证」）只做了交叉编译类型检查。
7. **Windows 与 macOS 的悬停隐藏回归测试未在本机运行（2026-10-13 新增）。** 命中测试那一行是平台
   适配器里的代码，本机跑不了 Windows 目标；Windows 的两个新测试只经过 `cargo check
   --target x86_64-pc-windows-msvc` 的类型检查，需要由 Windows CI 实际运行确认。

## 验证

- `bongocat-input`：`accumulator_seeds_from_the_absolute_cursor_then_follows_relative_motion`、
  `accumulator_clamps_inside_the_viewport_and_saturates_at_the_edge`、
  `accumulator_reseeds_when_the_viewport_changes_or_is_reset`、
  `accumulator_tolerates_a_degenerate_viewport`、
  `force_move_state_discards_the_accumulated_position_on_a_mode_change`、
  `force_move_state_reseeds_for_a_device_that_reports_a_position`、
  `force_move_state_keeps_accumulating_when_a_moving_location_also_reports_motion`。
- `bongocat-config`：`the_force_move_switch_round_trips_and_defaults_off_for_older_data`，以及
  `shared/config/fixtures` 的 `force-move-mouse` / `input-without-mouse` / `force-move-mouse-not-a-boolean`
  三个 fixture。
- `bongocat-ui-protocol`：`cursor_settings_command_preserves_typed_values`、
  `cursor_settings_default_to_the_absolute_cursor`。
- `bongocat-platform`（Windows）：`raw_mouse_decoder_reads_relative_motion_and_marks_absolute_devices`、
  `force_move_sums_relative_motion_and_ignores_absolute_packets`；macOS 侧
  `cursor_callback_slot_sums_motion_across_coalesced_samples`（见残余风险 2）。
- `bongocat-ui`：`settings.input_interaction.mouse.force_move.label` 在七个语言资源与设置窗口 smoke
  清单中都被点名。
- 决策 8（2026-10-13）：
  - `bongocat-input`：`a_reading_without_accumulation_reports_the_pointer_as_the_model_position`、
    `a_reading_keeps_the_pointer_position_when_the_model_follows_another`，以及
    `invalid_geometry_non_monotonic_time_and_stop_are_explicit` 里新增的「模型位置非有限」用例。
  - `bongocat-platform`（Windows）：`a_forced_sample_publishes_the_pointer_position_apart_from_the_model_position`。
  - `bongocat-overlay`（Windows）：`hover_hide_hit_tests_the_pointer_position_rather_than_the_model_position`
    ——把两个位置放在窗口矩形的两侧，断言命中测试只跟指针位置走。
  - 前两项在本机（macOS）跑过；后两项只有 `cargo check --target x86_64-pc-windows-msvc` 的类型检查，
    见残余风险 7。

---
<a id="adr-0082"></a>
# ADR-0082: 手柄摇杆显示参数与多手柄轴投影

状态：已接受（2026-10-04）

## 背景

维护者报告手柄模式仍有多处行为不对，要求对照重构前 Tauri 版本与 BongoCat-Mver 复核
按键图片、摇杆、扳机和整体行为。复核结果是三处产品侧缺口，加一处第三方 backend 缺口。

**1. `CatParamStickShowLeftHand` / `CatParamStickShowRightHand` 从未被写入。**
`bongocat-live2d::ProductParameter` 声明了这两个参数，预置 `gamepad` 模型和 Mver 转换出的
手柄模型都带它们（前者 `demomodel3.cdi3.json` 里就叫「显示摇杆左手」/「显示摇杆右手」），
但 `apply_model_input` 的参数表里没有它们。重构前的 `useGamepad.ts` 明确写这两个参数：

```ts
watch(sticks.left, ({ x, y, moved, pressed }) => {
  sticks.left.moved = x !== 0 || y !== 0
  live2d.setParameterValue('CatParamStickShowLeftHand', moved || pressed)
}, { deep: true })
```

结果：手柄模式下两个模拟摇杆的美术永远不出现。`CatParamStickLX/LY/RX/RY` 有写，所以摇杆
的**位置**参数是对的，只是可见性参数没人写——这正是「参数在、效果不在」的典型形态。

**2. 摇杆在动时爪子不落。** 同一段旧代码还通过 `stickActive` 参与爪子判定：

```ts
handleKeyChange(true, stickActive.left || hasLeft)
handleKeyChange(false, stickActive.right || hasRight)
```

`stickActive` 就是 `moved || pressed`。也就是说旧版里推动左摇杆或按下 L3 会让左爪落下。
当前实现只有按键/按钮的 hand 归属能让爪子落下，摇杆完全不参与。

**3. 多手柄时第二个手柄不动。** `GamepadAxisValues::project` 先用
`.keys().filter(connected).map(connection).min()` 选**一个** connection，再只投影它的六个
轴值。重构前没有 device 概念，所有手柄的同名轴写进同一份状态，最新一次写入获胜。两个手柄
同时连接时，先连接的那个即使完全不动也一直占着全部六个轴，用户后来插上的手柄推动摇杆、
扣扳机都不产生任何变化。

**4. Windows WGI 的默认 mapping 把扳机和右摇杆 Y 轴对调（第三方 backend，见 ADR-0066）。**
这条不在本仓库修复，单独记录在 ADR-0066 的「已知阻塞」里。

## 决策

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

## 后果

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

## 验证

- `bongocat-runtime`：`every_gamepad_axis_reaches_its_own_model_input_field` 走真实 transport
  （`GamepadAxisProducer` 样本 + `InputProducer` 边沿），逐轴断言六个 `ModelInputSnapshot` 字段
  各自承载自己的样本、且**该次发布没有改动其余五个**——latest-value 槽位会保留上一次的值，
  所以断言的是「只动了自己」而不是「其余为零」。
- `bongocat-runtime`：`every_stick_control_reaches_its_own_parameter_and_nothing_else` 在真实预置
  `gamepad` 模型上逐个控制（六轴 + 两个摇杆键 + 双摇杆同时 + 双扳机）断言**全部十个**摇杆与爪子
  参数，每次只允许一个（该侧的 XY / `*Down` / `StickShow*` / 爪子）变化。双扳机那一行断言它不
  触碰任何摇杆参数。
- `bongocat-runtime`：`a_displaced_stick_shows_and_presses_its_paw_only_where_the_model_has_a_stick`
  在真实预置 `gamepad` 与 `keyboard` 模型上逐项断言可见性参数、爪子参数与 `StickLeftDown`，
  并断言回中后同一帧复原；
  `the_stick_axes_and_the_stick_button_are_projected_independently` 断言三者不会互相串。
- `bongocat-runtime`：`worker::axes` 的三条测试固定多手柄语义——最新样本获胜、回中读零、
  已释放的 connection 即使样本最新也不贡献，以及死区只作用于获胜样本。
- `bongocat-app`：`every_gamepad_button_matches_the_live_model_artwork` 遍历全部十六个产品按钮，
  **期望值由模型自己的 `KeyImageInventory` 推导**而不是写死：对每个按钮同时断言 hand 归属、
  按键层数量与所在手、渲染器实际解析到的文件名、以及哪一侧爪子落下；没有美术的按钮断言完全
  惰性（不绑定、不画图、不落爪）。预置 `gamepad` 今天有十二个按钮有美术，缺美术的四个是
  `Select`、`Start`、`LeftStick`、`RightStick`——这是唯一允许随美术变化的数字，模型掉了图会在
  这里变红而不是静默失效。
- `bongocat-platform`：`the_shoulder_and_the_analog_trigger_keep_their_own_buttons_and_artwork`
  与 `only_the_analog_trigger_reports_a_continuous_axis` 逐个名字钉住肩键与模拟扳机的
  按钮、美术名和轴，避免 ADR-0070 修过的那类互换无声回归。
- 变异验证：把 `CatParamStickLX` 接到 `stick_right_x` 并把 `CatParamStickRY` 接到 `stick_left_y`，
  `every_stick_control_reaches_its_own_parameter_and_nothing_else` 报
  `left stick X must move CatParamStickLX`；把 `gamepad_hands_for_model` 的手固定成左手，
  `every_gamepad_button_matches_the_live_model_artwork` 报
  `South hand must come from the directory its image lives in`；去掉
  `CatParamStickShowLeftHand` 的写入、以及去掉爪子的模型参数门禁，各自让
  `a_displaced_stick_shows_and_presses_its_paw_only_where_the_model_has_a_stick` 变红；
  恢复旧的「先挑一个 connection」投影让
  `the_newest_sample_of_an_axis_wins_across_every_connected_pad` 变红。
- 既有门禁不变：`cargo fmt --all -- --check`、workspace clippy（`bongocat-app` 按 feature
  分组）、`cargo test --locked --workspace`、`cargo check --locked --workspace --release`。
- **未做**：真实手柄实机。摇杆美术与爪子落下的最终画面仍需 Windows/macOS 各一台物理
  手柄确认，ADR-0066 的物理设备矩阵继续作为发布门禁。

---
<a id="adr-0083"></a>
# ADR-0083: 重复触发同一表情可以把它关掉

状态：已接受（2026-10-06）
补充：ADR-0079（新增配置字段的可选读取约定）、ADR-0030（实现决策阶梯）
需求：Issue #1109

## 背景

Issue #1109 报告：触发一个表情后，再次触发同一个表情没有任何效果——表情一直保持，只有触发别的
表情或切换模型才会变回去。Mver 版本里再点一次同一个表情可以把它关掉，回到模型默认的表情，现在的
版本缺这个能力。

这不是缺少一个「清空表情」的动作，而是**默认表情没有入口**。`RuntimeRenderer::evaluate` 每帧先
`restore_parameter_defaults()` 再 `apply_expression_layers()`，所以「回到默认表情」在渲染器里就是
「没有任何表达式图层在生效」；但产品里唯一能把图层清空的事件是切换模型或 shutdown，两者都不是用户
能随手做出的选择。因此一个用户想撤掉刚才那个表情，只能换一个模型再换回来。

Mver 的行为是「点第二次 = 关掉」，但 issue 同时明确指出不能照搬：现在的用户已经习惯了重复触发只是
重新应用同一个表情，直接沿用旧行为会让两套语义无法对比。所以这是一个**配置项而不是默认行为**。

## 决策

### 1. `model.toggle_repeated_expression` 是唯一的开关，默认关闭

放在 `ModelConfig`，`bool`、默认 `false`、正向语义，设置页呈现为「重复触发同一表情时关闭它」，
位置紧邻「记住每个模型上次使用的表情」。

默认 `false` 是为了不改变任何现有用户的画面：这是一个可观察的行为差异，只有一次显式选择才引入。

字段带 `#[serde(default)]`，与 `model.show_all_pressed_keys` 同一理由（ADR-0079 §1）：v1 解析入口是
严格的，一个必填新字段会让该字段加入之前写下的每一份 `config.json` 解析失败，走「最新有效备份 →
默认配置」的恢复流程并丢掉用户设置。缺少该字段的旧文档读出 `false`，即升级前既有的行为。

它是**独立开关**而不是 `remember_last_expression` 的一个取值或第二个字段：两者回答不同问题——一个
是模型从哪里开始（记住上次那张脸），另一个是重复触发时做什么。把它们合成一个枚举会让「记住但不
切换」「切换但不记住」两种组合都不可表达。

### 2. 判定在 runtime，因为那里才知道当前生效的是哪个表情

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

### 3. 关闭动作不写「记住上次使用的表情」

`user_expression_memory` 只在「应用」分支写。用户正在撤销一个选择，把刚被关掉的表情记下来会让它
在下次启动、或切回该模型时以「用户选过」的名义回来。因此关闭之后 `last_expressions` 保持不变，
记住的仍然是用户最后选择穿上的那张脸。

这也是它作为独立开关的另一个理由：记忆的语义（「用户选过什么」）和开关的行为（「重复触发做什么」）
在关闭这条路径上必须解耦。

### 4. 随机播放不参与

随机选择器通过 `renderer.set_expression` 直接播放，不走 command 队列——这与「自动选择不能进入
记忆集合」是同一条既有边界（`SetExpression` 是「用户主动」的唯一判据）。把开关的判定放在
command 分支里，随机播放天然不在其中，不需要额外门禁。

### 5. 「恢复上次表情」不会被读成重复触发

`restore_remembered_expression` 在 `prepare_model` / `select_model` 之后发送 `SetExpression`。worker
在 `pending_model` 非空时把除 `Shutdown` 与 `ApplyInput` 以外的所有命令推迟
（`crates/bongocat-runtime/src/worker/mod.rs`），而 `active_expression` 在模型 commit 时被清空，所以
恢复命令执行时 `active_expression` 一定是 `None`，不可能与请求的表达式相等。

这条不是新加的保证，而是**必须被钉住的不变量**：一旦有人让模型切换不再清空 `active_expression`，
或者不再推迟命令，打开开关的用户就会看到「刚恢复的表情立刻自己关掉了」。

## 后果

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
