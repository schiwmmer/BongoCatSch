# 产品拆分与命名

> 状态：已定（2026-11-05）
> 一句话：**同一份数值内核，两个产品** —— **BongoCat Desktop** 本地养猫（有模型、输入即修炼），**BongoCat Cloud** 云端修仙（有账号、会话即修炼）。

## 0. 拆分结论

| 项 | 结论 |
| --- | --- |
| 仓库 | **不拆**。单 cargo workspace 双产物，共享内核只实现一次 |
| 产品数 | **2 个**（Cloud 是「一个产品两个宿主」，不是一个产品两个） |
| 共享内核 | `crates/bongocat-pomodoro`（番茄钟状态机 + `cultivation/` 全部修仙数值） |
| 拆分依据 | 不是代码不同，而是**成长值来源**和**数据落地**不同 |

## 1. 产品定义

| 项 | `desktop` | `cloud` |
| --- | --- | --- |
| 产品名 | **BongoCat Desktop** | **BongoCat Cloud** |
| 中文名 | BongoCat 桌面版 | BongoCat 云端版 |
| 载体 | macOS 12+ 原生 App（`com.ayangweb.bongo-cat`） | 浏览器 `web/` + 微信小程序 `miniprogram/` |
| 入口 | `crates/bongocat-app` | `web/index.html`、小程序 `pages/` |
| 数据落地 | 本地 `pomodoro-tasks.json` | web：PG `pomodoro_saves`；小程序：云开发 `saves` 集合 |
| 登录 | **无** | 邮箱 OTP（web）/ 微信身份（小程序） |
| 存档归属 | 单机一份，无身份 | 一账号一份，RLS / 集合权限隔离 |
| 内核运行形态 | 原生线程 + 文件 | wasm：web 前端直调，小程序在**云函数**里调同一份 `kernel.wasm` |

Cloud 的两个宿主共享同一份 wasm 内核，客户端零游戏逻辑：web 每次 tick 调 `web_tick`，小程序只发 `{op, ...}` 拿快照。

## 2. 功能边界

| 能力 | `desktop` | `cloud` | 代码依据 |
| --- | :---: | :---: | --- |
| 番茄钟基础（计时/任务/日历） | ✅ | ✅ | `bongocat-pomodoro` |
| 修仙（境界/功法/装备/灵田/集市/锁妖塔/试炼/探索/打工/日常） | ✅ | ✅ | `pomodoro/src/cultivation/` |
| 登录注册 | ❌ | ✅ | `web/cloud.js`、`miniprogram/cloudfunctions/game/` |
| Live2D 模型 + 渲染 | ✅ | ❌ | `bongocat-live2d*`、`bongocat-overlay`、`bongocat-render` |
| 鼠标 / 键盘输入 → 成长值 | ✅ | ❌ | `PomodoroCommand::RecordInput`（web 桥明确不解析） |
| 每日股票复盘任务自动建 | ✅ | ❌ | `with_daily_stock_tasks(false)`（`pomodoro-web/src/lib.rs:129`） |
| 音效 | ✅ | ❌ | `bongocat-audio`、`platform/pomodoro_sound.rs` |
| 菜单栏 / 全局快捷键 / 开机启动 | ✅ | ❌ | `bongocat-platform` |
| 自动更新 + 签名 | ✅ | ❌ | `bongocat-update` |
| 安装包 / dmg | ✅ | ❌ | `bongocat-packaging` |
| 云端多设备同步 | ❌ | ✅ | — |

## 3. 唯一实质差异：成长值口径

```
desktop: growth = Σ(完成会话) + Σ(键盘鼠标输入折算)
cloud  : growth = Σ(完成会话)
```

两者喂的是**同一套修仙曲线**（`level_for_growth`、`threshold_for`），所以境界不能跨端对比。

**铁律：不做桌面档 → 云端档的搬迁。** 桌面档里的输入加成无法在云端复现，强行导入等于凭空产生成长值。若将来要做互通，只搬「会话事实」（专注域事务），由云端按会话重算 growth，绝不搬 growth 总数。

## 4. 代码命名约定

### 4.1 分层与归属（现有 crate 一律**不改名**）

| 层 | crate | 约定 |
| --- | --- | --- |
| 共享内核 | `bongocat-pomodoro`、`bongocat-i18n` | 不得依赖任何平台 / UI / 网络 crate |
| 云桥 | `bongocat-pomodoro-web` | 只依赖共享内核 + serde |
| 桌面专属 | `bongocat-app`、`-overlay`、`-live2d`、`-live2d-playback`、`-live2d-render`、`-input`、`-platform`、`-ui`、`-ui-protocol`、`-render`、`-runtime`、`-update`、`-config`、`-storage`、`-log`、`-model`、`-model-store`、`-audio`、`-packaging` | 云侧禁止引用 |

### 4.2 新增 crate 的前缀

- 桌面专属：`bongocat-desktop-<area>`
- 云端专属：`bongocat-cloud-<area>`
- 两端共享：`bongocat-<area>`（**不加后缀**）

### 4.3 为什么现有 crate 不改名

`bongocat-app` 有 109 处引用、33 个文件，且被执行名、更新清单、CI workflow、打包契约（`test_update_release_contract.py` 等）钉死为**发布身份**。改名是发布事故风险换零语义收益。同理，其余 `bongocat-*` 保持原样，归属靠 §4.1 表格 + 依赖守护测试约束，不靠改名表达。

唯一低成本的候选改名：`bongocat-pomodoro-web` → `bongocat-cloud-wasm`（15 处引用 / 9 文件，含 `justfile`、`web/app.js`、`miniprogram/cloudfunctions/game/core.js` 注释）。**非必需**，可随时单独一次 commit 完成。

## 5. 构建与产物命名

| 项 | `desktop` | `cloud` |
| --- | --- | --- |
| 配方 | `just dev` / `just test` / `just verify-app` | `just web` / `just web-serve` / `just mp-kernel` / `just mp-check` |
| 中间产物 | — | `target-web/wasm32-unknown-unknown/release/bongocat_pomodoro_web.wasm` |
| 分发 | `target/package/BongoCat.app` → dmg | 静态站点 `web/`；小程序包 `miniprogram/` |
| 环境隔离 | Development / Production 编译期特性 | 无环境概念，只分部署环境 |

## 6. 落地顺序

| 优先级 | 动作 | 成本 |
| --- | --- | --- |
| P1 | 本文件为边界事实源；`AGENTS.md` 增一节指向它 | 零 |
| P2 | 加契约测试：断言 cloud 侧（`bongocat-pomodoro-web`、`web/`、`miniprogram/`）不引用 §4.1 桌面专属 crate | 小，仿 `tools/tests/test_supported_platform_cfg.py` |
| P3 | 可选改名 `bongocat-pomodoro-web` → `bongocat-cloud-wasm` | 小，独立 commit |

## 7. 明确不做

- 不物理拆成两个仓库：共享内核会立刻分叉，修仙数值双处维护必然漂移。
- 不复制修仙代码到 JS：web / 小程序跑的就是 Rust 内核编译出的 wasm。
- 不给 desktop 加登录：桌面产品定位是「装在本机的猫」，账号体系属于 Cloud。
- 不让 cloud 引入 Live2D / 输入统计 / 更新器：这些能力在浏览器和小程序里都没有物理承载。
