# 模型、Live2D 与渲染

> 本文件由 `docs/adr/` 下 **16 个 ADR** 合并而成（2026-10-31）。
> 每个 ADR 保留原始编号（`ADR-XXXX`）与完整正文，可继续按编号引用；原始单文件已归档到 `docs/adr/archive/`，仅作考古与回滚，勿直接编辑。

---
<a id="adr-0036"></a>
# ADR-0036: 模型导入的压缩包来源与解压边界

状态：已接受（2026-09-16）→ **已撤回（2026-09-22）**
依赖：ADR-0030（先复用既有方案）、ADR-0027（诊断包已使用 `zip` 写入器）、ADR-0011（渐进实现与发布门禁）

> 本文是已撤回的历史设计记录；当前 `ModelStore`、Mver 转换和用户侧封面 writer 位于
> `bongocat-model-store`，只读包解析与 `PreparedModel` 仍位于 `bongocat-model`。

## 撤回（2026-09-22）

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

## 原始决策（已撤回，保留作设计输入）

## 背景

到本次改动为止，用户只能通过「选择文件夹」导入模型：`ModelStore::import_with_observer` 接收一个
目录，`PreparedModel::prepare` 就地校验它，`copy_package` 复制进 store 自己的 staging 目录，最后
`rename` 原子提交。

但模型站与社区导出普遍给的是 `.zip`。两个真实样本已经暴露了这件事：

| 归档文件 | 内部根目录 | 入口 |
| --- | --- | --- |
| `经典小键盘 · 标准模式.zip` | `经典小键盘 · 标准模式/` | `cat.model3.json` |
| `送葬人 · 标准模式.zip` | `图弟 · 标准模式/` | `demomodel.model3.json` |

两点值得注意：两个归档都把包放在**一层包装目录**下面，而第二个的包装目录名与归档文件名
**并不一致**。用户要在导入前手动解压，且解压后仍然要自己去猜该选哪一层目录。

同时，Technical Design 早已写明「模型导入防止路径穿越、符号链接逃逸、压缩炸弹和覆盖现有用户
数据」，但在此之前没有任何实现对应「压缩炸弹」——因为不存在解压路径。本 ADR 记录把压缩包接成
一等来源所采用的决策，以及解压这一步的安全边界。

## 决策

### 1. 来源类型由内容识别，不由扩展名或调用方标志决定

`detect_source_kind` 只看文件系统类型与文件头：目录 → 目录来源；常规文件且以 `PK\x03\x04` /
`PK\x05\x06` / `PK\x07\x08` 开头 → 压缩包来源；其余 → `model_store_source_archive_unsupported`。

因此：归档被改名（`model.package`）仍能导入；叫 `looks-like.zip` 的**目录**仍按目录处理；`.rar`
或损坏下载得到的是一个稳定诊断，而不是"先解压失败、再报包无效"。用户不需要告诉产品他选的是
什么，产品也不需要靠字符串猜。

### 2. 压缩包不是第二个解析器

压缩包来源与目录来源**共用同一套校验**：解压写满 staging 之后，仍然调用同一个
`PreparedModel::prepare`。压缩包只是"把同样的字节放进 staging 的另一种方式"，因此不可能绕过
路径规范化、符号链接拒绝、JSON/PNG/FLAC 侧车校验、纹理尺寸上限或包级上限。

这是本 ADR 最重要的一条：任何"为压缩包单独写一遍检查"的做法都会在两侧漂移。

### 3. 解压写进 store 自己的 staging 目录

解压目标就是 `create_staging_directory` 建好的 `.importing-<id>-<pid>-<seq>`，不引入第二个临时
目录、不复用系统 temp。由此得到的性质都是既有的，不需要新机制：

- 提交仍是**一次 `rename`**；失败时 `StagingCleanup` 删掉整个 staging，store 根目录不留痕迹。
- 权限仍是 store 的 owner-only（目录 0700、文件 0600），解压出来的字节不会经过一个"更宽松的
  中间位置"。
- 配置里没有指向临时路径的引用，取消与崩溃恢复沿用既有 `recover_abandoned_operations`。

### 4. 先校验、后解压：plan → extract 两遍

`plan_archive` 只读中央目录（`by_index_raw`，不解压、不解密），在这一遍里完成：条目名规范化、
条目类型、压缩方法、加密标志、条目数、深度、单文件与整包声明字节。`extract_archive` 才真正
解压。

这样做的直接收益是**压缩炸弹在还只是一个头部数字的时候就被拒绝**：恶意归档不需要被解压一个
字节就会失败，store 也不会先建好 staging 再报错。

代价是文件被读两遍，以及两遍之间归档可能被替换。后者由第二遍的**逐条目复核**处理：索引位置的
条目名、声明大小、`is_file` 必须与 plan 一致，实际写出的字节数必须等于声明值，否则
`model_store_source_changed`。`zip` 默认校验 CRC32，损坏的载荷因此也不会被当作成功。

### 5. 包装目录剥离

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

### 6. 归档工具元数据被丢弃

`__MACOSX/**` 与 `._*` / `.DS_Store` / `.localized` / `Thumbs.db` / `desktop.ini` 不参与包：
它们既不参与包装目录判定，也不被解压。理由与 `ModelStore::list` 忽略同样文件的理由一致——它们
属于文件管理器，不属于模型。副作用是 `__MACOSX/` 与真正的包目录并列时，包装目录仍能被正确
识别（否则这一常见形态会直接导入失败）。

如果一个模型真的引用了被丢弃的 `._*` 文件，它会得到"资源缺失"，这是诚实的结果：包确实不完整。

### 7. 上限：给"容器"一个自己的界

`ModelPackageLimits` 新增 `maximum_archive_bytes`（默认 1 GiB，与 `maximum_package_bytes` 同值）。

不能复用 `maximum_package_bytes`：那条上限约束的是归档**产出**的包，在归档读取器解析中央目录
**之前**就要用它挡住输入；而"用 `Stored` 存一个 8 字节包上限的归档"本身可能远大于 8 字节。
把两者合并会让包字节上限对归档来源永不可达。也不能复用 `maximum_file_bytes`，否则单文件上限
同样变得不可达。

此外有一个独立的**结构上限**：条目数超过 `maximum_file_count × 4` 直接拒绝。乘数给目录条目与
被丢弃的元数据条目留出余量，同时保证"声明海量条目"的输入不会进入逐条目循环。

### 8. 诊断码：新增一个，复用其余

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

### 9. 依赖：`zip` 的 `deflate-flate2`，并显式依赖 `flate2`

`zip =8.6.0` 已在 workspace 依赖里（诊断包用它写 `.zip`），但当时是 `default-features = false`
且只走 `Stored`，没有解压能力。真实模型归档用 deflate（两个样本的压缩方法都是 8），因此打开
`features = ["deflate-flate2"]`。

`zip` 对 `flate2` 声明的是 `default-features = false`，自己不选后端；而 `flate2` 一个后端都没
选中时以 `compile_error!` 编译失败。因此 `bongocat-model` **直接**依赖 workspace 的
`flate2 =1.1.10`，让 `rust_backend`（miniz_oxide，纯 Rust）成为本 crate 在任何构建图里的确定
事实，而不是依赖"某个兄弟 crate 恰好打开了默认 feature"——那只在测试构建里成立。

其余 feature 一律不开（无 AES、无 bzip2/zstd/lzma/ppmd/deflate64）：模型归档不需要它们，而未
开启的压缩方法会以稳定诊断被拒绝（这也是"加密归档"的拒绝路径）。按 ADR-0030 与 `AGENTS.md` §9，
两个 crate 的版本都在当次用 `cargo info` 核对了 crates.io 最新非 yanked 稳定版：`zip` 8.6.0
（`9.0.0-pre3` 是预发布，不采用）、`flate2` 1.1.10。

### 10. UI：一个来源一个按钮（已撤回，见文首）

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

### 11. picker 模块与稳定码改名

`bongocat-platform` 的 `directory_picker` 模块改名为 `model_source_picker`，
`DirectoryPickerOutcome` / `DirectoryPickerError` 改为 `ModelSourcePickerOutcome` /
`ModelSourcePickerError`，稳定码前缀由 `directory_picker_*` 改为 `model_source_picker_*`，
示例改名为 `examples/model_source_picker_smoke.rs` 并新增 `--kind directory|archive`。

理由是这次改动**引入**了一处不准确：让"压缩包选择器"返回 `DirectoryPickerError` 是错的。与之
相邻的既有模块名一并修正，代价是 4 个文件里的机械替换与一次示例改名。

### 12. `zip` 归档内容与第三方模型不进入仓库

两个真实样本是第三方模型（含 Live2D 模型数据），不提交到仓库。真实归档的验证改为一个由环境
变量驱动、未设置即跳过的测试：
`imports_the_archive_samples_named_by_the_environment`（`BONGOCAT_MODEL_ARCHIVE_SAMPLES` 指向一个
放 `.zip` 的目录）。仓库内的永久测试用测试期生成的归档（真 deflate 流）覆盖结构与拒绝面。

## 明确不做

- **`.rar` / `.7z` / `.tar.gz`**：只有 zip 被真实需求驱动。新增格式需要各自的边界分析（tar 的
  符号链接与硬链接、gzip 的单流特性、7z/rar 的许可证），不在本 ADR 范围。
- **加密归档**：没有密码输入面，也没有需求。加密条目被拒绝。
- **新增"解压中"进度阶段**：typed 操作契约的 stage 集合（prepare/copy/validate/commit）不变，
  解压计入 `Copying` ——它描述的是"把来源字节物化进 staging"，对归档而言就是解压。新增阶段会
  让 model/ui/app 三处的单调性契约与测试同时改，收益只是一个更细的动词，不值得。
- **把归档解压到已安装目录**：本产品不允许任何就地更新语义；导入永远是"新 id + 新目录"。
- **压缩包内的多模型选择**：一个归档 = 一个模型，仍是入口发现规则（0 个 → 缺失，≥2 个 → 歧义）。

## 残余风险与待验证项（不得当作已确认）

1. **中央目录的分配没有在解析前被界住**。`maximum_archive_bytes` 限制输入文件大小，但
   `ZipArchive::new` 会在返回**之后**我们才拿到 `len()`；在此期间它已按中央目录声明分配。条目数
   上限（`× 4`）是在那之后生效的。最坏内存未测量，也没有针对"1 GiB 输入 + 海量声明条目"的测试。
2. **真实样本只有两个**，且都是单层包装、deflate、无加密、条目数 < 70。zip64、data descriptor、
   非 UTF-8 条目名、大小写混合扩展名只有合成测试覆盖。
3. **Windows 未实机验证**：本机是 macOS。归档解压路径本身是平台无关的（`std::fs` + `zip`），
   但 `SourceArchiveUnsupported` 在 Windows 上的真实触发路径（例如从资源管理器拖出的 zip）没有
   实测；`just check` 的 Windows 部分按仓库约定在 Windows 实机/CI 上运行。
4. **UI 实机交互未执行**：两个按钮、压缩包面板、`SelectionInvalid` 文案都只有无头测试覆盖。真实
   `NSOpenPanel` 选 zip 的路径需要在 macOS 上运行
   `cargo run -p bongocat-platform --example model_source_picker_smoke -- --expect-selected-any --kind archive`
   才算验证过。
5. **`maximum_archive_bytes` 的取值（1 GiB）没有基于实测压力测试**，它与包字节上限同值只是出于
   "容器通常不大于内容"的判断。
6. **本次 `cargo update` 顺带了 4 个无关传递依赖的补丁升级**（`synstructure` 0.13.2 → 0.14.0，
   连带切到 `syn 3`；`yoke-derive`、`zerofrom-derive`、`zlib-rs`）。这是 `AGENTS.md` §9 要求的
   完整 `cargo update` 的结果，不是本功能需要，但会让 diff 变大。

