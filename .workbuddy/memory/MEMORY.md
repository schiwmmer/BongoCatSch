# MEMORY.md — Bingocat 项目长期记忆

## 执行纪律（省 token；2026-10-20 实测 413M 输入 token / 1998 步）
- 成本≈步数×上下文：先 Grep 定位行号→Read offset/limit 只读 60~120 行，禁整文件读。
- 编译输出须过滤（grep -E "^error|test result: FAILED" -A6）；cargo test 定向，全量仅提交前一次。
- 批量改动用脚本（python 锚点+assert 计数）；同类失败 2 次即回退重做；一个需求一个会话。

## 项目身份
- BongoCat fork，Rust workspace（入口 crates/bongocat-app），macOS 优先；远程 https://github.com/schiwmmer/BongoCatSch.git（HTTPS+7890），分支 feat/pomodoro。
- build script 缓存旧绝对路径致链接失败（CubismCore）→ cargo clean 重建；构建瘦身：just prune 仅清 incremental+aarch64，孤儿变体/权限流在 target/debug 清不掉，须 just clean-dev（2026-10-29 实测修正）。

## 硬规则：本机 APP 同步（每次提交+push 后必须做）
- export PATH="$HOME/bin/swift-nosandbox-shim:$PATH" → just verify-app → pkill -f bongocat-app → mv /Applications/BongoCat.app /Applications/BongoCat.app.<ts>.previous → cp -R target/package/BongoCat.app /Applications/ → xattr -dr com.apple.quarantine → open。
- 禁 rm -rf /Applications/BongoCat.app；.previous 只留 1 个（最新一份作唯一回滚副本，旧 .previous 同步后即刻移入废纸篓，不再堆积）。
- agent 环境无法带参启动 GUI app（coalition 继承宿主→panic abort）；真机冒烟只能用户 Terminal 跑。

## 工具链与环境坑
- SwiftPM 沙箱不可用→PATH shim ~/bin/swift-nosandbox-shim；just v1.59.0 在 ~/bin/just。
- just check 判成败必须 `just check > log 2>&1; echo $?`（管道接 grep 拿到管道退出码）。
- 本机 bash grep 偶发假阴性（多模式/中文）→ 确认「有无」一律用 Grep 工具。
- i18n：cultivation.attributes 是叶子字符串不能有子键；JSON 重写须 indent=2。
- rm 被 shim 成 safe-bin 对 ~/.Trash 静默不删，删个人目录用 /bin/rm。
- 本仓库无 commit-msg hook：Agent: trailer 须手写进 message 末尾（AGENT_NAME=CatPaw）；git log -1 --format='%(trailers:key=Agent,valueonly=true)' 核对。
- push 遇 `SSL_ERROR_SYSCALL`：先看 /tmp/mihomo.out.log 是否**整个机场出口节点挂**（所有 PROXY 出站 dial 同一 IP i/o timeout，google/github 全超时，非 github 单线路）。`update_mihomo_sub.py` 须用 `/opt/homebrew/bin/python3` 调（默认 python3=WorkBuddy 自带 3.13.12 无 yaml），更新订阅后 google/github 均 200 即可 push（2026-10-30 实测）。

## 会话与战力构成
- 会话落盘：Document.live 只 start/pause/resume/abandon/complete 镜像（tick 不写）；墙上时间语义，暂停冻结。
- 战力 power()=attributes().power()+equipment_bonus.power()+art_bonus.power()；PowerBreakdown 7 项之和严格等于 total；外功不计总战力。
- 状态行 at_home=exploration_ready && !working；塔/试炼 Working 拒绝改倒计时。

## 构建与磁盘
- ✅ [profile.dev] incremental=false 已落地：target 20G→7.8G，全量重建 2m28s（代价：单 crate 改一行 1.05s→2.34s）；改后须先清空 target 再重建。
- sccache 与 incremental 互斥（关掉后 sccache 才命中）。
- ✅ just target-health（只读体检）：阈值 >10240 MiB / permission-flow >6 / 非 bongocat_ 变体 >8；稳态 ≈8027 MiB；建议按根因分组：orphan/pf→clean-dev，仅总超 10GiB→prune（2026-10-29 修）。
- 已知 flaky：bongocat-packaging 5 测试并行因共享 temp_dir fixture 竞态失败（--test-threads=1 全过）。
- LAYERS_PER_REALM=10 vs LAYERS_PER_STAGE=100；clippy -D warnings 会抓测试 useless_conversion。

