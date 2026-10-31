# 构建、更新与交付

> 本文件由 `docs/adr/` 下 **18 个 ADR** 合并而成（2026-10-31）。
> 每个 ADR 保留原始编号（`ADR-XXXX`）与完整正文，可继续按编号引用；原始单文件已归档到 `docs/adr/archive/`，仅作考古与回滚，勿直接编辑。

---
<a id="adr-0010"></a>
# ADR-0010: Windows CPU Targets

状态：已被 ADR-0033 取代（2026-09-14，Windows ARM64 不再是产品目标，只剩 x64）

## Context

BongoCat 需要明确 Windows 发布架构，避免继续为不再支持的 target
维护输入、renderer、安装和更新分支。当前固定的 Cubism 5 SDK for Native R5
提供 desktop Windows x86/x64 Core，但没有 desktop Windows ARM64 Core；其
experimental UWP ARM64 DLL 不能用于 Win32 desktop 应用。

## Decision

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

## Consequences

- 新的 target matrix、binding fixture 和发布配置不得加入 `i686-pc-windows-msvc`
  或 `aarch64-pc-windows-msvc`。
- `deny.toml` 的 `[graph].targets`、`bongocat-update::UpdateTargetTriple` 与
  发布流水线必须只包含 `x86_64-pc-windows-msvc`；契约测试强制三者一致。
- Windows 平台依赖与 renderer 只在 x64 上验证。ARM64 上的实际体验通过 Windows
  的 x64 仿真覆盖，不再有独立的原生 ARM64 证据要求。

## Verification

- `tools/inspect-cubism-sdk.py` 把 Windows ARM64 报告为 `unsupported_by_r5`，
  把 i686 报告为 `excluded_by_product`。
- `tools/cubism-bindgen` 只为当前 R5 可用且仍在产品矩阵内的 Windows x64 和
  macOS targets 生成合成 bindings；它拒绝 Windows ARM64 和 i686。
- Phase 0 CI 不安装或编译 i686，也不再安装或编译 Windows ARM64。
- `tools/tests/test_packaging_contract.py` 断言打包工具的 target 集合、
  `deny.toml` 与 `UpdateTargetTriple` 三者一致且不含 ARM64。

---
<a id="adr-0016"></a>
# ADR-0016: Anonymous Diagnostics Export Contract

状态：已接受（2026-09-01）

## 背景

Diagnostics 页面已经聚合 runtime、输入、配置和模型目录状态，但用户和维护者需要一个可预览
的文件来复现问题。直接打包原始日志、模型目录或输入事件会泄漏用户路径、模型身份和按键内容，
也会让导出格式依赖 Rust `Debug` 文本而无法稳定解析。

## 决策

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

## 后续边界

应用级日志 writer 的实现与有界清理已记录在 ADR-0017；跨域历史日志合并、预览器和更新系统的
诊断 manifest 仍需单独设计。
实现不得为了导出而放宽日志隐私、环境隔离或原子写入约束。

---
<a id="adr-0017"></a>
# ADR-0017: Bounded Application Logging

状态：已接受（2026-09-01）；2026-09-24 由 ADR-0064 修订文本格式、过滤与统一 retention

## 背景

BongoCat 需要可诊断的应用生命周期记录，但不能把按键、路径、剪贴板、配置/模型正文、URL、密钥/凭据或用户文件
内容写入日志。Cubism Core callback 只覆盖厂商边界，不能替代应用级日志；无界文件增长也会破坏环境
隔离和稳定性验收。早期实现使用 JSONL 与两个独立 writer，ADR-0064 已把输出改为普通文本，并让
application/Core 共用一套 writer policy。

## 决策

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

## 验证

`bongocat-log` 单元测试覆盖级别顺序、共享 controller、UTC 日切换、1 MiB 轮转、配置化保留、目录级
8 MiB/32 文件预算、未知文件/符号链接隔离、严格文本 parser 与 owner-only 权限。`bongocat-app` 覆盖
固定 event catalog、panic 脱敏、运行标记分类、共享过滤/retention 和脱敏 preview；`bongocat-live2d`
覆盖 Core callback contention、queue saturation、shutdown drain、文本写入和跨 writer prune 统计。
macOS Development release smoke 已验证 diagnostics-export 固定 entries、panic=abort 恢复和
settings-window lifecycle；独立 bounded normal run 在本机最新尝试中超时并留下 unclean marker，仍需复核。
Windows release smoke 与完整双平台证据仍待发布门禁补齐。

## 后续边界

远程上传、Core message 导出和第三方稳定日志 schema 不在本 ADR 范围。`.log` 面向用户与支持人员，
需要机器处理时使用匿名 `diagnostics.json` 或严格重写后的 preview entry。

---
<a id="adr-0021"></a>
# ADR-0021: Signed Update Manifest Trust Boundary

状态：已被 ADR-0029 取代（2026-09-13）

> 后续修订（2026-09-23）：本文已由 ADR-0029、再由 ADR-0034 取代。当前更新目标为 Windows x64、macOS aarch64 与 macOS x64；Windows 原生 ARM64 已由 ADR-0033 退役。

## 背景

BongoCat 将自动/手动更新列为首发行为，但旧版 updater 使用 HTTP endpoint、前端固定凭据和
旧 payload，不能进入新架构。下载和安装前必须先建立独立、可离线验证的信任边界；否则网络 client、
设置 UI 和高权限 installer 会各自解释版本、target、hash 或签名，形成不一致的安全判断。

Development 与 Production 构建也不能共享可变 channel。Windows 只支持 x64/ARM64，macOS 支持
Intel/Apple Silicon；Windows ARM64 在官方 desktop Cubism Core 可用前仍是发布阻塞，而不是由更新
manifest 绕过。

## 决策

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

## 依赖与替换边界

- `ed25519-dalek = 3.0.0`（BSD-3-Clause）提供纯 Rust Ed25519 严格验签；关闭默认 zeroize feature，
  只启用 verifier 所需 fast 表。维护仓库为 dalek-cryptography，Rust 1.85+。
- `sha2 = 0.11.0`、`semver = 1.0.28` 和 `url = 2.5.8` 均为当前稳定版，许可证为
  MIT OR Apache-2.0，支持项目 Rust 1.97。它们分别只位于 digest、版本和 HTTPS 解析边界。
- `bongocat-config` 是同一 workspace 的平台无关依赖，只提供不可变的 `StorageLayout` 与
  `BuildEnvironment`，用于把更新 sequence 固定在当前环境的 `updates/` 根；它不向 update
  verifier 暴露配置内容、平台 handle 或 UI 类型。
- 这些依赖均由 `bongocat-update` 封装；若停止维护，可替换实现而不改变 app/runtime/UI 公共协议。
  manifest 算法或字段变化仍需新 schema/ADR，不能在 v1 中静默替换。

## 验证

- 单元测试使用固定测试私钥覆盖有效签名、篡改、环境错配、公钥轮换窗、sequence 降级、最低版本、
  target/arch 错配、重复 target、未知字段、HTTP URL、artifact 长度/hash 和读取失败。
- Draft 2020-12 门禁验证共享 accept/reject fixtures；Rust 测试对同一 valid fixture 签名并解析，防止
  schema 与实现漂移。
- 完整 workspace 在 macOS、Windows 和 Ubuntu 执行 format、严格 Clippy、测试与 release
  check；dependency policy 继续覆盖许可证和 registry source。

## 后续边界

下一步建立环境绑定 endpoint 与 24 小时自动/手动检查调度，再实现有界下载、取消、临时文件清理、
平台 installer、操作系统签名验证和失败回滚。没有这些证据前不得声称更新功能或 stable 发布完成。

---
<a id="adr-0022"></a>
# ADR-0022: Update Manifest Transport Envelope

状态：已被 ADR-0029 取代（2026-09-13）

## 背景

ADR-0021 已固定签名 manifest 的离线验证边界，但尚未定义网络响应如何无损携带 key ID、detached
signature 和原始 body。让 HTTP client、设置服务或发布脚本各自选择 JSON wrapper、base64 或 redirect
策略会使待验签 bytes 产生歧义，也会把传输库类型带入信任判断。

## 决策

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

## 验证

`bongocat-update` 单元测试覆盖 envelope 的 body 上限、key ID/signature 编码拒绝、exact-byte 保留以及
通过 envelope 的验签路径。后续网络 adapter 必须补充真实 HTTPS、redirect、代理、非-200、截断和超限
body 测试。

## 后续边界

本 ADR 不选择 HTTP crate、Production endpoint 或公钥注入方式，也不授权下载、包签名、installer 权限或
回滚行为。

---
<a id="adr-0023"></a>
# ADR-0023: Windows Per-User Installer
> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

状态：已接受（2026-09-05）；NSIS toolchain 与 wrapper 边界已由 ADR-0033 取代（2026-09-14）

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

## 背景

BongoCat 已使用当前用户 HKCU Run 启动项、环境隔离的数据根和独立的签名更新 trust boundary。
Windows 首发仍需要固定安装格式，以便明确 installer 权限、卸载语义和未来 update helper 的交接。

## 决策

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

## 备选方案

- MSIX 会把启动、activation 与更新交由 package model，不能作为当前 HKCU Run 与进程内
  `self_update` 路径的无附加重构替代。
- WiX / MSI 主要服务机器级部署，通常引入管理员权限与企业安装策略；首发不需要该权限面。
  `cargo-packager` 的 `wix` format 因此不进入发布矩阵。

## 验证

后续 installer job 必须在干净 Windows 10 1903+ 与 Windows 11 用户 profile 验证安装、升级、卸载、
签名、环境数据保留、x64 artifact 选择和失败 rollback。这些证据尚未取得，不能宣称 Windows
package 可发布。

---
<a id="adr-0025"></a>
# ADR-0025: Signed Manifest HTTPS Transport

状态：已被 ADR-0029 取代（2026-09-13）

## 背景

ADR-0021 和 ADR-0022 已定义离线验签及 HTTP envelope 字节协议，但没有实际 HTTPS client 时，
endpoint、redirect、压缩和 response-body 限制仍可能在后续 app 接线时被各自实现。尤其是 transparent
compression 会改变 detached Ed25519 signature 必须覆盖的原始 bytes。

## 决策

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

## 依赖与替换边界

`ureq 3.4.0` 为 MIT OR Apache-2.0、Rust 1.85+ 的公开维护 crate。它是纯 Rust blocking client；只在
`UreqUpdateManifestSource` 私有字段出现。若维护或安全边界不再满足要求，可替换该 source，不改变
`UpdateManifestEndpoint`、`UpdateManifestEnvelope`、verifier、runtime 或 UI contract。

## 验证

- 单元测试锁定 HTTPS/credential/fragment/长度 endpoint 边界、HTTPS-only、no-redirect agent config 和
  transport code 唯一性。
- 注入式 source 回归验证通过时仅写入验证后的 release sequence，而 source failure 后 sequence 保持不变。
- workspace 的 format、strict Clippy、test、release check 与 dependency policy 覆盖新 crate graph。
- 后续 app-owned worker 必须在真实 HTTPS endpoint 验证断网、代理、redirect、non-200、截断、超限、
  签名错误和降级攻击，且不影响本地 overlay。

## 后续

本 ADR 不选择 Production endpoint 或公钥，不实现 UI command、自动/手动 dispatch、installer 或 rollback。
它们仍需要各自的发布基础设施与安全验收。

---
<a id="adr-0026"></a>
# ADR-0026: Update Helper Replacement Boundary

状态：已被 ADR-0029 取代（2026-09-13）

## 背景

ADR-0021 至 ADR-0025 已将 manifest、artifact URL、下载和 staging 限制为不可变、已验证的
项目类型。它们不授权网络 worker、GPUI 或 installer 写入正在运行的 product files。若将这一
边界隐含在后续 helper 中，path substitution、部分替换、权限扩大和 rollback 降级都会绕过已有
的签名与 anti-rollback 结论。

## 威胁模型

- 不可信输入包括 update endpoint、manifest transport、artifact bytes、staging directory contents、
  product directory contents、环境变量、当前工作目录和用户可修改的 registry/launch arguments。
- 可信输入只包括编译进当前 app 的环境、target/arch、trust key、当前 installation root，和由
  VerifiedArtifact 只读 accessor 传递的 release identity、精确长度、digest 与 staging file。
  私钥、endpoint 配置和任意用户配置均不进入 helper。
- helper 与 app 同为当前用户权限；它不得请求 elevation、安装 service/driver、写入 HKLM、修改其他
  用户 profile，或以 Development/Production 用户数据目录作为 product replacement source/target。
- 被攻击者替换、删除或锁定的 app/helper/staging file 必须导致失败关闭和保留已知可运行 product，
  而不是重试到另一路径或启动未知 binary。

## 决策

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

## 验证

- 平台 contract tests 覆盖拒绝裸输入、root/staging link substitution、path traversal、cross-environment、
  cross-target、invalid signature、concurrent helper、parent alive、prepare failure、atomic commit failure、
  launch/health failure rollback 与成功后 cleanup。
- Windows 10 1903+ 和 Windows 11 以非管理员 profile 验证真实 Authenticode、lock、upgrade、rollback、
  uninstall 后无残留和 Development/Production 数据保留；macOS 12+ 验证 signed/notarized bundle、
  quarantine、rollback 和 data preservation。
- 断电/kill fault injection 在每个 prepare/rename/acknowledgement 点重启，断言只会启动完整已验证的
  current 或 rollback product，且 never deletes user data。