## 验证

> 以下证据属于**已撤回**的实现，其中的测试与辅助已随实现删除；保留它只为说明这些决策当年确实被
> 验证过，恢复时可以直接复用同一批用例。

已完成（2026-09-16，本机 macOS 26.5 / aarch64）：

- `bongocat-model` 60 个测试通过，其中 15 个新增（`store.rs` 10 个 + `archive.rs` 5 个），新增的
  `store.rs` 用例直接针对压缩包来源：
  同一包以目录与归档两种来源导入后 `ModelPackageIndex` **逐字段相等**（
  `zip_and_directory_sources_of_the_same_package_import_identically`，并用"包装目录名与归档名
  不一致"的形态构造）；按内容而非名字识别（无扩展名、`Stored` 归档、名为 `.zip` 的目录）；
  嵌套包装剥离与深度上限；`__MACOSX` / `.DS_Store` 被丢弃且不干扰包装识别；非归档 / 截断
  / 空 / 只有目录的归档；路径穿越（`../`、绝对、平台前缀）与重复条目（`猫//model.moc3` 与
  `猫/model.moc3` 归一化后相撞）、文件与目录冲突；符号链接条目；四类上限（文件数、整包字节、
  单文件字节、结构条目数）在解压前拒绝；进度单调 + 取消后无残留 staging 且不影响已装模型。
  另有 `archive.rs` 内 5 个单元测试覆盖包装判定、剥离、祖先冲突、条目名规范化、元数据识别。
- **真实归档验证**：`BONGOCAT_MODEL_ARCHIVE_SAMPLES=/tmp/bongocat-samples cargo test -p bongocat-model
  imports_the_archive_samples` 通过（样本 = 两个真实 zip）。逐字段结果：`经典小键盘 · 标准模式.zip`
  → 入口 `cat.model3.json`、moc `demomodel.moc3`、3 张 1024x512 纹理、cdi3、3 个表情、
  2 组动作（各 2 条，1 条带 FLAC 音轨）、31 文件 / 1 218 791 字节；`送葬人 · 标准模式.zip`
  → 入口 `demomodel.model3.json`、1 张 1024x512 纹理、cdi3、61 文件 / 791 595 字节。两个归档的包装
  目录都被剥离（安装根目录下直接是 `.model3.json`），源归档未被修改。
- `bongocat-app` 120（lib）+ 21（bin）个测试通过，新增
  `application_imports_folder_and_archive_sources_of_the_same_model`：
  同一模型分别以文件夹与 `.zip` 导入，两个 UUID id 各自独立、索引一致、目录侧标题为 `非 ASCII 模型`、
  归档侧标题为归档名去掉 `.zip`，两者都出现在 installed 目录里。
- `bongocat-ui` 115 个测试通过，新增 `model_source_display_name` 的 UTF-8 边界与大小写回归、
  目录名为 `something.zip` 时保留原名、归档草稿的状态文案为"已选择压缩包/所选压缩包不可用"、
  两个按钮的 AccessKit 节点与禁用传播。
- `bongocat-platform` 新增 `pick_model_archive` 的验证测试（归档与"不是归档的文件"都算合法选择，
  目录/文件互斥拒绝，相对与不存在路径拒绝，稳定码唯一性），示例改为支持 `--kind`。
- `cargo fmt --all --check`、三组 clippy（workspace `--all-targets --all-features`、
  `bongocat-app` 的 `storage-test-injection` 与 `production`）、`cargo test --locked --workspace`、
  `cargo check --locked --workspace --release` 全部通过。

**顺带修复的既有缺陷（与本功能无关，需要单独复核）**：`bongocat-platform` 的示例
`model_source_picker_smoke` **本来无法编译**（已用 `git stash` 在 HEAD 上复现）：
它引用 `objc2_app_kit::NSBackingStoreType`，而该类型属于 `NSGraphics` feature，清单里没有打开。
因为本次需要改这个示例，所以补上了 `"NSGraphics"`；这也是 `cargo check --all-targets` 能通过的前提。
若不接受这次修复，应单独回滚该清单行，并另行处理该示例。

---
<a id="adr-0037"></a>
# ADR-0037: 在应用内导入 BongoCatMver 模型

状态：已接受（2026-09-17）
依赖：ADR-0030（先复用既有方案）、ADR-0036（模型来源按内容识别、压缩包与目录共用校验；
**该 ADR 已于 2026-09-22 撤回**）、ADR-0011（渐进实现与发布门禁）、ADR-0060（model/store 边界）、
ADR-0065（模型模式元数据与卡片 Badge）

修订（2026-09-22）：ADR-0036 撤回后，本文**决策 4「归档来源不解压」**连同它描述的
`ArchivePlan` / `MverSource::Archive` 一并失去实现——Mver 源现在只从用户选中的文件夹就地读取，
`MverSource` 退化为一个持有已 canonicalize 根路径的 struct。其余决策（转换边界、多模型产出、
键位图合成、重编码、诊断码）**全部不变**。归档侧的读取上限逻辑（用声明大小在解压前判定）随实现
删除，但 `LEGACY_RESOURCE_MAXIMUM_BYTES` 仍然约束目录侧的单次读入。

## 背景

`Bongo-Cat-Mver` 是 BongoCat 的上游原版实现（`docs/migration/bongo-cat-mver-reference.md`
固定了参考 commit）。它的模型格式与 BongoCat 不同，而且在两处不一致：

| | BongoCatMver | BongoCat |
| --- | --- | --- |
| Live2D 包位置 | `<源>/img/<模式>/cat_model/` | 包根（入口发现只看根目录） |
| 键位图 | 每个键分成 `hand/*.png` 与 `keyboard/*.png` 两层，另有一张与模式一一对应的背景和封面 | 每个键一张合成图，按 HID usage 解析出的名字放在 `resources/{left-keys,right-keys}` |
| 键位表 | 根 `config.json` 的 `standard`/`keyboard`/`gamepad` 三个 section | 无需键位表，模型只声明 Live2D 资源 |
| 一个"模型" | 每种输入模式各自一个 Live2D 包 + 一套键位图 | 一个目录 = 一个模型 |

历史上这件事由外部工具 `BongoCat-Converter`（TypeScript/浏览器）完成：用户选中整个 Mver 应用
文件夹，工具按模式产出 `BongoCat - <模式>` 目录，再在 BongoCat 里逐个导入。这要求用户离开产品、
理解两个格式的差异、并在两次产物之间手动搬运文件。用户要求把这一步内置到产品里：导入时直接
识别 BongoCatMver 模型并触发转换，转换产出的模型进入既有模型存储与管理流程。

三个只有看真实数据才会发现的事实决定了本 ADR 的形态：

1. **三种模式的键位表不在同一个编码空间里。** `standard`/`keyboard` 用 Windows 虚拟键码
   （`49` = `1`，`65` = `A`），`gamepad` 用 XInput 按键序号（`10` = 十字键左，`0` = A）。同一
   个 `13` 在前者是 `Return`、在后者是十字键下。
2. **一个源最多对应三个模型**，因为三种模式带的是三份不同的 `.moc3`（参考样本里分别是
   `demomodel.moc3`/`demomodel2.moc3`/`demomodel3.moc3`），无法合并成一个包。
3. **`gamepad.lefthand` 与 `righthand` 共用同一份 `keyboard` 图集**，右手的图集下标从左手的
   长度继续（`lefthand.length + index`），而不是从 0 重新开始。

## 决策

### 1. 检测仍按内容识别，且要求两条独立证据

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

### 2. 一个源产出多个模型，每个模型独立安装

每个模式转换成一个 BongoCat 包，各自分配一个随机 UUID v4 存储键，各自写一条
`imported_models` 元数据记录和对应的 `input_mode`，因此三种模式成为模型列表里三个可以独立
启用、改名、删除的普通条目。模式值来自本次导入选择的 legacy section，不从转换后目录形状
或标题反推。

**逐模式提交**，不做整体回滚：某个模式的键位图损坏时，已经转换成功的模式保留下来。理由是这些
模型彼此独立，把已经转换成功的模式一起丢弃比留下一个用户可重试的部分结果更糟。

标题是「用户标题 · 本地化模式名」（`我的猫 · 标准模式`），即社区导出模型已经在用的命名。模式名
在**截断之后再拼接**：模式名是三个模型之间唯一的区别，长来源名不能把它挤掉，所以先按标题上限
减去后缀长度裁剪来源名。

### 3. 转换写进 store 自己的 staging，与包来源共用提交尾部

`ModelStore` 的导入尾部（`PreparedModel::prepare` 校验 + 单次 `rename` 提交 + 失败清理 staging）
被抽成 `commit_installed_staging`，目录复制、归档解压与 legacy 转换三条路径共用它。因此转换
结果不可能绕过任何包校验，也不存在第二个临时位置：失败的转换不留痕，成功的转换仍是一次原子
rename。

### 4. 归档来源不解压（**已随 ADR-0036 撤回，见文首**）

目录来源就地读取；归档来源**只按需读取被命名的条目**（`ArchivePlan::read_file`，仍逐条目复核
名字/声明大小/实际字节数）。导入一个 Mver `.zip` 不会把整个应用——exe、DLL 和用不到的模式——
解压到磁盘上，只把转换真正需要的那几十个文件读进内存。

读取有独立上限 `LEGACY_RESOURCE_MAXIMUM_BYTES`（64 MiB）：它约束的是转换期临时读入的量，与
约束"可被安装的包"的 `ModelPackageLimits::maximum_file_bytes` 是两件事。归档侧在解压前用声明
大小判定，因此超限只花一次头部读取。

### 5. 键位图合成 = 最小画布上的 Porter-Duff over

合成画布取两层中较小的宽高，两层都锚在原点绘制，因此比画布大的一层被裁切而不是缩放——这与
canvas `drawImage` 的行为一致。合成用整数形式的 `over` 算子：不透明来源整体替换、全透明来源
不改变，只有抗锯齿边缘像素会四舍五入一次（浮点形式在此处最多每通道差 1 档）。

模式里根本没有 `keyboard/` 图集时，不合成，直接按字节安装 paw 图（复古模型可能已经把两层画在
一起了）。某个绑定的 paw 或配套键帽缺失时该绑定被跳过而不是让整次转换失败——与参考实现一致，
且"少一张键位图"不等于"这个模型不可用"。

### 6. 输出图使用 lossless 重编码，`oxipng` 是唯一选择

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

### 7. 键位名必须落在产品自己的词汇表里

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

### 8. 诊断码：新增一个 store 码，复用既有用户可见码

新增 `ModelStoreDiagnostic::SourceConversionFailed`
（`model_store_source_conversion_failed`，`ALL` 由 12 增至 13），只用于"已经认出这是
BongoCatMver 源，但无法把它变成 BongoCat 包"：键位图不是可读 PNG、合成图无法编码、绑定引用的
资源超出读取上限、请求的模式不在此源中。

它刻意与 `InvalidPackage` 区分：后者是"按包读入并且包校验失败"，前者从未成为包。settings 层
把它映射到既有 `SettingsErrorCode::ModelImportSourceUnsupported`（沿用 ADR-0036 §8 的做法），
因此没有新增用户可见错误码；具体原因留在 `detail`/`resource` 里，进入日志与诊断包。

### 9. 进度跨模型保持单调

settings 的 `report_progress` 会丢弃回退的更新，而 store 每个模型都从 `Preparing` 与 0 重新
开始。`Application` 因此在跨模型处折叠进度：累计已完成模型的文件数与字节数、并对 stage 取
最大值后再上报。于是用户看到的是一个不倒退、且最终等于三个模型总和的一条序列，而不是卡在
第一个模型的终值上。

### 10. 模式选择与卡片投影