## 修仙体系 / UI 要点 / 数值口径
- 详见 archive/2026-10.md（修仙体系全量数值口径：等级/成长/潜质/功法/守卫/战斗/破境/探索/打工/锁妖塔/品质/灵石集市/仙草园/灵草/事件日志；UI 坑清单；i18n 仅 zh-CN+en-US）。
- 潜质草药乘数 `HERB_GRADE_MULTIPLIER_PERMILLE=[760,850,1000,1100,1250]`（仅潜质草药；garden 收获仍用 250/600/1000/1600/2500）。不变量：慧心草黄阶(+1.0) > 悟心草神阶(+0.9)。地阶锚点 0.7/1.3；仙草园 MAX_PLOT_COUNT=12，PLOT_PRICES 到 19200，UI 一行 4 格。
- 种植保底（PITY）已移除（2026-10-30）：种植完全按 `plant_permille` 真实成功率判定，无「必定成功」。原全局 `plant_fail_streak` 曾让低等级花园对高品阶/高阶级种子错误显示并触发「必定成功」。
- **仙草园种子排序（2026-10-30）**：种子面板主键=种子阶(tier)，不再按品质；同阶内按 herbs/属性、品质仅决胜。低阶先显示（面板"今天能种什么"口径）。测试用阶/品质去相关种子（1阶神阶挨2阶黄阶）验证。
- **种植速度口径（2026-10-30 更新）**：`grow_seconds(tier)`（garden.rs:82）仅依赖种子阶 1-10（8h/12h/18h/1d/36h/2d/3d/4d/6d/10d），**保留给收获价值 `harvest_points`**（升阶不拉低收获）。新增 `grow_seconds_at(rank, tier) = round(grow_seconds × 0.95^(rank−1))`，**仙草园每升 1 阶 -5% 生长时长**（rank=1 即基础，rank=15 ≈ 半时）；`CropRecord::planted/herb_planted/reveal_at_unix` 均带 rank，`ready_at_unix/revealed_at_unix` 用缩短时长。品质仍不进生长（只乘收获）。升阶还提升 `plant_permille`（成功率，公式 `10×(rank−tier)−grade_step`，≥0 时 800+4×tenths 封顶 1000，<0 时 800×0.984^|tenths| 保底 80）与 `seed_return_permille`（+1%/rank）。`growth_multiplier_permille` 是**修为**成长，与种子无关。MAX_GARDEN_RANK=15。UI `seed_facts/seed_odds_line` 显示 `grow_seconds_at`（shop 经 shop_panel 线程化 garden_rank）。
- 旧存档 herb seed tier 曾反置（悟心草写 8、慧心草写 6，`herb_tier` 现为悟 6/慧 8），致 UI 显错阶数 + `plant_one` 匹配失败静默无响应；`document.rs normalized()` 已自愈校正 `tier = herb_tier(herb)`（2026-10-30，ea1cfd5b）。
- **锁妖塔战斗随机化口径（2026-10-30，5bd5c0c3）**：对手战力 = `floor_rung_power(floor) × challenge_band_permille(rng)`，band=`800+(below(451)+below(451))/2` ∈ [800,1250]‰ 三角分布居中（均值 1025）。`warden(floor)` 经 `level_near` 二分找最近楼层等级；`TrialOutcome.warden_power` 打印 band 中点；`trial_sweepable` 对照 `floor_rung_power`（`player > rung×1.5`）。**floor 锚定（基准阶梯+band）** 而非玩家锚定：玩家锚定会让 floor 1 对强号也 ~50/50 且 sweep 自相矛盾；floor 锚定保 on-ramp/sweep/基准一致。band 低尾使「四分之一处是墙」阈值放宽 ≤30%。
- **资源系统口径（2026-10-31，未提交）**：四资源 木材/石块(常见)+铁/煤(稀有)；三采集点 森林(伐木→木, 煤@伐木5)/山(采石→石, 铁@采石3)/矿洞(双稀有)；两技能 伐木/采石（**技能是乘数非门槛**，每级 +5% 产出）；四建筑 工坊(+10%产出/级)/仓库(+25% cap/级)/铁匠铺(-10%升星成本/级)/道场(+10%采集xp/级)；`MAX_BUILDING_LEVEL=MAX_TRADE_LEVEL=10`。常量：`BASE_COMMON_YIELD=10`、`BASE_RARE_YIELD=3`、`IRON_UNLOCK_LEVEL=3`、`COAL_UNLOCK_LEVEL=5`、`BASE_SKILL_XP=100`、`BASE_RESOURCE_CAP=200`、`BUILDING_STONES_PER_RANK=40`。公式：`resource_cap(lvl)=200×(1000+250×lvl)/1000`；`skill_xp_total(lvl)=lvl×100+lvl×(lvl−1)×25`；建筑升级 `Cost{石=40×rank, 木=4×rank, 石=4×rank, 铁=2×rank(rank≥6)}`（rank=current+1）；升星 `Cost{石=sell_price/3×smithy‰/1000, 木3+石2(+铁2 if next star≥6)}`。**`Cost` 在升星与建筑升级间共享**，原子扣费（`can_spend_bag`/`spend_bag`）；`EventDetail::Star.cost` 与 `BuildingUpgrade{level,cost}` 均用 `Cost`。UI：资源面板在仙草园后（`resources_panel`），`resource_bag_text` 打印「木×3 · 石×2」；采集命令 `CollectResources{site}`、`UpgradeBuilding{building}` 入 `changes_the_document()`。**i18n 命名空间教训**：`Resource::key()` 返回 `cultivation.resource.*`（域层权威），i18n 键必须同命名空间，否则 `catalog_keys_are_referenced_by_source` 覆盖测试失败（曾误写成 `cultivation.item.resource.*`）。**UI 测试教训**：`scroll_until_visible` 只向下滚，页面新增面板推高后顶部 hint 滚出视口导致 `every_rule_is_read_beside_its_title` 失败 → 改为双向滚动（按 bounds 与 viewport 判断方向）。
- **番茄钟采集模式口径（2026-10-31，未提交）**：`SessionMode{Study,Woodcut,Mine}` **互斥**（一个 pomodoro 一件事：要么修习要么采集，非 both）；`session_mode.site()`：Study→None、Woodcut→Forest、Mine→Mountain（唯一映射）。`gather_duration_permille(realm_index)=1000+realm×250`（`GATHER_HOURS_PER_REALM_PERMILLE=250`；炼气 index0=1h、仙人 index9=3.25h）。`collect_scaled(site,wc,sc,ws,dojo,permille)`：把 `collect()` 基础 round 的 gained/xp 按 `permille/1000` 缩放（`count×permille/1000`）。`collect_session(site, now, today)`：算 realm 时长→`collect_scaled`→共享 `bank_collection`（仓库上限/经验/升级 while-loop）→只置 `last_active_day/missed_streak=0/charged_through_day` + 推 `EventDetail::SessionGather{site,gained,woodcut_xp,stonecut_xp}`，**不付修习阶梯**（无 growth/功法/灵石/试炼/roll）。`PomodoroCommand::SelectSessionMode{mode}` 持久化；完成路径 `PomodoroState::record_cultivation_session` 按 `session_mode()` 分支：Study→`on_session_completed`，gathering→`collect_session`（不再付 ladder）。`CultivationView.session_mode` 暴露给 UI。UI：resources 面板 `mode_row` 三按钮（armed=disabled），事件行 `cultivation.event.session_gather`。**坑：realm 是 `CultivationDocument.realm:u8` 显式存储字段，不随 growth 推导**（造高阶测试须直接 `realm:9`）。**坑：测试函数名含 `'s` 会致 `unknown prefix` 解析错误并级联拖垮该模块全部导出**。