## 后续边界

本 ADR 不实现 helper、OS signature verification、archive format、health protocol、endpoint、公钥或
release hosting。每个实现需要独立平台代码、自动化与实机证据；在上述验证通过前，更新安装和 stable
发布仍不可宣称完成。

---
<a id="adr-0027"></a>
# ADR-0027: Diagnostics Preview Bundle Boundary

状态：已接受（2026-09-06）；2026-09-24 由 ADR-0064 修订 application event 输入与输出格式

> 后续修订（2026-09-23）：ADR-0054 后设置 UI 不再以 AccessKit 语义作为当前契约；本文的 “AccessKit 语义”验收应理解为当时的历史证据，当前改以可见文案和 typed result 为准。

## 背景

ADR-0016 定义的 `diagnostics.json` 是当前环境内原子写入的匿名摘要。它足以显示
runtime、输入、配置和日志 owner 的聚合状态，但不能让维护者查看有界的应用生命周期 code
历史。直接复制 `logs/` 的所有内容不可接受：Cubism Core 的 message 不能被当作稳定、无用户内容
的记录，文件名、路径和未来日志 owner 也不能隐式进入导出。

## 决策

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

## 验证

- unit tests 覆盖 active/rotated 固定文件名、固定 entry 名称、严格 text record reserialization、
  全部 catalog code、unknown/mismatched module/level/code、malformed timestamp/delimiter、整文件跳过、
  旧 JSONL/Core 排除，以及路径/context/message/时间戳不进入输出。
- transaction tests 覆盖成功原子替换、parse/write/sync/replace failure 清理、已有 preview 保留、
  symlink/non-regular source 拒绝、目录枚举失败和 Development/Production 目录隔离。
- settings/UI contract tests 覆盖 typed result、pending/error/retry，不显示动态 path 或 archive 内容。
  Windows/macOS release smoke 验证 owner-only storage、真实 application event 重写和预览包可由系统
  工具打开，但不需要把原始日志上传或读回产品 runtime。

## 后续边界

本 ADR 不实现 remote upload、Core history export 或 support endpoint。具体 archive writer lifetime
必须在实现提交中记录；在完整验证前，Phase 7 日志导出门禁保持未勾选。

首个 writer 使用 `zip = 9.0.2`（MIT，`zip-rs/zip2`，Rust 1.88+），精确 pin 且关闭默认 feature，
只写标准 `Stored` entries；它不提供 encryption、compression 或 archive extraction。替换边界是同样
能在双平台写出并验证此固定 v1 ZIP entry 集合的维护中 Rust crate，不能改变本 ADR 的隐私/原子性约束。

---
<a id="adr-0029"></a>
# ADR-0029: Third-Party Update Library Boundary

状态：已被 ADR-0034 取代（2026-09-14）
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

## 背景

ADR-0021、ADR-0022、ADR-0025 与 ADR-0026 共同建立了一套自研更新栈：严格 manifest v1 schema、
detached Ed25519 清单签名、Development/Production channel 绑定、单调 `release_sequence` 防降级、
精确固定的 `ureq 3.4.0` 传输，以及一个独立承担全部校验职责的 update helper。

维护者决定：更新属于通用机制，不重复实现。第三方库提供完整实现时直接采用，包括其验证模型，
不再并行保留自研验证层。本 ADR 记录该决定并取代上述四个 ADR；被取代的 ADR 保留正文作为历史记录。

## 调研结论

已核实（2026-09-13，`cargo info` / `cargo tree` / 源码阅读）：

- `self_update 1.3.0`（MIT，Rust 1.88+）提供下载、tar/zip 解压、`zipsign` 归档签名校验、
  单文件自身替换、macOS `.app` 整包 rename 交换（含 stash 与 best-effort 回滚）以及进程重启。
  项目 toolchain 为 `1.97.1`，满足要求；`deny.toml` 允许 MIT 且只允许 crates.io registry。
- `UpdateConfig` 与 `ReleaseUpdate` 均为 **sealed**，外部无法自行实现，只能通过后端 builder
  构造。接入自有发行源需实现其 `ReleaseSource` trait 并使用 `backends::custom`，或使用内置
  `github` / `gitlab` / `gitea` / `s3` / `manifest` 后端。
- `self_update 1.3.0` **公开导出**本地下载、解压与文件搬运原语：`Download`、`Extract`、`Move`、
  `MoveAll` 都在 crate 根导出（`src/lib.rs:2216` / `1390` / `1922` / `2045`）。其中 `MoveAll` 是
  **事务式多文件安装器**：逐个把被替换的目标 stash 走再 `rename` 新文件就位，任一移动失败则逆序
  回滚全部已应用的移动。`github::Update::update()` 自身只走「单文件」与「macOS bundle」两条安装
  路径，但这些原语可用于自行编排多文件更新。
  > 更正（2026-09-13）：本 ADR 初稿曾写「`self_update 1.3.0` 不提供公开的本地归档解压或文件搬运
  > 工具（无 `Extract`、无 `MoveAll`），这些能力只在它自己的 `update()` 编排内部使用」。该结论
  > **错误**，且是我此前一次 BSD `grep` 误用（`\|` 交替在 BSD grep 的 BRE 下不生效）造成的假阴性。
  > 上述可见性与行为均已重新按源码确认，并由
  > `crates/bongocat-update/tests/multi_file_install_capability.rs` 在本机实测锁定。
- 未启用任何 HTTP client feature 时 `self_update` **无法编译**，上游使用无条件 `compile_error!`。
  因此依赖它就必然引入其网络层。
- 依赖 `ureq` feature 时，`self_update` 声明
  `ureq = { version = "3.0.6", default-features = false, features = ["gzip", "json", "socks-proxy", "charset"] }`。
  Cargo feature 在整个构建图中取并集，调用方的 `default-features = false` 无法阻止这些 feature 被启用。
  该冲突在 ADR-0025 下不可接受，在本 ADR 下随 ADR-0025 一并作废。
- **fail-open 行为**：`self_update::verify_signature` 在公钥集为空时**直接返回 `Ok(())`**，
  即静默跳过验签。任何转发空公钥列表的封装都会安装未签名产物而不自知。
- macOS 上 `bundle_install_path` 会从运行中的可执行文件自动推导其所属 `.app`，
  并对 Gatekeeper translocation（隔离属性导致运行于只读临时挂载）返回专门错误，
  而不是在中途 swap 时报出只读文件系统错误。

## 决策

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
- **channel 隔离保留**：`AGENTS.md` §10 要求更新 channel 按环境隔离，这是本 ADR 之外的约束。
  Development 构建的 `ReleaseChannel` 不允许联网或安装，`check()` / `install()` 在发出任何请求前
  即失败关闭。
- **签名密钥缺失时失败关闭**：由于上游空公钥集是 fail-open，`RELEASE_SIGNING_KEY` 为 `None` 时
  runtime 拒绝安装并返回 `update_signature_key_missing`。这是本项目对该 fail-open 的补偿。
- **稳定错误码保留**：原 32 个错误码收敛为 13 个，覆盖新实现的实际失败面。其中
  `update_download_transport_failed` 沿用原名，因为它是既有诊断导出契约的一部分
  （`ADR-0016` / `ADR-0027`）且被测试固定。
- 第三方错误、配置与平台类型不得扩散为项目公共 API。`self_update::Error` 在本 crate 边界内
  被映射为自有稳定码；`UpdateDiagnostics` 不含任何库类型。
- 依赖按 `AGENTS.md` §9 精确 pin（`=1.3.0`）并提交 `Cargo.lock`。
- 随本 ADR 一并移除因验证层退役而失效的 workspace 依赖：`ed25519-dalek`、`ureq`、`sha2`、`semver`。

## 被取代的 ADR 与其损失

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

`AGENTS.md` §10「更新只允许 HTTPS，校验版本、target、arch、hash 和签名，并提供失败回滚」
仍然成立：库分别以 HTTPS-only 传输、资产 target 匹配、构建期 target 常量、`checksums`
feature 与 `signatures` feature、install 阶段的 stash 回滚覆盖这些点。

## 验证

已完成（2026-09-13）：

- 依赖解析与编译：`self_update 1.3.0` 在 Rust 1.97.1 下与项目精确 pin 无冲突，
  `cargo check -p bongocat-update` 通过。
- 真实编译图增量实测为 +17 个包，且 `reqwest` / `aws-lc-rs` / `cmake` / `tower-http` /
  `cookie_store` / `rustls-platform-verifier` 均不在图中。