Mver 导入仍由服务层执行阻塞文件与模型解析；用户在检查来源后通过现有模式选择对话框选择要转换
的 section。选择的每个 section 生成一个独立模型，并将该 section 写入 installed metadata 的
`input_mode`。Models 页面只消费 `SettingsModelEntry.input_mode`，不在 UI 线程重新检查资源或从
标题反解析模式。`Application::import_models*` 仍是唯一的导入入口。

### 11. 按键图按字节安装，legacy 窗口坐标由渲染器重框（issue #1129）

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

## 明确不做

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

## 残余风险与待验证项（不得当作已确认）

1. **真实样本有两份**（用户提供的 `bongo_cat_mver_0.1.6_64`，三模式齐全、deflate 归档、
   `<img>/<模式>` 布局；issue #1080 的菲比模型，`<img>/<模式>` 布局 + 带注释的键位表 +
   一个悬空 `Sound` 引用）。把模式文件夹摊在根上、缺少 `keyboard/` 图集、非 UTF-8 文件名、
   非 ASCII 路径只有合成测试覆盖。
2. **Windows 已在 issue #1080 的修订中实机验证**（本机 Windows 11 / x86_64-pc-windows-msvc，
   `oxipng` 的 C 工具链在该平台构建通过并跑通了真实模型）。此项在本修订之前记录为未验证。
3. **`gamepad` 的键位图目前不可达**：`bongocat-runtime` 只对键盘按键产生 `KeyPress`，手柄按键
   仅置 `left_hand_down`/`right_hand_down`/`stick_*_down`。转换输出与预置 gamepad 模型同名，
   但两者在当前运行时下都只是随包携带的资源。这是既有缺口，不由本 ADR 引入，也不在本 ADR 修复。
4. **64 MiB 的读取上限没有基于实测压力测试**，取值依据是"真实模型的单文件是几 MB 量级"。
5. **Zopfli 之外的压缩收益未再评估**：只测了 libdeflate 与 Zopfli 两条后端，没有试
   `oxipng` 的高阶 preset 或 filter 集合调参。
6. **`cargo update` 顺带升了 3 个无关传递依赖**（`granit-parser` 1.2.1 → 1.3.0、
   `serde-saphyr` 1.2.0 → 1.3.0、`redox_users` 0.5.2 → 0.5.3）。这是 `AGENTS.md` §9 要求的完整
   `cargo update` 的结果，不是本功能需要。

## 验证

已完成（2026-09-17，本机 macOS / aarch64）：

- `bongocat-model-store` 83 测试（`mver.rs` 新增 18 个单元测试 + `store.rs` 新增 5 个）：检测的两条
  证据与"配置了模式却没有模型"的跳过、模式文件夹摊在根上的布局、左右手共用键盘图集的
  `lefthand.length + index` 下标、没有 `keyboard/` 图集时按字节复制、合成后包根布局与
  `[128, 0, 127, 255]` 的合成像素、`over` 算子边界（不透明 / 全透明 / 半透明 / 空画布）、
  lossless 重编码在缩小文件的同时保持**每一个 alpha > 0 的像素逐通道不变**、鼠标键与非法码被
  跳过、缺 layer 时跳过绑定、重复绑定只写一次、符号链接被拒绝、归档来源不解压即转换、
  超限资源被拒绝、两个编码空间的键名。
- **真实模型验证**：`cargo run -p bongocat-model-store --example model_conversion_smoke -- --source
  /Users/ayang/Downloads/bongo_cat_mver_0.1.6_64`（也不带 `BONGOCAT_MVER_SAMPLE` 时由
  `converts_the_legacy_sample_named_by_the_environment` 覆盖）。逐模式结果：
  `standard` → 3 张纹理、15 张键位图（`Num1..Num7`/`KeyQ`/`KeyE`/`KeyR`/`Space`/`KeyA`/`KeyD`/
  `KeyS`/`KeyW`，与 `config.json` 的 15 条绑定逐个对应）、31 文件 / 1 081 672 字节 / 0.87 s；
  `keyboard` → `left-keys`{`Control`,`KeyR`,`Shift`} + `right-keys`{四方向键}、23 文件 /
  1 015 987 字节 / 0.46 s；`gamepad` → `left-keys`{`DPadDown`,`DPadLeft`,`DPadRight`,`DPadUp`,
  `LeftTrigger`,`LeftTrigger2`} + `right-keys`{`East`,`North`,`RightTrigger`,`RightTrigger2`,
  `South`,`West`}、28 文件 / 1 082 753 字节 / 0.74 s。gamepad 的两个集合与仓库内
  `resources/models/gamepad/{left-keys,right-keys}` 的文件名**逐字符一致**；源目录未被修改。
- `bongocat-app` 现在还验证：一个 Mver 源导入出 3 个模型、3 个不同 UUID、三条 metadata 的
  `input_mode` 分别为 `standard`/`keyboard`/`gamepad`，改名只改变 title，模式在重启后仍从
  config 读回；普通包按 `left-keys`/`right-keys` 资源判定模式，缺少可用键图时导入失败。
  `bongocat-ui` 验证三种模式的可见标签和 Badge 几何不改变卡片编辑面。
- `bongocat-i18n` 的 `models.mver.mode.*` 继续用于标题后缀；`models.mode.*` 用于卡片 Badge 的
  简短可见标签。两种 locale 键与占位符完全一致。
- `cargo fmt --all --check`、三组 clippy（workspace `--all-targets --all-features` 与
  `bongocat-app` 的 `storage-test-injection`/`production`）、`cargo test --locked --workspace`
  （全部二进制全绿）、`cargo check --locked --workspace --release` 全部通过。

**未运行**：Windows 编译与实机导入、`NSOpenPanel`/Windows 对话框的实机交互（本次没有改动选择器）、
任意 UI 实机点击（本次只在服务层与文案层改动，Models 页面的控件集合未变）。

---

已完成（2026-10-02，本机 Windows 11 / x86_64-pc-windows-msvc，issue #1080）：

- **真实模型验证**：`cargo run --locked -p bongocat-model-store --example model_conversion_smoke
  -- --source <用户提供的菲比模型目录>`。该源此前被报成「模型包无效」；修复后被判为三模式的
  BongoCatMver 源，逐模式结果：`standard` → 126 文件 / 23 940 725 字节 / 0.17 s、
  `left-keys` 91 张；`keyboard` → 18 文件 / 516 461 字节 / 1.50 s、`left-keys` 3 张
  （`Tab`/`Num1`/`Num2`——该模式的 `lefthand/` 只带了 0–2 三张，其余绑定按决策 5 跳过）；
  `gamepad` → 47 文件 / 16 574 934 字节 / 0.08 s、`left-keys`/`right-keys` 各 6 张，
  与仓库内预置 gamepad 模型的按钮词表一致。`standard` 与 `gamepad` 的 entry 与源**逐字节相同**，
  只有含悬空 `Sound` 的 `keyboard` entry 被改写；源目录未被修改。
- `bongocat-model-store` 新增测试：带注释的键位表仍被识别为 legacy 源且三种模式的绑定逐个
  对应；注释移除只作用于字符串之外（URL、`\"` 转义、多字节中文注释、块注释保留换行、文件末尾
  无换行的注释）；非 JSON 与非 UTF-8 仍然不是 config；悬空 `Sound` 被丢弃而其余声明保留、
  能解析的 `Sound` 保持 entry 逐字节不变、越出包根的 `Sound` 留给包校验；含悬空音频的 legacy
  模式经完整导入事务安装成功且源文件不被改写。
- `bongocat-app` 新增测试：带注释的 legacy 源经 `inspect_model_source` 报出三种模式（即进入模式
  选择对话框而非回退成包）并导入出 3 个模型；store 失败写入日志时携带 store 自己的码。

---
<a id="adr-0047"></a>
# ADR-0047: 模型库只做选择与元数据编辑

状态：已接受（2026-09-18；2026-09-25 修订）
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

## 背景

1. **旧版模型页的交互是封面卡片网格**：每张卡片显示模型的 `cover.png` 与标题，卡片底部只有
   「选中」「打开所在文件夹」这类入口。BongoCat 此前实现成「导入面板 + 行列表（选中/删除）+
   活动模型的行为预览列表」，与旧版页面的信息层级完全不同，用户要求在保持旧版交互风格的前提下
   重做，并明确移除表情入口。
2. **页面当时拿不到封面与目录。** `SettingsModelEntry` 只有 `id`/`title`/`origin`/`availability`，
   既没有包目录（无法提供「打开所在文件夹」），也没有封面路径（无法画封面），于是页面既不能显示
   模型自己的图，也不能把用户送到模型文件夹。
3. **`resources/cover.png` 的布局事实当时只存在于转换代码里。** BongoCatMver 转换用私有的
   `OUTPUT_RESOURCES`/`OUTPUT_COVER` 常量把旧版 `cat.png` 装成 `resources/cover.png`；如果设置页
   自行拼这条路径，两处一旦漂移就会出现「转换装好了但页面找不到」。
4. **标题是可编辑元数据，且只有导入模型有记录。** 显示名来自
   `config.model.imported_models[].title`；预设模型没有记录，显示名回落到稳定 id，而预置模型本身
   位于随产品分发的 `resources/models/`。`Application::delete_model` 已经拒绝删除预设。
5. **行为预览是当时模型页唯一的行为 UI，而快捷键页已经列出同一批行为。** 快捷键页的模型页签从
   当前激活且 Ready 的模型目录生成 motion/expression 行用于绑定；模型页的预览列表是同一批数据的
   第二个入口，用户认为重复。
6. **模型页的错误提示混用过四种呈现方式**：导入失败是危险色 `Tag`、目录失败是危险色文本、
   行为预览失败与其余命令失败走通用 `Notification`、封面选择失败一度内联在卡片里。同一页面上
   同级别的失败有不同的样式与生命周期。

## 决策

### 1. 页面 = 网格首位导入卡片 + 封面卡片

模型库页面只承担两件事：**选择模型** 与 **编辑该模型的标题/封面**。导入卡片占据网格第一格，点击
后仍走既有的来源选择流程（撰写时为「选择文件夹 / 选择 .zip」按钮加标题输入，2026-09-22 起为
单一文件夹选择器加自动导入，压缩包入口暂缓）。设置窗口根视图同时接收 `ExternalPaths` 文件拖放，
因此拖入不必先切换到 Model library 页面，也不要求指针落在导入卡片上；拖放成功后仍由同一来源检查、导入、
取消和封面截取流程处理。其余每张卡片是
`cover.png` + 模式 Badge + 标题 + 可用性 + 操作行（选中 / 打开所在文件夹 / 编辑 / 删除，删除前弹出确认浮层）。

### 2. 预置模型可以改名和换封面，但与删除无关

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

### 3. 封面位置成为跨 crate 契约

`bongocat-model` 公开布局事实，`bongocat-model-store` 提供两个用户侧写入口：

- `PACKAGE_RESOURCES_DIRECTORY`、`PACKAGE_COVER_FILE`、`package_cover_path(root)` 是唯一来源，
  BongoCatMver 转换改用同一组常量（原先的私有 `OUTPUT_*` 常量删除）。
- `ModelStore::replace_cover(id, bytes)` 负责「同目录临时文件 + rename」的原子替换，并按既有
  Unix 私有模式落盘；失败时不留半写的封面。`PresetCoverStore::replace_cover(id, bytes)` 是同一
  形状的写入口，只是根目录在用户侧（决策 2），因此两边的封面路径都由同一个
  `package_cover_path` 推出。
- 设置页**不推导路径**：它只消费 `SettingsModelEntry.directory`/`cover`，两者都由 app 层读取磁盘
  后填充（`cover` 为 `None` 表示这个模型没有任何封面，页面画占位符而不是猜一条不存在的路径）。

### 4. 封面只接受 PNG，字节原样落盘

