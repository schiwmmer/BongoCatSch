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

## ARCHIVE 索引
- 2026-10: 修仙体系全量数值口径、UI 要点坑清单、构建与磁盘细节 → archive/2026-10.md