- `cargo fmt --all --check` 通过。
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` 零警告。
- `cargo test --locked --workspace` 全部通过：497 passed / 0 failed / 5 ignored。新增 13 个单元
  测试覆盖 channel 门禁、签名密钥失败关闭、错误码稳定性与诊断计数；既有 `bongocat-app` 诊断
  测试保持通过。
- `cargo check --locked --workspace --release` 通过。
- `./tools/check-dependencies.sh` 通过：四个首发 target 的 release 依赖树均无
  `tauri` / `wry` / `webview2` / `nodejs-sys` / `neon` / `deno_core` / `quickjs` /
  `javascriptcore`；14 个 manifest 全部 `licenses ok, sources ok`。新增的 `self_update` 及其
  传递依赖未触发任何许可证或来源违规，`deny.toml` 无需为它新增例外。
- `python3 tools/validate-json-schema.py` 通过（9 input / 9 expected / 10 config / 6 state），
  已不含 update 校验入口。
- 三平台 CI（run `34737269808`）首次运行暴露一个本机不可见的问题：Linux runner 不属于四个首发
  target，`ReleaseConfiguration::for_current_build` 在那里返回 `None`，导致两个直接用它构造
  runtime 的测试断言了错误的期望值（`left: None, right: Some("development")` 与
  `left: NotConfigured, right: SignatureKeyMissing`），`Test workspace (ubuntu-latest)`
  因此失败 2 项。已把这两个测试改为从显式 `ReleaseConfiguration` 构造 runtime，使其在任意宿主上
  都真正执行 channel 与签名密钥门禁，而不是被静默跳过；并新增
  `a_host_outside_the_shipped_targets_has_no_release_configuration` 覆盖 `None` 配置路径。
  本地复检 14 项测试通过、workspace 498 passed / 0 failed。
- 修复后三平台 CI（run `34738189816`，commit `b2b1f51`）**全绿：23/23 作业成功**，包括
  `Test workspace (ubuntu-latest)` / `(macos-latest)` / `(windows-latest)`、
  `Check dependency policy` 与全部平台 smoke。

- `bongocat-config::StorageLayout` 的 `update_staging` 字段（`<env>/updates/staging/`）已移除：
  `staging` 模块删除后该字段无任何写入方，`create_directories`、环境形状断言与 0700 权限断言
  已同步更新。
- `RELEASE_BINARY_NAME` 原先声明为 `BongoCat`，与实际发布的可执行文件 `bongocat-app` 不一致。
  由于 `self_update` 用它派生 Windows 归档内的提取路径，这个偏差会让第一次真实更新在解压阶段
  失败。已改为 `bongocat-app`，并新增 `tools/tests/test_update_release_contract.py` 把这层
  编译期不可见的耦合固定下来：二进制名 ↔ `bongocat-app` 包名 ↔ `build-windows.ps1` 的 exe 名、
  bundle 名 ↔ `package-macos.sh` 的 `.app` 名、仓库 owner/name ↔ workspace manifest 的
  `repository`、四个 target ↔ `deny.toml` 的 `[graph] targets`。
- 上述最后两项（`update_staging` 移除、`RELEASE_BINARY_NAME` 对齐与契约测试）由 run `34740872994`
  （commit `fe5a5de`）覆盖，同为 **23/23 作业全绿**；其中 `Validate shared fixtures` 作业运行
  `python3 -m unittest discover -s tools/tests`，即新增的契约测试已在 CI 上执行并通过。
- `MoveAll` 的事务语义已在本机实测，不依赖文档：`crates/bongocat-update/tests/multi_file_install_capability.rs`
  验证「可执行文件 + 资源文件」两条移动全部生效，以及第二条源缺失时第一条已应用的移动被回滚、
  未被触及的目标保持原内容。该测试**不覆盖**本项目尚未实现的多文件编排路径，只锁定库原语语义。
- 两种发行资产布局已在本机实测：`crates/bongocat-update/tests/archive_layout_capability.rs`
  用库自身的 `Extract` 验证 macOS 归档整包解压后 `BongoCat.app/` 位于归档根并携带
  `Contents/Resources/`、Windows 归档根级的 `bongocat-app.exe` 可被取出、以及多包一层目录时
  失败且目标目录保持为空。这三项均不依赖网络或签名密钥。
- 完整安装链路已在本机端到端演练（`crates/bongocat-update/tests/local_install_rehearsal.rs`）：
  用 `backends::custom`（未门控，可接任意 `ReleaseSource`）配合一个只回放固定字节的 loopback
  HTTP 服务，跑通**资产按 target triple 选择 → 下载 → 按扩展名识别归档 → 解压配置路径 → 安装**
  全链路，且使用的就是生产常量 `RELEASE_BINARY_NAME` / `RELEASE_BUNDLE_NAME`。两项覆盖：
  单文件替换落到配置的安装路径；bundle 模式整棵 `.app` 被替换、`Contents/Resources/` 随包到达、
  旧包遗留文件被清除。**不含**签名校验、GitHub 后端与进程重启。

## 待验证项（不得当作已确认）

1. 替换机制只在本机以「假安装目标」验证过，**未在真实发行物上验证**。已实测（见上一条）：
   单文件替换、`.app` 整包交换与旧包清理。**未实测**：Windows 上运行中的 exe 被改名替换、
   安装阶段失败后的回滚、替换后的进程重启、以及从真实 GitHub 发行下载。真实 `.app` 还需
   签名与公证，当前打包脚本只做 ad-hoc 签名。
2. 未配置真实 endpoint 与签名密钥。`RELEASE_SIGNING_KEY` 为 `None`，因此当前构建
   `update_check_available()` 恒为 `false`，更新入口在 UI 中不显示。
3. **发行流程尚未产出 `self_update` 可消费的归档。** 读 `self_update 1.3.0` 源码确认的硬性要求：
   - **资产名匹配**：`Release::asset_for` 先用**完整 target triple** 匹配资产名，失败后退化为
     `arch` + `os` 标记（`arch` 取 triple 首段，`os` ∈ `linux`/`darwin`/`windows`/…）。
     **`bin_name` 不参与资产名匹配**。因此资产名必须包含 target triple，例如
     `BongoCat-<version>-aarch64-apple-darwin.tar.gz`。
   - **归档类型按扩展名判定**（`detect_archive`）：`.zip` / `.tar` / `.tar.gz` / `.tar.xz` / `.gz` /
     `.xz` 走对应解压器，**其余任何扩展名（含 `.exe`）落入 `ArchiveKind::Plain(None)`，即当作裸
     单文件直接使用**。因此 Windows 资产**不必是压缩包**，一个裸 `BongoCat.exe` 就能被消费；macOS
     bundle 模式则必须用归档。
   - **归档内布局**：bundle 模式走 `install_bundle`，它用 `Extract::extract_into` **解压整个归档**
     再从解压根取 `bundle_path_in_archive`，因此归档根必须是 `BongoCat.app/` 目录
     （注意 `Extract::extract_file` 只能取单个文件，取目录会失败——这就是 bundle 模式用
     `extract_into` 的原因）。单文件模式下 `bin_path_in_archive` 由 `bin_name` 派生为
     `bongocat-app.exe`，裸文件或归档内的该路径都必须是**根级**，多套一层目录会直接报错而非
     静默取错文件。
   - **现状**：`scripts/package-macos.sh` 只产出 `target/package/BongoCat.app`，**不产归档**；
     `scripts/build-windows.ps1` 只产出 NSIS 安装器 `BongoCat-$version-x64-setup.exe`。
     两者都还需要新增发行步骤，本次未实现。
     > 注（2026-09-14）：上面两个脚本已随 ADR-0033 删除。当前打包入口只产出 `.app`、`.dmg` 与
     > Windows NSIS 安装器，**仍然不产可更新归档**，结论与上面一致：更新资产是一项未完成的
     > 发布门禁。ADR-0033 记录了新的打包链路与这条缺口。
   - 上述布局要求已在本机实测（不依赖网络）：
     `crates/bongocat-update/tests/archive_layout_capability.rs` 用库自身的 `Extract` 验证
     macOS 归档整包解压后 `BongoCat.app/` 位于根部且携带 `Resources/`、Windows 归档根级的
     `bongocat-app.exe` 可被取出、以及被多包一层目录时会失败且不留残留。
   - **Windows 多文件更新需要自行编排**：`github::Update::update()` 的单文件模式只替换可执行文件，
     **不会更新 `resources/`**，预置模型或 `resources/` 内容的变更无法经由此路径下发。库为此提供了
     公开原语：用 `Download` + `Extract` 取得文件，再用 `MoveAll` 完成「二进制 + 附属资源」的事务式
     替换（全成或全回滚）。该语义已在本机实测确认，见
     `crates/bongocat-update/tests/multi_file_install_capability.rs`。代价是这条路径**不走**
     `update()` 编排，需自行保证暂存目录与目标同文件系统、zipsign 校验仍然执行、以及失败后的
     启动恢复。**本项目尚未实现这条路径。**
   - macOS 归档需携带已签名并公证的 `.app`；`package-macos.sh` 目前只做 ad-hoc 签名，
     分发签名与公证仍是独立发布门禁。
4. **断电/强杀残留的清理策略仍未决定，但残留形态已查实**（读 `self_update 1.3.0`、
   `self-replace 1.5.0` 与 `tempfile` 源码）：
   - 下载与解压暂存使用 `tempfile::TempDir`（默认前缀 `.tmp`）：单文件模式建在**系统临时目录**，
     macOS bundle 模式建在**安装目标的父目录**（即 `.app` 旁边）。正常返回或 panic 时 `Drop` 会删除，
     **只有硬断电或 SIGKILL 才会留下 `.tmp*` 目录**。
   - Windows 的单文件替换由 `self-replace` 完成：它先把当前 exe **改名挪开**，再复制自身为
     `*.__selfdelete__.exe`、以 `FILE_FLAG_DELETE_ON_CLOSE` 打开并 spawn 该副本，等父进程退出后
     删除被挪开的旧 exe。硬断电可能同时留下被挪开的旧 exe 与 `.__selfdelete__.exe` 副本。
   - 本项目**不提供**启动时清理。任何清理都必须 (a) 不在更新路径内运行、(b) 绝不删除可能属于
     进行中替换的 `.__selfdelete__.exe`、(c) 不与并发更新争用同一暂存目录。
   - 附带约束：自有可执行文件**不得**以 `.__selfdelete__.exe` 结尾——`self-replace` 的删除胶水按该
     后缀判定，误命名会触发非预期行为。
5. CI 覆盖面：`ci.yml` 的 `dependency-policy` 作业会运行
   `./tools/check-dependencies.sh`，`workspace` 作业在 Windows/macOS/Ubuntu 三平台
   运行 fmt / Clippy / test / release check / production 环境构建。因此本 ADR 的门禁已在 CI 覆盖，
   但仍未在**真实发行流程**中验证：发行资产命名、签名与安装包产物未与本 ADR 的 target 匹配规则
   对齐（见待验证项 3）。
6. `StorageLayout::updates`（`<env>/updates/`）当前**无写入方**：`self_update` 把解压暂存目录建在
   可执行文件旁边，不落在环境根下。该目录被保留为环境私有的保留命名空间，仍由环境形状断言与
   0700 权限断言覆盖。若确认不需要，可另行移除；本次未动，以免改变已文档化的环境目录形状。

## 后续边界

本 ADR 不实现 endpoint、公钥注入、update worker、更新 UI、OS 包签名验证或启动恢复。
在待验证项 1 与 2 完成前，不得声称更新功能或 stable 发布完成。

---
<a id="adr-0033"></a>
# ADR-0033: Build, Packaging and Release Toolchain

状态：已接受（2026-09-14）
取代：ADR-0010 的 Windows target 集合、ADR-0023 的 NSIS toolchain 与 wrapper 边界

## 背景

BongoCat 之前的构建体系是自己维护的：

| 组件 | 之前的状态 |
| --- | --- |
| `justfile` | 通过 `os()` 分支调用两个平台脚本 |
| `scripts/` | `build-macos.sh`、`package-macos.sh`、`build-windows.ps1`、`package-windows.ps1` |
| `windows/installer/BongoCat.nsi` | 自维护的 56 行 NSIS 脚本 |
| GitHub Actions | 只有 `ci.yml`（PR 触发）；**没有任何 release workflow** |
| 版本号 | 只有 `[workspace.package].version` 一个来源，但 macOS / Windows 各自重新解析一次 |

这套体系的问题不是"实现得不好"，而是它同时承担了三件事：编译、打包、发布。
平台判断、架构判断、目录布局、`Info.plist` 注入、图标校验、provenance、NSIS 调用
全部由项目自己实现，任何一个平台细节变化都要改项目代码。Windows 侧还要求发布机器
预置 `nsis-3.11-setup.exe` 与 `makensis.exe` 并通过 `BONGOCAT_NSIS_SETUP_PATH` /
`BONGOCAT_MAKENSIS_PATH` 注入——这是"依赖个人机器状态"的构建，不是项目级构建。

本次重构的目标由用户明确给出：三个发布目标、项目级配置、成熟开源工具、薄 `just`
入口、删除 `scripts/`、本地与 CI 同一套体系、版本号唯一来源。用户同时明确：现有 ADR
是参考资料而不是约束，与最终目标冲突时以目标为准。

### 发布目标

- `x86_64-pc-windows-msvc` → NSIS 安装器（`.exe`）
- `x86_64-apple-darwin` → `.app` + `.dmg`
- `aarch64-apple-darwin` → `.app` + `.dmg`

Windows ARM64 **不再是产品目标**。Cubism Native R5 没有 desktop ARM64 Core，而
Windows 自身的 x64 仿真可以运行 x64 应用，因此维护一个无法端到端验证的原生
ARM64 目标只增加成本和发布风险。ADR-0010 的 ARM64 结论作废。

## 决策

### 1. `cargo-packager` 作为唯一打包工具，并以库形式集成

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

### 2. 删除自研构建与打包脚本

- 删除 `scripts/`（4 个文件，约 14 KB shell / PowerShell）。
- 删除 `windows/installer/BongoCat.nsi`（由 `cargo-packager` 的模板与
  `install-mode` 配置取代）。
- `justfile` 只保留统一入口，不含平台判断、复制、打包或版本处理逻辑：
  `just build [--target ...] [--environment ...] [--formats ...]`。
- `BONGOCAT_NSIS_SETUP_PATH` / `BONGOCAT_MAKENSIS_PATH` 与 NSIS 3.11 的 MD5 固定
  要求一并作废：NSIS toolchain 由 `cargo-packager` 自动获取并缓存。

### 3. 版本号唯一来源

`[workspace.package].version` 是唯一来源。`bongocat-packaging` 继承该版本，
因此 Cargo 在打包进程启动前就解析完成；`--print-version` 把同一个值暴露给发布流水线，
release workflow 用它校验 tag 与版本一致，不再出现"脚本一个版本、CI 一个版本"的局面。

### 4. `.dmg` 由操作系统磁盘映像工具产出（有明确退出条件的偏离）

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

### 5. Release workflow 由项目自己描述，原生 runner 构建

新增 `.github/workflows/release.yml`，在 tag 推送时于原生 runner 上运行同一套
`just build`：

- `windows-latest` → `just build`（NSIS 安装器）
- `macos-latest` → `just build --target aarch64-apple-darwin`（`.app` + `.dmg`）
- `macos-26-intel` → `just build --target x86_64-apple-darwin`（`.app` + `.dmg`）

两个架构都用 `--target` 显式指定，因此产物集合不依赖 runner 恰好是哪一种架构。runner 则
按目标架构分开，修订见文末「macOS 两条 leg 分 runner 构建（2026-09-30，ADR-0078）」。

## 备选方案

- **`cargo-dist` 0.32.0**：官方 GitHub Actions 生成、archives 与包管理器集成都很强，
  但它的产物模型里**没有 `.app` 或 `.dmg`**——macOS 只产出 `tar.xz` 归档，installer
  是 shell/PowerShell/Homebrew/NPM。BongoCat 明确要求 `.app` 与 `.dmg`，这是硬性阻塞，
  不是偏好差异。它的设计中心是"CI 发布归档"，也不适合"开发者本地一次命令拿到 `.dmg`"。
- **直接使用 `cargo-packager` CLI**：需要 `cargo install cargo-packager`，版本无法随仓库
  固定，且把配置放到 `Packager.toml` 会削弱编译期检查。库形式严格更好。
- **保留自研脚本**：正是本次要删除的东西；它已导致 Windows 打包依赖发布机器预置的
  NSIS 路径。
- **其他 Rust DMG 构建方案**：crates.io 上不存在成熟的 DMG 安装器构建 crate
  （`apple-dmg` / `udif` 是读写库，不是安装器构建器）。
- **`tauri-bundler`**：`cargo-packager` 的前身，桌面框架耦合更深，没有额外收益。

## 影响

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

## 已接受的残余风险

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

## 验证

- `just build` 在本机（macOS 26.5 / aarch64）产出 `BongoCat.app` 与
  `BongoCat-<version>-aarch64.dmg`；`plutil` 断言 bundle id、`LSMinimumSystemVersion`、
  `LSMultipleInstancesProhibited`、版本号；`codesign --verify --deep --strict` 通过；
  `.app` 内含 `Contents/Resources/models/{standard,keyboard,gamepad}` 与
  `Contents/Resources/build-provenance.json`。
- `just build --target x86_64-apple-darwin` 产出 x86_64 的 `.app` 与 `.dmg`。
- `tools/tests/` 下的契约测试改为断言新的打包配置：target 集合、产物集合、
  Windows 安装器发布名、bundle id、最低系统版本、版本号来源、与 `bongocat-update` 的
  target/命名一致性。
- release workflow 用 `just version` 校验 tag。

## 后续

1. ~~为更新流程产出按 target triple 命名的归档并接入 `zipsign`/`RELEASE_SIGNING_KEY`。~~
   已由 ADR-0034 取代：更新载荷改为共享 manifest 指向的
   `BongoCat-<version>-<arch>.app.tar.gz`（`aarch64` / `x64`）与 NSIS 安装器，并用 detached
   minisign 签名；`RELEASE_SIGNING_KEY` 已内嵌公钥。真实 GitHub 发布与安装链路仍需端到端验证。
2. `cargo-packager` 修复 DMG 后删除本项目第 4 条的偏离。
3. 原生 Intel 机器与干净 Windows 10 1903+ / Windows 11 profile 上的安装、升级、
   卸载与回滚验证。
4. 上报 `create-dmg` / `cargo-packager` 的 `hdiutil` 解析缺陷。

## macOS 两条 leg 分 runner 构建（2026-09-30，ADR-0078）

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

---
<a id="adr-0034"></a>
# ADR-0034: Detached Minisign Update Trust Model
> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

状态：已接受（2026-09-14；§3 的 manifest 形状于同日修订，见该节修订说明）
取代：ADR-0029 的更新库与签名部分（正文保留为历史记录）

## 背景

ADR-0029 把更新栈换成 `self_update 1.3.0`，信任模型为 zipsign ed25519 **归档内嵌**签名。
换实现之后暴露了两个问题，两个都是"能力缺失"而不是"实现缺陷"，且都由源码阅读确认：

1. **zipsign 只能签 `.zip` 与 `.tar.gz`。** `self_update` 的 `detect_archive()` 按扩展名判定
   归档类型，裸 `.exe` 与 `.dmg` 落入 `ArchiveKind::Plain(None)`，`verify_signature()` 直接返回
   `Error::NoSignatures`。而产品在 Windows 上发布的更新产物就是那个 NSIS 安装器 `.exe`。
   一旦 `verifying_keys` 非空，**Windows 必然验签失败**；不配置密钥则等于不验签。
2. **`self_update` 的替换语义不适用于系统安装器。** 上游 `src/lib.rs:302` 明确：
   `.deb` / `.msi` 这类系统安装器不在 replace-and-verify 语义内，需要调用方自己交给
   `dpkg -i` / `msiexec /i`。NSIS `.exe` 同理——用安装器去替换 `bongocat-app.exe` 在语义上
   就是把应用本体换成一个安装器。走 zipsign 就意味着 Windows 必须自研
   「下载 → 验签 → 静默运行安装器 → 重启」这条编排。

同时维护者给定了产物形状约束：macOS 为 `.dmg` + `.app.tar.gz`，Windows 只有 `.exe`，
两平台各只额外增加一个 `.sig`，不引入其他文件。这正是 Tauri 的 minisign 分离签名模型。

调研过程与结论的完整记录见 `docs/update-signing-guide.md`（解释性文档，非规范性）。

## 调研结论

已核实（2026-09-14，读 `cargo-packager 0.11.8`、`cargo-packager-updater 0.2.3` 与
`minisign 0.7.9` 源码，未执行任何真实签名或真实更新）：

- **签名端已经在依赖图里。** `cargo-packager 0.11.8` 公开 `pub mod sign`：
  `generate_key`、`save_keypair`、`sign_file`、`sign_file_with_secret_key`、`SigningConfig`；
  其 `minisign 0.7.9` 已由 `cargo-packager` 引入。**签名工具与产物出自同一个 crate**，
  不存在"两个工具各自演进"的漂移面。
- **`cargo-packager-updater 0.2.3` 是同一项目的消费端**，覆盖 manifest 获取 → 版本比较 →
  下载 → 验签 → 安装的完整链路，且**自带 Windows 安装步骤**：`UpdateFormat::Nsis` 会用
  PowerShell `Start-Process` 运行下载到的安装器，随后 `std::process::exit(0)`；
  `UpdateFormat::App` 则整包替换 macOS bundle。这正是 ADR-0029 缺失的那一半。
- **验签发生在安装之前。** `Update::download()` 读完 payload 后立即调
  `verify_signature(...)`，`install()` 只接受已验签的字节。失败即返回，不进入安装。
- **manifest 有两种形状**（`RemoteReleaseData`，`#[serde(untagged)]`）：*dynamic*（顶层
  `url`/`signature`/`format`，一份 manifest 描述一个 target）与 *static*（`platforms` 映射，
  键为 `<os>-<arch>`）。dynamic 形状没有 per-platform 条目，因此**它不可能报 target miss**。