封面是显示用美术，不是模型数据，因此校验只到「PNG 签名 + 不超过包的每文件上限
（`ModelPackageLimits::maximum_file_bytes`）」；通过后按原字节写入，与转换安装旧版 `cat.png`
的做法一致，不做重新编码。这样也避免为 JPEG 等格式引入解码 feature（workspace 的 `image` 只开
`png`）。

### 5. 「打开所在文件夹」沿用备份目录的能力注入

`bongocat-platform::open_directory` 已有实现（`opener` + 绝对路径/存在性校验），复用即可；
settings 服务通过 `ModelLocationCapability` 注入，与配置备份目录同一 seam。原因是单元测试必须能
断言「打开所在文件夹」的结果，而真实实现会启动系统文件管理器。

### 6. 模型页的错误只用通用 Notification 组件

页面级错误（源选择器失败、导入失败、封面选择器失败、目录读不出、改名/换封面/打开位置失败）一律
经 `Window::push_notification` + `NotificationType::Error` 呈现；内联 `Tag`/文本只表达**进度与选择
状态**（未选择/已选择/正在选择/导入进度/完成/取消），失败状态的标签留空。目录读取失败只在
「从可读转为不可读」时推送一次，页面用 `model_catalog_error_reported` 记住已经说过，避免每次
快照重复同一条消息。

### 7. 行为预览退出模型页

模型页不再列出/预览行为，`SettingsCommand::PreviewModelBehavior`、
`SettingsErrorCode::ModelBehaviorPreviewUnavailable/Failed` 及其文案随之删除。行为目录仍由快捷键
页消费（未激活或未声明的行为不生成编辑入口）。runtime 侧的预览能力
（`Application::preview_motion`/`set_expression`）保留不动，恢复设置页预览是新增一个命令的事。

### 8. 窗口级拖放使用单文件夹契约

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

## 明确不做

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

## 残余风险与待验证项（不得当作已确认）

1. **没有「恢复内置」入口**：改名后只能自己再把名字打回去；换过的封面覆盖了内置封面在页面上的
   位置，包内那张原图仍在，但没有任何入口指回它。这与已安装模型是对称的（它的原封面被就地覆盖，
   同样拿不回来），但预置的原始封面其实还躺在 app 包里，所以这是**有意为之的不对称**而不是丢失。
2. **配置备份不含用户侧封面**：`model-overrides/` 是新的用户数据根，配置备份只覆盖 `config.json`；
   一份恢复回来的配置会重新指向用户侧的覆盖文件。文件被删掉时页面回落到包内封面，不算错误。
3. **预置元数据记录不做裁剪**：某个预置从这个构建里消失时，它的记录仍然留着（不可见、无副作用）；
   若将来某个构建又重新带上同一个 id，那时的名字会被这条旧记录接管。反向的取舍是：不裁剪就不会
   误删用户改过的名字。
4. **封面必须是 PNG**：选到 JPEG/WebP 会被拒绝（`model_cover_invalid`）。放宽需要为 `image`
   打开对应 feature 并决定是否重编码。
5. **封面替换后必须显式失效图像缓存**：替换保持同一路径（用户侧那份也是），GPUI 的资源缓存按路径
   命中，所以保存成功后由页面调用 `ImageSource::remove_asset`。这条依赖是隐式的，将来若把封面改成
   版本化文件名即可去掉。
6. **封面按 `object_fit(Cover)` 裁切**：非卡片比例的封面会被裁边而不是拉伸，极端比例会损失内容。
7. **导入前的标题输入仍在**：本项保留了在导入卡片里命名，因此「卡片上的改名」与「导入时的命名」
   都是标题来源；两者都写同一条元数据，不存在冲突，但不是一个入口。
8. **实机 UI 未验证**：未运行 `--settings-window-smoke` / 模型页 opt-in smoke，未在双平台实机
   点击卡片、选择封面或打开文件夹。
9. **Windows 的 `open_directory` 未实机验证**：本机 macOS 只能证明调用形态与 macOS 行为。
10. **编辑当前使用的模型时标题不再是强调色**：输入框内的文字颜色由输入控件自身的 editor style
   （`theme.foreground`）决定，行的文字颜色管不到它，所以显示态的强调色在编辑态失效。
11. **正文卡片高度 232 → 238**：标题行从「一行文本」（约 26px）变成「一个控件高度」（32px）后，
   卡片自然高度增加 6px，`MODEL_CARD_MIN_HEIGHT` 同步改为 238 并由测试钉住。封面、操作行和
   间距都没变。
12. **编辑态的标题停在原地，但文字会在框内让出内边距**：输入框是 GPUI Kit 的 `Input` 原样
    （边框 + 背景 + 聚焦环），组件自带 `px 10 / py 8` 的留白，所以打开编辑器时名字会向右移动
    10px（垂直方向因为两侧对称且盒子同高，位置不变）。显示态的标题没有这段缩进——它和封面左边缘
    对齐——这是刻意保留的：给显示态也加内边距会让卡片标题永远比封面缩进 10px。
13. **编辑态输入框的文字比组件默认大一档**：为了让两个面的字一样大，输入框的文本用 `text_base()`
    （16px）而不是组件为该尺寸准备的 14px；在 32px 的框里垂直留白因此比组件的设计值紧一些。
    想回到组件原样，删掉 `.text_base()` 即可（代价是编辑时字号从 16px 变 14px）。
14. **预置卡片的「打开所在文件夹」仍指向 app 包内的模型目录（只读）**：决策 5 把这个入口称作
    「手工替换美术」的退路，这对预置不再成立——在那里放一张 `cover.png` 不会生效，封面要走编辑器
    里的「选择封面」。按钮本身没有改（它打开的确实是这个模型所在的目录），因此这是一条**已知的
    不对称**，而不是被忽略的缺陷；若将来要收口，方向是让预置的这个入口指向
    `model-overrides/<id>/`，代价是它与「模型在哪」不再一致。
15. **拖放尚未完成双平台实机验收**：当前已覆盖 headless GPUI 的完整窗口蒙层、离开清理、单目录
    进入既有检查命令和多项目拒绝；尚未在 Windows Explorer 与 macOS Finder 中完成真实文件拖放，
    也未做 125/150/200% DPI、Retina 和真实主题下的目视检查。实现和自动化 contract 不等于这两个
    平台门禁已经通过。

## 验证

已完成（2026-09-18，本机 macOS arm64，固定工具链 1.97.1。**历史记录**：「预置模型只读」相关的
几条已被 2026-09-22 的修订取代，见下文）：

- `bongocat-model` 95 测试（新增 2）：`a_cover_replacement_lands_on_the_package_cover_and_leaves_no_staging_file`
  断言替换落在 `<包根>/resources/cover.png`、内容逐字节相等且不留 `.cover.png.new`；
  `a_cover_cannot_be_replaced_on_a_model_that_is_not_installed` 断言缺失模型返回 `NotFound`。
- `bongocat-platform` 58 测试（新增 1，改写 1）：`a_cover_selection_is_a_canonical_regular_file`
  断言封面选择器只要求「真实常规文件」并把选择 canonical 化，目录与缺失文件仍报
  `SelectionInvalid`；后台线程用例扩展到 `pick_model_cover`。
- `bongocat-app` 130 测试（新增 2，改写 2）：`service_renames_and_recovers_an_installed_models_title_and_cover`
  走完整服务链路断言改名持久化到配置、空标题报 `ModelTitleInvalid`、封面字节落到包内并出现在快照、
  非 PNG 报 `ModelCoverInvalid` 且原封面不变、预设改名/换封面报 `PresetModelMetadataImmutable`；
  `service_opens_a_models_own_folder_without_advancing_revision` 用注入的文件管理器断言打开的正是
  模型目录、配置 revision 不变、后端拒绝与模型目录消失都报 `ModelLocationOpenFailed`。
- `bongocat-ui` 115 测试（改写 6）：`model_row_actions` 断言预置恒不可编辑/删除、目录缺失不呈现
  打开入口；tab 顺序断言覆盖五个控件与确认删除态；导入状态断言改为「失败态内联文本为空、
  失败经通知呈现」；行为标识的作用域改由快捷键行断言（`shortcut_behavior_rows` 区分同一模型的
  motion/expression，也区分两个模型的同一行为）。
- 文案：`tools/validate-locales.py` 通过（386 键 × 2，删除 12 键、新增 15 键）；另以脚本核对
  Rust 中引用的 203 个字面量键全部存在于目录，且 `models.`/`errors.settings.` 命名空间无孤立键
  （该脚本发现并修掉了一处 `models.edit.cover.selected` → `models.edit.cover.replace` 的静默失配，
  这正是 `validate-locales.py` 不覆盖的语义漂移）。
- `just check` 的六道门全过：`cargo fmt --all -- --check`；三组严格 Clippy（workspace 排除 app、
  app `storage-test-injection`、app `production`）；`cargo test --locked --workspace` 全绿
  （app 130 + bin 21、ui 115、model 95、platform 58、runtime 73、config 51、live2d 53、overlay 43、
  update 36、packaging 23、render 15、i18n 4 等）；`cargo check --locked --workspace --release`；
  `tools/validate-fixtures.py`（9 输入 + 8 模型用例）、`tools/validate-json-schema.py`
  （14 config + 6 state + 9/9）与 `tools/tests`（63 项）。

**未运行**：双平台实机设置页点击（选中/编辑/换封面/打开文件夹）、`--settings-window-smoke`、
模型页 opt-in smoke、Windows 实机 `open_directory`、真实社区模型回归。

### 修订（2026-09-22，卡片编辑改为原地）已完成

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

### 修订（2026-09-22，预置模型同样可编辑）已完成

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

### 修订（2026-09-24，窗口级文件拖放）已完成自动化部分

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

---
<a id="adr-0055"></a>
# ADR-0055: 导入后的模型封面由渲染一帧截取

状态：已接受（2026-09-22）
依赖：ADR-0002（GPUI for Settings UI）、ADR-0003（Native Overlay Renderers）、ADR-0037（在应用内导入 BongoCatMver 模型）、ADR-0047（模型库只做选择与元数据编辑）

## 背景

1. **转换出来的封面是源包里的同一张占位图。** BongoCatMver 转换按 ADR-0037 的输出布局，每个输入模式各取
   `img/<模式>/cat.png` 原字节装成 `resources/cover.png`。上游各模式的 `cat.png` 本身就是同一张手绘
   占位图，于是「标准 / 键盘 / 手柄」三个模型导入后封面逐字节相同，模型页看起来是三张重复卡片。
2. **封面本来就可以由模型自己画出来。** overlay 的渲染路径能把一个模型渲染成完整一帧：模型自己的背景图、
   Live2D drawable，以及键位层；预置模型的卡片图就是这个构图。缺的不是渲染能力，而是「谁来截、在哪截、
   什么时候截」。
3. **要被截的模型不是当前激活的模型。** 刚导入的模型此刻并没有被 overlay 渲染，因此必须新起一个私有
   runtime 和一个自己的原生窗口——形态上就是既有的 `run_model_preview`，区别只在于窗口永不显示、帧在
   present 之前就被读回。原生窗口只能在拥有它的线程上创建：macOS 是 AppKit 主线程，Windows 是创建 HWND
   的线程；本产品里两者都是 GPUI 线程（overlay 也由它 tick）。
4. **导入发生在 settings service worker 线程上。** 该 worker 执行 import command，不能建窗口，因此
   「把模型渲染成封面」必须跨线程交接。而 `bongocat-ui` 不依赖 runtime / render / Live2D（ADR-0002 与
   Technical Design 的 crate 边界），协议层不能携带模型类型，交接只能走 app 自己拥有、两个线程共享的队列。
