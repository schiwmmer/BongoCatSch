# ADR-0095: 试炼塔——层即等级，在家才可爬

状态：已接受（2026-10-21）

## 背景

修仙页已有两个拿奖励的口子：番茄钟掉落（装备 60‰、种子 50‰）与探索（每 12 小时一次，
种子/装备/功法四选一，另有遭遇战）。两者都由「时间」或「专注」看守：掉落要熬番茄钟，
探索要等半天。需求要第三个口子——**试炼闯关**：空闲时每天五次，从第 1 层起逐层往上，
难度从炼气层 1 阶 1 级开始逐级上升，获胜有几率获得种子、装备或功法等奖励。

三个约束决定了形状：

1. **不能变成第二个成长曲线。** 层数如果自带一条独立的难度公式，就要另配一张数值表、
   另一套奖励档位，等于维护两条阶梯。层必须复用现有的等级/境界/属性换算。
2. **不能把探索挤掉。** 五次/天如果每次都给一次探索的期望，探索就再没人做；而灵石只从
   专注与探索来，经济会只剩一个进水口。
3. **「空闲」是硬门**。需求原文把「外出」（含探索等行为）算作非空闲：塔要在家爬。

## 决策

### 一层 = 一个等级，守关人是那一层的裸装修士打九折

`trial_floor(f)` 映射到等级 `f`（`floor_level`），守关人取 `base_attributes(f)` 的
**属性**再乘 `TRIAL_OPPONENT_PERMILLE`（900‰），走与玩家完全相同的
`Attributes → Derived → Fighter` 换算。于是：

- 首层就是炼气 1 层 1 阶（需求原文），第 101 层起自动进筑基，因为等级跨境界时
  `base_attributes` 有整段跃升。
- 塔的难度曲线不需要新表：它**就是**等级曲线。层数成为战力的另一种读法——
  「你打到第 37 层」等于「你的战力相当于 37 级的裸装修士」，一句可验证的话。
- 面板显示的战力 `warden_power(f)` 与 `guardian` 同口径（裸装战力 × 千分比），
  显示的数和交手的数是同一条公式。

九折是**入门缓冲**而非难度斜坡：早期境界里 `攻击 10 / 生命 100` 这类常数占比重，
0.9 倍属性在低层几乎等于白送（第 1 层胜率约 85%），高层才是真门槛。测试
`a_naked_save_holds_its_own_level_and_not_much_past_it` 把这条钉住：裸装修士在自己的
层上胜率 ≥85%，在 1.25 倍层上 ≤15%。**换任何属性公式或战斗公式都必须重新过这条测试**,
它就是这一层的「守卫」。

### 每天五次，且只在「家中」开塔

`TRIAL_ATTEMPTS_PER_DAY = 5`，按本地日记数（`trial_day` + `trial_attempts_today`），
与集市货架同一天界，所以「昨天没爬」不会累积成 10 次。

「外出」在现有实现里就是探索的 12 小时冷却（文案本来就写「上次外出尚未回返」）。
`trial_blocked` 因此有三个理由，页面按其一显示：`Out`（探索未回）、`Drained`
（今日次数尽）、`Top`（已通关 1000 层）。三者都是 `Refusal`，走既有的 toast 通道。

这条门把探索与试炼做成**二选一**：走一趟换灵石与随机产出，留在家换五次可控的闯关。
不做成「都能做」是因为两者都在发放同一批物品，同时开等于把每日产出翻倍。

### 胜负用同一套战斗引擎，赢才掷奖

`trial()` 复用 `duel()`：玩家的 `fighter()`（含最强的三个外功之一）对
`warden_fighter(floor)`，战斗回放与破境、遭遇战完全同一套弹窗。赢→
`document.trial_floor = floor`（推进一层）→ 掷一次奖励；输→ 层数不动，只费一次机会。

奖励是**一次互斥掷奖**，比例照探索的静走轮（种子多、装备少、功法极少）压低：

| 结果 | 千分比 |
| --- | --- |
| 种子 | 300 |
| 装备 | 60 |
| 功法 | 10 |
| 一无所获 | 630 |

品阶不随概率变，随**层所属的境界**变（`floor_realm(floor)`）：越往上打，出的东西档次
越高，这是「爬得越高越值钱」的唯一实现方式，不需要给概率再加一条深度曲线。按五次全胜
估算期望约 1.8 件/天，低于「探索 + 掉落」同时做满时的水平，但试炼不出灵石——那仍归
专注与探索。

### 每日额度、层数与已通关层都落盘

`CultivationDocument` 新增三个 `#[serde(default)]` 字段：`trial_floor`（已通关层）、
`trial_day`（额度所属的本地日）、`trial_attempts_today`（当日已用次数），外加
`trials`（累计尝试次数，供 UI 判断「这是新战报」）。`normalized()` 把层数夹到
`TRIAL_MAX_FLOOR`、次数夹到 5，手改存档不能凭空多爬一层或多打一次。

### 常驻提示不再复述事件日志

需求同时指出「最近事件」上方与日志内容重复。原先常驻提示读的是 `last_training`、
`last_breakthrough`、`last_drop`、`last_seed_return`、`last_absence`，其中前两者与日志
逐字重复。修法是**结构性的**，不是去重比较：日志能装的两件事（修习、破境）从提示里
删除，日志装不下的三件事（缺勤扣分、败北掉装备、收获返种）留下。破境日志条目顺带补上
`stage: Option<u8>`（`#[serde(default)]`，旧事件读作 `None` 走裸行），于是提示删掉之后
「破境成功，晋入元婴」这句并没有丢。

## 后果

- 试炼塔与探索共用「外出」这一个状态，因此**同时只能做一件**。想两头都要的存档做不到，
  这是刻意的：两个口子发同一批物品。
- `FightKind::Trial` 进入战报记录；试炼的胜场同样计入战斗维度，进而抬升成长值倍率——
  塔不只是发物品，也是练战的地方。
- 第 1000 层之后 `trial_blocked` 恒为 `Top`，塔顶不可再爬；这是阶梯的长度的直接结果，
  不是额外的封顶开关。
- 事件日志新增 `EventDetail::Trial { floor, won, art, gear, seed }`；试炼因此和散步一样
  留下「第几层、得了什么」的一行，弹窗关掉也还在。
- 删掉 `cultivation.notice.art_failed / art_ranked / art_mastered` 与
  `cultivation.breakthrough.lost` 四个键（`catalog_keys_are_referenced_by_source` 会抓
  孤儿键），常驻提示组只剩三条。

## 验证

- `cargo test -p bongocat-pomodoro`：
  `a_naked_save_holds_its_own_level_and_not_much_past_it`（层=等级的调参锁）、
  `the_tower_opens_only_at_home_and_only_while_the_day_has_floors`（外出/额度/跨日）、
  `a_win_moves_the_frontier_and_a_loss_holds_it`（推进与不退层）、
  `floor_one_is_the_first_rung_of_the_ladder`、`a_cleared_floor_lands_on_every_reward_sooner_or_later`。
- `cargo test -p bongocat-ui`：`clicking_the_tower_asks_the_worker_for_a_floor`、
  `a_closed_tower_says_why_and_publishes_nothing`、
  `the_log_is_not_repeated_above_itself`（重复提示的回归守卫）、
  `the_trial_popup_names_the_floor_and_what_it_paid`。
- `cargo test -p bongocat-i18n`：新增 `cultivation.trial.*` 与 `cultivation.duel.title_trial`
  在两语言中键集一致，且每个键都有源码调用点。
