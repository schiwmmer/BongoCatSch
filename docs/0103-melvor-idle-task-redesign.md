# 修仙玩法 Melvor Idle 化重设计

状态：草案（待确认）
范围：`crates/bongocat-pomodoro/src/cultivation/` 全域 + App/web/小程序三端外壳
前提：用户已确认三个方向——① 除探索外全部改为「点击开始 → 实时进行 → 每完成一次得奖励」的秒级任务；② 战斗类（锁妖塔/试炼/突破）也实时化；③ 离线进度保留（关 App 后按耗时继续结算）。

已确认取舍（2026-11-17）：单任务互斥（接受）；闭关保留且**不占任务槽**、可与采集/打工并行；升星材料按 §5.6 加倍表（更耐玩）；按 S1→S8 切片开始编码，先出 S1 内核任务态。

---

## 1. 为什么改

现状的修仙产出几乎全部挂在「番茄钟 session」这个一次性结算上：一次 25 分钟专注结算一次修为/采集，打工是 4–24 小时任务，探索是 4/8/12 小时离线档，仙草园是种下后等数小时。这套模型的问题是产出节奏由「长计时器」决定，玩家点击之后到第一次奖励之间有数十分钟到数小时的空窗，中途没有正反馈，也没有「再完成一次」的即时循环。

Melvor Idle 的核心循环恰好相反：玩家点击一个动作（砍树/挖矿/钓鱼/烹饪），动作以固定节拍（通常 2–6 秒）自动重复，每完成一次立刻给资源 + 经验，可随时停止、切换、挂机，关闭游戏后按离线时长继续结算。它的四层逻辑是：内容层（选哪个动作）→ 数值层（回合时长/等级/成功率/产出）→ 状态层（每账号一套、互不串档）→ 触发层（时间驱动，玩家只需开始）。

本次重设计就是把修仙域搬到这四层上：玩法内容不变（还是伐木、采石、打坐、打工、种药、爬塔），但触发与结算从「长计时器一次性」改为「可重复的实时任务」，并重新平衡资源产出、奖励数量与采集等级升级。

---

## 2. 设计核候选

设计核是一句话的主张，候选必须互斥（改了不同层），推荐一个并说明理由。

- **核 A（推荐）：「单点开始的可重复实时任务」**——每个玩法都是一次点击开始、以固定节拍循环、每回合立即结算的任务；玩家只做「选哪个、什么时候停」。改动落在触发层 + 状态层：新增任务态与回合推进，旧 session 结算退居为「闭关」一种。
- **核 B：「队列式自动流水线」**——玩家可排一个任务队列（如：砍树 100 次 → 自动切采石），系统按序执行。改动落在内容层：玩法变成可编排的配方。更自动化，但玩家失去逐次正反馈，且与「每完成一次得奖励」的即时感冲突。
- **核 C：「等级驱动的动作池」**——不做固定回合，而是升级解锁新动作（更高级的采集点/工作），动作本身仍是一次性长任务。改动落在数值层：只重排等级与产出，不改触发。与用户「实时任务」的要求不符。

推荐 A：它直接对应「点击开始实时进行、每完成一次得奖励」，且能复用现有番茄钟/探索已有的「计时 → 到期结算」基础设施（`advance` 每次 tick 检查到期），改动最小、正反馈最强。B 的队列留作后续增强（先有 A 的逐回合结算，队列只是它的批处理外壳）。

---

## 3. 假设台账

| # | 假设 | 证据来源 | 置信 | 若错怎么改 |
|---|---|---|---|---|
| H1 | 玩家每天在 App 上活跃 2–4 小时，其中挂机类占一半 | 用户历史（番茄钟使用） | 中 | 若实际更短，等比压缩每回合产出 |
| H2 | 秒级回合（3–10s）不会造成性能/电量问题 | 待实测（GPUI tick 频率） | 低 | 拉长回合或改为「按经过时间批量结算」 |
| H3 | 离线结算是纯增量、可幂等 | 现有 expedition/walk 已验证同机制 | 高 | 复用现有 `advance` 到期结算路径 |
| H4 | 采集等级可从 10 级扩到 30 级，老存档 10 级封顶可平滑映射 | 待迁移实测 | 中 | 保留 10 级上限，只重排曲线 |
| H5 | 战斗实时化后每日 5 次/机会制仍是有效 bottleneck | 待实测（爬塔时长） | 中 | 改为体力制或无限爬 |
| H6 | 保留探索 4/8/12h 不动，作为唯一「长离线」层 | 用户明确 | 高 | — |
| H7 | 三端共用 wasm 内核，任务态放内核即可三端一致 | PROJECT_MAP | 高 | — |

---

## 4. 任务化改造总表

所有非探索玩法统一为「任务」模型：`{ kind, site/grade, start_unix, ticks_done, tick_seconds }`。点击开始 → `start_task`；每 tick（App 端 worker、web 端 `web_tick`）推进回合，达成整回合即结算奖励；点击停止 → `stop_task` 结算已完成的整回合，丢弃不足一回合的零头（或按零头比例给，见 §6.7）。

| 玩法 | 触发 | 回合时长 | 每回合奖励 | 限制/互斥 | 离线 |
|---|---|---|---|---|---|
| 吐纳（修习） | 点击「吐纳」 | 6s | 修为（§6.1）+ 每 10 回合 1 试炼机会 + 概率掉落 | 与采集/打工/战斗互斥（单任务） | 是 |
| 闭关（番茄钟） | 点击「闭关」 | 25min 一次 | = 250 回合吐纳 ×1.1 加成，结算一次 | **不占任务槽，可与采集/打工并行** | 是 |
| 伐木/采石 | 点击站点 | 8s | 常见资源（§6.2）+ 技能经验 | 单任务 | 是 |
| 打工 | 点击 offer | 4s | 灵石（§6.3）+ 概率积分 | 单任务；offer 每日刷新 | 是 |
| 仙草园 | 点击种植 | 生长实时（§6.5） | 收获点数/种子/药材 | 无（并行） | 是 |
| 锁妖塔 | 点击「挑战」 | 1s/回合，30 回合/场 | 每层奖励（§6.4），自动爬塔直到失败/停止 | 每日 5 次 | 否（战斗不离线） |
| 试炼 | 点击「试炼」 | 1s/回合 | 资质增益 + 钱袋 | 机会制（上限 10） | 否 |
| 突破 | 点击「突破」 | 1s/回合实时演出 | 境界提升 + guardian 战利品 | 冷却 12h | 否 |
| 探索 | 点击 4/8/12h | 不变 | 不变 | 与任务互斥 | 是 |

单任务互斥是刻意的：Melvor 一次只做一件事，这样「选哪个动作」才有重量，也避免多任务并行把经济表乘崩。

旧「采集远征」（选 4/8/12 小时 → 到点一次性结算，`StartExpedition`/`settle_expedition`）不在本表内，且与「每完成一次就获得奖励」相矛盾，已随本次改造从三端 UI 下线：App 的「时长 + 森林/山麓/矿洞」三行、web 外出页的「采集远征」行、小程序外出页的同名行与三个 8 小时钮全部删除，职能由实时伐木/采石接管（同一个森林/山，只是改成 8 秒一回合）。内核的 `start_expedition`/`apply_expedition_return` 保留：老存档里在途的远征仍要按时钟到期结算，删掉它们会让那部分已花的时间白费；`Refusal::ExpeditionOut` 与 i18n `cultivation.expedition.refuse_out` 同理保留，其余 `cultivation.expedition.*` 四个键与 `cultivation.resources.length` 已确认无引用后删除。

---

## 5. 数值骨架（公式 + 参数 + 边界）

### 5.1 修为（吐纳 / 闭关）

公式：`growth_per_tick = round(BASE_GROWTH × (1000 + GROWTH_REALM_STEP × realm_index) / 1000 × growth_permille / 1000)`