5. **ADR-0047 的「明确不做」里写着「不让 renderer 或 runtime 参与封面」。** 本条明确推翻它：封面从此是
   渲染产物，而不是被原样搬运的源文件。ADR-0047 的其余决策（封面位置契约、PNG 契约、原子替换、
   错误只用通用 Notification）保持不变；其中「预置模型只读」一条后来由 ADR-0047 自己的 2026-09-22
   修订取代（预置可改名、可换封面，定制落在用户侧），与本条无冲突：本条只覆盖导入模型的截取。

## 决策

### 1. 导入成功后为每个安装的模型排队一次截取

`ApplicationMainThreadSignals`（由 `ApplicationShortcutSignals` 更名，因为它现在承载的不只是快捷键信号）
新增一条 cover capture 队列。settings worker 在 import command 成功后，把每个已安装模型连同它在
settings 协议里的身份（`SettingsModelKey`，origin 恒为 installed）入队。worker 只排队：它不渲染、不读
封面、不写任何字节，也不为截取失败而改变导入结果。

### 2. 由 GPUI 线程在 50 ms 轮询里完成渲染

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

### 3. 两个平台后端共用一份「帧 → 封面」逻辑

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

### 4. 截取失败不改变任何东西（~~2026-09-23 起改为终止导入~~，见修订）

失败时保留源封面、不重试、不阻塞导入、不经 UI 报错。封面是显示用美术而非模型数据（ADR-0047 决策 4），
错误的图不是坏掉的模型。可观测性为零这件事是本节的一部分，见残余风险 2。

**本节已被 2026-09-23 的修订取代**：截取失败现在会撤销这次导入并报「模型导入失败」。原文保留为当时
的决策记录；取代它的理由（截取与激活共用同一条 GPU 准备路径）见文末修订。

### 5. 封面写入仍走同一条 store 通道

`Application::set_model_cover_bytes` 与 `set_model_cover` 共享同一个 `install_model_cover` 契约：PNG 签名
+ 不超过包的每文件上限，然后 `ModelStore::replace_cover` 原字节落盘。`SettingsCommand::ReplaceModelCover`
与 `SetModelCover` 的差别只是载荷（bytes 还是 path），因为捕获出来的封面在被安装之前根本不是一个文件。

### 6. `capture-cover` 子命令作为可复现证据

`bongocat-overlay capture-cover <standard|keyboard|gamepad> <out.png>` 让整条链路（隐藏窗口、GPU 读回、
裁切、编码）脱离导入流程单独执行，因此平台 backend 可以在没有 UI 的情况下被检查与回归。

## 修订（2026-09-22，当日）

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

## 修订（2026-09-23，截取失败改为终止导入）

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

## 明确不做

- **不加偏好开关**：用户要的是「导入后就是模型自己的样子」，而不是多一个开关。此前考虑过的「偏好设置
  增加一个封面字段」被否决。
- **不放松 ADR-0047 决策 4 的 PNG 契约**：仍然只校验签名与大小，不重编码，不为 JPEG/WebP 打开 `image`
  feature。
- **不为已存在的模型回填封面**：本次只覆盖导入路径（含 MVer 三模式各自的导入），不扫描历史模型，
  也不新增「重新生成封面」入口。
- **不让 `bongocat-ui` 依赖 runtime / render / Live2D**：协议只携带 `Vec<u8>`，交接走 app 侧共享队列。
- **不改预置模型的包**：预置包仍是 product files，永远不被写入。用户给预置换的封面存在用户侧
  （见 ADR-0047 决策 2 修订），本决策的截取路径只服务导入模型，不参与预置封面。

## 残余风险与待验证项（不得当作已确认）

1. **~~截取在 GPUI 线程上同步进行~~ 已消除（2026-09-22 修订）**：最初的实现把约 30 帧的截取整段
   同步跑在 GPUI 线程上，期间 overlay 帧循环与设置窗口都会停顿。同日即改为决策 2 所述的可让步
   会话，见下方修订。
2. ~~**失败是静默的**~~ **已消除（2026-09-23）**：截取失败现在撤销这次导入并报「模型导入失败」
   （`models.import.failed`，双语，`tools/validate-locales.py` 校验键集合）。当时记的是「报出失败需要
   新增 `ApplicationLogCode`、双语 i18n 文案或复用 `models.cover` 的失败通知，本次一条都没引入」——
   修订选了新增 i18n 文案这条路，并且不止于报告：失败的模型会被删除，见文末修订。
3. **Windows 后端未编译、未实机验证**：本机 macOS 无法交叉编译 `windows-msvc`（`libdeflate-sys` 的 C 构建
   需要 Windows SDK），仓库的 Windows 验证在 `windows-latest` runner 上执行。Windows 侧的风险集中在
   `Renderer::draw_inner` 新增的 `capture` 分支与 `read_staging_frame` 的行拷贝。
4. **截的是 idle 一帧**：若模型第一秒的物理尚未收敛，截出来的姿态可能与用户随后看到的略有差异；
   30 帧与 5s 上限是为此设的收敛窗口，不是精度保证。
5. **隐藏窗口仍需合成器环境**：D3D11 + DComp 或一个可用的 Metal drawable。没有 GPU、没有窗口服务器的
   环境会失败；2026-09-23 起该模型不会被导入（原行为是保留源封面），因此在无 GPU 环境里导入会整体
   不可用。这条风险的代价变大了，但它同时也是「模型激活不了」的正确信号。
6. **图像缓存失效依赖捕获方显式调用**：设置窗口未打开（句柄为 `None`）时跳过，依赖窗口之后重新加载
   同一路径时读到新字节。
7. **每次导入多一个隐藏窗口 + 一个独立 runtime**：MVer 三模式会串行处理三次，模型越大耗时越长。

## 验证

已完成（2026-09-22，本机 macOS arm64）：

- 新增单元测试：`cover.rs` 覆盖预乘 BGRA 转换、行 pitch 校验、内容裁切、缩放与 PNG 编码（`bongocat-overlay`
  52 测试，此前 43）；`bongocat-app` 新增
  `service_queues_a_cover_capture_per_imported_model_and_installs_the_result`，走完整服务链路断言导入后队列里
  恰好一个请求、其 id 与快照里 installed 条目的 id 一致、`ReplaceModelCover` 的字节落到
  `<包根>/resources/cover.png` 并出现在快照 `cover` 字段、非 PNG 被拒绝且原封面不变（app lib 139 测试）。
- 端到端证据：`cargo run --locked -p bongocat-overlay -- capture-cover keyboard /tmp/cover-check.png`
  → 186,091 bytes / 640×352 PNG，与常量搬迁前的同一命令输出逐字节相同；standard / keyboard / gamepad
  三个预置模型各自产出不同构图的封面（此前三个模式共用一张占位图）。
- `just check` 的六道门全过：`cargo fmt --all -- --check`；三组严格 Clippy（workspace 排除 app、app
  `storage-test-injection`、app `production`）；`cargo test --locked --workspace` 全绿（816 项）；
  `cargo check --locked --workspace --release`。

修订验证（2026-09-22，本机 macOS arm64）：`cargo fmt --all`；三组严格 Clippy 全过；
`cargo test --locked --workspace` 全绿（34 目标 / 0 failed）；`cargo check --locked --workspace --release`
通过；`capture-cover keyboard` 经新会话路径产出 186,091 bytes / 640×352，与修订前逐字节相同。

修订验证（2026-09-23，本机 macOS arm64）：`cargo test -p bongocat-ui --lib` 158 通过——
`model_import_success_waits_for_every_queued_cover_capture` 改用成功截取表达原意，新增
`a_failed_cover_capture_abandons_the_import`（变异验证：去掉 `model_import_failed_pending` 赋值即报
「the failed capture must be reported to the user」）；`cargo check --workspace --all-targets` 通过；
`tools/validate-locales.py` 2 locale × 243 键通过；`just check` 六道门全过。真实模型证据（本机 Metal）：
把两个源各用 `bongocat-model-store --example model_conversion_smoke` 转成包后跑 `capture_model_cover`，修复
渲染快照 preflight 前报 `drawable geometry is empty`；修复后两个模型都截出封面——`芙宁娜(无气泡)`
384423 B / 625×640（2.10 s）、`芙宁娜(按键气泡）` 384116 B / 625×640（1.94 s）。`GpuModel::prepare`
（含 2 张 2048² 模型纹理与 76 张 1200×1040 键位图上传）实测 395 ms，远在 runtime 的 2 s 激活超时
之内——这也排除了「截取失败其实是超时」这一替代解释。
**这条真实模型证据来自一次性探针，探针没有留在仓库里**；留在仓库里的是上面两项自动化测试。

Windows 侧（2026-09-23）：`bongocat-overlay` 整体无法交叉编译到 `x86_64-pc-windows-msvc`
（`bongocat-model-store` 拉进 `libdeflate-sys`，其 C 构建需要 Windows SDK），因此按
`bongocat-rust-ci-triage` §5.1 的手法搭隔离 crate，把 `GpuModel::prepare` 的缓冲创建与
`sync_snapshot` 的顶点上传守卫逐字抄入后 `cargo check --target x86_64-pc-windows-msvc` 通过，
并用 `compile_error!` 探针与一次 `E0308` 变异确认该检查真的在编译、也真的能抓住类型错误。
**这只是类型证据，Windows 实机截取与启用仍未验证。**

**未运行**：双平台实机导入（导入后观察卡片封面变化）、`--settings-window-smoke`、模型页 opt-in smoke、
Windows 实机截取与启用、真实社区 MVer 模型的批量回归；** Spinner 在截取期间持续旋转属目视行为，
未自动化验证，需人工观察导入过程确认。**

---
<a id="adr-0060"></a>
# ADR-0060: Model package and model-store boundary

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0036, ADR-0037, ADR-0038, ADR-0041, ADR-0047, ADR-0050, ADR-0055

## Context

`bongocat-model` combined two different responsibilities: read-only model3
package parsing and the mutable lifecycle that copies external sources into an
environment-owned store. The latter also owned UUID allocation, writer locks,
staging and atomic commit, legacy Mver conversion, key-image normalization, and
user-side preset cover replacement. As a result, parser-only consumers compiled
against image composition, UUID, and storage-write dependencies, while the
application's storage errors and import progress were mixed with package
validation types.

## Decision

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

### Commit ownership

Rust cannot express “only this sibling crate may call this constructor” without
adding another lower-level crate. The split therefore uses a narrow, documented
product seam instead of claiming compile-time sealing: `PreparedModel::relocate`
rebinds an already validated package after the store's atomic rename, and
`InstalledModel::from_prepared` is called by the store transaction before the
existing `CommittedModel` conversion. The model-store/app wiring is the only
supported caller. A future requirement for hostile third-party callers would
need a separate sealed commit type; this ADR does not pretend the current Rust
visibility provides that stronger guarantee.

## Consequences

- parser-only crates no longer compile the Mver image/PNG/UUID/storage-write
  dependency set;
- model-store tests cover permissions, locks, staging, cancellation, recovery,
  Mver conversion, cover writes, and every registered shared fixture;
- source directories remain read-only inputs; only store staging and committed
  destinations are written;
- `CommittedModel` construction is a documented store/preset product invariant,
  not a compiler-enforced sealed type in this slice;
- Cubism Core lifecycle and Core-coupled parameter application remain in `bongocat-live2d`; pure motion/expression playback is owned by `bongocat-live2d-playback`.

## Rejected alternatives

- Moving `CommittedModel` and all model consumers into the store crate:
  rejected because it would make every renderer/runtime crate depend on the
  mutable import layer merely to hold a parsed model.
- Moving the read-only preset catalog into the store crate: rejected for this
  slice because it would unnecessarily broaden the dependency graph of
  overlay, Live2D, and runtime.
- Keeping the old modules in `bongocat-model` behind re-exports: rejected
  because it would preserve the dependency direction this boundary is intended
  to remove.