- **`WindowsConfig { install_mode: Quiet }` → NSIS 参数 `["/S", "/R"]`**；`installer_args: None`
  不追加任何参数。`/S` 静默，`/R` 要求安装器结束后重启应用——Windows 路径依赖它，因为库
  自己 `exit(0)`。
- **`SigningConfig.private_key` 是 base64 文本，不是路径**（`decode_private_key` 先 base64
  解码再解析 minisign `SecretKeyBox`），所以 CI secret 可以直接整段传入，不需要先落盘。
- **minisign 与 zipsign 密码学上不互通**：minisign 用 BLAKE2b-512 预哈希并比对 8 字节 key ID，
  zipsign 用 SHA-512。换模型等于换信任模型，不是换封装。
- **`Config.pubkey` 是单个 `String`**：验签只支持**一把**公钥，没有多密钥 any-of 语义。

## 决策

### 1. 以 `cargo-packager-updater` 取代 `self_update`，保留 `bongocat-update` 的对外契约

采用 `cargo-packager-updater =0.2.3`（`default-features = false`，仅 `rustls-tls`）。
`bongocat-update` 的公开面不变：`ReleaseConfiguration`、`ReleaseChannel`、
`UpdateTargetTriple`、`UpdateRuntime::{check, install, restart}`、`UpdateError`、
`UpdateOutcome`、14 个稳定错误码与 10 项匿名计数。（错误码目录于 2026-09-15 增至 14 个：`ReleaseFetchFailed` 原本兼任「取不到发布信息」与「取到了但读不懂」，ADR-0035 把这两件事拆成两个码。新增码向后兼容，见该 ADR 的「真实端点首次运行的结果」。）

库的 `Error`（`#[non_exhaustive]`）、`Config`、`semver::Version` 与 `Url` 全部映射为项目
自有类型，不出现在 `diagnostics.rs` 或 app/UI 协议中；未识别的库变体降级为
`update_internal_failed`，不把库文本带进诊断导出。

### 2. 信任模型：detached minisign 签名，密钥缺失时失败关闭

`RELEASE_SIGNING_KEY: Option<&str>` 存放 **base64 的 minisign 公钥盒文本**（即
`cargo packager signer generate-key` 写出的 `.pub` 文件内容）。仓库现已内嵌一把发布公钥
（key ID `DF5E2C9D255DD85E`），Production 构建的 `UpdateRuntime::is_available()` 因而可以为真。
公钥为 `None`、空串或纯空白时，runtime 在**发出任何请求之前**返回
`update_signature_key_missing`；系统菜单不会在没有有效公钥时显示「检查更新」入口。

这条门禁是刻意的第二道防线。库对空 `pubkey` 的行为是解码失败（`Error::Base64` /
`Minisign`），即 fail-closed 而非 ADR-0029 记录的 `self_update` fail-open；但项目仍保留
自己的门禁，因为它产出的是稳定、无路径的错误码，而不是一个库错误。

### 3. 一份共享 manifest，static 形状

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

### 4. 更新载荷：macOS 用 bundle 归档，Windows 复用已发布的安装器

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

### 5. 签名在 `crates/bongocat-packaging` 内完成，且是最后一步

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

### 6. 退役 `self_update` 专属的能力测试，改为钉住真实的签名/验签对

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

### 7. 更新源的 GitHub 代理回退（2026-09-15 增补）

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

## 被取代的能力与损失

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

## 已接受的残余风险

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

## 验证

已完成（2026-09-14，本机 macOS 26.5 / aarch64）：

