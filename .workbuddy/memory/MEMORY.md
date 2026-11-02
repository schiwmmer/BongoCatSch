# MEMORY.md — Bingocat 项目长期记忆

> 详细数值口径/UI 坑清单/构建细节 → `archive/2026-*.md`。本文件只留每会话必须遵守的约束与索引，条目超 30 天蒸馏入 archive。

## 执行纪律（省 token；2026-10-20 实测 413M 输入 / 1998 步）
- 成本 ≈ 步数 × 上下文：先 Grep 定位行号 → Read offset/limit 只读 60~120 行，禁整文件读。
- 编译输出必须过滤（`grep -E "^error|test result: FAILED" -A6`）；定向 `cargo test -p <crate> <片段>`，全量仅提交前一次。
- 批量改动用脚本（python 锚点 + assert 计数）；同类失败 2 次即回退重做；一个需求一个会话。

## 项目身份
- BongoCat fork，Rust workspace（入口 crates/bongocat-app），macOS 优先；远程 `https://github.com/schiwmmer/BongoCatSch.git`（HTTPS + 7890），分支 feat/pomodoro。
- build script 缓存旧绝对路径致链接失败（CubismCore）→ `cargo clean` 重建；**cross-target 目录 `target/aarch64-apple-darwin/release/build/bongocat-live2d-*` 用 `cargo clean -p` 清不掉（Removed 0 files），必须 `/bin/rm -rf` 该目录**，否则 verify-app 链接到旧沙箱路径 `/private/tmp/wt-bingocat/...`；沙箱内 codesign 报 No such file（文件其实在）→ verify-app 与 app 同步须非沙箱执行。瘦身 `just prune`（仅 incremental+aarch64），孤儿变体/权限流须 `just clean-dev`。

## 硬规则：本机 APP 同步（每次 commit + push 后必须做）
- `export PATH="$HOME/bin/swift-nosandbox-shim:$PATH"` → `just verify-app` → `pkill -f bongocat-app` → 旧 app 改名 `.previous` → `cp -R target/package/BongoCat.app /Applications/` → `xattr -dr com.apple.quarantine` → `open`。
- 禁 `rm -rf /Applications/BongoCat.app`；`.previous` 只留最新 1 份（旧的入废纸篓）。
- agent 环境无法带参启动 GUI app（coalition 继承宿主→panic）；真机冒烟只能用户 Terminal 跑。

## 工具链与环境坑
- SwiftPM 沙箱不可用 → PATH shim `~/bin/swift-nosandbox-shim`；just v1.59.0 在 `~/bin/just`。
- `just check` 判成败必须 `just check > log 2>&1; echo $?`（管道接 grep 拿到管道退出码）。
- 本机 bash grep 偶发假阴性（多模式/中文）→ 确认「有无」一律用 Grep 工具；grep 多模式必 `-E`。
- i18n：`cultivation.attributes` 是叶子字符串不能有子键；JSON 重写须 `indent=2`。
- `rm` 被 shim 成 safe-bin 对 `~/.Trash` 静默不删 → 删个人目录用 `/bin/rm`。
- 无 commit-msg hook：`Agent:` trailer 手写进 message 末尾（AGENT_NAME=CatPaw），`git log -1 --format='%(trailers:key=Agent,valueonly=true)'` 核对。
- push 遇 `SSL_ERROR_SYSCALL`：先看 `/tmp/mihomo.out.log` 是否整个机场出口挂（全部 PROXY 出站 dial 同一 IP i/o timeout）；`update_mihomo_sub.py` 须用 `/opt/homebrew/bin/python3`（默认 python3 无 yaml）。

## 会话与战力
- 会话落盘：`Document.live` 只 start/pause/resume/abandon/complete 镜像（tick 不写）；墙上时间语义，暂停冻结。
- 战力 `power()` = `attributes().power()` + `equipment_bonus.power()` + `art_bonus.power()`；`PowerBreakdown` 7 项之和严格等于 total；外功不计总战力。
- 状态行 `at_home = exploration_ready && !working`；塔/试炼 Working 拒绝改倒计时。

## 构建与磁盘
- ✅ `[profile.dev] incremental=false`：target 20G→7.8G，全量重建 2m28s（单 crate 改一行 1.05s→2.34s）；改后须清空 target 再重建。sccache 与 incremental 互斥。
- ✅ `just target-health`（只读）：阈值 >10240 MiB / permission-flow >6 / 非 bongocat_ 变体 >8；稳态 ≈8027 MiB。
- 已知 flaky：`bongocat-packaging` 5 测试并行因共享 temp_dir 竞态失败（`--test-threads=1` 全过）。
- `LAYERS_PER_REALM=10` vs `LAYERS_PER_STAGE=100`；`clippy -D warnings` 会抓测试 `useless_conversion`。