- Adding a new lower-level commit crate just to regain compile-time sealing:
  rejected as disproportionate for the current in-process, application-owned
  store; revisit only if an untrusted plugin boundary appears.

---
<a id="adr-0061"></a>
# ADR-0061: Live2D playback boundary

- Status: accepted
- Date: 2026-09-24
- Amended: 2026-09-25
- Depends on: ADR-0060, ADR-0041, ADR-0050, ADR-0055

## Context

`bongocat-live2d` combined Cubism Core ownership with motion3/exp3 parsing and
pure curve evaluation. The Core wrapper is tied to raw bindings, model
resources, parameter IDs, part-opacity sinks, and GPU-facing snapshots, while
the motion and expression parsers only need bytes, monotonic elapsed time, and
project-owned numeric results. Keeping both responsibilities in one crate made
pure playback tests compile the vendor SDK and obscured which layer owns
resource loading versus Core writes.

## Decision

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

## Consequences

- motion/expression parser and numeric tests no longer compile the vendor SDK;
- Core-coupled loading and application tests remain in `bongocat-live2d`;
- runtime's existing frame order and playback ownership do not change;
- no physics/pose crate or placeholder is introduced by this slice; those
  evaluators remain a future extension of the same pure boundary;
- the old `bongocat-live2d` playback re-export is removed after runtime imports
  the playback types directly.

## Amendment: completed motions hold their terminal pose

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

### Superseded amendment: automatic breath is an explicit weighted model-range group

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

### Superseding amendment: restore the Mver reference breath and physics path

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

## Rejected alternatives

- Moving Cubism Core, model loading, or GPU resource preparation into the new
  crate: rejected because that would make pure playback depend on FFI and
  rendering.
- Moving runtime `MotionPlayback`/`ExpressionPlayback` state into playback:
  rejected because those values encode product command, priority, generation,
  and audio/shutdown semantics.
- Keeping file loaders in playback with a `CommittedModel` dependency:
  rejected because resource I/O and model identity are not part of numeric clip
  evaluation.
- Creating empty physics/pose modules: rejected until authorized fixtures and
  the R5 compatibility gate define their contracts. A bounded physics evaluator
  is now allowed only for declared, validated v3 resources; pose remains gated.

---
<a id="adr-0062"></a>
# ADR-0062: Live2D render-resource preparation boundary

- Status: accepted
- Date: 2026-09-24
- Depends on: ADR-0042, ADR-0060, ADR-0061, ADR-0003

## Context

`bongocat-live2d` combined the Cubism Core owner with model-package render
resource work. The Core owner needs a committed model, a Moc, Core parameter
IDs, and drawable snapshots. Key-image inventory, background discovery, PNG
dimension checks, and key-overlay resolution do not need Core or a GPU; they
only need the committed package and the immutable `bongocat-render` resource
types.

Keeping those CPU resource concerns in `bongocat-live2d` made the app and
runtime depend on the Core crate for key-image contracts, and made it harder
to test the package-to-render boundary without compiling the vendor SDK.

## Decision

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

## Consequences

- model resource preparation and key-image contracts can be tested without the
  Cubism SDK;
- `bongocat-live2d` no longer owns the image decoder or key-image directory
  scan;
- app/runtime no longer acquire those contracts indirectly through the Core
  crate;
- the existing "missing artwork means no binding" behavior and side-specific
  overlay rules remain unchanged;
- no new renderer backend, window owner, or GPU lifecycle is introduced.

## Rejected alternatives

- Moving all of `bongocat-overlay` into a new crate: rejected because its
  platform files also own windows, input routing, placement, and shutdown;
  separating those owners requires a separate platform seam.
- Moving PNG decoding into `bongocat-render`: rejected because that crate is the
  immutable contract and must remain independent of filesystem/image details.
- Letting the renderer decide whether a key has a binding: rejected by ADR-0042;
  the app gates bindings using the shared inventory contract.
- Reimplementing key-image scanning in app and runtime: rejected because the
  inventory and the loader must remain one source of truth.

---
<a id="adr-0063"></a>
# ADR-0063: 模型渲染采用 encoded-space 兼容颜色管线

状态：Accepted
日期：2026-09-24
取代：ADR-0046
修订（2026-09-24）：根据多层模型在逐 drawable 透明度下出现重影的实机反馈，presentation opacity
改为在内部 drawable 合成完成后由平台最终 surface 统一施加。

## 背景与证据边界

用户反馈 Windows 上的模型颜色与 Bongo-Cat-Mver 明显不同。固定上游 commit
`4da0b9468ad3b6ffaa096eba3f080501d6ab0b5c` 的源码树能直接确认以下有限事实：

- `myUserModel.cpp` 使用官方 Cubism Framework 的 OpenGL renderer，并通过
  `LAppTextureManager::CreateTextureFromPngFile` 加载模型纹理。
- `myUserModel.cpp` 的 `PREMULTIPLIED_ALPHA_ENABLE` 条件分支在宏未定义时调用
  `IsPremultipliedAlpha(false)`。
- drawable 的 clipping、order 和 blend 交给官方 renderer；背景、设备和按键资源还在 mode 层
  另行组合。
- 行为清单基线 `44f44bc` 的旧版 `src/pages/main/index.vue` 将 `window.opacity / 100` 施加到包含
  背景、Live2D canvas 和按键图的根容器；这支持“窗口 opacity 属于最终 surface”的产品语义，
  但它不是固定 Mver C++ commit 对 opacity 实现细节的证明。

该 commit **不包含** `LAppTextureManager` 的实现、官方 OpenGL shader、完整 build flags，也没有
固定 SFML 或 Cubism Framework 的实际版本。因此不能仅凭 Mver commit 断言最终二进制的纹理上传
格式、shader 数学、背景/按键上传语义或精确像素结果。

项目另行固定了 Cubism Native Framework R5 `5-r.5` 的行为来源
（`docs/cubism/cubism-framework-behavior-sources.md`）。其中 OpenGL 参考路径使用普通
`GL_RGBA` render target 与 encoded-space shader 颜色处理，但它是独立的 R5 oracle，不是 Mver
二进制来源的直接证明。
本 ADR 因用户报告、上述有限 Mver 证据和独立参考路径而选择兼容性方向；来源链和像素等价仍是
TODO 的验收门禁，不被本 ADR 的 Accepted 状态掩盖。

## 决策

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

## 影响

- 在来源链尚未完全核实的前提下，优先选择能解释用户色差、且不改变模型资源的产品兼容语义；
  最终是否与实际 Mver 像素一致仍由硬件 readback 决定。
- 两端使用相同的 encoded RGBA 输入和 blend state；presentation opacity 在内部 drawable 合成
  完成后由各自平台的最终 surface 机制统一施加，避免自动 gamma 转换或逐层衰减造成单边漂移。
- macOS Core Animation 和 Windows DirectComposition 仍可能在广色域显示器上对最终 surface
  做不同的系统级显示转换；renderer 不承诺跨系统色彩管理绝对一致。
- 不把 Cubism Framework 的 C++ 源码、shader 文本或厂商二进制复制进产品；实现只使用项目
  自有 contract，并遵守 Cubism 许可边界。

## 当前验证与待完成门禁

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

---
<a id="adr-0065"></a>
# ADR-0065: 模型模式元数据与卡片 Badge

状态：已接受（2026-09-25）
依赖：ADR-0037（在应用内导入 BongoCatMver 模型）、ADR-0047（模型元数据编辑边界）、ADR-0059（UI protocol 边界）

## 背景

模型管理页需要让用户直接看出每个模型属于标准、键盘还是手柄模式。模式必须跨重启稳定，不能只从标题或目录名临时推断。

普通 BongoCat 模型包没有独立的模式字段，但产品现有的键图资源约定提供了可验证的形状：

- `resources/left-keys` 的 PNG 键图是标准模式的基础；
- `resources/right-keys` 的 PNG 键图表示额外的右手输入资源；
- 手柄模式使用 `DPad*`、`*Trigger*`、`South/East/West/North`、`Start/Select` 等手柄专用键名。

这些是**导入时的分类规则**，不是运行时每次渲染都重新执行的猜测。分类在模型 staging 通过完整包校验后、原子提交前完成；没有任何可用键图、无法归入三类资源形状的普通包直接拒绝，不写入 store，也不写入配置。

BongoCatMver 是例外：它有明确的 legacy `standard`、`keyboard`、`gamepad` source section，所选 section 比转换后目录形状更权威。构建预置模型则以稳定 id `standard`、`keyboard`、`gamepad` 标识自身模式。

## 决策

### 1. installed 模型元数据持久化 `input_mode`

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

### 2. 导入时判定普通包模式

`bongocat-model-store` 在普通包复制、键名归一化和第二次 `PreparedModel` 校验之后扫描 staging 树，使用与 runtime 键图读取相同的直接 `.png`/文件名语义：

1. 任一手存在手柄专用键名 → `gamepad`；
2. 否则任一手存在 `right-keys` → `keyboard`；
3. 否则存在 `left-keys` → `standard`；
4. 两者都不存在 → `InvalidPackage`，导入事务失败。

判定结果和模型一起从 store API 返回，Application 在同一导入循环中写入 metadata。模式判定失败时，用户源目录、store 目的地、staging 和 config 都保持不变。`right-keys` 单独存在即可判定 keyboard；手柄判定不只依赖 `East`，还检查完整的预置/转换手柄词汇子集。

Mver 转换不重新用上述启发式覆盖 source section，而是把用户选择的 legacy mode 传入同一提交尾部。预置模型也不从资源目录反推，直接由稳定 preset id 映射。

### 3. settings snapshot 投影强类型模式

`bongocat-ui-protocol` 独立拥有 `SettingsModelMode`。Application 把 config/model 值穷尽映射为该协议类型，并把它放进 `SettingsModelEntry.input_mode`。正常导入的模型一定有该值；只有手工放入 store 且无法通过包校验的异常目录才会没有模式 Badge。

GPUI 页面只消费 snapshot，不读取 config、不扫描模型目录，也不自行解释 id 或标题。

配置、model-store、UI protocol 各自保留自己的稳定枚举，避免跨层依赖或第三方类型泄漏。Application 的映射必须覆盖全部枚举值。

### 4. 卡片右上角使用无图标、中性色的 GPUI Kit 标记

固定 GPUI Kit revision 的 `Badge` 原生支持数字、圆点和图标，不是文字标签 primitive。Models 页面保留 `Badge` 作为封面覆盖层容器，但不再使用它的 icon/number overlay；可见内容改为与原实现一致的 compact `secondary` `Tag`：

- 标准、键盘、手柄使用相同的中性主题色；
- 模式差异由本地化文字表达，不增加彩色背景噪声。

标记定位在封面右上角，不新增卡片行，也不改变封面、标题编辑面或卡片高度。模式文字始终可见，标记是只读展示，不加入 Tab 顺序。

### 5. 本 ADR 不改变 runtime 输入绑定

`input_mode` 在本阶段是导入来源/模型类别元数据，用于配置完整性、快照和模型卡片展示。它不在本 ADR 中切换 runtime 的 key/gamepad binding；现有 installed 模型默认键盘绑定及预置 gamepad 特例仍按其独立任务维护。

因此“卡片显示 Gamepad”表示该模型来自或被构建标识为 gamepad 模式，不等于本 ADR 已完成 installed Mver gamepad 的 runtime 输入接线。未来若让该字段驱动输入行为，必须单独修改 runtime/overlay contract、迁移既有绑定测试并完成双平台手柄验证，不能在纯 UI 改动中静默发生。

### 6. 当前 v1 直接增加必填字段

完整 v1 尚未发布，因此不增加 schema 版本、migration、alias 或旧字段 fallback。缺少 `input_mode` 的 installed metadata、未知枚举值或其它旧开发结构都不是当前完整 v1，按现有严格配置恢复边界处理。Development 与 Production 使用相同结构、不同存储根。