- `cargo fmt --all --check` 通过。
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` 零警告。
- `cargo test --locked -p bongocat-update`：17 个单元测试 + 8 个 `release_manifest_capability`
  测试通过，含共享 manifest 的名称、平台键查找与 target 无关的 endpoint 断言。
- `cargo test --locked -p bongocat-packaging`：12 个单元测试通过，含合并产物被更新库自身读取
  类型反序列化成功、版本不一致/非法平台键/空输入被拒绝。
- `python3 -m unittest discover -s tools/tests` 全部通过，含 manifest 资产名一致性契约与
  依赖策略目标一致性契约。
- 端到端冒烟：用三份 fragment 跑 `just manifest`，产出的 `latest.json` 含 `version` 与三个
  `platforms` 键，Windows 条目为 `nsis`、macOS 条目为 `app`。
- 依赖图：`self_update`、`zipsign-api`、`ureq`、`sha2`、`semver` 已从 workspace 依赖与
  `Cargo.lock` 中移除；`ed25519-dalek` 的失效声明一并删除。`cargo update` 无待解析变更。
- 签名器与验签器的互操作由 `release_manifest_capability.rs` 实测锁定（**生成真实密钥对、
  真实签名、真实验签**，只是走 loopback 而非 GitHub）。

## 待验证项（不得当作已确认）

1. **真实发布链路**：从真实 GitHub Release 下载、验签并安装，未执行。
   - 补充（2026-09-15）：**manifest 获取这一步已在真实端点上执行**。结论是端点被旧产物占用——
     `releases/latest/download/latest.json` 返回 200，但内容是旧 Tauri 版本的 updater manifest
     （平台键 `darwin-*`、条目缺本库必需的 `format`、签名为 Tauri 私钥），因此客户端在平台查找前
     反序列化失败。下载、验签与安装仍未执行，线上尚无可用的新流程 release。排查细节、由此修正的
     诊断粒度与新增的能力测试见 ADR-0035 的"真实端点首次运行的结果"。
2. **Windows**：安装器静默安装、`/R` 重启、被占用可执行文件的替换、安装失败回滚，均未实测。
3. **`.app` 的 codesign + notarize**：当前打包只做 ad-hoc 或注入身份的 `codesign`，
   公证仍是独立发布门禁。归档必须在公证**之后**生成，否则签名对象不是最终分发的 bundle。
4. **密钥轮换**：单公钥模型下，换钥匙必须先发布一个"认识新钥匙"的版本，否则已安装用户
   的更新链直接断裂。轮换流程尚未设计。
5. **降级攻击**：仍只按 semver 比较。manifest 由 HTTPS 保护，但能够替换 manifest 的攻击者
   可以把客户端指向一个**旧但签名有效**的载荷。
6. **内存占用**：库把整个载荷读进内存后验签，大产物下的峰值内存未测量。

## 后续边界

本 ADR 不实现 endpoint 配置界面、公钥注入的自动化、update worker、更新 UI、操作系统包签名
验证或失败启动恢复。在待验证项 1 与 2 完成前，不得声称更新功能或 stable 发布完成。

---
<a id="adr-0035"></a>
# ADR-0035: 更新 worker、更新窗口与发布说明
> ⚠️ 本决策（或其 Windows 部分）随 **Windows 支持的移除（ADR-0099）** 失效，正文保留作历史记录。

状态：已接受（2026-09-15；2026-09-25 修订自动检查默认值与间隔）
依赖：ADR-0034（detached minisign 信任模型）、ADR-0029（第三方库边界）、ADR-0016（匿名诊断导出）、ADR-0059（UI protocol 边界）

## 背景

ADR-0034 交付了更新链路与信任模型，并在「后续边界」中明确排除了四件事：endpoint 配置界面、
公钥注入自动化、**update worker**、**更新 UI**，以及操作系统包签名验证与失败启动恢复。

在此之前，产品里与更新相关的可见能力只有两处，且都不完整：

- 设置页有一个「自动检查更新」开关，它只把 `updates.check_automatically` 写进配置，
  **没有任何代码读取它**——打开它不会有任何行为。
- 系统菜单有一个「检查更新」入口，由 `bongocat_app::update_check_available()` 决定是否显示，
  但它的处理分支是 `SystemMenuAction::CheckForUpdates => Ok(true)`，即**点下去什么都不发生**。

也就是说：更新管线是完整且被测试覆盖的，但**没有任何调用方**。本 ADR 记录把这条链路接成
一个完整产品功能所采用的决策。

## 决策

### 1. 更新 worker 与设置 worker 分开，各自一条线程

新增 `bongocat-app::ApplicationUpdateService`：一条名为 `bongocat-update-service` 的线程独占
`UpdateRuntime`，是**唯一**触碰网络、下载和安装的组件。

不复用 `ApplicationSettingsService` 的命令循环。设置循环是串行阻塞的：把 check/download/install
放进去，会让一次百 MB 级传输期间的所有设置读写排队。两者生命周期也不同——设置服务在退出序列里
先于 overlay 结束，更新服务可以在安装完成后仍在运行。

### 2. worker 只发布状态，不推送事件

worker 把当前阶段写入 `UpdateStateHandle`（`Arc<Mutex<UpdateSnapshot>>` + revision），窗口按
`UPDATE_STATE_POLL_INTERVAL`（250 ms）读取。理由：

- 窗口关闭、销毁或渲染慢，都不会阻塞或反压 worker；下载进度是覆盖写，不是队列。
- revision 只在阶段真正变化时推进，同一个阶段重复发布不推进 revision，轮询因此不会无意义重绘。
- 命令通道（`UpdateClient` → `UpdateCommand`）保持有界且非阻塞（`try_send`），窗口不会因为
  worker 忙碌而卡住。

**关闭窗口不取消任何操作**，因为操作不属于窗口。这是刻意的：库没有取消钩子（见 §9），一个
"取消"按钮只能让 UI 停止显示，而传输仍在继续——那是在撒谎。窗口因此始终可以关闭。

### 3. UI protocol 拥有跨层 contract，app 负责适配

`bongocat-ui-protocol` 定义 `UpdateSnapshot` / `UpdatePhase` / `UpdateClient` 等全部更新面
类型，`bongocat-ui` 只负责 GPUI view 和展示策略。`bongocat-ui-protocol` 与 `bongocat-ui` 都不
依赖 `bongocat-update`。`bongocat-app` 做映射，且映射是穷尽 `match`：

- `UpdateStage` → `UpdateFailureStage`
- 14 个 `UpdateErrorCode` → UI 的同名目录

新增一个 stage 或 code 会让 `bongocat-app` 编译失败，直到它被赋予用户可见的含义。这沿用了
`SettingsRuntimeErrorCode` 等既有做法，也让 `cargo-packager-updater` 的类型不越过应用边界。
代价是两份字符串目录，由 `bongocat-app` 的测试逐项比对锁定。

### 4. 三个阶段可观测，靠拆分库调用实现

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

### 5. 重启要求是平台事实，不是运行时判断

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

### 6. 自动检查由 GPUI 侧驱动，并在发现更新时打开窗口

开关与间隔值在用户配置里，只有设置服务读得到，所以调度放在 `main.rs` 的 GPUI 侧，而不是 worker 里：
启动后等 10 秒（不与启动争网络和窗口），之后按配置中的整小时数等待下一次检查。自动检查开关
默认 `false`，因此新配置不会主动发起检查；用户显式打开后才启动该调度。间隔默认 `24` 小时，
接受 `1..=8760`；调度器以最近一次实际派发为期限锚点，并通过只读取自动更新设置的轻量轮询重新
安排下一次检查，因此修改间隔会重排期限。关闭自动检查不会清空该值，手动检查也不受它影响。该补充取代本 ADR 最初
记录的“固定 24 小时且间隔未持久化”状态。

发现可用更新且窗口未打开时**打开更新窗口**。替代方案（什么都不做）会让这个开关变成一个
不可观测的设置；这与 §8.4 里"开关存在但无行为"的旧状态没有本质区别。窗口是单例，重复检查
不会叠窗口。

### 7. 发布说明随 manifest 一起走

`crates/bongocat-packaging` 的 `--merge-manifests` 增加 `--release-notes <file>`，把说明写进共享
`latest.json` 的顶层 `notes`；runtime 把它透传为 `UpdateRelease.notes`，窗口在「更新内容」区域
渲染，并另给一个指向 `releases/tag/v<version>` 的链接。

- **不新增第二次网络请求**：说明和版本、载荷地址来自同一份 manifest，检查更新时一次拿到。
- **不新增依赖**：说明由发布工作流用 `gh api .../releases/generate-notes` 生成（工作流本来就用
  `--generate-notes`），只是改为先生成再同时喂给 manifest 和 `gh release create --notes-file`，
  因此发布页和客户端看到的说明不会分叉。
  - 补充（2026-09-16）：**这一条已被取代**。`generate-notes` 给的是两次 tag 之间的提交摘要（PR 与
    commit 列表），而本项目对外发布的是手写的双语 changelog；两者内容不同，让发布页显示提交摘要
    就等于放弃了 `CHANGELOG.md` / `CHANGELOG.zh-CN.md` 作为事实来源。现在由
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
    `docs/changelog-conventions.md` §1.3。
- `pub_date` 仍然不写：库把它当可选，新鲜度由版本决定，而打包工具没有日期格式化能力。

### 8. 传输有界

`UpdaterBuilder::timeout(UPDATE_REQUEST_TIMEOUT)` = 30 分钟。transport 自身没有超时，不设界的话
一条卡死的连接会让 worker 永久阻塞。取值刻意宽松（覆盖整条载荷传输，而不是单次读取），因为它
要逃离死连接，不是约束慢链路；窗口不阻塞在传输上，可以随时关闭。

### 9. 明确不做的三件事

- **取消下载**：库在 `download_extended` 内部读完全部载荷并验签，没有 abort 钩子。要做到真取消
  只能自研下载与验签——而自研验证层正是 ADR-0029/0034 删掉的东西。因此不提供取消按钮，并在
  runtime 的文档里写明原因。
- **安装前 quiescence**：见下节。
- **操作系统包签名验证（codesign / Authenticode 的客户端校验）**：仍是独立发布门禁，本 ADR 不碰。

## 需要维护者复核的一处偏差

`AGENTS.md` §10 与 TODO §8.4 的条目是「**安装前**协调 runtime/renderer shutdown，失败可回滚」。
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

## 真实端点首次运行的结果（2026-09-15）

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

## 残余风险与待验证项（不得当作已确认）

1. **真实发布链路未跑通**：manifest 获取这一步已在真实端点上执行并完成定位（见上一节），但没有
   任何一次真实 GitHub Release 被**下载、验签、安装**过——线上没有由新流程发布的 release。窗口的
   其余状态由离线脚本化 engine 测试覆盖，真实链路仍见 ADR-0034 待验证项 1。
2. **Windows 安装路径未在本机验证**：本机是 macOS。`Installed` 在 Windows 不可观测这一点来自源码
   阅读，未实测；窗口在该平台的 `installed_relaunching` 文案因此也未被真实观察到。
3. **macOS 自动重启未随真实安装端到端验证**：重启已从 `exec` 改为 `spawn`（见 §5，附托盘图标消失的
   原因与不变量测试），但「安装成功 → 自动重启 → 托盘图标仍在」这条完整链路仍需要一次真实安装
   才能确认。启动新构建失败时进程以退出码 1 结束并写 stderr，用户需要手动启动。
4. **发布说明注入未在真实发布中跑过**：`gh api .../generate-notes` 的形状、`--notes-file` 与
   manifest `notes` 的一致性只有单元测试与工作流静态检查，没有真实 tag 发布证据。说明来源已于
   2026-09-16 改为 changelog 提取（见 §7 补充），本项仍未消除：`just release-notes` →
   `just release-manifest` 已在本机端到端跑通，但没有真实 tag 发布证据，也没有在 GitHub 发布页上
   验证过合成后的 Markdown 渲染。
5. **manifest 仍无防降级保护**：说明字段不改变这一点，能替换 manifest 的攻击者仍可把客户端指向
   旧但签名有效的载荷。
6. **库的内存行为未变**：验签前把整个载荷读进内存，大产物下的峰值内存仍未测量。

## 验证

已完成（2026-09-15，本机 macOS 26.5 / aarch64）：

- `cargo fmt --all --check` 通过。
- `cargo clippy --locked --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings`
  零警告；`bongocat-app` 的 `storage-test-injection` 与 `production` 两种 feature 组合各自
  `--all-targets -D warnings` 通过。
- `cargo test --locked --workspace` 全部通过。
- `bongocat-update`：27 个单元测试 + 10 个 `release_manifest_capability` 测试通过，新增 stage 归属、
  `UpdateEvent`、`unavailability()`、传输上限、发布页 URL，以及"取不到 manifest"与"manifest 读不懂"
  分成两个码的定向测试；能力测试另在 loopback 上固定了两种失败形态（无 manifest →
  `ReleaseNotFound`；旧 Tauri 文档 → `Serialization`，并用只补 `format` 的对照实验证明原因）。
- `bongocat-app::update`：26 个测试通过，其中 11 个用脚本化 engine 在**真实 worker 线程**上驱动
  状态机，覆盖 check 的三种结果、失败的 stage/code 归属、安装的四步顺序、无 release 时 install
  不改状态、不可用构建不进入管线、重启请求只被消费一次，以及 drop 会停止线程。
- `bongocat-app`（bin）：新增 `restart_delay_elapsed` 的边界测试（未到延时、刚好到、以及单调时钟
  回退时不得提前重启）。
- `bongocat-ui` 的**无头渲染测试**（`update_window::render_tests`，7 个）：这是本项目第一批真正
  渲染窗口的测试。`bongocat-ui` 的 dev-dependency 里给 `gpui-kit` 打开 `test-support`，GPUI 因此
  使用测试平台，可以在没有显示器的情况下开窗、布局、绘制，并对注册过的元素做查询与点击。
  覆盖：18 个阶段逐一绘制且 footer 只提供该阶段允许的动作（`render` 是否真的遵守 `offers_*`
  以及每个变体能否活着画完，只有这里能看出来）、检查按钮文案随阶段区分、更新内容区跟随
  manifest 的 `notes`、以及**完整链路**（共享状态 → 窗口的 250 ms 轮询 → view 快照 → 渲染）。
  为可查询，产品代码里给 footer 动作与更新内容区加了 `.test_support()`；`test-support` 关闭时
  它是恒等函数，正常构建不受影响。
- 临时模拟面板（`update_simulation.rs`，为人工逐一切换查看每种状态而临时加入）已于 2026-09-15
  整体删除：全部 `SIMULATION-HOOK` 标记清除，默认窗口高度恢复 460，面板自身 6 个测试与 3 个
  依赖面板的渲染测试一并移除，`render_tests` 只保留不依赖面板的永久用例。
- `bongocat-ui::update` / `update_window`：13 个测试覆盖错误码目录唯一性、stage 往返、阶段可用动作、
  revision 语义、客户端与共享状态的一致性，以及"每个错误码都能解析出文案而不是解析出它自己的
  key"。后者用删除 key 的方式双向反证过：两个语言都删则它失败，只删 `zh-CN` 则它通过而
  `bongocat-i18n` 的跨语言一致性测试失败——`rust_i18n` 的 `en-US` fallback 意味着"某个语言缺
  文案"由那条既有测试守卫，本测试只负责"所有语言都缺"这一种。
- `bongocat-packaging`：16 个测试通过，新增发布说明写入、空说明不写入、超长按字符边界截断。
- `python3 -m unittest discover -s tools/tests` 全部通过（57 项）。
- 端到端冒烟：用三份 fragment 跑 `just release-manifest`，产出的 `latest.json` 含 `version`、
  `notes` 与三个 `platforms` 键，Windows 条目为 `nsis`、macOS 条目为 `app`。
- 产品启动冒烟：`target/release/bongocat-app --run-seconds 12` 退出码 0、无 stderr 输出。12 秒跨越
  了 10 秒的自动检查起点，因此同时验证了更新服务能启动、自动检查在 Development 构建上被
  `unavailability()` 拦下而不发请求、以及退出序列能 join 更新 worker 而不记录失败。
- 真实端点首次运行（2026-09-15）：在真实安装产物上点击检查更新，窗口正确显示"当前版本"、真的
  发起了请求，并诚实报出失败；随后用 `curl` 取回线上 manifest 完成字段级定位（见上一节）。
  这是真实链路第一次被走到网络层，结果是"端点被旧产物占用"而非客户端缺陷。
- **未验证**：更新窗口的**视觉**（间距、字号、是否重叠）没有实机截图证据，键盘导航也没有实机
  走查；无头渲染测试证明的是"每个阶段都画得出来、该有的动作都在、文案正确"，不是"看起来好看"。
  按 §11 的 800x600 / 125-200% 缩放条件核对仍待人工完成。下载与安装仍未在真实 release 上跑通
  （线上尚无可用的新流程 release）。
- **依赖面变化**：本次给 `bongocat-ui` 增加了一个 **dev-dependency**（`gpui-kit` 的
  `test-support` feature），只影响测试构建。它把 `proptest` 及其传递依赖（`bit-set`、`bit-vec`、
  `rusty-fork`、`wait-timeout`、`quick-error`、`convert_case`、`proptest-macro`）带入 lock；
  `proptest` 本就在 workspace 依赖里，因此没有引入新的依赖家族。按 §9 跑了完整 `cargo update`，
  只升了 `camino 1.2.5 → 1.2.6` 与 `rustls 0.23.44 → 0.23.45` 两个补丁版本。
- **正式依赖变化**：更新内容渲染 Markdown 新增了一个正式依赖 `pulldown-cmark =0.13.4`
  （当时 crates.io 最新稳定版，MIT，`default-features = false` 去掉 `getopts` CLI 解析与 `html`
  渲染器——说明渲染为 GPUI 元素，不产出 markup）。纯解析器、无 I/O、无 `unsafe`，停止维护时的
  替换边界是自写一个受限于标题/列表/强调/链接/代码块的极简解析器；GitHub 风格表格刻意不支持。

已完成（2026-09-16，说明来源改为 changelog 提取，本机 macOS / aarch64）：

- `bongocat-packaging`：23 个测试通过（原 16 个 + 新增 7 个）。新增覆盖：
  条目按二级标题匹配（正文提及的版本、`###` 子标题、围栏代码块里的标题都不参与）、两种标题写法
  与"版本必须是完整 token"（`<version>-rc.1`、`<version>.1`、`11.1.0`、`##1.1.0` 都不匹配）、
  CRLF 归一、缺失条目的错误信息含该文件实际记录的版本、`--extract-release-notes` 与构建/合并选项
  互斥、仓库两份 changelog 记录的版本列表一致且非空。
- 端到端：`just release-notes` → `just release-manifest` 用三份 fragment 跑通，产出的 `latest.json`
  的 `notes` 为 6168 字节，结构为「英文条目 → `---` → 中文条目」，不含版本标题，远低于 32 KiB 上限。
  另单独验证了门禁：版本在 changelog 里没有条目时命令以退出码 1 失败，stderr 为
  `CHANGELOG.md has no release notes for <version>; it documents <列表>`。
- `tools/tests/test_release_changelog_contract.py` 新增 6 个用例；`python3 -m unittest discover
  -s tools/tests` 共 63 项。
