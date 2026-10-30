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
- 旧存档 herb seed tier 曾反置（悟心草写 8、慧心草写 6，`herb_tier` 现为悟 6/慧 8），致 UI 显错阶数 + `plant_one` 匹配失败静默无响应；`document.rs normalized()` 已自愈校正 `tier = herb_tier(herb)`（2026-10-30，ea1cfd5b）。

## 预置模型行为绑定（2026-10-30 实测，三模型一致）
- standard/gamepad/keyboard 三份不同 moc3，绑定完全相同：`ParamEyeLOpen/R` 各 1 drawable；`ParamAngleX` 10、`ParamAngleY` 10；**`ParamAngleZ`、`ParamMouthForm/MouthOpenY/Cheek/Brow*` 全 0 绑定**。
- 结论：tilt/smile/surprised/angry 在这些模型上物理不可呈现（非代码 bug），已从三个 `cat.model3.json` 移除并删资源。换脸靠 `Param4`/`Param5`（位移/opacity 换图），不是面部形变。
- 不变量测试 `every_preset_model_declares_only_behaviours_it_can_draw`：从各模型 model3.json 推导参数逐项实测，全惰性即判死；空参数表表情（`live2d_expression0`）是中性重置，跳过。
- 帧顺序铁律：motion 声明的参数归 motion，其余归指针（`MotionClip::parameter_ids()` + `apply_model_input` 跳过集合）。用「指针是否精确为 0」做守卫等于没写。
- **motion 文件未反（2026-10-30 复核）**：三模型 `nod.motion3.json`/`headshake.motion3.json` 逐字节相同。点头/摇头「看不见」的根因不是文件反了，而是帧顺序——指针跟踪在 motion 之后把 `ParamAngleX/Y` 重置回光标位置（evaluate.rs 注释明示）。`ProductParameter::AngleX.id()=="ParamAngleX"` 与 motion 声明一致，跳过判定成立。
- **保留集确认（三模型一致）**：7 表达式（live2d_expression0/1/2、exp_1、exp_2、wink、focus）+ 每组 4 motion（live2d_motion1/2、nod、headshake）。
- **自动眨眼守卫**：`expression_driven_parameter_ids`（apply_expression_layers 每帧重写，无表达式时清空）让自动眨眼跳过「本帧表达式驱动过的眼睛参数」（lib.rs L292）→ wink/exp_1/exp_2/focus 不被眨眼覆盖。守卫依赖每帧重写，非持久字段。
- **瞬态表达式结论**：换脸唯一机制 = `Param4`(表情:thuglife)/`Param5`(表情:升天)，共 2 张备选脸。无 smile/surprised/angry 贴图 → 复活需新美术（新贴图+新参数+新表达式），超出资源改造范畴。

## ARCHIVE 索引
- 2026-10: 修仙体系全量数值口径、UI 要点坑清单、构建与磁盘细节 → archive/2026-10.md
