# ADR-0092: 修仙阶梯取代交互次数等级，破境必须靠装备

状态：已接受（2026-10-19）

## 背景

原先的等级来自 `bongocat-platform` 的交互计数：按一次键或鼠标键算一次交互，累计次数
按 `LEVEL_ANCHORS` 曲线换算出一千个等级，HUD 与额头徽章各自读这条曲线并查 catalog。
需求把整套体系换成修仙：十个大境界各一百层，属性、装备、仙草、掉落、破境与未修炼
扣分都要落在同一个成长值上。

四个约束决定了实现形状：

1. **领域不能看时钟。** 现有约定是所有时间从外部注入（`now_unix`、`CalendarDay`、
   UTC offset），`bongocat-pomodoro` 的测试因此不需要等待也不存在跨日抖动。
2. **只能有一份文档。** 番茄钟的状态、任务、活动记录与修仙进度同存于一份 JSON、
   同一次原子写（ADR-0090），不能为修仙另开文件。
3. **破境必须打得过有装备的修士。** 需求原文要求守卫「比正常 100 级裸装强」，
   即破境不能靠挂机攒成长值自动通过。
4. **扣分不能撤销已得的境界。** 惩罚要能累积翻倍，但不能把已经破境的人打回上一层。

## 决策

### 阶梯是一个纯值状态机，挂在番茄钟领域里

`bongocat-pomodoro/src/cultivation/` 新增子模块：`mod.rs`（境界、属性、战力、
SplitMix64）、`curve.rs`（成长值阶梯）、`items.rs`（装备）、`garden.rs`（仙草园）、
`battle.rs`（守卫）、`document.rs`（持久化形状）、`state.rs`（状态机与视图）。
`Cultivation` 不持有文件句柄、不读时钟、不碰线程；所有时间由 `PomodoroState` 传入。
`CultivationDocument` 作为 `Document` 的一个 `#[serde(default)]` 字段落在同一份文件里，
因此旧文档读进来就是「一个刚入门的新修士」。

### 成长值是唯一货币，境界是硬上限

当天第一个完成的番茄钟固定 +500 点（`SESSION_BASE_GROWTH`）；同一天第 N 个（N≥2）按
`500 × 1.1^(N-1) × 境界序号`（`session_growth`），键鼠每次输入各 +1（合批下发，
避免逐键落盘）。`level_for_growth` 把成长值换回等级，`STAGE_FLOORS` 定义每境第一层的
门槛（炼气 0 → 仙人 600,000，Lv.1000 需 1,095,000）。等级一律 `min(realm.cap_level())`：
**成长值攒得再多也停在 100 层**，必须破境才看得见后面的层。这一条把「境界」做成
真正的门槛而不是显示分组。

### 战力两条分系、每系配对联动，守卫按裸装的 120% 取值

属性四项（根骨/力量/仙力/身法）线性增长并在每次破境整段跃升。战力分两系：

- **法术** = `(根骨×15 + 仙力×20 + min(根骨, 仙力)×10) / 10`
- **武术** = `(力量×20 + 身法×12 + min(力量, 身法)×10) / 10`

两系的最后一项是**联动**：同系两个属性互为表里（根骨是法术的根基、仙力是法力；
力量是武技的劲、身法是发劲的准头），所以取较低者再计一份默契加成。结果是「只堆同系
里权重更高的那个」会白费加成，20 点均分（法术 45）胜过 20 点全押仙力（40）——这条由
测试 `a_balanced_pair_out_powers_a_lopsided_one` 钉住。

总战力 `power()` = 法术 + 武术，等级、装备、仙草三处加成全部先落到 `Attributes`
再走同一条公式。守卫战力 `base_attributes(cap).power() × 120%`，即同境界满层、**不带
任何装备**的修士的 1.2 倍。于是「裸装刚好差一点、补装备就能赢」是算出来的而不是调参
出来的，测试直接断言两条。

### 掉落互斥且是概率，RNG 可种子化

每次完成专注各掷一次：装备 60‰、种子 50‰，**不共存**（一次只掉一样）。随机源是模块内
实现的 SplitMix64，`Rng::seeded` 让测试用固定种子复现同一串掉落，测试因此能断言
「1000 次会话掉落数落在 20..=250」而不是断言某个具体结果。

### 扣分幂等、翻倍、有下限、碰不到已破的境界

`apply_absence(today)` 只结算到**昨天**，并用 `charged_through_day` 记住结算到哪一天，
所以同一天多次 tick、多次重启都不会重复扣。一天不修炼扣 `800 × 境界序号`（起点境界
恰为 800），连续未修炼再按 `1.2^missed_streak` 放大（`missed_streak` 自 0 起，首日即
指数 0）。扣完再 `max(stage_floor(realm))`：成长值可以被扣掉，境界不能。

### 时间从启动处注入一次

`PomodoroService::start_at(path, utc_offset_seconds)` 取代只传路径的入口，app 启动时传
`bongocat_platform::local_utc_offset_seconds()`。旧的 `start(path)` 保留为 offset 0 的
委托，仅测试与 Windows 使用。跨夜结算发生在 worker 的 tick 里：每个 tick 先结算缺勤，
有扣分才发布新快照并落盘。