- **既有失败（非本次引入）**：`test_product_version_contract.py` 的
  `test_runtime_and_ui_use_the_compiled_product_version` 失败——该测试要求当前工作区版本
  字面量不出现在指定源文件里，而 `crates/bongocat-update/src/runtime.rs` 的测试夹具 URL 里有三处
  `v1.1.0`（1144、1149、1172 行，`HEAD` 版本即如此，本次未改动该文件）。夹具用哪个版本号不影响它
  要固定的形状，但修与不修属于该测试自己的取舍，未在本次改动范围内。本次新增的测试数据因此一律
  使用合成版本号（`9.9.9` 一类），避免新写死一个"当前版本"。
- **未验证**：合成后的说明在 GitHub 发布页上的 Markdown 渲染没有实机证据；真实 tag 发布仍未跑过
  （见残余风险 4）。

当前修订验证（2026-09-25）：

- `bongocat-config`、`bongocat-ui-protocol`、`bongocat-ui` 的定向/全量单元测试，独立
  `config-store` contract（含两个恢复测试），Draft 2020-12 schema、locale 校验、定向 Clippy 和
  `cargo check --workspace --release` 通过。
- 调度器纯函数测试覆盖 `1` 小时、默认 `24` 小时、自定义 `48` 小时、最大 `8760` 小时、从实际派发
  锚点重排期限和轻量轮询上限；配置服务测试覆盖 `48` 小时持久化、关闭开关保留值、stale revision、
  `0`/`8761` 拒绝以及重启恢复。
- 当前 Windows 环境的全量 workspace 测试仍有一个未触及的模型封面路径规范化失败；工具测试另有
  symlink 权限和路径分隔符环境失败，均不改变本项配置/调度证据。

## 补充：更新窗口改为按内容定高（2026-09-26）

### 为什么改

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

### 改了什么

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

### 验证（2026-09-26，本机 macOS 26.5 / aarch64）

- `cargo fmt --all -- --check`、`cargo clippy -p bongocat-ui --all-targets --all-features -D warnings`、
  `cargo test --workspace`、`cargo check --workspace --release`、`python3 -m unittest discover -s
  tools/tests` 全部通过。
- `bongocat-ui::update_window` 的无头渲染测试从 7 个增加到 13 个。新增的 6 个分别固定：窗口高度落在
  自身区间内、单行状态塌缩而长说明撑开、同一内容从上下两个起点收敛到同一高度、每个状态都放得下动作、
  超预算的说明滚动而不是撑高窗口、用户拖动后不再被内容改动。另加 2 个纯函数测试（帧高度算术与夹取
  规则）和 1 个内边距对称性测试。
- 实机截图（macOS 26.5）：`Unavailable` 状态的窗口按内容收紧，标题、版本、状态与关闭按钮的位置和
  间距正常，无裁剪。
- **仍未验证**：Windows 上的窗口缩放没有实机证据（`window.resize` 走 GPUI 的跨平台路径，但按
  AGENTS.md §4 不得由 macOS 推断）；800x600 与 125/150/200% 缩放下的高度仍待人工核对。带说明的
  状态在 2026-09-26 之前没有实机截图，原因见下一节。

## 补充：更新窗口状态浏览器（2026-09-26 已删除）

### 为什么当时需要它

按内容定高之后，「这个状态看起来对不对」只能靠看，而大部分状态**没法看到**：Development 构建的更新
服务在发出任何请求前就被 `unavailability()` 拦下，所以 `Available` / `Downloading` / `Verifying` /
`Installing` / `Installed` 与各阶段 `Failed` 在构建它的机器上一条都到不了。此前唯一能看到的状态是
`Unavailable`——恰好是内容最少、最不需要看的那个。

合成点击也不可靠：一次 `System Events` 的合成点击会落到别的窗口上，而拿不到按钮的人不该被要求去按
真实的按钮。

### 做了什么，以及为什么现在删掉

`bongocat-ui::update_state_browser` 开一个开发专用窗口，每个按钮往**共享更新状态**里发布一个状态，
并请求应用把更新窗口叫起来。20 个按钮按「紧凑 / 更新内容 / 已安装 / 失败」分组——分组是重点：哪些
状态高，从状态名上看不出来，而分组把「过一遍」变成一次检查。驱动的是**真实窗口**而不是副本：副本就
不是要发布的那份代码了。

它是定高这项工作的脚手架，不是产品的一部分。定高与闪烁都改完、无头渲染测试已经逐个状态覆盖之后，
它唯一剩下的用途是人工过一遍十个状态——这件事无头测试在做，而且做得更多（每个状态都断言动作、
高度、内容）。留着一个 700 行的开发窗口、CLI 开关和 9 个只为它存在的测试，等于让发布构建多带一个
没人维护的界面。所以删除，`bongocat-ui` 与 `bongocat-app` 随之回到没有它的样子。

### 当时的验证（2026-09-26，本机 macOS 26.5 / aarch64）

- 9 个测试：点击按钮会发布该按钮自己的状态并请求显示窗口（而不是只把自己标成选中）；`Back to a real
  check` 真的退回真实状态；窗口能渲染的每种状态都有按钮；有两个分支的状态（`Unavailable` /
  `Installed`）与四个失败阶段各有一个按钮；分组名称连续且顺序稳定；面板不会开到屏幕外（含屏幕太窄的
  情况）；脚本入口发布的内容与按钮一致；示例更新内容确实长到窗口放不下。
- 实机截图：紧凑状态约 187px；`Installing` + 完整双语更新内容为 431px，更新内容取满预算后滚动，
  「在 GitHub 上查看」与「关闭」两个动作都在窗口内。这批截图是下面那节定高结论的实机依据。

### 删除时一并失去的东西

- **未验证**：Windows 上浏览器与更新窗口的并排位置；面板的键盘导航。这两项随浏览器一起消失，
  没有留下结论。
- 定高本身在 Windows 上的行为**不受影响**：它由无头渲染测试与下面的 trace 覆盖，与浏览器无关。
## 补充：消除开窗与状态切换时的高度闪烁（2026-09-26）

### 现象

用户报告：更新窗口在高度变化过程中会先瞬间变成上限高度，再迅速回退到内容实际需要的高度。

### 原因不是「算错」，是「算得太晚」

原来的实现把窗口开在上限高度（460），等第一帧测出内容高度再收下来。内容定高只能在布局之后知道，
所以从上限高度开始是当时能想到的唯一安全猜测——上限装得下任何内容，不会裁掉动作。代价就是那一下
可见的收缩。

真正多余的是**中间那一步**。旧代码在窗口比内容矮时返回 `TooSmall`，目标高度直接取上限：意思是
「先长到上限，再收敛回来」。而这个中间高度并非必需——内容列不收缩、空隙塌成 0，所以即使窗口装不下
内容，这一帧报出来的仍然是它**需要**的高度。`Fit::TooSmall` 分支因此整条删掉，测量一步到位。

### 显示时机：隐藏状态下量好、画好，再显示

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

### 顺带：窗口不允许拖拽改变尺寸

`is_resizable: false`，一行跨平台：macOS 去掉 `NSWindowStyleMaskResizable`，Windows 去掉
`WS_THICKFRAME`/`WS_MAXIMIZEBOX`。理由与「按内容定高」是同一条——允许拖拽等于让用户选一个下一次
状态变化就会撤销的高度。

于是「用户拖过就接管高度」那条分支（`observe_window_bounds` 比对上次请求的高度、`pinned` 状态、
`start_height_pinning`）连同它的测试一起删除：窗口没有 resize 手柄，那条分支是死代码。

### 平台取整

trace 里平台把请求的 410.5 变成 411，下一帧量到 410.5、又问一次。平台不给尺寸所以也不会回调，
循环自己停了，但每帧问一次平台给不出的高度是没道理的事，所以「已经在目标高度」允许半像素误差
（`HEIGHT_MATCH_TOLERANCE`）。

### 验证（2026-09-26，本机 macOS 26.5 / aarch64）

- 实机 trace（临时插桩，已移除）：开窗 `180 -> 187` 且 **在 187 才显示**；状态切到完整双语更新内容
  `187 -> 410.5` 一步到位；切到下载中 `187 -> 257.5` 一步到位；切到失败 `187 -> 207` 一步到位。
  **上限 460 从未成为目标高度。**
- 实机辅助功能读数：更新窗口 560x219（含标题栏，即内容 187），`AXZoomButton` 为 `missing value`
  （对照组：同一次运行里的其它窗口有 zoom 按钮）——不可缩放在 macOS 上确实生效。
- `bongocat-ui::update_window` 的无头渲染测试 13 -> 20 个。删掉 1 个（用户拖动接管），新增 3 个纯
  函数断言（过矮时测量仍准确、目标高度夹取、半像素容差），新增 1 个渲染测试固定「窗口被显示时那一帧
  是按它最终落定的高度画出来的」（从上限高度和从下限高度两个起点各验一次）。该测试在去掉
  `bounds_changed` 时确实失败，不是空转。
- 15 秒长跑：trace 行数不增长，无 resize 循环。
- `cargo fmt --all -- --check`、`cargo clippy`（workspace 去 `bongocat-app`，另加 `bongocat-app` 的
  `storage-test-injection` 与 `production` 两种 feature）、`cargo test --workspace`（44 个 target）、
  `cargo check --workspace --release`、11 个 `tools/tests` 脚本与 `tools/validate-locales.py` 全通过。

### 边界

- **未验证**：Windows 上的开窗时序。`resize` 在 Windows 上是否同样异步、隐藏窗口是否同样拿不到帧，
  本机无法验证；上面两处都写了 macOS 的具体原因，若 Windows 行为不同则需要按平台分支。
- **未验证**：800x600 与 125/150/200% 缩放下的高度。

## 补充：开窗即显示正在检查（2026-09-26）

### 现象

用户报告：打开更新窗口时先显示上次检查的结果（“已是最新”、上一次的更新内容或失败原因），
过一会儿才开始显示“正在检查”和进度条。

### 原因不是“慢”，是“请求发得太晚”

系统菜单与 About 页的「检查更新」都走 `open_update_window_and_check`，而它当时的顺序是
**先开窗并显示，再请求检查**。两件事叠起来才是这一次闪烁：

1. 窗口在 `open_window` 闭包里构造时读的是 `client.snapshot()`，此刻 worker 发布的仍然是上一次
   检查的阶段。窗口的第一帧——也就是用户看到的那一帧——因此画的是上一次的答案。
2. `view.check()` 随后才把 `Check` 命令发出去。命令是发给 worker 线程的消息，worker 取到它之后
   才发布 `Checking`；而窗口按 `UPDATE_STATE_POLL_INTERVAL`（250 ms）轮询共享状态。所以从点击到
   进度条出现之间，屏幕上一直画着旧阶段。

只把请求提前并不能消除它：即使开窗前就把命令发出去，worker 仍是另一个线程，窗口构造那一刻
`Checking` 未必已经发布。第一帧要显示这次检查，就得由窗口自己渲染它。

### 改了什么

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

### 与既有决定的关系

§2「worker 只发布状态，不推送事件」不变：worker 仍是共享状态的唯一写者，窗口仍然按 250 ms 轮询。
变化的是窗口对**自己刚发出的请求**的本地渲染，它有一个明确且可验证的终止条件（revision 前进），
而不是一个超时或猜测。窗口关闭不取消任何操作也不变：这次渲染的检查在窗口关掉后照常由 worker 完成。

### 验证（2026-09-26，本机 Windows）

- `bongocat-ui::update_window` 新增 5 个测试：2 个纯函数（`a_requested_check_renders_instead_of_the_previous_result`
  固定 revision 语义、`a_check_is_only_asked_for_when_one_would_start` 固定三个准入条件），3 个无头
  渲染（`a_window_opened_for_a_check_paints_that_check_from_its_first_frame` 固定第一帧与轮询都不把
  上一次结果放回来，`a_check_asked_over_an_unpolled_result_is_not_overwritten_by_it` 固定锚点取的是
  请求那一刻的 revision 而非上次轮询的，`a_window_opened_onto_the_published_state_offers_to_check`
  固定 `Current` 语义未被这次改动带偏）。渲染测试总数 20 -> 23，`bongocat-ui` 170 项通过。
- 反向验证：新渲染测试在去掉「构造期发请求」时失败（报 `a window opened for a check opened onto the
  result of the last one`），在去掉「轮询不采用未前进的 revision」时失败（报 `the result of the last
  check came back over the check being asked for`），在把锚点改回窗口上次轮询的 revision 时也失败（报
  `a result this window had not polled replaced the check it just asked for`）——三块改动各自承重，
  不是空转。
- `cargo fmt --all -- --check`、`cargo clippy`（workspace 去 `bongocat-app`，另加 `bongocat-app` 的
  `storage-test-injection` 与 `production` 两种 feature）、`cargo test --workspace`（44 个 target）、
  `cargo check --workspace --release` 通过。`tools/tests` 中 symlink 权限与路径分隔符两项为既有
  Windows 环境失败，与本改动无关。

### 边界

- **未验证**：实机上第一帧的观感。逻辑由无头渲染测试固定，但「打开时立刻看到进度条」这一眼仍然
  需要人看；本次没有启动产品做人工核对。

---
<a id="adr-0064"></a>
# ADR-0064: Human-readable bounded application logging

状态：已接受（2026-09-24）

## 背景

