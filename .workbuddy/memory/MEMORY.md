# MEMORY.md — Bingocat 项目长期记忆

> 详细数值/UI坑/构建/部署 → `archive/2026-*.md`；本文件只留每会话必须遵守的约束与索引。

## 执行纪律（省 token；2026-10-20 实测 413M 输入 / 1998 步）
- 成本 ≈ 步数 × 上下文：先 Grep 定位行号 → Read offset/limit 只读 60~120 行，禁整文件读。
- 编译输出必须过滤（`grep -E "^error|test result: FAILED" -A6`）；定向 `cargo test -p <crate> <片段>`，全量仅提交前一次。
- 批量改动用脚本（python 锚点 + assert 计数）；同类失败 2 次即回退重做；一个需求一个会话。

## 项目身份
- BongoCat fork，Rust workspace（入口 crates/bongocat-app），macOS 优先；远程 BongoCatSch.git（HTTPS+7890），分支 feat/pomodoro。
- build script 缓存旧路径致链接失败（CubismCore）→ `cargo clean` 重建；**cross-target `bongocat-live2d-*` 必须 `/bin/rm -rf`（`cargo clean -p` 清不掉）**，否则 verify-app 链旧沙箱路径；沙箱内 codesign 报 No such file → verify-app 与 app 同步须非沙箱执行。瘦身 `just prune`，孤儿变体/权限流须 `just clean-dev`。

## 硬规则：本机 APP 同步（每次 commit + push 后必须做）
- `export PATH="$HOME/bin/swift-nosandbox-shim:$PATH"` → `just verify-app` → `pkill -f bongocat-app` → 旧 app 改名 `.previous` → `cp -R target/package/BongoCat.app /Applications/` → `xattr -dr com.apple.quarantine` → `open`。
- 禁 `rm -rf /Applications/BongoCat.app`；`.previous` 只留最新 1 份。agent 环境无法带参启动 GUI app（coalition 继承宿主→panic）；真机冒烟只能用户 Terminal 跑。

## 工具链与环境坑
- SwiftPM 沙箱不可用 → PATH shim `~/bin/swift-nosandbox-shim`；just v1.59.0 在 `~/bin/just`。
- `just check` 判成败必须 `just check > log 2>&1; echo $?`（管道接 grep 拿退出码）。
- `rm` 被 shim 成 safe-bin 对 `~/.Trash` 静默不删 → 删个人目录用 `/bin/rm`。
- 无 commit-msg hook：`Agent:` trailer 手写进 message 末尾（AGENT_NAME=CatPaw），`git log -1 --format='%(trailers:key=Agent,valueonly=true)'` 核对。
- push 遇 `SSL_ERROR_SYSCALL`：先看 `/tmp/mihomo.out.log` 是否机场出口挂；`update_mihomo_sub.py` 须用 `/opt/homebrew/bin/python3`。

## 会话与战力
- 会话落盘：`Document.live` 只 start/pause/resume/abandon/complete 镜像（tick 不写），墙上时间语义、暂停冻结。战力 `power()`=三分量之和，`PowerBreakdown` 7 项严格=total；外功不计总战力；`at_home=exploration_ready && !working`，塔/试炼 Working 拒绝改倒计时。

## 构建与磁盘（关键）
- ✅ `[profile.dev] incremental=false`：target 20G→7.8G；改后须清空 target 再重建。sccache 与 incremental 互斥。
- ✅ `just target-health`（只读）：阈值 >10240 MiB / permission-flow >6 / 非 bongocat_ 变体 >8；稳态 ≈8027 MiB。`LAYERS_PER_REALM=10` vs `LAYERS_PER_STAGE=100`；`clippy -D warnings` 抓测试 `useless_conversion`。

## 修仙体系（全量数值口径/UI坑/构建细节 → archive/2026-10.md）
- 纯值状态机（时间由 PomodoroState 注入）；**日界必须用本地日**；等级=（境界−1）×100+层×10+阶，total_levels=1000；铁律①②（level 上限 cap_level() / spell=根骨×19+仙力×26、martial=力量×26+身法×15）→ archive/2026-10.md。
- 锁妖塔分带（2026-11-08，细节见 memory/2026-11-08.md）：≤100 层三角带 1000–1200‰；≥101 层宽带逆 CDF（`deep_band_at` 按层插值）；横扫=打印 `warden_power`×1.3。

## 预置模型行为绑定（三模型一致）
- 绑定完全相同：`ParamEyeL/ROpen` 各1、`ParamAngleX/Y` 各10；`ParamAngleZ`/`ParamMouth*`/`ParamCheek`/`ParamBrow*` 全0 → tilt/smile/surprised/angry 物理不可呈现（非 bug）。帧顺序铁律：motion 声明的参数归 motion，其余归指针；用「指针精确为0」做守卫等于没写。

## 部署形态（浏览器版 2026-11-01 / 小程序版 2026-11-02，详情 → archive/2026-10.md）
- 浏览器版/小程序版部署形态（wasm 复用 bongocat-pomodoro/云库 pomodoro_saves RLS FORCE/OTP 登录/已上线域名/服务端硬规则/集合 saves/AppID touristappid）→ archive/2026-10.md。

## ARCHIVE 索引
- 2026-10 → `archive/2026-10.md`：修仙全量数值口径、UI 要点坑清单、构建与磁盘细节、浏览器版/小程序版部署形态。