## 结果

- 普通模型不会以“未分类”进入 store 或 config；无法判定就是导入失败。
- Mver 三种转换模式在配置、快照与卡片之间有强类型闭环。
- 普通包的标题、目录名和具体美术文件不会改变已保存的模式；改名、删图、目录变化和重启不会重新推断。
- 预置模式不与用户可编辑标题或封面元数据混在一起。
- UI 使用官方 GPUI Kit 组件、主题色和本地化可见文案，没有自绘状态；模式标记无图标、位于封面右上，并保持中性 `secondary` 颜色。

## 未验证 / 后续

- Windows/macOS 设置窗口中右上角模式标记的真实观感、Retina/DPI 和长本地化文本仍需平台 smoke 与截图检查。
- installed Mver gamepad 的 runtime 输入接线不属于本 ADR；如产品要让模式 Badge 同时代表实际输入行为，需要独立任务。

---
<a id="adr-0065"></a>
# ADR-0065: Runtime-Owned Random Model Behavior Scheduler

状态：已接受（2026-09-25）

> 部分被 ADR-0072 取代：`model.random_behavior.enabled` 已改为
> `model.random_behavior.mode`（`off` 为其中一个取值），候选集合不再是固定的
> motion 与 expression 合并列表。间隔字段、`RandomBehaviorSettings`、
> `Idle` priority、seed 注入与 admission gate 的决策不变，仍然有效。

## 背景

模型同时声明 motion 和 expression。设置页需要一个正交的开关和时间间隔，让用户在不让
GPUI 或 renderer 自己决定业务行为的情况下启用周期性随机播放。两个字段必须进入当前 v1
配置、typed settings protocol 和 runtime，而不能作为 UI-only 草稿或由 renderer 读取配置文件。

## 决策

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

## 验证

- `bongocat-config` fixture 覆盖默认 round-trip 和非法间隔；协议测试覆盖 typed command。
- runtime scheduler 单元测试覆盖固定 seed、间隔等待、禁用/模型切换重锚和时钟回退；另一个
  Core-backed runtime 集成测试验证模型在一个间隔后确实产生 motion 或 expression。固定 seed 的
  Core-backed 构造器仍属于后续测试基础设施，不把生产 seed 当作可重复性证据。
- audio/runtime 回归测试覆盖独立音频序号分配、序号回绕比较、overflow recovery 保留模型
  `Prepare`、隐藏 fade 的逻辑 settled，以及 shutdown admission barrier 和非阻塞 request；这些测试
  不替代双平台实机和长时间 soak 证据。
- 仍需双平台实机观察、长时间随机序列/时钟变化 soak，以及正式发布门禁；本 ADR 不把
  scheduler scaffold 宣称为完整 Live2D 兼容或平台发布证据。

## 后果

随机行为不再由第三方 `easy-live2d` helper 或旧版配置隐式继承；v1 明确拥有该产品
行为。配置初始关闭不会改变现有启动观感，用户打开后需要等待一个完整间隔才看到第一次选择。

---
<a id="adr-0072"></a>
# ADR-0072: Random Behavior Becomes a Mode Enumeration

状态：已接受（2026-09-27）

## 背景

ADR-0065 给模型空闲随机播放定义了两个字段：`model.random_behavior.enabled` 门禁和
`model.random_behavior.interval_seconds` 间隔，候选集合恒为模型声明的 motion 与
expression 合并列表。这只回答了「要不要随机」和「多久一次」，没有回答「随机什么」。

用户反馈这个组合解决不了两类真实需求：只想让模型换个表情（保持它对按键的姿态响应
完全由自己控制），或者只想让它自己动一动（不改表情）。旧实现把「动作或表情」写死在
一个布尔门禁后面，用户要么两个都要，要么两个都不要。

## 决策

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

## 验证

- `bongocat-config` 覆盖四个取值都通过文档往返、持久化拼写等于文档化的
  `snake_case` 名、目录外的取值被拒而不是忽略、间隔在 `off` 下同样被校验；JSON Schema
  与全部共享 fixture 同步。
- `bongocat-runtime` 覆盖 `off` 在 24 个 seed 下从不选择行为、每个单一类别 mode 在
  24 个 seed 下从不越界、合并 mode 确实能选到两类、模型只声明一类时另一类 mode 保持
  无操作、关闭 mode 与模型 commit 都清除待触发截止时间。
- `bongocat-app` 覆盖 mode 经 config → runtime → settings 全链持久化并在重启后恢复，
  越界间隔仍使配置与 runtime 一起保持不变。
- `bongocat-ui` 覆盖四个选项在全部语言下非空、唯一且可逆，mode 与待发间隔合成同一次
  patch，`off` 保留用户已选的间隔，关闭状态下改间隔不触达服务，以及打开/关闭的连续选择
  排成一条 typed command 链；settings smoke 断言新增双语 key 非空。
- 变异验证：把 `admits` 的两个类别对调、让 `off` 也进入候选过滤、把 `is_active` 反向读取、
  或让间隔 mutator 去掉 `off` 守卫，都会让上述测试变红。
- 未运行：Windows 10 1903+ 与 macOS 12+ 实机目视确认下拉在 125/150/200% 与 Retina 下的
  排版，以及长时间随机序列 soak 与双平台 GPUI 渲染门禁。这些仍属 ADR-0065 与 ADR-0066
  的完成门禁，本 ADR 的自动化证据不替代它们。

## 后果

用户可以在「表情」「动作」「两者」之间三选一，也可以整体关闭，且默认行为与之前一致。
配置少一个布尔字段，多一个枚举字段；两者都是 v1 直接变更。`enabled` 消失后，
`RandomBehaviorSettings` 只剩 mode 与间隔，runtime 不再需要「门禁」与「模式」两个概念
去描述同一件事。

---
<a id="adr-0077"></a>
# ADR-0077: 接受没有输出项的物理设置

状态：已接受（2026-09-29）
补充：ADR-0060（模型包 store 边界）、ADR-0061（Live2D playback 边界）

## 背景

`bongocat-model` 的 `validate_physics_resource_value` 曾要求每个 `PhysicsSetting` 同时有
输入、输出和至少两个顶点，缺输出项即以 `physics3 settings require input, output, and at
least two vertices` 拒绝整个包。

这条规则比格式本身严格。Live2D 官方 `physics3.json` schema
（`Live2D/CubismSpecs` 的 `FileFormats/physics3.json.md`）把 `Output` 列为 required **键**，
但对数组长度没有任何要求；Cubism Editor 导出的模型里，"模拟了粒子但不写回任何参数"的
设置项是正常存在的。

用户报告的模型正是这种情况。`cat.physics3.json` 的 32 个 `PhysicsSettings` 里有 16 个
`"Output": []`，名字是「头发Y轴」「翅膀物理」「耳坠」「呆毛2」等——正是编辑器里搭好、随后
把输出接到别处的设置项。三个模式的标准/键盘/手柄版本都是这个形态（16/15/14 个），而这个
`cat.physics3.json` 与原始 MVer 模型 `img/standard/cat_model/` 下那份逐字节相同
（`c6722ed4f8b815b4`），说明转换过程没有改动它，问题一直在校验侧。旧版本能导入，是因为它
走官方 Cubism Framework 的解析，那条路径同样不要求非空输出。

结果是：模型包校验通过前就失败，用户看到的是「模型包无效」，而模型本身完全可用。

## 决策

### 1. 输出项可以为空，模拟照做但不写回

删除 `setting.outputs.is_empty()` 这一项要求，保留输入项非空和至少两个顶点。

理由不是"放宽一点试试"，而是求值器本来就按这个语义实现。`bongocat-live2d` 的
`PhysicsRuntime` 用 `setting.outputs.len()` 分配每个设置的输出缓冲区
（`crates/bongocat-live2d/src/physics.rs` 的 `reset`），`step` 与 `interpolate` 都按输出
列表迭代：列表为空时循环体不执行，粒子照常模拟，结果无处可写。这正是官方 Framework 的行为
——`csmUpdatePhysics` 为每个设置计算粒子，再逐个 `Output` 写回，没有输出就没有回写。

因此空输出是**惰性**而不是**错误**：它描述的是"这个设置驱动不了任何参数"，而不是"这个
设置坏了"。

### 2. 保留的两条要求仍然有牙齿

- **输入项非空**：没有任何输入的设置无从反应，等于没有刚体。
- **至少两个顶点**：一个顶点挂不出摆锤，`reset` 也要靠前一个顶点递推初始位置。

两者都继续拒绝，且各自的反例被测试固定。

### 3. Meta 计数仍然照实校验

`TotalOutputCount` 为 0 是合法的，计数一致性检查（`Meta` 声明值必须等于数组实际长度）
不变。这条检查防的是元数据自相矛盾，与输出项是否为空无关。

## 后果

- 报告的模型与另一个同病模型（`魈二代 · 标准模式`）现在都能导入，行为为
  `Ready`，物理照常求值。
- 与 Cubism Framework 的一致性提高：两条路径现在对同一个文件给出同一个答案，而校验层不再
  比被参照的实现更严格。
- 只放宽了输出项。字段级规则（权重范围、`VertexIndex` 边界、Meta 计数、Id 唯一性等）
  一条未动，因此这不是一次泛化的"宽松化"，而是一处定向修正。
- 回归测试分两层：`bongocat-model` 固定"空输出合法、缺输入/缺摆锤仍拒绝、混合 rig 正常"，
  `bongocat-live2d` 固定"空输出设置被跳过而旁边的有效设置照常驱动参数"——后者防止跳过逻辑
  退化成"整个 rig 变成 no-op"而测试仍然通过。

---
<a id="adr-0085"></a>
# ADR-0085: 动作和表情支持自定义名称

状态：已接受（2026-10-06）
补充：ADR-0067（配置域名与模型身份）、ADR-0079（新增配置字段的可选读取约定）
需求：Issue #1111

## 背景

Issue #1111 报告：快捷键页面里模型的动作和表情是按位置编号显示的，像「动作 1」「表情 1」，而模型包
里的资源名是内部的编号拼写，对不上用户心里想的是哪一个，所以每次要重新数一遍位置才能确认录的是哪个键。
动作和表情较多时尤其麻烦：想给第 7 个动作绑键，得先确认它到底显示成「动作几」。

issue 同时给出了边界：未设置名称的行仍显示现在的编号，不强制用户都去命名；名称跟随模型保存；需要限制
长度并拒绝空白名称；改名不影响已有的快捷键绑定和播放预览。

已经存在的两件事决定了实现形状：

- **行标签是按位置编号的，而且是唯一一处编号实现。** `presentation::shortcut_rows` 把模型声明的行为
  按「先所有 motion 组、再所有 expression」摊平成 `BehaviorOrdinal`，`shortcut_behavior_name` 据此得到
  「动作 3」。编号跨 motion 组连续、每类内不重复，另算一套编号会让「动作 3」在两页指不同行为。
- **行的标签紧邻一个按下即录制的控件。** `shortcut_page` 里每一行的 chord frame 在框内任意点击都会
  开始录制，Enter 也是；三个控件的 tab 序号按行 ×3 分配，并且被测试逐个钉住。

## 决策

### 1. `model.behavior_names` 是每模型每行为一行，默认空

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

### 2. 空白名字是删除，不是名字

`set_model_behavior_name` 先删掉该行，名字 trim 后非空才写回。清空一个文本字段的含义是「回到编号名称」，
所以它是删除请求；把空字符串存下来会让页面自己判断空名字算不算名字。窗口与配置两侧各做一次 trim 与
长度限制，用的是**同一个常量**：窗口不能送出配置会拒绝的值。

### 3. 标签优先用名字，行的其余部分一字不改