BongoCat 已由 `bongocat-log`、`bongocat-app::ApplicationLogHandle` 和
`bongocat-live2d::CoreLogHandle` 提供环境隔离、固定事件字段、文件权限、按 UTC 日/文件大小轮转、
目录级容量上限、Core callback 队列和 panic 脱敏。现有输出仍采用 application/Core JSONL；机器解析
较严格，但普通用户难以直接阅读，现有事件目录也只覆盖少量生命周期与导出失败路径，不能充分支持
模型、配置、输入、窗口、权限、更新和跨层交互排障。

本决策不重新引入第二套日志框架。现有 writer 已拥有项目需要的生命周期、隐私、容量和非阻塞 panic
边界；缺口是输出格式、可配置过滤、更多有界事件以及 application/Core 对同一文本格式与 retention
policy 的复用。

## 决策

### 1. 单一人类可读文本方案

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

### 2. 日志过滤与保留策略

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

### 3. 事件和级别

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

### 4. 隐私和性能边界

日志不得包含用户路径、真实按键序列、剪贴板、配置原文、模型正文、URL、密钥/凭据、签名材料或动态 OS 错误文本。
允许的上下文只包括闭合 stable code、匿名/可移植 model ID、origin、阶段、计数、字节数、revision、
能力/服务状态和构建版本；文本统一转义换行/控制字符并限制长度。panic hook 仍不读取 payload，
使用非阻塞记录。

默认 `info` 只写低频业务和错误边界。输入事件、frame tick、cursor/axis 样本、下载进度、轮询 snapshot
和可恢复的 temporary presentation unavailable 不逐次记录。轮转与清理失败只进入匿名 writer 统计，
不得阻止 overlay/runtime 继续运行。

### 5. Diagnostics preview

匿名 diagnostics JSON 保持不变。preview bundle 中的 application lifecycle 历史从新的 `.log` 文件
严格解析固定 code，并重新输出为脱敏的 `application-events.log`；每条固定为
`<LEVEL padded> [<module>] <code>`，不复制原始日志行、消息、上下文、路径或时间戳。输入文件或
其中任一记录无法通过共享文本语法、闭合 code/module/level 契约时，整份来源文件跳过并只增加匿名计数。
Cubism Core 原文仍不进入 preview，只保留匿名
written/dropped/rotated/pruned/bytes/file 统计。

## 影响

- 不新增第三方 logging 依赖，也不增加 GPUI/runtime/OS 类型到公共日志 API。
- application/Core 输出、文件发现、retention helper、diagnostics preview 和相关测试必须一起迁移；
  活跃代码不再保留 JSONL writer/解析分支。
- `.log` 是用户可支持格式，不承诺稳定的第三方日志解析 schema；需要机器处理时使用匿名
  `diagnostics.json` 或 preview bundle 中的严格重写记录。
- 日志设置是首版 v1 字段，遵循仓库规则直接更新 schema/default/fixture/实现，不增加迁移或旧字段
  fallback。

---
<a id="adr-0069"></a>
# ADR-0069: Release notes render through gpui-kit TextView behind a claim-based refusal layer

- Status: accepted
- Date: 2026-09-26
- Depends on: ADR-0020, ADR-0054, ADR-0068
- Amends: none

## Context

`bongocat-ui` rendered the update window's release notes with a hand-written
intermediate representation and a hand-written element tree. The changelog arrives in
the release manifest, so it is untrusted input and the only text the product renders
that it did not author. That made the module a security boundary, and it had grown to
carry its own Markdown parser, its own inline layout, and roughly seven hundred lines
of tests describing what a manifest is allowed to become.

The layout half was reimplementing what the product already depends on.
`gpui-kit` (ADR-0020) ships `TextView`, which parses CommonMark plus GFM, runs a real
inline layout, and is the same renderer every other part of the settings UI uses.

The obstacle was that `TextView` cannot be handed untrusted Markdown unchanged.
Checked against the pinned `gpui-kit` (`=0.7.3`, previously `=0.7.1`, `=0.7.0` and
upstream revision `500852f`; the observation below holds for all of them, and the
precedence `crates/bongocat-ui/src/update_markdown/tests/mod.rs` pins still passes on
`0.7.3`):

- `TextView` renders `![alt](url)` by handing `url` to GPUI's resource loader, which
  fetches it, and `ImageNode::source` only exempts `data:` URLs.
- A Markdown HTML node is passed to `gpui-kit`'s HTML parser and laid out, and that
  parser builds `ImageNode`s — so `<img src="https://…">` is a second route to the
  same request.
- There is no image-policy or link-scheme hook, and the depth of the parsed tree is
  not bounded anywhere.

So the decision was never "delegate the rendering" but "delegate the rendering and own
the policy". The question this ADR records is how that policy is expressed.

## Decision

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

## Consequences

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

## Verification

- `the_refusals_claim_their_nodes_before_the_built_in_renderer` drives a real
  `TextView` in a test window over a document containing an inline image, a reference
  image, a raw HTML block, an accepted link and a refused link. It asserts the probe
  never saw an image, reference image or HTML node, and that the rendered text contains
  every alt text, both link labels and the literal HTML — and none of the URLs the
  manifest named.
- `nesting_deeper_than_the_stack_allows_is_cut` and
  `a_long_list_is_not_mistaken_for_nesting` pin the marker cap from both directions: a
  document written to exhaust the stack is cut, and a list long enough to outgrow the
  notes area is not mistaken for nesting.
- `a_changelog_of_every_refused_shape_still_reaches_the_window` drives the real update
  window with the same document and asserts the notes stay inside their height budget,
  the window stays inside its ceiling, and the phase's install action is still
  reachable.
- The remaining module tests cover the three refusal decisions and the two bounds
  directly, against constructed `mdast` nodes.

---
<a id="adr-0074"></a>
# ADR-0074: 验证流水线按缓存作用域与构建产物拆分

状态：已接受（2026-09-28）

## 背景

`.github/workflows/verify.yml` 此前只有一个 `Test workspace (${{ matrix.os }})` 作业
承担两个平台的全部验证：静态门禁、release 构建、所有 smoke、macOS 打包。实测
（PR #1055，Windows 53.6 分钟、macOS 44.9 分钟）里有两处纯粹的结构性浪费：

1. **每个新 PR 都从零编译。** 流水线只在 `pull_request` 上触发。GitHub 的 cache 作用域
   按 ref 划分：PR 只能读取基线分支创建的缓存，而它自己写入的缓存落在
   `refs/pull/<n>/merge`，之后任何运行都读不到。仓库的 cache 清单证实 20 条记录全部
   挂在 `refs/pull/*/merge` 上，没有一条在 `master`。于是每个新分支的第一次运行都
   `Cache not found`，clippy / test / release check / release 构建全部冷跑。
2. **同一份 release 依赖图被编译两遍。** storage-injection smoke 用
   `--target-dir target/storage-test-injection` 单独构建，而 `storage-test-injection` 是
   `bongocat-app` 的叶子 feature，不激活任何依赖上的 feature。第二个 target 目录因此与前
   面的 release 构建零共享：实测在 Windows 多编译 538 个 crate、耗时 14.2 分钟，在 macOS
   多编译 575 个 crate、耗时 8.4 分钟。

这两项都不是门禁成本：`just check` 要求的门禁集合没有变化，减少的是重复的编译和串行
等待。

## 决策

### 1. 只在 `pull_request` 上运行；master push 与手动触发都不保留

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

### 2. `Test workspace` 拆成四个作业，按产物而非按平台切分

| 作业 | 平台 | 内容 |
| --- | --- | --- |
| `workspace` | Windows + macOS | fmt、clippy、`cargo test --workspace`、`cargo check --workspace --release`、production feature check、provenance |
| `product-smoke` | Windows + macOS | `bongocat-app` release 构建与全部默认 feature 的 product/storage smoke |
| `packaging-smoke` | macOS | `just build` 的两个 bundle smoke 与未知参数拒绝 |
| `windows-platform-smoke` | Windows | startup item、missing-release recovery、D3D11 模型切换 |

切分依据是**产物是否共享**，不是步骤数量：`just build` 为显式 target triple 构建，与
`product-smoke` 不共享任何 artifact；storage smoke 必须跑在默认 feature 构建之后，因此
留在同一个作业里。四个作业的验证集合与拆分前逐条对应，没有任何 smoke 被删除。

### 3. storage-injection 构建与默认构建共用 target 目录

`storage-test-injection` 只作用于 `bongocat-app` 自身，在同一 target 目录下构建只需重新
链接该 crate（实测本机 8 个 workspace crate、零第三方依赖），构建后把二进制复制到
`target/storage-smoke/`，storage smoke 继续跑属于自己的一份产物。`product-smoke` 中所有
读取默认 feature 二进制的步骤（single-instance、product icon、system menu）因此排在
storage 构建之前。

### 4. 缓存按作业分键，路径尽量不重叠

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

### 5. toolchain 作业的 stable canary 改为真实检查

`rust-toolchain.toml` 把 toolchain 钉在 1.97.1，因此作业里裸跑的 `cargo check` 始终解析
到被钉住的版本——"Build workspace with current stable" 与随后的 minimum 检查跑的是同一个
编译器，第二次是空操作（实测 0 秒）。现在 stable 检查显式使用 `cargo +stable`，并单独放在
`target/toolchain-stable`，让这个此前静默失效的门禁真正生效。它在关键路径之外，冷跑多编译
一次 workspace 的代价不进入任何 PR 的等待时间。minimum 半边随后按第 7 条删除，所以本作业
现在只做 stable canary 一件事。

### 6. `spikes/model-package` 不再在 Linux 上重复跑

`model-package-platforms` 已经在两个发布平台上运行它的 fmt、clippy、test 与 release
check，Linux 上的一份是严格子集，从 `contract-spikes` 的矩阵里移除。

### 7. 三处只有重复成本、没有覆盖的步骤被删除

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

### 8. 失败证据契约从固定数字改为按作业推导

`tools/tests/test_failure_evidence_workflow.py` 原先断言 `verify.yml` 里恰好有 10 个
`Collect redacted failure evidence`。拆分后这个数字会立刻过期，而过期的数字无法区分"某个
作业丢了证据步骤"和"某个作业本来就没有"。测试改为解析 `jobs:` 块、断言每个作业各自持有
一对证据步骤。

### 9. 文档改动跳过编译作业，但只有 `fixtures` 永远运行

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
   `docs/product-runtime.md`——所以"改文档"不等于"不需要验证"。`.github/workflows/` 也因此
   不在文档白名单里：改流水线必须由这条流水线自己验证。

`tools/changed-paths.py` 承担判定，`tools/tests/` 里有两条契约测试守护：一条覆盖分类本身
（含"空改动集判为非文档"这个失败方向），另一条断言 **`verify.yml` 里每个作业必须要么在
永远运行的白名单里，要么带显式 `if:`**——新增作业时不可能"忘了决定它能不能跳"就把门禁关掉。

## 备选方案

- **只删步骤不改结构**：删掉 `cargo check --workspace --release` 或某个 smoke 可以立刻省
  时间，但那是降低门禁强度。已接受的残余风险清单里没有任何一条授权这样做。
- **把 macOS 打包 smoke 留在原作业**：省掉一个作业，但把 13 分钟的 `just build` 串行接在
  两条链后面，macOS 仍然是 40 分钟量级。
- **`Swatinem/rust-cache` 之类的第三方 cache action**：底层仍是同一个 GitHub cache 后端，
  同样的 10 GB 预算和同样的 ref 作用域限制，解决不了根因。
- **让 storage smoke 与默认 smoke 分处两个作业**：可以并行，但两个作业都要各自持有 release
  树，缓存占用翻倍；留在同一作业里共享 target 目录既省时间又省空间。

## 影响

- PR 的墙钟时间从"冷编译 + 全部串行"变成"冷编译 + 并行"，约 50 分钟降到约 20 分钟。
  **每个新 PR 的第一次运行都是冷的**，缓存只对同一个 PR 内的后续 push 有效。
- 拆分后的作业各自 `checkout` 并各自 restore 缓存，runner 分钟数会上升；换来的是墙钟
  时间和每个 PR 的反馈速度。
- 只改文档的改动只跑 `fixtures`，约 16 秒（见决策第 9 条）。
- 14 个作业各自持有失败证据上传，`retention-days: 7` 与脱敏工具不变。
- 本 ADR 只改变 CI 的执行方式。`just check`、`just build` 与本地开发流程不变。

## 已接受的残余风险

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

## 验证

- 流水线结构由 `tools/tests/` 的契约测试守护：每个作业必须持有且仅持有一对失败证据步骤，
  `retention-days: 7`、`collect-failure-evidence.py` 的脱敏入口与 `verify.yml` 中的
  `x86_64-pc-windows-msvc` 断言保持不变。
- 本机实测共用 target 目录后的 storage-injection 构建只重新编译 8 个 workspace crate、
  零第三方依赖（对照：改动前在 Windows CI 上是 538 个 crate、14.2 分钟）。
- 拆分后各作业的验证集合与拆分前逐条对应：Windows 与 macOS 的每一个 smoke 步骤都仍在
  workflow 中，且读取默认 feature 二进制的步骤全部排在 storage 构建之前。
- `python3 -m unittest discover -s tools/tests -p 'test_*.py'`：73 passed。

### 关键路径（实测）

