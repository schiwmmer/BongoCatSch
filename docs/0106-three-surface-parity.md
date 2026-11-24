# 三端修仙信息面与数值对齐：通知、事件、回放、批量园艺、同源读数

状态：已采纳并落地（2026-11-22）
范围：`web/cultivate.js`、`web/app.js`、`web/styles.css`、`miniprogram/utils/views.js`、
`miniprogram/pages/cultivate/*`、`crates/bongocat-pomodoro` 的 `SeedView` 与 `RulesView`
上游：接 `docs/0103` §9.1 的「三端一致」与 `docs/0105` 的「页面只读内核读数」两条原则

---

## 1. 问题

App 端（`cultivation_settings/records.rs`、`mod.rs::notices`）已经把「上一次动作的结果」
和「按时排的历史」做成了两块固定界面：通知条在页面顶端，记录页是「最近 5 件事 + 查看全部」，
每条战斗事件还能展开逐回合回放。网页端与小程序端此前只有：

- 状态页的一个「下次专注 +N 成长」；
- 洞府卡里的一行「最近采集」；
- 塔页的一张「最近战报」名单，点不开。

于是同一个存档在三端看到的不是同一件事：

| 信息 | App | 网页 | 小程序 |
|---|---|---|---|
| 今日境界 / 战力增量 | `today_level_delta` / `today_power_delta` 进头部 | 无 | 无 |
| 通知：缺勤、掉落、返种、升星、满仓、建筑 | `notices()` 六条 | 只有采集 | 只有采集 |
| 最近事件（带时间戳） | `events` 全量 + 5 条折叠 | 无 | 无 |
| 战斗逐回合回放 | 动画回放弹窗 | 无 | 无 |
| 横扫结果 | `last_trial_sweep` 一行 | 无 | 无 |
| 仙草园批量收 / 种、成活率 | 有 | 无 | 无 |
| 采集深度倍率 | 内核 `GatherTier` | 页面写死 ×1.0 / ×1.5 / ×2.0 / ×3.0 | 同网页 |
| 探索时长与倍率 | `ExplorationHours::ALL` | 页面写死 4 / 8 / 12 小时 | 同网页 |
| 闭关总回合 / 时长 | `SECLUSION_TICKS` × 每回合秒数 | 页面写死 250 回合 / 25 分钟 | 同网页 |
| 区域数 / 每区层数 | `Region::ALL` / `LEVELS_PER_REGION` | 写死「五城十层」 | 同网页 |

这些都不是「新玩法」，是同一份快照已经有、只有 App 在读的字段。差异会让玩家以为
网页/小程序丢了记录或丢了收成，也会让「今天到底涨了多少」变成要自己心算的事。

## 2. 决策

### 2.1 事实仍然只有一份

- **今日增量、通知、事件、横扫结果**：网页与小程序只读 `CultivationView` 已有字段，
  不在页面里做第二次聚合，也不补一套「本地历史」。
- **成活率**：`SeedView` 新增 `plant_permille`，在快照构建时用内核自己的
  `plant_permille(garden_rank, tier, grade)` 算好。网页与小程序不再各自抄一遍公式
  （这正是 `docs/0105` 对采集深度做过的事）。动作返回值里的 `SeedView` 同样带真值。
- **事件文案**：三端共用同一组模板句子（`cultivation.event.*` 的中文口径）。网页与小程序
  各有一份 `eventLine` 翻译函数，逐条对照 `records.rs::event_line`；不认识的变体返回空串，
  宁可显示「没有可读的记述」，也不把结构化字段摊给用户。

### 2.2 回放：同一份回合表，文字版

App 的回放是带动画的；网页/小程序没有可用的动画层，也不该为它造第二套战斗表现。
两端展开的是 `DuelReport.rounds` 的文字表：回合号、出手方、功法、伤害/闪避、剩余气血，
字段与 App 的 `DuelRound` 一一对应。**同数据、不同表现**，不是少了功能。

### 2.3 放置位置

- **网页**：通知条贴在 `stage` 顶部，切页签不消失（它讲的是上一次动作，与当前页签无关）；
  新增「记录」页签（行踪 + 最近 5 件事 + 查看全部 + 回放）。
- **小程序**：通知条在顶栏之下、分段条之上；新增「记录」分区，行踪卡带「提前返回」。
- **仙草园**：两端都加「一键收获 / 一键种植」，动作映射到内核既有的
  `harvest_all_ready` / `plant_all_by_grade`；种子行读 `plant_permille`。
- **挖掉**：App 是两级确认；网页用同一句确认文案的原生确认框，小程序用 `wx.showModal`
  （文案已存在），语义一致。

### 2.4 数值只从快照来

凡是玩家会读到的设计数字，都不在页面里写第二份：

- `CultivationView.rules`（`RulesView`）带 `max_star`、`seclusion_ticks`、
  `seclusion_seconds`、`region_count`、`region_levels`、`explore_hours`、
  `gather_depths`，全部由内核常量与枚举表算出。
- `TradeView` 带 `depth` 与 `depth_base_permille`，采集行的当前深度与倍率直接读它；
  页面只保留「Ⅰ/Ⅱ/Ⅲ/Ⅳ」这套显示用罗马数字，不再维护 `DEPTH_LADDER` 或
  `gatherDepth(level)` 的第二套推导。App 的深度标签同样由 `GatherTier::base_permille`
  现算，i18n 里只留罗马数字，不再存一份 `×1.0 / ×1.5 / ×2.0 / ×3.0`。
- 区域卡、当前层、探索按钮、闭关回合与分钟数、深度门等级（20/50/80）、升星上限
  都由 `rules` 渲染。内核改表，三端同时改口，不用逐页找常量。
- 批量操作按钮的「×5」是三端共用的交互约定，不是内核规则，保持原样。

## 3. 结果

三端现在都能看到：今日增量、六类通知、最近事件（时间戳 + 全部展开 + 战斗回放）、
上次闯塔结果与横扫结果、种子成活率、一键收种；采集深度、探索时长、闭关时长、
区域规模与升星上限也不再各写一套数字，而是同读 `rules` / `TradeView`。

保留的已知差异：

- 回放是文字回合表，不是 App 的动画条；不新增战斗动画。
- 网页的「挖掉」确认是浏览器原生对话框；小程序是原生 modal。App 是页内两级按钮。
- App 的记录区在一次长页里，网页/小程序是独立页签/分区；信息集合一致，导航不同。

## 4. 验证

```text
node web/tools/selftest.mjs           # 23/23：记录页、事件模板、通知条、园艺、rules 数值
node miniprogram/tools/views-smoke.js # 记录页、今日增量、事件行、rules 数值
node miniprogram/tools/wxml-check.js  # 模板字段存在性
node miniprogram/tools/selftest.js    # 云端命令通道 15/15
cargo test -p bongocat-pomodoro --lib # 524 passed：SeedView / RulesView / 深度倍率
cargo test -p bongocat-pomodoro-web   # 7 passed：wasm 快照含 rules
cargo test -p bongocat-ui depth_labels # App 深度倍率由 base_permille 现算
```