参数：`BASE_GROWTH = 2`，`GROWTH_REALM_STEP = 750`，`growth_permille` 为现有四维倍率（功法/训练/丹药，上限 4000‰，`mod.rs:1207`）。公式**先乘后取整**，所以下表按未训练的 ×1.000 基准读出，满倍率下不能把表值直接乘 4——元婴在 4000‰ 是 `round(2 × 3250 × 4000 / 1e6) = 26`/回合，而不是 7 × 4 = 28。

各境界每回合修为与满 3h/天挂机的境界耗时：

| 境界 | 每回合 | 每小时 | 境门槛 | 100 层耗时（3h/天） |
|---|---|---|---|---|
| 炼气 | 2 | 1200 | 50k | ≈14 天 |
| 筑基 | 4 | 2400 | 150k | ≈14 天 |
| 金丹 | 5 | 3000 | 400k | ≈28 天 |
| 元婴 | 7 | 4200 | 800k | ≈32 天 |
| 化神 | 8 | 4800 | 1.78M | ≈68 天 |
| 合道 | 10 | 6000 | 4.78M | ≈167 天 |
| 渡劫 | 11 | 6600 | 10.78M | ≈303 天 |
| 大乘 | 13 | 7800 | 22.78M | ≈513 天 |
| 真仙 | 14 | 8400 | 46.78M | ≈952 天 |
| 金仙 | 16 | 9600 | 94.78M | ≈1667 天 |

边界：合道以后是超长期（与 `curve.rs` 注释「last realm is a multi-year horizon」一致），靠功法倍率（最高 ×4）、闭关 ×1.1、丹药加速。若实测前期太快/太慢，调 `BASE_GROWTH`（2 → 1 或 3）即可等比缩放，曲线门槛不动。

闭关 = 一次 25 分钟实时任务，按 250 回合结算，额外 ×1.1 闭关加成；它保留番茄钟的仪式感，但产出已并入吐纳曲线，不再有独立公式。

### 5.2 采集（伐木 / 采石）

公式：`yield_per_tick = BASE_YIELD × (1000 + SKILL_YIELD_STEP × (skill_level - 1)) / 1000 × workshop_permille / 1000 × (1000 + 150 × realm_index) / 1000`

参数：`BASE_YIELD = 2`（常见）/ `1`（稀有），`SKILL_YIELD_STEP = 50`（每级 +5%），`workshop_permille = 1000 + 90 × 工坊等级`（沿用现状 `resources.rs:237`）。回合 8s → 450 回合/小时。

稀有解锁：铁 @ 采石 3 级、煤 @ 伐木 5 级（沿用现状 `IRON_UNLOCK_LEVEL`/`COAL_UNLOCK_LEVEL`）。

采集点（等级解锁，base 更高）：

| 伐木点 | base | 需要伐木 | | 采石点 | base | 需要采石 |
|---|---|---|---|---|---|---|
| 凡木林 | 2 | 1 | | 凡石山 | 2 | 1 |
| 灵木林 | 3 | 10 | | 铁矿山 | 3 | 10 |
| 神木林 | 4 | 20 | | 寒铁窟 | 4 | 20 |
| 混沌神木 | 6 | 30 | | 星辰矿脉 | 6 | 30 |

技能经验与等级曲线（重新设计）：

公式：`xp_needed(L) = XP_BASE + XP_STEP × (L - 1)`，累计 `xp_total(L) = XP_BASE × L + XP_STEP × L × (L - 1) / 2`

参数：`XP_BASE = 3000`，`XP_STEP = 1800`，每回合经验 `100 × (1000 + 80 × 道场等级) / 1000`（沿用道场），上限 30 级。

| 等级 | 累计 XP | 无道场耗时（100/回合） | 满级产量倍率 |
|---|---|---|---|
| 1→10 | 111,000 | ≈2.5 小时 | 1.45× |
| 10→20 | 402,000 | ≈8.9 小时 | 1.95× |
| 20→30 | 873,000 | ≈19.4 小时 | 2.45× |

反函数：`level_for_xp` 解二次方程取正根向下取整（与 `curve.rs` 的整数双向一致要求对齐）。

边界：30 级是长期目标（≈19.4 小时纯采集，8s/回合）；道场 10 级（180 xp/回合）把它压到 ≈10.8 小时。

### 5.3 打工

公式：`stones_per_tick = JOB_BASE_STONES × grade_multiplier / 1000`，回合 4s → 900 回合/小时。

参数：`JOB_BASE_STONES = 6`，品阶倍率沿用 `[1000, 1600, 2800, 4500, 7200]`（`jobs.rs:92`）。

| 品阶 | 每小时灵石 |
|---|---|
| 黄 | 5,400 |
| 玄 | 8,100 |
| 地 | 14,400 |
| 天 | 24,300 |
| 神 | 38,700 |

（每回合整数截断：玄 9.6→9、地 16.8→16、神 43.2→43，故略低于精确乘积。）

边界：无次数限制（实时任务，随时停）；offer 每日刷新。对比升星灵石成本（§5.6），黄阶打工 ≈ 5 分钟买一件低星装备，神阶 ≈ 40 秒。若过强，调 `JOB_BASE_STONES`。

**看板工钱与实发不一致（2026-11-17 实测发现，未修）**：看板的 `%{hours} 小时 · %{stones} 灵石` 用的是 `JobOffer::stones()` = `job_stones(tier, hours, grade)`，即改造前那套「一次结清」的 session 钱包（`jobs.rs:398`，`JOB_BASE_PURSE(6,10)` × `hours×250‰` → 按 tier 取带 → 取中值 → ×2 → ×品阶）。S4 之后 `start_work_task` 只把 `offer.grade` 与 `offer.kind` 带上任务，`offer.stones()` 在任务路径里**根本不参与支付**——它只剩两处用处：`settle_legacy_work`（旧 `document.work` 字段的遗留结算，S8 随字段一起删）和 `refresh_cost`（刷新费 = 4 小时黄阶的一半）。于是看板广告的数字与实发差 **350 倍**：看板「符箓 黄阶 … 7 小时 · 108 灵石」，实发 `6 × 1000/1000 = 6` 灵石/回合 × 900 回合/小时 = 5400/h，7 小时 = 37800。三处权威口径（本节表格、§5.6「黄阶打工 5400/h」、内核测试 `a_work_task_pays_a_full_hour_at_its_grade_rate`）都说 5400/h，看板是唯一的异类。

已修（2026-11-17）：内核新增 `job_tick_stones` / `job_hourly_stones`（时薪 = 每回合灵石 × 900，每回合先截断），`JobView` 的 `stones` 改为 `hourly_stones`，`JobOffer::hourly_stones()` 供看板与 `working` 行共用；三端文案改为「N 灵石/小时」（zh-CN `%{stones} 灵石/小时`、en-US `%{stones} stones/hour`），并去掉看板的「%{hours} 小时」chip——任务化之后时长不再是接取时的一个选项。`settle_task_tick` 也改为调用 `job_tick_stones`，让「看板印的时薪」与「实发」共用同一个函数，而不是两份抄着的数字。

**刷新费已随新工钱重定标**：`refresh_cost()` 改成 `job_hourly_stones(黄) × 2` = **10800 灵石**（4 小时黄阶工钱的一半），并且**不再随境界缩放**——工钱本身与境界无关，刷新费也就没有理由分档。旧值 40 灵石是 `job_stones(tier,4,黄)/2` 的残骸，约等于 27 秒工钱，等于免费重掷。原「per tier」的注释与测试（`refresh_cost(10) > refresh_cost(1)`）一并删除。

### 5.4 战斗（锁妖塔 / 试炼 / 突破）