| | 改动前 | 改动后 |
| --- | --- | --- |
| 冷跑（每个新 PR 的第一次） | `Test workspace (windows-latest)` 53.6 分钟 | 约 20–28 分钟，关键路径 `Smoke product (windows-latest)` |
| 同一 PR 的后续 push | 53.6 分钟 | 约 20 分钟（该 PR 作用域有缓存） |
| 只改文档 | 53.6 分钟 | 约 16 秒，只跑 `fixtures` |

改动前的数字取自 PR #1055 的逐步实测。改动后的冷跑区间来自合并后两次真实运行
（20.1 与 28.6 分钟），差异来自 10 GB 缓存预算下哪些条目幸存——这正是残余风险第 2 条。
**不要再把缓存命中当作时间承诺**：只有同一 PR 内的后续 push 才是可预期的，那个场景实测
`Test workspace (windows)` 为 6.4 分钟。

---
<a id="adr-0075"></a>
# ADR-0075: 磁盘映像的 Finder 窗口布局与卷图标

状态：已接受（2026-09-29）
补充：ADR-0033 第 4 条（`.dmg` 由操作系统磁盘映像工具产出）

## 背景

ADR-0033 把 `.dmg` 的构建从 `cargo-packager` 收回到 `crates/bongocat-packaging`，用
`ditto` + `hdiutil` + `codesign` 产出镜像。当时镜像里只有 bundle 和一个指向
`/Applications` 的符号链接，没有任何 Finder 元数据，因此挂载后的窗口完全由 Finder
的默认值决定：按内容自适应的大小、48 px 图标、条目按名称排列、工具栏和状态栏都在，
标题栏和路径栏显示的是通用的磁盘映像图标。

Tauri v2 的默认 `.dmg` 不是这样。它的 `bundle.macOS.dmg` 默认值是

| 配置 | 默认值 |
| --- | --- |
| `windowSize` | `660 × 400` |
| `appPosition` | `180, 170` |
| `applicationFolderPosition` | `480, 170` |
| `background` | `null` |
| `windowPosition` | `null` |

> 2026-09-29 修订：安装器窗口多了一个修复命令条目，改为三个条目的倒三角排列，
> 窗口与图标随之改为 660×420、96 px 图标、13 pt 标签（ADR-0076）。宽度与原点不变。

并且它把窗口交给 `create-dmg` 脚本去排（`--icon <app> 180 170 --app-drop-link 480 170
--window-size 660 400 --volicon <icns> --hide-extension <app>`）。Tauri 没有传的
`--window-pos`、`--icon-size`、`--text-size` 落到 `create-dmg` 自己的默认值
（`WINX=10`、`WINY=60`、`ICON_SIZE=128`、`TEXT_SIZE=16`），所以它的默认镜像实际是
660×400 的窗口、128 px 图标、16 pt 标签、app 在左 `Applications` 在右、无工具栏与状态栏、
路径栏保留，并且卷有应用图标。

这些数字里没有一个来自本仓库，因此在打包侧也必须有一处明确的来源。

## 决策

### 1. 布局由构建期写入的 `.DS_Store` 描述，不经过 Finder

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

### 2. `.DS_Store` 编码器由本仓库实现

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

### 3. 卷图标需要一次可写挂载，因此镜像先建可写再转码

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

### 4. `icvp` 的键集必须与 Finder 自己写的完全一致

这一条是实测出来的：只写 `iconSize`、`textSize`、`arrangeBy` 等少数键时，Finder 打开
窗口仍然是 48 px 图标；把 Finder 自己写的那套键补齐（`backgroundType`、三个
`backgroundColor*`、`gridOffsetX/Y`、`gridSpacing` 也在内）之后，128 px 才生效。图标视图
的默认值显然不是"缺键时沿用已有值"，而是整条记录一起判定。

因此键集是对 Finder 的契约而不是偏好，`finder_store` 的单元测试按整份键名列表断言。

### 5. 压缩格式、文件系统与文件系统事件都不再改动

`ULMO` 与 ADR-0033 的实测结论保持一致；文件系统仍是 `hdiutil` 的默认（APFS）；镜像里
不再额外删除 `.fseventsd`，因为第 3 条已经让它的来源消失。

## 备选方案

- **引入第三方 `.DS_Store` crate**：`ds_parser` 与 `cargo-codesign` 的理由见决策第 2 条。
  两者都会为了一个约 200 行的固定记录集合扩大依赖面，而本项目能从源码读懂并测试这段布局。
- **提交一份预生成的 `.DS_Store` 二进制**：`node-appdmg` 之类的项目这样做，但文件名和
  坐标因此变成二进制里不可见的常量，改产品名或坐标就静默失效，而且契约测试无法断言它
  的内容。
- **只做布局、不做卷图标**：少一次挂载，但标题栏和路径栏会继续显示通用磁盘映像图标，
  与 Tauri 默认产物仍有可见差异。
- **把镜像建成 HFS+**（`create-dmg` 的默认文件系统）：HFS+ 卷没有 filesystem-events
  日志，第 3 条的问题自然消失；代价是把已经弃用的文件系统写进新产物。
- **改用 `diskutil`**：macOS 27 对 `hdiutil create` / `attach` / `convert` / `detach` 打印
  弃用提示，建议改用 `diskutil image create` / `image attach` / `image create from`。
  本次不换工具链，见残余风险第 3 条。

## 影响

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

## 已接受的残余风险

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

## 验证

- `just check`：fmt、clippy（`--all-targets --all-features`，`-D warnings`）、workspace
  测试、`cargo check --release` 全绿；`python3 -m unittest discover -s tools/tests`
  85 passed。
- `just build --target x86_64-apple-darwin` 产出的镜像，挂载后用 AppleScript 读回
  Finder 实际采用的设置：窗口 `218, 569, 878, 969`（660×400）、`BongoCat.app` 在
  `180, 170`、`Applications` 在 `480, 170`、图标 128 px、标签 16 pt、排列方式
  `not arranged`、工具栏与状态栏均不可见。
- 独立实现交叉验证：`ds_parser 0.4.0` 解析镜像内那份 `.DS_Store`，`warnings: []`，五条
  记录的键值与写入值逐项一致。
- `hdiutil imageinfo` → `Format: ULMO`；`codesign --verify --deep --strict` 通过。
- `GetFileInfo -a <挂载点>` → `avbstClinmedz`（`C` 即卷使用自定义图标）。
- 挂载后卷根只有 `.DS_Store`、`.VolumeIcon.icns`、`Applications`、`BongoCat.app`。
- `tools/tests/test_packaging_contract.py` 断言五个窗口数值、三个条目与倒三角坐标、镜像
  构建路径不出现 `osascript`，以及 `SetFile`、卷图标与转码步骤仍在。三个条目在挂载后由
  AppleScript 读回：`175, 110`、`485, 110`、`330, 255`，窗口 660×420。

---
<a id="adr-0076"></a>
# ADR-0076: 磁盘映像里的修复命令

状态：已接受（2026-09-29）
补充：ADR-0075（磁盘映像的 Finder 窗口布局）、ADR-0033 第 4 条

## 背景

BongoCat 以拖拽安装的磁盘映像分发，而它尚未公证（ADR-0033：发行签名与公证是独立的
发布门禁）。从下载来的映像里拖出应用包时，应用包会带上 Gatekeeper 打在下载物上的
`com.apple.quarantine`，于是 macOS 拒绝打开它——提示「已损坏，无法打开」或「无法验证
开发者」。读起来像是安装坏了，实际是缺公证。

这个故障的修法只有一条：移除该属性，而 `/Applications` 需要管理员权限，所以这一步
必须由用户在终端里输入自己的密码。目前发布说明里没有这条命令，遇到的人要么搜索、
要么直接重装。

参考实现是第三方为同类软件随包附带的 `已损坏修复` 脚本：中文输出、彩色提示、先判断
是否安装、然后依次执行

```bash
sudo xattr -r -d com.apple.quarantine /Applications/BongoCat.app
sudo xattr -d com.apple.quarantine /Applications/BongoCat.app
```

第一条在 macOS 15 及以上会被拒绝，第二条是替代形式。它还额外执行了
`spctl --master-disable`、`xattr -rc` 与 `codesign --force --deep --sign -`，并在最后
打开厂商网站。

## 决策

### 1. 随磁盘映像附带一个命令脚本，双击在终端里跑

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

### 2. 只移除隔离属性，不做别的

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

### 3. 脚本自己关掉它所在的窗口

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

### 4. 脚本是 shell 逻辑，这是本决策里唯一的一处例外

仓库对自有代码的要求是 Rust。修复逻辑没有做成产品的一个 run mode，而是留在脚本里，
原因是具体的：

- 它要修的正是**产品装不起来**的情况。让一个可能已经跑不起来的二进制去执行修复，
  逻辑上就自相矛盾。
- 它只用到 macOS 自带的 `sudo`、`xattr`、`open` 与 shell，没有产品代码可以复用的东西。
- 它是随安装包分发的**用户操作脚本**，不是构建逻辑：`AGENTS.md` §9.2 禁止的是把平台
  判断、目录复制、bundle 组装重新实现到 `Justfile`／CI／自维护脚本里，而这里没有任何
  东西进入构建流程。

代价是这部分行为不在 `just check` 的覆盖内，因此它的每一条分支都用真实 `xattr` 加
PATH 上的 `sudo`／`xattr`／`open` 替身逐条验证（见「验证」）。

### 4. 文案是英文

安装器窗口里另外两项也是英文名（`BongoCat.app`、`Applications`），第三个条目跟随它们，
脚本内的提示同样是英文。`LANG` 不参与判断：这个脚本没有用户可见的本地化面需要跟随
系统语言，只有 Terminal 里的几行提示。

## 备选方案

- **做成产品的一个 run mode**（`--repair-install`）：逻辑可以进 `just check`，但产品
  本身坏了的时候它跑不起来，且需要给产品二进制加一个与产品无关的 flag。已放弃。
- **做一个带界面的修复 `.app`**：图标可以随便做，但 `sudo` 需要 tty，图形程序没有
  tty，除非引入授权提权 API 或驱动 Terminal 的 AppleScript（会多一次自动化授权）。
- **沿用参考脚本的 `spctl --master-disable`**：确实能"修好"，但它降低的是整台机器的
  安全水位，与故障原因无关。
- **只把命令写进发布说明**：不增加安装包内容，但读者仍然要自己判断该不该运行一条
  `sudo` 命令。

## 影响

- macOS 磁盘映像卷根从两个可见条目变成三个：`BongoCat.app`、`Applications`、
  `Fix Damaged App`（可执行命令文件，约 3.1 KB）。`ULMO` 压缩后的镜像体积变化在测量
  噪声内。
- 用户可见的新能力：安装后打不开时，双击第三个条目、输入密码、修好，应用自动打开，
  窗口随后自己关闭。
- 修复命令目前使用系统自带的 shell 脚本图标；自定义图标是后续项（见「后续」）。
- Windows 不受影响：磁盘映像是 macOS 专属产物，NSIS 安装器里没有对应条目。

## 已接受的残余风险

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

## 验证

- `just check` 全绿；`python3 -m unittest discover -s tools/tests` 97 passed。
- `tools/tests/test_packaging_contract.py` 断言第三个条目存在、两条 `xattr` 命令都在、
  没有任何 `spctl`、`codesign` 只作为提示里的手动命令出现、两种「没有可修」的文案各自
  说了什么、密码只在需要它的分支上问，以及关窗口的条件（`TERM_PROGRAM`、tty、标题、
  延迟）与 `tty` 必须在放进后台之前读取。
- `bongocat-packaging` 的单元测试对渲染出来的脚本跑 `bash -n`。
- 挂载产出的镜像后，三个条目的坐标由 AppleScript 读回为 `175, 110`、`485, 110`、
  `330, 255`，窗口 660×420，Finder 采用的图标尺寸与标签尺寸为 96 px 与 13 pt。
- 脚本分支逐条实跑（用真实 `xattr` 与 PATH 上的 `sudo`／`xattr`／`open`／`osascript`
  替身）：

  | 场景 | 结果 |
  | --- | --- |
  | 未安装到 `/Applications` | 提示先安装，**一次工具都不调用**，退出码 1 |
  | 应用已安装且无隔离属性 | 说明两种可能并给出「先拖一份新的覆盖再重跑」，**不弹密码框**，退出码 0 |
  | 有隔离属性，递归形式成功 | 移除后打开应用，属性确认已消失 |
  | 有隔离属性，`-r` 被拒（macOS 15+） | 自动改用普通形式成功，属性确认已消失 |
  | 两条形式都失败 | 打印手动 `codesign` 命令，**不打开应用**，退出码 1 |
  | 从 Apple_Terminal 以外的终端运行 | 不尝试关窗口，提示语相应改变 |

- 关窗口整条路径在真实双击下实跑（`just build` 出的镜像，双击 `Fix Damaged App`，回车）：
  窗口在 2 秒内自己消失，**无确认弹窗**，同名的其他窗口与读者的 `-zsh` 窗口都不受影响。
  第一版把 `tty` 放在后台子 shell 里读，替身跑不出来，只有真实双击才暴露：bash 给后台
  作业的 stdin 是 `/dev/null`，`tty` 于是回答 `not a tty`，窗口永远不关。

## 后续

1. 为 `Fix Damaged App` 提供自定义图标（`Icon\r` 或 bundle 图标；实测 APFS 上的普通
   文件不认 resource fork 图标，HFS+ 卷可以——见 ADR-0075 的文件系统讨论）。
2. 未来发行签名与公证落地后（ADR-0033 的发布门禁），评估这个条目是否还有存在的必要。