## 修仙体系 · 关键不变量（全量口径 → archive/2026-10.md）
- 潜能/品质：`HERB_GRADE_MULTIPLIER_PERMILLE=[760,850,1000,1100,1250]`（仅潜质草药）；不变量 慧心草黄阶(+1.0) > 悟心草神阶(+0.9)。仙草园 `MAX_PLOT_COUNT=12`、`MAX_GARDEN_RANK=15`；种植无 PITY，纯 `plant_permille`。
- 生长：`grow_seconds(tier)`（1-10 阶 → 8h/12h/18h/1d/36h/2d/3d/4d/6d/10d）只驱动收获价值；`grow_seconds_at(rank,tier)=round(grow×0.95^(rank-1))` 驱动实际倒计时。品质只乘收获，不进生长。
- 旧存档 herb tier 反置由 `document.rs normalized()` 自愈（`tier = herb_tier(herb)`；悟 6/慧 8）。
- 锁妖塔：对手战力 = `floor_rung_power(floor) × challenge_band_permille`（band∈[800,1250]‰ 三角分布）；**floor 锚定**而非玩家锚定（玩家锚定会让 sweep 自相矛盾）。
- 外出：`Cultivation::return_early` 覆盖走路+打工；返程 `(spent/3).max(6)`；`JobRun.early` 早退不动 `jobs_roll`；判「已回家」必须用 work 前后状态（早退 payout 也是 None）。
- 集市（20000 天实测）：行数 5–10 均 7.5；装备 36%/功法 22%/种子行 41.7%；种子池 245 单位 = 4 属性×10 阶×6 + 悟心草 3 + 慧心草 2；阶以境界自身阶为中心 ±3 窗口，权重 8/4/2/1 裁剪不回绕。灵草：悟心 1.22% 种子行、慧心 0.82%；番茄钟另各 6%/3%。
- 资源与采集（v2.2 落地，449a7c06）：四资源 木/石(常见)+铁/煤(稀有)；三采集点 森林(煤@伐木5)/山(铁@采石3)/矿洞(双稀有)；`BASE_COMMON_YIELD=4`、`BASE_RARE_YIELD=2`、`BASE_SKILL_XP=100`、`BASE_RESOURCE_CAP=400`、`MAX_BUILDING_LEVEL=10`、**`MAX_WAREHOUSE_LEVEL=50`**、`MAX_TRADE_LEVEL=10`；技能每级 +5%、建筑 工坊+90‰/仓+192 绝对值/铁匠-6%(0.94^lv)/道场+80‰。**关键不变量**：①番茄钟拆双量 `rounds_per_hour_permille`(+150/境界，外出用) vs `gather_duration_permille`(+250，番茄钟用) ②`normalized()` 必须用 `MAX_WAREHOUSE_LEVEL` clamp 仓库（曾误 clamp 回 10）③采集链**只除一次**（2026-10-31 修 6e794a1f）：`collect_scaled` 必须 `原始乘积×时长/1e9`，**禁止把 `collect` 已截断的单轮值再乘时长**——否则满配仙人 12h 只到 310（设计 321）、半数工坊等级不加产（169,169,197,197…）；per-round 整数仍可能持平，判「工坊有增益」须看 session 级而非单轮 ④探索 0.6:1：`ENCOUNTER≤112`（`roll_walk` 断言 `(SEED+GEAR+ART+ENCOUNTER)×3.2≤1000` 卡 140）、`WANDERER` 48-68、`GUARDIAN` 70-120（断言链随动）⑤技能经验总量 6,400（`l×200+l(l+1)×40`；45,121 是错值）。
- **`Cost` 跨域共享**（升星/建筑升级），`can_spend_bag`/`spend_bag` 原子扣费；`Resource::key()` 返 `cultivation.resource.*`，i18n 键必须同命名空间（`catalog_keys_are_referenced_by_source` 覆盖测试）。
- UI 坑：`scroll_until_visible` 只向下滚 → 新增面板推高后须双向滚动；`text()` 返 `&'static str`（静态翻译缓存），hover 闭包须传 `&'static str`（否则 E0521）。
- UI 建筑：进度显示 `Lv x/max`（仓库显示 /50）+ `×5` 连升按钮（`PomodoroCommand::UpgradeBuildingMany`，state 早停在顶/缺钱处，买 0 拒、买部分成功）。
- 造高阶测试须直接写 `realm: 9`（`realm` 是显式字段，不随 growth 推导）；测试函数名含 `'s` 会致 `unknown prefix` 并级联拖垮该模块导出。
- 打包坑：`target/debug/bongocat-packaging` 若由沙箱 temp 挂载编译，`env!("CARGO_MANIFEST_DIR")` 烤入错误根路径 → verify-app 报「not workspace root」，touch 该 crate 源文件强制重建。

## 预置模型行为绑定（三模型一致）
- 绑定完全相同：`ParamEyeL/ROpen` 各 1、`ParamAngleX/Y` 各 10；**`ParamAngleZ`、`ParamMouth*`、`ParamCheek`、`ParamBrow*` 全 0 绑定** → tilt/smile/surprised/angry 物理不可呈现（非 bug），已移除。换脸靠 `Param4`/`Param5`（2 张备选脸），复活需新美术。
- 守卫：`every_preset_model_declares_only_behaviours_it_can_draw`（全惰性即判死；空参数表表情是中性重置，跳过）。
- 帧顺序铁律：motion 声明的参数归 motion，其余归指针（`apply_model_input` 跳过集合）；用「指针精确为 0」做守卫等于没写。
- motion 轴已修：nod→`ParamAngleY`、headshake→`ParamAngleX`（三模型 md5 一致；behavior.rs 期望 18.105）。
- 自动眨眼守卫：`expression_driven_parameter_ids` 每帧重写，眨眼跳过本帧表达式驱动的眼睛参数（wink/exp_1/exp_2/focus 不被覆盖）。

## 浏览器版（2026-11-01）
- `crates/bongocat-pomodoro-web`（wasm 桥，无 wasm-bindgen）+ `web/`（原生 JS 页面）；`just web` 构建、`just web-serve` 预览。修仙逻辑零重写，直接复用 `bongocat-pomodoro`；`record_input` 不进站。细节见 daily log 2026-11-01。

## ARCHIVE 索引
- 2026-10 → `archive/2026-10.md`：修仙全量数值口径、UI 要点坑清单、构建与磁盘细节。