- 回合 1s，一场 30 回合上限（沿用 `DUEL_ROUND_CAP`），可 2/4/8 倍速（沿用 `ReplaySpeed`）。
- 锁妖塔：点击「挑战」后从当前层连续实时爬塔，每层结算奖励，直到失败或玩家停止；每日 5 次挑战（沿用 `TRIAL_ATTEMPTS_PER_DAY`）。每层奖励沿用概率（种子 300‰ / 装备 60‰ / 功法 10‰，`trial.rs:64-66`）+ 灵石 `TRIAL_STONES(12,18) × (1 + floor/100)`。
- 试炼：机会制（吐纳每 10 回合 +1，上限 10），实时回合，强度带沿用 1200–2000‰。
- 突破：改用 `duel` 引擎实时演出（原来纯战力比较 `battle.rs:67`），胜负判定不变；失败仍 +12h 冷却、250‰ 掉装备。

边界：战斗不参与离线结算（离线爬塔会被秒挂，失去意义）；离线收益只来自吐纳/采集/打工/仙草园/探索。

### 5.5 仙草园

已是「点击种植 → 实时生长 → 点击收获」，符合任务化定义，保留结构。**2026-11-17 回退**：生长等待恢复为原 8 小时–10 天阶梯（1 阶 8h、3 阶 18h、4 阶 1 天 … 10 阶 10d），`grow_seconds` 与 `harvest_points` 重新读同一张 `GROW_SECONDS` 表。此前 S6 曾把等待缩为 `10 分钟 × 种子阶`（1 阶 10 分钟 … 10 阶 100 分钟）并把计价基准冻结在旧 8h–10d 阶梯，用户要求调回原样，故一并撤销；每次收获点数与改动前完全一致（回退前即已冻结在旧阶梯），种子返还、园圃 15 级曲线（`garden.rs`）不变。

### 5.6 经济 faucet/sink 总表

新升星材料表（每件装备 0→10 星，取代现状 `shop.rs:209` 的 `2+2s` 系）：

| 材料 | 公式 | 满星合计 |
|---|---|---|
| 木材 | `20 + 20×s` | 1100 |
| 石料 | `10 + 10×s` | 550 |
| 铁 | `s≥6` 时 `4 + 4×(s-5)` | 60 |
| 煤 | `s≥4` 时 `2 + 2×(s-3)` | 56 |

建筑主材料 ×5（现状 `5×1.45^(rank-1)` → `25×1.45^(rank-1)`），副资源比例不变；仓库容量不变。

自洽校验（以「凡木林 L1 无建筑」为基准，炼气 `realm_index = 0`，450 回合/h × 2 = 900 木/h；realm 每高一阶 ×1.15，所以 900 是**最低境界**的速率。每回合整数截断会吃掉零头：藏神境 index 3 在 L1 是 2.9 → 2（仍 900/h），L2 是 3.045 → 3（1350/h））：

- 一件装备满星木料 1100 ≈ 73 分钟纯伐木；四件 ≈ 5 小时。
- 铁 60 个：稀有 1/回合（450/h）≈ 8 分钟；煤 56 ≈ 7.5 分钟——稀有 gate 在等级（3/5 级）而非数量，符合「稀有是深度奖励」。
- 建筑主材料 per-rank `[25,36,53,76,111,160,232,337,489,708]`，10 级合计 ≈ 2227/座（`25×(1.45^10−1)/0.45 ≈ 2227`，文档早期写的 1025 是算术错误）；四座全满 ≈ 6833 主材料（2227×3 座 + 仓库自己的浅曲线 152，早期写的 8908 是漏了仓库曲线不同）≈ 7.6 小时 L1 采集（L30 凡木林约 3.8 小时）。
- 灵石：黄阶打工 5400/h，升星灵石 = 装备售价/10×铁匠铺折扣；低阶升星秒级（tier1 ≈ 4s）、顶阶 ≈ 3 分钟。

结论：材料靠采集、灵石靠打工、稀有靠等级、装备/建筑是主 sink，四方小时产出与 sink 在同一量级，无单一资源过剩。

仓库上限与速率的关系（2026-11-17 实测补记）：`BASE_RESOURCE_CAP = 400` 是**每种资源**的 ceiling，`resource_cap(仓库) = 400 + 192×等级`。所以上表的 900 木/h 在仓库 0 级时 27 分钟就把木头装满，之后每回合的采集被整份丢弃。这是设计好的闸门而不是漏洞——仓库自己的浅曲线（第 n 级 `10×1.09^(n-1)` 木 + 一半石料，10 级合计 152）就是泄压阀，10 木换 192 容量约等于 40 秒采集。但闸门不会自己打开：满仓时 `bank_collection` 直接把超额部分丢掉，而在此之前 App 只画一行「获得 」（空袋），看起来像渲染坏了。2026-11-17 补 `CollectOutcome.lost`（被丢弃的那一份）与 `cultivation.notice.collect_capped`「仓库已满，N 未能入袋」，把静默丢弃改成显式告警；web/小程序目前不画采集通知，留待三端一致化时补。

### 5.7 离线结算

所有实时任务记录 `(kind, start_unix, ticks_done, tick_seconds)`；`advance(now)` 按经过秒数补齐整回合，丢弃不足一回合的零头。离线上限 12 小时（对齐探索 12h 档）。幂等保证：`ticks_done` 持久化，重复调用不重复结算。战斗不离线。

---

## 6. 数据迁移

- 新增任务态字段默认 `None`，老存档无任务，行为不变。
- 采集等级 10 级封顶存档：把旧累计 XP 映射到新曲线同一 XP 的等级（新曲线 10 级 = 111,000 XP，旧曲线 10 级 ≈ 6,400 XP，映射后老玩家约 1–2 级，给一次「老玩家补偿」：按旧满级直接送新 10 级）。推荐补偿方案：旧 10 级 → 新 10 级（保底），旧累计 XP 超出部分按新曲线继续。
- 升星材料表改变：已有装备星级与材料数量保留，不追溯；新升星按新表。
- 修为曲线 `curve.rs` rev4 不动（growth 总量语义不变，只是每回合公式变）。

---

## 7. 范围门（DoD）

做：任务态状态机（开始/推进/停止/离线补齐）、吐纳/闭关、采集任务化 + 新等级曲线 + 新采集点、打工实时化、锁妖塔/试炼/突破实时化、仙草园生长重排、升星/建筑 sink 重排、三端 UI 接入、三端下线旧采集远征入口、老存档迁移、验证契约全绿。

不做（本次）：探索 4/8/12h 不动；功法/装备/套装/精通数值不动；竞技场/天梯不动；不引入任务队列（核 B 留作后续）；不改番茄钟计时功能本身（只把修仙产出从它上面解绑）。

---

## 8. 验证契约

| 项 | 内容 |
|---|---|
| decision | 用「可重复实时任务」取代「长计时器一次性结算」作为修仙产出主路径 |
| evidence_level | A（实机 measured，三端一致） |
| primary_bottleneck | 玩家从「点击开始」到「第一次奖励到账」的延迟（目标 ≤ 一个回合，且 UI 有可见进度） |
| released_variables | 回合时长（6/8/4/1s）、每回合奖励（修为/资源/灵石）、采集等级上限（10→30） |
| success_criteria | 实机挂 1 小时：采集升 ≥1 级、修为增长符合 §5.1 表、经济表（§5.6）四方产出误差 ≤20%；三端 wasm 快照一致；离线 12h 补齐无重复结算 |
| rollback_trigger | 离线补齐导致存档 growth/资源为负或翻倍；或三端快照不一致；或实机 1 小时产出偏离表 >50% |

采纳门槛：验证计划至少含 1 条 E 级（实机 measured）证据；主要瓶颈证据不足时 J 一票否决；存在 C 级假设时先降为 J。

---

## 9. 落地切片

- S1 内核任务态：`Task` 类型、`start_task/stop_task`、`advance` 补齐、离线幂等。纯内核 + 单测。
- S2 采集：新等级曲线（30 级）、采集点、每回合结算、迁移映射。
- S3 修习：吐纳 6s + 闭关 25min，接管原 `on_session_completed` 产出。
- S4 打工：4s 回合接管 `settle_work`。
- S5 战斗：锁妖塔/试炼/突破接 `duel` 引擎实时爬塔。
- S6 仙草园：生长重排（**其中的等待缩短已于 2026-11-17 回退，见 §5.5**）。
- S7 经济：升星/建筑 sink 重排 + 校验脚本。
- S8 三端 UI 与迁移实机验收。