`ShortcutRow` 多一个 `custom_name: Option<String>`，`name()` 先用它再回落到编号标签。`Option` 而不是
`String`：未命名是常态，`None` 让页面不需要为它编造占位文本。

改名不动 capture target、播放控件、清除控件和已录制的组合键——它只换标签文字。应用 command 因此
不需要碰 shortcut table，改名期间正在录制的组合键也不会被取消。

### 4. 改名在行内完成：名称后面一个编辑图标，只有图标开启编辑

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

### 5. 名字随模型走，删除模型时一并删除

snapshot 只带**当前模型**的已命名行，由配置的 catalog entry 过滤掉该模型已不再声明的行为——是否还存在是
模型包的属性，一个指向已不存在的名字的行在页面上无处安放。

删除导入模型时清掉它的名字，与 `last_expressions`、`gamepad_auto_switch` 的清理走**同一次 commit**：
分开写会留下一个「配置里还记着一个 store 已经没有了」的窗口。内置模型与导入模型同名时保留内置那一行，
因为那指的是另一个模型（与记忆表情的既有规则一致）。

## 后果

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

---
<a id="adr-0086"></a>
# ADR-0086: 帧 smoke 校验不再要求「采到一个半透明像素」

状态：已接受（2026-10-07）
补充：ADR-0055（导入模型的封面由渲染截取）、ADR-0030（实现决策阶梯）
需求：用户报告——两个 Mver 模型同时导入后，后导入的那个启用时频繁提示「无法启用所选模型」，连续点击多次才偶尔成功；单独导入任一个则正常。

## 背景

用户给的两个模型是同一次 BongoCatMver 转换的产物，逐字节相同（只有 `resources/cover.png` 不同），所以
「哪个模型先导入」不是变量：真正决定成败的是**切换时那一帧的合成结果**。

两个事实把问题钉在渲染侧的 smoke 校验上：

- 产品日志（`production/logs/application-2026-10-07.log`）只有一条去重后的失败记录：
  `model/activation_failed | phase=selection | reason=runtime_command_failed`。`record_once` 会去重，
  所以「频繁提示」在日志里就是这一条；`RuntimeCommandFailed` 对应的都是 runtime 侧对本次 sequence
  报出的失败，不是模型包、store 或配置问题。
- 模型本身能启用：实测该模型 `drawables=300`、`masked_drawables=106`、`textures=1`，封面（导入时走
  **同一条** `GpuModel::prepare` + 首帧校验路径，见 ADR-0055）截取成功，所以「模型损坏」这个解释不成立。

根因在 `bongocat-overlay::frame::validate_frame_smoke`。它在 drawable 上按 17×17 固定网格采样，
原先要求 5 件事同时成立，其中一条是 `translucent_pixels > 0`——**采样点里必须至少有一个 `0 < alpha < 255`
的像素**。这条判据把「有没有抗锯齿」当成了「这一帧是不是一张真的图」，而抗锯齿是美术和网格落点的属性，
不是渲染正确性的属性：一张只有单张 8192² 纹理、alpha 边缘偏硬、在画布中占比较小的模型，合成后每个采样
点都可能是透明或全不透明。此时校验失败，而这条校验的位置决定了后果：

- 模型切换时它在 `create_overlay` 里先于 `report_model_commit` 执行（`replacement.draw(true)`），
  失败即 `reject_model_commit` → runtime 发布 `GpuPreparationFailed` → app 映射为
  `RuntimeCommandFailed` → 设置页显示「无法启用所选模型」。
- 因为它随姿态、光标位置和按键图变化（同一模型在封面截取那次通过、在切换这次不通过），所以表现为
  「点很多次偶尔成功」。用户观察到的「后导入的那个更容易失败」只是当时正在切到它。

本机复现（macOS arm64，真实 Metal，把两个已安装模型放进一个模型目录用 `--switch-cycles` 切）：

| 对象 | drawables / masked / textures | 切换结果（修复前） |
| --- | --- | --- |
| 用户的模型 | 300 / 106 / 1 | 3/3 次都在**第一次**切换失败：`Metal renderer readback found no translucent alpha coverage` |
| 仓库预置模型 | 21 / 5 / 3 | 通过 |

## 决策

### 1. 删掉 `translucent_pixels > 0` 这一条，保留其余三条

`validate_frame_smoke` 现在只要求：

- 至少一个透明像素——模型之外确实还没被画过，能区分「一片不透明的垃圾表面」；
- 至少一个模型像素（不透明 + 半透明 > 0）——确实画了东西；
- 至少两种可见颜色——是一张图，不是一块纯色填充。

这三条已经完整覆盖这条校验真正要回答的问题（「这块表面是不是一张图」）。删掉的那一条既不能补充这个
答案（未初始化的垃圾表面照样能满足它，纯色填充则被第三条拦住），又会让合法模型失败。`FramePixelStatistics`
仍统计 `translucent_pixels`，它只作诊断数据，不再作判据。

### 2. 保留「校验失败即拒绝这次模型提交」的语义

没有把失败改成「当作临时不可用去重试」。真正的空表面（无模型像素）在无合成器环境里是稳定状态，
ADR-0055 残余风险 5 已经把「失败」当作正确信号；无限重试只会把它变成卡住的窗口。本次只消除误判。

## 后果

- 与该模型同类的模型（单张大纹理、硬 alpha 边缘、画布留白多）现在第一次启用即可成功，不再需要连点。
- 预置模型与既有行为不变：仍要通过同三条判据，`just` 的预览与封面截取路径不受影响。
- 自动化契约：
  - `bongocat-overlay` 新增 `frame_smoke_accepts_a_frame_without_a_semi_transparent_sample`
    （只有透明 + 不透明采样时通过，钉住本 ADR），原拒绝用例改为钉住三条判据各自的消息；
  - `frame_smoke_requires_transparent_and_antialiased_model_coverage` 保留，半透明采样仍然算作合法采样。
- `Renderer readback` 的读回与统计口径不变，双后端（Metal / D3D11）共用同一个函数，Windows 侧同步生效。

## 明确不做

- **不调网格密度或改成自适应采样**：网格是 smoke 的固定口径，加密只会把「有时采不到」变成「更晚才采不到」，
  不改变判据本身的错误。
- **不给这条校验加重试次数**：issue 要求的是找出冲突原因，加重试是把误判藏起来。
- **不改 `reject_model_commit` 的语义**，也不动 `RUNTIME_TIMEOUT`（app 侧 2 s / overlay 侧 250 ms）。
- **不在本次处理 8192² 纹理与 106 张按 drawable 尺寸分配的 mask 纹理带来的显存占用**：实测单次 overlay
  已占约 620 MiB，切模型时新旧两份并存。这是独立的资源问题，见残余风险 1。

## 残余风险与待验证项

1. **该模型的显存占用偏高且未被本次改动影响**：`drawables=300`、`masked_drawables=106`，每张 mask
   纹理按 drawable 尺寸分配；实测 `device.currentAllocatedSize()` 单次约 620 MiB，切换期间新旧两份并存。
   本机（Apple Silicon 统一内存）没有失败，低显存机器与 Windows 侧未验证。
2. **产品 UI 路径未实机点击验证**：复现走的是 `bongocat-overlay --switch-cycles` 的切换预览
   （`sync_frame` + `draw(true)`），失败信息来自与产品相同的 `validate_frame_smoke`。产品
   `ProductOverlaySession` 走 `create_overlay` + `reject_model_commit`，两处的差别是错误处理方式而不是
   判据，但「设置页点启用后不再报错」这一点仍需要一次实机确认。
3. **Windows 后端未实机验证**：`validate_frame_smoke` 是平台无关的共享实现，Windows 侧只在本仓库
   交叉编译视角下成立（见 ADR-0055 残余风险 3 的同类限制）。
4. **`switch-cycles` 预览自带的显存增长断言本身不稳定**：修复后切该模型时它仍会以
   `Metal allocation grew from 655622144 to 655884288 bytes` 报错，增量 256 KiB（0.04%），预置模型也会
   偶发。这是诊断预览的断言口径问题（Metal 分配器不是单调的），产品路径没有这条断言；本次未处理。

## 验证

已完成（2026-10-07，本机 macOS arm64，真实 Metal 设备）：

- 复现与回归证据都来自同一个一次性探针（`bongocat-overlay` 的 `examples/switch_probe`，用完已删除），
  它把两个已安装模型放进一个模型目录并调用公开的 `run_model_switch_preview`：
  - 修复前 3/3 次在第一次切换失败，错误为
    `Metal renderer readback found no translucent alpha coverage`；
  - 修复后 3/3 次**全部 6 次切换提交成功**（只有上面残余风险 4 的断言报错，而该断言在循环之后才执行，
    因此它报错本身就证明每次切换都已提交）；同一探针对仓库预置模型仍通过。
- `cargo fmt --all -- --check` 通过；`cargo clippy --locked -p bongocat-overlay --all-targets --all-features
  -- -D warnings` 通过；`cargo test -p bongocat-overlay --locked` 全绿（78 项）。
- **未运行**：Windows 实机、设置页实机点击启用、低显存机器、`--settings-window-smoke`。

---
<a id="adr-0087"></a>
# ADR-0087: 嵌套模型文件夹发现与顺序导入

状态：Accepted（issue #1105）
日期：2026-10-08
依赖：ADR-0037、ADR-0047、ADR-0055、ADR-0059

## 决策

- settings worker 调用 model store 只读扫描所选文件夹。普通包仍由 `PreparedModel` 与既有输入模式分类验证，Mver 源仍由现有内容检测识别。识别到模型根后停止向其内部寻找其他模型，避免把 Mver 的模式资源当成独立模型。
- 扫描复用包的目录深度和文件数量限制（目录也计入扫描条目预算），按相对路径排序。拒绝符号链接、非普通文件/目录、路径逃逸和包含目标 store 的来源；不写来源、不创建 staging。没有候选时保留原有普通包错误路径；直接模型根保留原有导入行为。
- 嵌套单模型自动进入原流程；多个模型用 GPUI Kit Dialog + Checkbox 选择，显示所选文件夹名与候选相对路径，默认全选，空选禁止确认。模型选择与模式选择共用弹框构建器，列表高度受窗口约束并支持滚动。
- 所选模型按显示顺序排队，每个模型重新检查来源并依次完成模式选择、导入和封面截取，再处理下一个。取消当前导入或关闭选择弹框终止剩余队列；失败报告通知并继续后续独立模型，成功结果不回滚。取消模式选择终止剩余队列。
- 队列仅是 UI 临时草稿，不持久化。所有复制、验证、转换、提交仍在 settings worker，使用已有单模型 command 与随机 UUID，不引入配置字段、依赖、平台 API 或第二种导入来源。

## 验证与门禁

自动化覆盖嵌套深度、单/多模型、Mver 不拆分、无效候选过滤、扫描预算、链接与 store 拒绝，以及选择门禁、顺序推进、取消和失败路径。双平台实机导入与 DPI/Retina 目视验收仍需各平台证据，不以 macOS 结果代替 Windows。

---
<a id="adr-0088"></a>
# ADR-0088: Concurrent motion playback

- Status: accepted
- Date: 2026-10-08
- Depends on: ADR-0061, ADR-0012 (ordered motion audio service)
- Issue: https://github.com/ayangweb/BongoCat/issues/1107

## Decision

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

## Validation And Remaining Work

Phase 3 playback and Phase 4 settings scope: verify old-document loading,
typed settings persistence, independent evaluation/fades/stops, deterministic
precedence in both start orders, standard-preset cross-group replays over held
terminal poses, repeated-trigger audio suppression, model cleanup and opt-out behavior.
Run workspace quality gates and macOS product smoke. Windows product smoke and
real-model visual comparison remain platform validation tasks until recorded;
this change does not close the broader Cubism compatibility or release gates.