### 每升一级发 1 点自由属性，按历史峰值结算

等级本身已经抬升四项属性，但那是固定曲线，用户没有选择。因此每升一级额外发
`FREE_POINTS_PER_LEVEL`（1 点）由用户手工分配：`Cultivation::allocate` 把 1 点加到某
一项上，落进 `CultivationDocument.allocated`，再与等级/装备/仙草一起进同一条战力公式。

点数按**历史最高等级**（`peak_level`）结算，不是按当前等级：缺勤扣分会把等级压下来，
但已经赚到并花掉的点不能被收回。`normalized()` 里从成长值反推峰值并夹住已分配点数，
所以手改文档既不能凭空多也不会有负数的点。`free_points = granted - allocated` 用饱和减法，
多分的非法状态只会暂停发点，不会变成负余额。

### 收获按低概率返种，让仙草园能自己续上

`harvest` 先付作物属性点（`5 + 3 × 阶`），再掷一次 `SEED_RETURN_PERMILLE`（150‰，约七分之
一）：中了就给同属性的种子，其中 `SEED_RETURN_UPGRADE_PERCENT`（30%）升一阶，其余同阶。
返种概率远低于掉落（装备 60‰ + 种子 50‰），所以「种一轮只回一次种子」是常态，仙草园
不会变成自给自足的循环经济，但也不会因为种子用完而彻底停摆。返回的种子与掉落走同一个
`push_seed`，因此自动并进已有的种子堆。

### 展示面继续分层：overlay 不读 catalog

额头徽章与悬浮 HUD 仍是 AppKit 面板，但它们只画被传进来的字符串。app 侧
`cultivation_copy(&CultivationView, locale, total, remaining)` 用 `bongocat_i18n` 合成
`OverlayCultivationText { badge, hud, hud_progress_permille }` 后交给 overlay，与番茄钟
菜单的分工一致：读 catalog 是产品的职责，画文字是面板的职责。

悬浮 HUD 两行：第一行境界·层·成长值进度，第二行累计番茄数与当前任务剩余数。这两个数
来自 `PomodoroSnapshot` 的 `total_pomodoros` / `remaining_pomodoros`，不是修仙状态机算的
东西——它们关于「喂养阶梯的会话」，而不是关于「阶梯」。累计数单独记在
`Document.total_completions` 而不是把任务计数器相加：删掉一条任务不该把已经干过的活
从历史里抹掉；剩余数在倒计时运行时答当前任务，空闲时答所有未完成任务之和。

### 删掉交互计数里的等级曲线

`bongocat-platform/src/interaction_stats.rs` 只保留计数，等级相关的
`LEVEL_ANCHORS`/`threshold_for`/`level_for`/升级音效/观察等级全部删除，模块文档改写为
「等级来自番茄钟修仙体系」。同一份成长值不再有两个来源。

## 后果

- 一千个 `stats.level.name.*` 键从全部 catalog 消失，换成 `stats.stage.name.1..10` 与
  `cultivation.*` 一组键；`tools/validate-locales.py` 与
  `catalog_keys_are_referenced_by_source` 双向覆盖仍然强制每键都有调用点。
- 旧用户升级后从炼气一层重新开始：交互次数不再是等级，成长值从 0 起算。这是需求的
  直接结果，changelog 的「移除」段明写。
- 时间依赖从「只有番茄钟倒计时需要时钟」扩展到「缺勤结算需要本地日历日」，因此
  `PomodoroState` 持有 offset；`CalendarDay` 仍是唯一的日历换算入口。
- 破境的失败次数被记录（`breakthrough_failures`）但暂不参与任何公式，为后续
  「连败保底」留下字段而不必再改文档形状。
- 仙草成熟按真实时间（每阶半小时），页面上的剩余分钟是展示层自己的减法，领域不轮询。
- 自由属性点与返种都是「多一个可以慢慢丢掉的字段」：`peak_level`、`allocated` 都有
  `#[serde(default)]`，旧文档读进来按已有成长值补算峰值，不会凭空少点也不会凭空发点。

## 验证

- `cargo test -p bongocat-pomodoro`：成长值换算可逆、满层才可破境、裸装打不过守卫、
  装备后可破境、连续缺勤翻倍且不破境界下限、同日不重复扣、掉落概率区间、文档回读；
  每级发 1 点且只发一次、缺勤掉级但不掉已发的点、收获返种约七分之一且升阶约三成、
  快照的累计/剩余数在删任务后仍然正确。
- `cargo test -p bongocat-app`：`cultivation_copy` 的徽章/进度条/百分比三者一致，
  「满层时进度条画满而不是画一个到不了的层」，以及悬浮面板第二行只跟会话数变化。
- `cargo test -p bongocat-i18n` + `python3 tools/validate-locales.py`：两种语言键集一致，
  且新增键在源码中都有字面量调用点。
- `cargo fmt --all -- --check`、三组 `cargo clippy ... -D warnings`、`cargo test --workspace`。