每片完成标准：内核单测绿 + 三端快照一致 + 实机截图/录屏验收（用户偏好）。

采纳门槛的「经济表（§5.6）四方产出误差 ≤20%」按**速率**核对（每回合产出 × 450），不按存量——`BASE_RESOURCE_CAP` 是每资源 400 的 ceiling，存量在 27 分钟内必然撞顶，用存量衡量永远不可能通过（详见 §9.1 的 1 小时对账）。

### 9.1 落地进度（2026-11-17）

- S1–S7 内核完成：`cargo test -p bongocat-pomodoro --lib` 506 passed（2 个既有 service 时间敏感 flaky 与本设计无关，已用暂存改动复核）。
- S8 App 完成：新增 `CultStartTask/CultStopTask/CultStartSeclusion/CultStopSeclusion` 命令、`CultivationView.task/seclusion`、GPUI 任务区/闭关行/战斗进度/打工「工作中」标记、i18n；`cargo test -p bongocat-ui --lib` 370 passed、`bongocat-i18n` 20 passed、`bongocat-pomodoro-web` 7 passed。
- S8 web/小程序：文案键与「当前任务」只读行完成；web `parse_command` 已映射 `cult-*`；wasm 已重建；小程序开始/停止按钮与「提前收工」分发完成；采集点等级门禁（`TierLocked`）落地。
- S8 web 任务控制补齐（2026-11-17）：审计发现 web 端此前只有只读的「当前任务」行，没有任何开始/停止入口——「点击开始实时进行」在网页端根本玩不到。`web/cultivate.js` 洞府页首卡新增实时任务卡（吐纳/伐木/采石三个开始钮 + 停止钮 + 采集点阶选择）与闭关卡（开始/结束，不占任务槽），`handleCultivationAction` 接 `cult_start_task`/`cult_stop_task`/`cult_start_seclusion`/`cult_stop_seclusion` 四条命令；阶的默认值经 `web/app.js` 的 `app.cultTier` 暂存、随命令下发，内核仍按采集等级二次门禁。同时清掉 `web/cultivate.js` 里「快照还没暴露 task/seclusion」的过期注释，并修掉一处把 `seclusion` 拼成 `seclosure` 的按钮 `data-act`（该拼写错误会让「结束闭关」点了没反应）。
- 数值表锚点测试（2026-11-17）：设计文档不能只靠人眼复核，给 §5.1/§5.2/§5.3 三张表各钉一条内核测试，让改数值的人先撞测试再改文档——`the_s5_1_realm_ladder_pays_two_to_sixteen`（十阶 2→16 与封顶）、`the_s5_2_tier_table_pays_its_documented_base`（四阶基础产出 1000/1500/2000/3000 与 `unlock_level` 1/10/20/30）、`the_s5_2_rare_finds_open_at_the_documented_levels`（煤 5 级、铁 3 级才出）、`the_s5_2_skill_curve_is_the_documented_ladder`（1/10/20/30 级所需 3000/111000/402000/873000）、`the_s5_3_grade_table_pays_its_documented_wage`（五档时薪 5400/8100/14400/24300/38700）。顺带把既有的 `a_star_wager_is_minutes_of_work_at_most` 从区间断言收紧成定值：凡品赌注 6 灵石、超凡 276 灵石、低阶 ≈4s、顶阶 ≈184s。6 条锚点全绿。
- §5.6 自洽表算术订正（2026-11-17，核自实测公式）：四座全满主材料 8908 → 6833（2227×3 座 + 仓库自己的浅曲线 152，早期漏了仓库曲线不同），对应 L1 采集 9.9 小时 → 7.6 小时、L30 凡木林 5 小时 → 3.8 小时；煤 56 个的时长 6 分钟 → 7.5 分钟。
- 收尾验证（2026-11-17）：`cargo fmt --check` 干净；`cargo clippy --workspace --all-targets` 无警告（仅既有 `block v0.1.6` future-incompat）；`cargo test -p bongocat-pomodoro --lib` 512 passed（另 2 个既有 flaky，见下条）；`bongocat-ui` 370 passed；`bongocat-i18n` 20 passed；`bongocat-pomodoro-web --test bridge` 7 passed；`just mp-check` 全绿。
- 两个失败测试已证基线同源：`a_second_command_within_the_window_resets_the_deadline`、`the_delayed_save_writes_after_the_debounce_even_when_nothing_changed`。把全部已跟踪改动 `git stash` 回到 HEAD 后重跑，两个依旧失败（HEAD 439 filtered / 0 passed）；根因是 `apply_daily_event` 在工作日第一条 tick 播种两支股票任务，而测试断言 `tasks.len() == 1`，与本设计无关。
- wasm 已内容级核对：`just mp-kernel` 重建后 `sha256` 与既有副本一致（`55f62aea…`，949904 bytes），`web/` 与 `miniprogram/cloudfunctions/game/` 两份同哈希。
- 小程序任务卡补测：`views-smoke.js` 增加 5 条断言（3 个开始钮、空转/进行中两态、进行中禁用开始钮、闭关卡待机），`pages/cultivate/index.wxml` 字段对齐 50 个，全绿。
- web 任务卡无头补测（临时脚本，跑完即删）：19 条断言全绿——洞府首卡是实时任务、空转三个开始钮且无停止钮、四个阶都在、L1 只解锁「凡」且锁定的阶标 `Lv10`、L30 全解锁、进行中画停止钮且开始钮全禁并显示回合数、闭关中画结束钮且不再画开始钮、六条命令映射与小程序逐字一致、脏阶被夹回 `common`、选阶只改页面状态不发命令；另外八个页签全部渲染无异常。
- App 端冒烟：`target/package/BongoCat.app`（`just verify-app` 的 production bundle）跑 `--cultivation-page-smoke --run-seconds 25`，退出码 0。
- 实机截图验收（2026-11-17，production bundle `target/package/BongoCat.app`，二进制 20261117.1003）：`--settings-window-open-smoke` 起窗；设置窗每次恢复到的 frame 会漂移，改用 CUA 直接取真实 frame 并 `setWindowFrame` 钉到 300,80,1116,1021，之后按窗口点坐标点「修仙 → 资源 → 伐木」。证据图 `docs/verification/0103-app-cultivation-task-running.png`：状态行「正在进行：伐木 · 森林·伐木 凡 · 已完成 15 回合 · 每回合 8 秒」+ 停止钮，同帧 `木 57/400`、`伐木 Lv 2 · 经验 9660/14400`。与开始前（`木 12/400`、`经验 7800/14400`）对账：15 回合 × 3 木 = +45 → 57 ✓；15 × 124 经验 = +1860 → 9660 ✓。任务进行中三个开始钮整排置灰（单任务互斥生效），停止钮出现在状态行右侧。另取相隔 9 秒的两帧，回合数 6 → 7，与「每回合 8 秒」一致。点「停止」后 `docs/verification/0103-app-cultivation-task-stopped.png`：回到「当前没有进行中的任务」，开始钮恢复可用，`木 90/400`（26 回合 × 3 = +78 ✓）、`经验 11024/14400`（26 × 124 = +3224 ✓）。
- 采集远征下线（2026-11-17）：审计「除探索外全部任务化」时发现旧远征仍三端在线——它是「选时长 → 到点一次性结算」，正是本次要取代的形态，而实时伐木/采石已完全覆盖其职能。App 删 `resources.rs` 的 `expedition_status`/`gather_row` 与 `cultivation_gather_hours` 状态（`mod.rs`/`view_state.rs`/面板调用同步），`cultivation_page.rs` 的资源面板测试改成断言时长与站点入口不再绘制、且点击「伐木」发的是 `CultStartTask{Woodcut, Forest}`；web 删 `web/cultivate.js` 外出页「采集远征」行与 `cult-expedition` 映射；小程序删 `pages/cultivate/index.wxml` 同名行与三个 8 小时钮、`index.js` 的 `expedition` 分支、`utils/views.js` 的 `expedition`/`sites` 视图字段、`utils/labels.js` 的 `SITE_KEYS`。i18n 删 `cultivation.expedition.status/hours/eta/returning_in` 与 `cultivation.resources.length`，保留仍在用的 `refuse_out`（`bongocat-i18n` 的 `catalog_keys_are_referenced_by_source` 覆盖测试佐证）。
- 采集远征下线后实机复验（2026-11-17，重建的 production bundle，二进制 11:06）：同一套 CUA 流水线（`setWindowFrame` 钉到 300,80,1116,1021 → 点「修仙 → 资源」）。`docs/verification/0103-app-resources-panel-tasks-only.png`：资源面板只剩「修习/伐木/采石 + 两条采集等级 + 实时任务卡（四阶 chip + 吐纳/伐木/采石）+ 闭关 + 资源行 + 四座建筑」，「时长 4/8/12 小时」与「森林/山麓/矿洞」三行不再出现。点「伐木」后 `docs/verification/0103-app-task-running-tasks-only.png`：状态行「正在进行：伐木 · 森林·伐木 凡 · 已完成 6 回合 · 每回合 8 秒」，同帧 `伐木 Lv 2 · 经验 11768/14400`、`木 108/400`——对账 11024 + 6×124 = 11768 ✓、90 + 6×3 = 108 ✓。点「停止」后 `docs/verification/0103-app-task-stopped-tasks-only.png`：回到「当前没有进行中的任务」，`经验 12388/14400`、`木 123/400`——对账 11024 + 11×124 = 12388 ✓、90 + 11×3 = 123 ✓。
- 「每回合 3 木」的来历（2026-11-17 实机存档核对）：不是隐藏乘数，就是 §5.2 公式在当前境界下的整数截断。存档 `realm = 3`（`realm_index = 3` → ×1.45）、`道场 = 3`（采集经验 ×1.24）、`工坊 = 0`、伐木 Lv2：`2 × (1000+50×1)/1000 × 1000/1000 × (1000+150×3)/1000 = 3.045` → **3**；Lv3 起 3.19 → 仍 3。稀有煤每回合 `1 × 1.05 × 1.45 = 1.52` → 1（Lv5 起 `1.2 × 1.45 = 1.74` → 仍 1）。所以 §5.6 的「900 木/h」只在炼气成立，藏神境 L2+ 是 1350/h——这也是 §5.6 那条自洽校验必须补上境界前提的原因。
- 实机挂 1 小时产出对账（2026-11-17，production bundle，伐木·森林·凡）：起态 11:22:32 `伐木 Lv2 · 经验 12388/14400`、`木 123/400`、`煤 0/400`、`修为 471846`、`灵石 8204`；1 小时刻度（12:23:01）存档 `伐木 Lv7 · 经验 69056/74400`、`木 400/400`、`煤 290/400`、`修为 471846`、`灵石 8204`。逐项：经验 `12388 + 457×124 = 69056` **精确**（124 = `scale_xp(道场 3)`）；457 回合 × 8s = 3656s，与 1 小时窗口同量级（实际间隔 3629s，7.94s/回合）；伐木 Lv2→Lv7 升 5 级，阈值 14400/22800/33000/45000/58800/74400 与 457 回合的 xp 轨迹吻合；煤 `290` = 第 168–457 回合（伐木 ≥5 级才解锁）共 290 回合 × 1，**精确**；修为与灵石**分文未动**——采集只给资源与采集经验，符合 §5.2。证据图 `docs/verification/0103-app-one-hour-woodcut-before.png` 与 `0103-app-one-hour-woodcut.png`（后者又过 16 回合：473 回合 → 经验 71040 = 12388 + 473×124 ✓、煤 306 = 473−168+1 ✓）。
- 木这一项对不上，而且是**必然**对不上：理论 `123 + 457×3 = 1491`，实际 400。仓库 0 级的 ceiling 就是 400（`BASE_RESOURCE_CAP`，**每种资源**），第 93 回合（约 12.4 分钟）装满，之后 364 回合的木材被整份丢弃。§5.6 的 900 木/h 与 400 上限相除 = 27 分钟，所以采纳门槛里「经济表（§5.6）四方产出误差 ≤20%」用**存量**衡量永远不可能通过——它必须用**速率**（每回合产出 × 450）衡量。设计上这不是漏洞：仓库自己的浅曲线（第 n 级 `10×1.09^(n-1)` 木 + 一半石料，10 级合计 152）就是泄压阀，10 木换 192 容量 ≈ 40 秒采集；§5.6 的「73 分钟一件满星」说的是**纯采集时间**，前提是材料持续流入升星/建筑。
- 但泄压阀不会自己打开，而满仓时的提示此前是坏的：`bank_collection` 丢掉超额部分后，App 只画一行「凡木林：获得  ，采集经验 +124」（空袋），看起来像渲染坏了。本次补 `CollectOutcome.lost`（被丢弃的那一份，`state.rs`）与 `cultivation.notice.collect_capped`「%{site}：仓库已满，%{gained} 未能入袋，采集经验 +%{xp}」（zh-CN/en-US），App 端改为显式告警；内核补测 `a_tick_whose_find_will_not_fit_reports_what_it_swallowed`（满仓 → gained 0 / lost = 整份 find；未满仓 → lost 为空）。web/小程序目前不画采集通知，三端一致化留作后续。
- 「仓库已满」告警实机截图（2026-11-17，production bundle，伐木·森林·凡，伐木 Lv10、`道场 = 3`、`realm = 3`）：`docs/verification/0103-app-warehouse-full-notice.png` 画的通知行是「森林·伐木：仓库已满，木 ×4・煤 ×2 未能入袋，采集经验 +124」。逐项对账：木 `2 × (1000+50×9)/1000 × (1000+150×3)/1000 = 2 × 1.45 × 1.45 = 4.205` → 4；煤 `1 × 1.45 × 1.45 = 2.10` → 2；采集经验 `100 × (1000+3×80)/1000 = 124`。三项与内核公式分文不差，且「未能入袋」的正是被 `bank_collection` 吞掉的那一份。同帧 `docs/verification/0103-app-resources-panel.png` 显示资源行 `木 400/400 · 石 10/400 · 铁 0/400 · 煤 400/400` 与四座建筑的造价行，`木`、`煤` 两项都贴在 400 的 ceiling 上。附带一条独立佐证：同帧「下一个番茄钟 +7.2K 成长值」= `SECLUSION_TICKS(250) × 26 × 1.1 = 7150`，正是 §5.1 的 26/回合（详下）。
- 吐纳实机切片与「修为漂移」归因（2026-11-17，同一 production bundle，吐纳·每回合 6 秒，`realm = 3`，四维倍率 4000‰）：1.5 秒间隔连续采样 40 次，**每个回合边界恰好 +26**（`round(2 × 3250 × 4000 / 1e6) = 26`），回合与回合之间另有 +0~+3 的零头。零头来源已定位到既有机制而非本设计：`Cultivation::on_input`（`state.rs:1473`）把键盘/鼠标原始交互按 `INPUT_BATCH = 16` 折算成 +1 修为，App 每次平台 flush 批量上报（`bongocat-app/src/application/startup.rs` 的 input hook）。因此「每回合 26」是任务本身的支付，任何跨越用户操作窗口的粗采样都会读到偏高值：600 秒 / 100 回合的窗口读得 2894（26×100 = 2600，其余 294 点来自交互），而 1.5 秒细采样逐回合都是 26。这与 §5.6 里伐木 1 小时「修为分文未动」并不矛盾——后者是采集任务不支付修为，前者是吐纳任务支付恒定修为外加交互零头。
- 收尾三项（2026-11-17）：① **打工看板工钱改「N 灵石/小时」**——内核新增 `job_tick_stones`/`job_hourly_stones`（时薪 = 每回合灵石 × 900，每回合先截断），`JobView.stones` 改名 `hourly_stones`，`JobOffer::hourly_stones()` 供看板与 `working` 行共用，`settle_task_tick` 也改为调 `job_tick_stones`，让「看板印的时薪」与「实发」是同一个函数而不是两份抄着的数字；三端文案改 `%{stones} 灵石/小时` / `%{stones} stones/hour`，并删掉看板的 `%{hours} 小时` chip（任务化后时长不再是接取时的选项）。**刷新费重定标为 10800**（`refresh_cost() = job_hourly_stones(黄) × 2`，即 4 小时黄阶的一半），且不再随境界缩放。② **settings 资源面板的旧 `mode_row` 退役**——那三个钮（修习/伐木/采石）武装的是「下一个番茄钟」，与番茄钟窗口自己的选择器重复，又和实时任务卡并排读作两条开始路径；内核 `SessionMode` 与 legacy session 采集入口保留（番茄钟窗口与旧存档仍用）。③ **web/小程序补「最近采集」通知行**，口径与 App `cultivation.notice.collect*` 逐字一致：满仓时说「仓库已满，N 未能入袋」而不是空着袋子让人以为渲染坏了。三端用真实内核快照核对：采集通知「森林·伐木：获得 木×2，采集经验 +100」、时薪 `[5400, 8100, 8100]`、刷新费 `10800`。
- 门禁（2026-11-17 更新）：`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings` 无警告；`cargo test -p bongocat-pomodoro --lib` 516 passed（另 2 个既有 flaky，根因见上）；`bongocat-ui` 369 passed；`bongocat-i18n` 20 passed；`just mp-check` 全绿；`validate-locales.py` 827 键两语言对齐；wasm 双份同哈希 `1a37652d…`（949568 bytes）。
- 采石实机切片（2026-11-17，同一 production bundle，采石 · 山 · 凡 · 每回合 8 秒，`realm = 3`、`道场 = 3`、仓库 2 级）：620 秒窗口推进 78 回合，`石 +199`、`铁 +0`、`木/煤 +0`、`灵石 +0`、采石经验 `+9672 = 78 × 124`（`scale_xp(3) = 100 × 1240/1000 = 124`）、修为 `+136`（全部来自上一条的交互零头，采集不支付修为）。石的 199 与内核**逐步**吻合，且吻合点本身暴露了一个读法：`settle_task_tick` 先按**当前等级**算出产出，`bank_collection` 才把经验入账并升档，所以升档那一回合仍按旧等级支付。逐回合是 Lv1 的 35 回合 × 2 + Lv2 的 43 回合 × 3 = 199（Lv1 `2 × 1.0 × 1.45 = 2.9 → 2`；Lv2 `2 × 1.05 × 1.45 = 3.045 → 3`；升档点在 `3000 + 38 × 124 = 7712 < 7800` 之后那一回合）。若误按「先升档后支付」会读成 200，实测 199 判定了前者。`铁 +0` 同时验证了 `IRON_UNLOCK_LEVEL = 3` 的等级门禁——窗口内采石只到 Lv2。证据图 `docs/verification/0103-app-stonecut-start.png`（起态：采石 0 回合、石 0/784）与 `0103-app-stonecut-end.png`（停手时：采石 Lv2 · 经验 14160/14400、石 231/784）。后者是同一个模型在 90 回合上的第二次精确命中：`3000 + 90 × 124 = 14160`，石 `39 × 2 + 51 × 3 = 231`。
- 打工：**未做** 1 小时实机切片。原因不是漏做，而是机器被占用——验证途中本机一直有用户在操作（同花顺交易、窗口被反复移动/缩放、Space 切换），GUI 点击落在别的窗口上，继续点有误触风险，故停手。打工的速率由内核测试 `a_work_task_pays_a_full_hour_at_its_grade_rate` 钉死：黄/玄/地/天/神五档每小时 `5400 / 8100 / 14400 / 24300 / 38700`，正是 §5.6「黄阶打工 5400/h」的出处；实机长窗口待机器空闲时补。
- 打工看板工钱与实发不符（2026-11-17 发现，**未修**）：证据图 `docs/verification/0103-app-job-board.png` 的看板写「符箓 黄阶 元婴 … 7 小时 · 108 灵石」，而 S4 的打工任务实发 `6` 灵石/回合 × 900 回合/小时 = 5400/h，7 小时 37800——差 350 倍。根因是看板仍在用改造前的 `JobOffer::stones()`（`job_stones`，一次结清的 session 钱包），而 `start_work_task` 只带 `grade` 与 `job_kind` 上任务，`offer.stones()` 在任务路径里不参与支付（只剩 `settle_legacy_work` 与 `refresh_cost` 两处）。详见 §5.3 末节。三处权威口径（§5.3 表格、§5.6、内核测试）一致说 5400/h，看板是唯一异类，属明确的显示缺陷而非平衡分歧；但「刷新费是否随之重定标」是新的平衡决定，故先记录不改。
- 发布与实机验收（2026-11-17）：`just check` 全绿后按主题拆两次显式暂存提交——`f8be74d4` 修仙 Melvor 化（53 文件）与 `eb9c4e51` 番茄钟选模式（8 文件，并行会话 WIP），全程未用 `git add -A`；`git -c http.proxy= -c https.proxy= push` 到 master（全局 git 代理指向未监听的 7890，直连可用）。`just verify-app` 出 production bundle（2.5.0），旧包备份为 `/Applications/BongoCat.app.20261117-152705.previous`（只留最新一份）后同步，`diff -rq` 与产物无差异、`codesign -v --deep` 通过、二进制 sha256 两边一致 `f9c799ec…`。顺带修掉两个**依赖星期的测试隔离缺陷**：`load_at` 无条件 `daily_stock_tasks: true`，工作日开板播 2 条 recurring 任务，而 `the_delayed_save_writes_after_the_debounce_even_when_nothing_changed` 与 `a_second_command_within_the_window_resets_the_deadline` 把 `tasks.len()` 写死成 1/2——2026-11-17（周二）稳定红，改为与 `client.snapshot().tasks.len()` 对账 + 按任务名匹配后全绿。
- 实机验收（2026-11-17，`/Applications/BongoCat.app`，PID 75112，日志 0 ERROR/panic）：二次 `open -a` 触发 `on_reopen` → `ensure_settings_window` 打开设置窗（宠物 overlay 的保存位置在屏幕外 `x=-200`，故走设置窗这条免重启路径），逐屏点「修仙 → 资源」。证据图 `docs/verification/0103-app-installed-cultivation-overview.png`（修仙总览）、`0103-app-installed-resources-idle.png`（资源与采集空转）、`0103-app-installed-woodcut-start.png`、`0103-app-installed-woodcut-running.png`、`0103-app-installed-woodcut-stopped.png`。伐木·森林·凡，起态伐木 Lv10、道场 3、工坊 0、仓库 2、`realm_index = 3`：`木 379/784`、`煤 400/784`、经验 `130188/132000`；进行中 8 回合 `木 411`、14 回合 `木 435`、经验 `131180/132000`；停手后 `木 499/784`、`煤 460/784`、伐木升 Lv11（`经验 133908/154800`）。**全程 30 回合，三项分文不差**：木 `+120 = 30 × 4`（`2 × (1000+50×9)/1000 × (1000+150×3)/1000 = 4.205 → 4`）、煤 `+60 = 30 × 2`（`1 × 1.45 × 1.45 = 2.10 → 2`）、经验 `+3720 = 30 × 124`（`100 × (1000+3×80)/1000`）；「每回合 8 秒」与 `GATHER_TICK_SECONDS` 一致，停止后回到「当前没有进行中的任务」。
- 实机验收（2026-11-17）：收尾三项已在 production 包上逐屏核对——打工看板显示 `5400 灵石/小时`（黄）/`8100 灵石/小时`（玄）、刷新费 `10800 灵石`，资源面板只剩采集 tier chips 与三个任务钮（旧 `mode_row` 消失），采集通知在页顶显示「森林·伐木：获得 木×4 · 煤×2，采集经验 +124」，实时任务状态行「伐木 · 森林 · 伐木 凡 · 已完成 N 回合 · 每回合 8 秒」可读。证据图 `docs/verification/0103-app-jobs-hourly-wage.png`、`0103-app-collect-notice.png`、`0103-app-task-running.png`。新包（2.5.0，sha256 `6cc054a1…`）已同步 `/Applications/BongoCat.app`（旧包备份 `BongoCat.app.20261117-180933.previous`）。
- 待办（未做）：打工 1 小时实机切片仍未做（见上，机器占用）；本轮改动**尚未提交、尚未 push**，等实机验收后的发布指示。
- 实时进度条（2026-11-17，用户要求「把实时采集和战斗动画做成和 melvor 一样有进度条的」）：给 §4 的实时任务与 §5.4 的实时战斗各加一条 Melvor 式动作条，三端一致。**口径**：内核 `ActiveTask::progress_permille` / `BattleRun::progress_permille` / `BattleRun::total_progress_permille` 三个纯函数（0..=1000，两端 clamp），读的是快照里的绝对时间戳 `start_unix`（最近一个整回合边）+ 界面自己的时钟——**不能**读快照的 `ticks_done`，因为 worker 只在整回合结算时 publish，两次发布之间快照不动，条会一跳一跳的。**粒度是毫秒不是秒**：结算边是整秒，按秒读一条条只能一秒一跳，而界面是 100ms/250ms 画一次的；`start_unix` 是整秒而下一格恰好是 `start_unix + tick_seconds`，所以毫秒份数正好是这一回合已经过去的那一段。战斗条读**整场**（`rounds_done × 1000 + 本回合`）/ `rounds_total`，不读当前那一回合——一回合一秒，读单回合会画成每秒归零的秒表；又因为本回合份数在回合边上恰好读到 1000、与 `rounds_done` 前移一格接得上，整场条是**连续**的而不是每秒一跳。回放条（`CultivationDuel::progress_permille`）取 `max(时钟, 已揭示回合)`，让「跳过」不会让条落在日志后面。
- 实时进度条的实现要点（2026-11-17）：App 端 `view_state.rs` 的单条轮询循环改成**自适应间隔**——`poll_interval()` 在修仙页且有实时条时返回 100ms，否则 500ms；`poll_cultivation_progress` 把三条条的读数**量化到整百分比**（`percent()`，0..=100）再做变化检测，所以 1s 回合也不会变成每秒 100 次重绘。这里踩到一个 gpui 测试陷阱并记录：`TestDispatcher::advance_clock` 会在测试仍持有 App 借用的情况下触发到期定时器，此时定时器里再 `this.update` 会 panic `RefCell already borrowed`（gpui `async_context.rs:66`）——所以**没有**加第二个常驻 100ms 定时器，而是复用既有的那一个循环改自适应（用两个探针实测：禁用 spawn → 13 passed；间隔改 1000ms → 13 passed；100ms → panic）。web 端 `liveProgress()` 从 `cultivate.js` 导出、`app.js` 每 250ms 只改填充块宽度（`paintLiveBars`，与 `paintClock` 同一廉价重绘模式），DOM id 落在 `.progress-fill` 上；小程序端 `views.js` 的 `liveProgress()` 进 `workshop` 视图（`taskBar/battleBar/taskPercent/battlePercent`），页面 `paintLiveBars()` 走 `setData` 路径只回画两个宽度，挂进 `store.startTick` 既有的 `clock` handler（1s 一跳，`clock` 本来就是给倒计时用的），不新起 interval。
- 实时进度条门禁（2026-11-17）：`cargo fmt --all --check` 干净；`cargo clippy --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings` 无警告（仅既有 `block v0.1.6` future-incompat）；`cargo test -p bongocat-pomodoro --lib` **517 passed / 0 failed**；`cargo test -p bongocat-ui --lib` **373 passed / 0 failed**；`validate-locales.py` 827 键两语言对齐；`just mp-check` 全绿（含新增 8 条实时进度断言：空转不画、进行中画任务条、缺 `start_unix` 读 0、只有战斗时画战斗条且卡片头改说「战斗进行中」、任务条读自己那一回合、战斗条读整场、什么都不跑返回 null）；wasm 双份同哈希 `1679c4d3…`（949504 bytes，`just mp-kernel` 重建后 `web/` 与 `miniprogram/cloudfunctions/game/` 一致）。新增内核测试：`the_current_tick_fills_from_empty_to_full_over_its_own_length`（1/4 → 250、1/2 → 500、整回合 → 1000）、`a_stale_reading_past_one_tick_is_full_and_never_wraps`、`a_battle_round_fills_and_the_whole_fight_reads_over_its_length`（半回合 500 与整场 500/total、回合边前后总场读数连续）。
- 实时进度条的范围限制（2026-11-17，刻意未做）：只有**锁妖塔**有实时战斗运行行（`trial.rs`），试炼/破境没有对应的运行表面，所以战斗条只落在锁妖塔行 + web/小程序的「当前任务」行（这两处的 `taskText` 会把战斗态也写出来）；没有为试炼/破境新造表面。
- 实时进度条实机验收（2026-11-18，`/Applications/BongoCat.app`，production 包）：三条条各自取帧。① **回放弹窗条** `docs/verification/0103-app-live-progress-bar-replay-{start,mid,end}.png` —— 3% → 45% → 78%/100% 连续填充。② **实时采集任务条** `…-task-{early,mid}.png` —— 8 秒回合内 40% → 60% 连续填充（条在回合内平滑推进，不是每 8 秒跳一格）。③ **锁妖塔行内战斗条** `…-battle-run-{early,mid,late}.png` —— 一场 7 回合实战，条 11%（第 0/7 回合）→ 43%（第 2/7）→ 92%（第 6/7），与 「战斗进行中：锁妖塔之战 · 第 N/7 回合」 逐帧对上，跑完即被战斗回放弹窗接走（同一组件）。**触发途径**：该档存档战力 9,989 > 守关人 7.6K × 横扫门槛 1,300‰ = 9,880，塔只给「横扫」不给「挑战」，而 `warden_power` 在 `TRIAL_DEEP_END = 300` 层后持平，抬层无效；故按 UI 逐件点「卸下」把战力压到 8,784（装备 1,406 → 228），按钮才变「闯塔」。**验收对存档的扰动已还原**：退出 App → 用 `/tmp/bongocat-pre-battle.json` 覆盖 `production/pomodoro-tasks.json` → 重启校验 sha256 `e2940f25ff898e2dbcfc8e242181540796179f2d8f0bfe5abde8fd4599b6fea1`，存档回到 4 件装备 + 8 小时出行。**机器陷阱（记牢）**：`screencapture -l<window>` 的图与窗口列表坐标**不是** 1:1（本次实测 x 比例 1.821、y 比例 2.196，且含标题栏），按它换算点击坐标会整体偏移；改用**全屏截图**（本机 2560×1440 与 `CGDisplayBounds` 1:1）做 OCR 与点击换算。另一个吞点击的是**别的 App 的窗口**：微信登录窗（`{1139,512,280×380}`）正好压在设置窗右侧，把装备行的「卸下」整列盖住，点击全落在微信上；把设置窗整体下移 300px 让出该区域后才点得动。
- 实时进度条 web / 小程序验收（2026-11-18，与 App 同一套真实内核）：网页端没有云会话就起不来页面，所以改用**真实 wasm 内核 + 真实视图层**的等效驱动——`WebAssembly.instantiate(web/bongocat_pomodoro_web.wasm)`（sha256 `1679c4d3…`，949504 bytes） boot 空存档，按页面的 250ms 节奏 `tick(250)` + `render()` 取真实快照，再喂给**未经改动的** `web/cultivate.js`（复制成 `.mjs` 导入）与 `miniprogram/utils/views.js`（CJS 直接 require）。两条条都画出来且连续推进：web 采集条 2.2% → 13.5% → 24.7% → 36% → 47.3%（8 秒回合，步长 ≈11.2%），小程序 10% → 22% → 33% → 44% → 56%；战斗条 web 6.9% → 70%（10 回合，步长 ≈9%）、小程序 2% → 44%，且**读数单调不回退**——战斗条读的是整场 `(rounds_done×1000+本回合)/rounds_total`，不是当前那一回合的秒表。空转时两端都不画。小程序端另有常驻门禁 `just mp-check`（`views-smoke.js` 8 条实时断言：空转不画、进行中画任务条、缺 `start_unix` 读 0、只有战斗时画战斗条且卡片头改说「战斗进行中」、任务读自己那一回合 500‰、战斗读整场 333‰、什么都不跑返回 null；`wxml-check.js` 49 个字段引用对齐），本轮复跑全绿。**网页端目前没有对应的常驻门禁**，这次是临时脚本验收（跑完即删）。**细线口径三端对齐**：App 的 `progress_bar`（`cultivation_settings/mod.rs`）固定 `h(px(6.0))`，web 的 `liveBar` 写死 `height:6px`，而小程序 `pages/cultivate/index.wxml` 用的是 `class="bar thin"`，`app.wxss` 里却只有 `.bar-fill.thin` —— `.bar.thin` 是**死类**（同文件 106/319 行的既有细条写法是 `.bar` + `.bar-fill thin`），轨道高度会退回常规条，细线意图在小程序端丢失。已补 `.bar.thin { height: 8rpx; }`（750rpx 屏下 = 4pt，同为细线），注释写明类名落在轨道上而非填充块上，与 WXML 的 `class="bar thin"` 对上；`just mp-check` 复跑全绿。
- 进度数学收口（2026-11-18）：三端的一致性此前是靠**三份实现互相看着对齐**维持的——内核 `ActiveTask::progress_permille`、web `cultivate.js` 的 `liveTaskPermille/liveBattlePermille`、小程序 `views.js` 的 `liveProgress`，外加战斗的「整场」公式各自抄了第二遍。本轮把数学收回内核：新增纯函数 `tick_progress_permille(start_unix, tick_seconds, now_ms)` 与 `battle_total_progress_permille(rounds_done, rounds_total, start_unix, tick_seconds, now_ms)`（`task.rs` / `battle_run.rs`），原来的三个方法只做委托；wasm 桥补 `web_task_progress` / `web_battle_progress` 两个导出（六 → 八），web 前端删掉本地公式、`liveBars(c, ctx)` 改读 `ctx.live()`，`app.js` 的 `kernelLive()` 读 wasm。**顺带钉死一个三端不一致的老状态**：`start_unix <= 0`（缺字段的老档反序列化成 0）现在三端一律读 0，此前只有小程序有这个守卫、内核会把它算成满条。**门禁固化**：网页端此前只有临时脚本验收，现成了常驻 `just web-check`（`web/tools/selftest.mjs`，16 条断言，真 wasm 驱动真 `cultivate.js`，无浏览器/云会话），小程序端新增 `miniprogram/tools/progress-contract.js`（108 条断言，把同一批样例喂给 wasm 导出与 `views.liveProgress` 要求逐位相等，覆盖 tick ∈{1,4,6,8}s、offset ∈{0,250,500,999,1000,1001,3000,16000}ms、战斗 total∈{7,10,30}/done∈{0,3,6}、空转、缺 `start_unix`、越界夹满），`mp-check` 挂上第二条，`verify.yml` 加一步 `just web-check`。`web/package.json` 声明 `type: module`，让 Node 按 ESM 解析 `web/*.js`（CI 固定 node 20，不必依赖版本相关的 `--disable-warning` flag）。
- 离线补结算批量化（2026-11-18）：恢复后的补账原来逐 tick 克隆整任务、再逐 tick 从文档里读回来，12 小时上限（`TASK_OFFLINE_CAP_SECONDS`）对上 4 秒的打工回合就是 10,800 次克隆——一次开机付 20,000 次复制。现改为 `advance_task` 只克隆一次，交给新的 `settle_ticks(task, due, now)`:吐纳与打工按整段入账（`meditate_growth() × due`、`job_tick_stones(grade) × due`），只在阈值边界逐个执行动作（吐纳每 10 回合的试炼机会与掉落、打工每 60 回合的积分 roll），阈值顺序不变所以 rng 序列与逐 tick 完全一致；**采集刻意保留逐 tick**——产量读的是采集等级，而等级在窗口内随经验落地升档，逐 tick 是唯一能复现 §5.2 与 §9.1 的 199 石切片的算法（去克隆后这个循环本身很便宜）。防漂移：打工的积分 roll 抽成 `roll_work_point`，逐 tick 路径与批量路径共用同一份。另补 `tick_seconds.max(1)`（老档若缺该字段，逐 tick 的裸除会 panic）。等价性由内核测试 `offline_catch_up_pays_what_ticks_would` 钉死：吐纳/伐木/打工各 250 回合，批量结果与「逐 tick + 同一个 now」的整份文档 `serde_json` 全等（rng 序列、升档、事件、`last_drop` 一并覆盖）。
- 状态机过渡块拆分（2026-11-18）：`state_actions.rs` 已长到 2,831 行 / 77 个函数，是仓里第三大文件，改一行要滚过整条升阶链。现按领域拆成 7 个**扁平兄弟文件**（`state_actions_garden/battle/explore/work/trial/view.rs`，加上保留采集/出行/会话/建筑的 `state_actions.rs`），全部是 `state` 的子模块（`state.rs` 用 `#[path = ...] mod` 显式声明），所以 `pub(super)` 语义与拆分前一致、公共路径零变化；跨文件互调的 7 个 helper（`resources_view`、`region_views`、`return_trip_now`、`working`、`roll_marrow_from_combat`、`roll_marrow_from_fortune`、`trained_art_rank`）从私有提到 `pub(super)`。契约测试 `tools/tests/test_state_module_split_contract.py` 从 3 条扩到 5 条：`state.rs` ≤ 3000 行、7 个过渡文件各自 ≤ 700 行、每个都在 `state.rs` 里有对应 `#[path]` 声明、每个都仍是 `impl Cultivation` 块。最大文件从 2,831 降到 589 行。
- 门禁（2026-11-18）：`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets --all-features --exclude bongocat-app -- -D warnings` 无警告（仅既有 `block v0.1.6` future-incompat）；`bongocat-app` 两组 feature（`storage-test-injection` / `production`）clippy 均干净；`cargo test --locked --workspace` **1938 passed / 0 failed**，其中 `bongocat-pomodoro --lib` **521 passed**（517 基线 + 本轮 4 条）、`bongocat-ui` 373、`bongocat-i18n` 20、`bongocat-pomodoro-web --test bridge` 7；`cargo check --locked --workspace --release` 通过；`validate-locales.py` 827 键两语言对齐；wasm 重建后双份同哈希 `7173c6f5…`（951204 bytes）；`just web-check` 16/16、`mp-check` 全绿（`progress-contract.js` 108 条）。**本机没装 `just`**，所有 recipe 用等价底层命令手工执行。`python3 -m unittest discover -s tools/tests -p 'test_*.py'` 112 条里有 2 条失败、2 条 import 错误，均与本轮无关：失败的是 `test_release_changelog_contract` 的双语条目对齐（两份 `CHANGELOG` 本轮未改动，HEAD 上已是红的），错误是本机 Python 3.14 缺 `PyYAML`（`test_release_targets` / `test_verify_documentation_filter` 导入 `yaml` 失败）。