## 预置模型行为绑定（2026-10-30 实测，三模型一致）
- standard/gamepad/keyboard 三份不同 moc3，绑定完全相同：`ParamEyeLOpen/R` 各 1 drawable；`ParamAngleX` 10、`ParamAngleY` 10；**`ParamAngleZ`、`ParamMouthForm/MouthOpenY/Cheek/Brow*` 全 0 绑定**。
- 结论：tilt/smile/surprised/angry 在这些模型上物理不可呈现（非代码 bug），已从三个 `cat.model3.json` 移除并删资源。换脸靠 `Param4`/`Param5`（位移/opacity 换图），不是面部形变。
- 不变量测试 `every_preset_model_declares_only_behaviours_it_can_draw`：从各模型 model3.json 推导参数逐项实测，全惰性即判死；空参数表表情（`live2d_expression0`）是中性重置，跳过。
- 帧顺序铁律：motion 声明的参数归 motion，其余归指针（`MotionClip::parameter_ids()` + `apply_model_input` 跳过集合）。用「指针是否精确为 0」做守卫等于没写。
- **motion 轴写反已修（2026-10-30）**：`nod.motion3.json` 原驱动 `ParamAngleX`（水平=摇头）、`headshake.motion3.json` 原驱动 `ParamAngleY`（垂直=点头）——UI 映射 `preset_motion` 正确但文件内容轴互换，致设置点「点头」模型摇头、点「摇头」模型点头。已交换：nod→`ParamAngleY`（25/-15，原 12/-8，+`ParamHairFront`）、headshake→`ParamAngleX`（28/-28/28，原 18/-18/18，+`ParamHairSide`），均在 [-30,30]。三模型（standard/gamepad/keyboard）motion 文件 md5 一致；behavior.rs 断言随轴改 `ParamAngleY`/期望 18.105。
- **保留集确认（三模型一致）**：7 表达式（live2d_expression0/1/2、exp_1、exp_2、wink、focus）+ 每组 4 motion（live2d_motion1/2、nod、headshake）。
- **自动眨眼守卫**：`expression_driven_parameter_ids`（apply_expression_layers 每帧重写，无表达式时清空）让自动眨眼跳过「本帧表达式驱动过的眼睛参数」（lib.rs L292）→ wink/exp_1/exp_2/focus 不被眨眼覆盖。守卫依赖每帧重写，非持久字段。
- **瞬态表达式结论**：换脸唯一机制 = `Param4`(表情:thuglife)/`Param5`(表情:升天)，共 2 张备选脸。无 smile/surprised/angry 贴图 → 复活需新美术（新贴图+新参数+新表达式），超出资源改造范畴。

## ARCHIVE 索引
- 2026-10: 修仙体系全量数值口径、UI 要点坑清单、构建与磁盘细节 → archive/2026-10.md
