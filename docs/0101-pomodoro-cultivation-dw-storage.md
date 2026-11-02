# 方案：修仙 + 番茄钟数据迁移数仓存储

> 状态：设计稿（待评审） · 2026-11-02
> 动机：现状单 JSON 文档（桌面 `pomodoro-tasks.json` / 云端 `pomodoro_saves.document` JSONB）已两次出事：v2.4.0 升级因 `#[serde(default)]` 缺失整档重置（2026-11-02 丢档事故）；JSONB 整档原子替换导致**无历史、无对账、无法回滚到任一时点**。
> 目标：把「一份 JSON 走天下」改为**实体表 + 追加型事实表**的分层存储，状态可对账、历史不可变、口径唯一。

## 0. 前提假设（待确认）

| # | 假设 | 影响 |
|---|------|------|
| A1 | 目标库 = 现有云服务 PostgreSQL（应用 `wbapp_PAb06CARldeQ5eqHS2W9Jl`，沿用 `auth.uid()` + RLS FORCE 模式） | 若改本地 SQLite，DDL 类型需降级（见 §7 待确认） |
| A2 | 单玩家数据量：每天 ≤ 几百行事实，年 ≤ 10 万行——**中小规模** | 不上 Data Vault/分区表，DWD 3NF + ADS 汇总即可 |
| A3 | 本方案只管**存储建模**；桌面端 Rust / web 端 JS 的读写改造另立 TODO | 迁移策略见 §6 |
| A4 | 枚举值以 `crates/bongocat-pomodoro/src` 现有代码为准（events.rs / garden.rs / mod.rs），DDL 注释列当前已知值 | 端侧改造时以代码为准回填枚举字典 |

## 1. 业务过程与数据域划分

| 数据域 | 业务过程 | 粒度 | 更新频率 |
|--------|----------|------|----------|
| 玩家域 | 玩家状态演进（境界/修为/灵石/资源/仙草园/技能） | 一玩家一行（当前态，SCD Type 1 覆盖写） | 每次会话完成/事件结算后 |
| 任务域 | 任务管理 | 一任务一行（实体） | 建任务/勾完成时 |
| 专注域 | **一次番茄钟完成**（学习/伐木/采矿三模式） | 一次完成一行（事务事实，追加不可改） | 每次倒计时归零 |
| 修仙事件域 | 探索/打工/锁妖塔/试炼/集市/收获/日常/采集结算 | 一事件一行（事务事实，追加不可改） | 每次事件结算 |
| 资产域 | 功法/装备/种子/灵田/建筑/技能 实体持有 | 一实例（或一叠/一块/一建筑）一行 | 获得与变动时 |
| 汇总层 | 日历与面板查询 | 玩家 × 自然日 一行 | 视图实时计算 |

**业务总线矩阵**（业务过程 × 维度）：

| 业务过程 \ 维度 | 玩家 | 日期 | 任务 | 功法/装备/种子 |
|---|---|---|---|---|
| 番茄钟完成 | ✔ | ✔ | ✔ | 采集模式✔（产出种子/材料） |
| 修仙事件 | ✔ | ✔ | — | ✔（获得/消耗） |
| 任务完成 | ✔ | ✔ | ✔ | — |

## 2. 分层架构

```
ODS  ods_document_raw        —— 原始 Document JSON 留档（每次写档前的快照，天然解决"升级吃档"）
DWD  dwd_*                   —— 三范式实体表（玩家/任务/资产）
     fact_focus_session      —— 事务事实表（番茄钟完成，追加）
     fact_cultivation_event  —— 事务事实表（修仙事件，追加）
ADS  ads_player_day (视图)   —— 日历/面板专用汇总，跨层只读 DWD
```

分层方法标注：ODS 贴源留档 | **DWD 三范式** | ADS 维度汇总（视图）。层内不混用：DWD 不做宽表，ADS 不追求 3NF。

## 3. DWD 范式化设计

### 3.1 函数依赖清单与范式结论

| 表 | 函数依赖 | 处理 |
|----|----------|------|
| `dwd_task` | task_id → name, estimated_pomodoros, status, created_at, completed_at | 满足 3NF |
| （原 JSON）TaskRecord | completed_pomodoros = COUNT(completion_timestamps) | **派生依赖，删列**，由视图/查询聚合 |
| （原 JSON）Document | total_completions = COUNT(全部完成) | **派生依赖，删列**（原注释已自证：任务删了计数仍要保留 → 本就该是事实表行数） |
| `fact_focus_session` | session_seq → owner, task_id, completed_at, focus_minutes, mode | 满足 3NF；`day` 列为 completed_at 的派生冗余，**保留**（收益：按日聚合免函数计算，分区键；一致性：写入时同事务生成） |
| `fact_cultivation_event` | event_seq → owner, event_at, kind, won, stones_delta, … | 满足 3NF；长尾明细（战报等）入 `raw JSONB`，核心口径全部列存 |
| `dwd_player` | owner_id → realm, growth, stones, … | 属性列不可再拆出实体，无传递依赖，满足 3NF |
| `dwd_building` / `dwd_skill` | (owner_id, kind) → level[, xp] | 原 JSON 把 8 个建筑/技能摊成列 → **列转行**，消除结构化改动即改表的问题 |

**范式达标结论：DWD 全部 3NF。** 保留的冗余仅 `fact_focus_session.day` 一处，已声明收益与一致性手段。

### 3.2 ER 关系

```
dwd_player (1) ──< (N) dwd_task ──< (N) fact_focus_session
     │(1)                        （task_id 可空：修仙采集会话无任务）
     ├──< (N) fact_cultivation_event
     ├──< (N) dwd_art_instance / dwd_equipment_instance
     ├──< (N) dwd_seed_stack / dwd_plot
     └──< (N) dwd_building / dwd_skill
ods_document_raw (1) ←—— 每次写档前整档快照（回滚保险）
ads_player_day = 聚合 fact_focus_session × fact_cultivation_event（视图）
```

## 4. 表设计（PostgreSQL DDL）

类型标准：时间 `timestamptz`（unix 秒经 `to_timestamp()` 转换）、业务日 `date`、灵石/计数 `bigint`、禁 FLOAT 存数值；所有表带粒度注释与枚举字典。

```sql
-- ============ ODS ============
CREATE TABLE ods_document_raw (          -- 粒度：一玩家最新一份原始文档快照
  owner_id      uuid PRIMARY KEY DEFAULT auth.uid(),
  schema_version integer NOT NULL,
  document      jsonb NOT NULL,          -- 原 pomodoro-tasks.json 全文
  synced_at     timestamptz NOT NULL DEFAULT now()
);
-- RLS: FORCE + POLICY owner_all USING (owner_id = auth.uid())（下同，全部表一致）

-- ============ DWD 实体 ============
CREATE TABLE dwd_player (                -- 粒度：一玩家一行当前态（SCD1 覆盖写）
  owner_id            uuid PRIMARY KEY DEFAULT auth.uid(),
  realm               smallint NOT NULL DEFAULT 0,  -- 境界 0=炼气…9=仙人，枚举见 curve.rs
  growth              integer NOT NULL DEFAULT 0,   -- 当前境界内修为
  stones              bigint   NOT NULL DEFAULT 0,  -- 灵石（整数，单位：颗）
  aptitude_tenths     integer NOT NULL DEFAULT 0,   -- 潜质（十分位）
  peak_level          smallint NOT NULL DEFAULT 1,  -- 峰塔层
  training_points     integer NOT NULL DEFAULT 0,
  breakthrough_failures integer NOT NULL DEFAULT 0,
  breakthrough_blocked_until timestamptz,
  garden_rank         smallint NOT NULL DEFAULT 1,
  garden_xp           integer NOT NULL DEFAULT 0,
  res_wood  integer NOT NULL DEFAULT 0,  -- 木材（单位：个）
  res_stone integer NOT NULL DEFAULT 0,  -- 石料
  res_iron  integer NOT NULL DEFAULT 0,  -- 铁矿
  res_coal  integer NOT NULL DEFAULT 0,  -- 煤
  insight_herbs integer NOT NULL DEFAULT 0,  -- 悟心草（颗）
  wisdom_herbs  integer NOT NULL DEFAULT 0,  -- 慧心草
  marrow_pills  integer NOT NULL DEFAULT 0,  -- 洗髓丹
  last_active_day date,                  -- 业务日（玩家本地时区自然日）
  charged_through_day date,
  daily_event_day date,
  updated_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE dwd_task (                  -- 粒度：一任务一行
  owner_id uuid NOT NULL,
  task_id  bigint NOT NULL,              -- 原 TaskRecord.id
  name     text NOT NULL,
  estimated_pomodoros integer NOT NULL DEFAULT 0,
  status   text NOT NULL DEFAULT 'in_progress' CHECK (status IN ('in_progress','done')),  -- 枚举: TaskStatus
  created_at   timestamptz NOT NULL,
  completed_at timestamptz,              -- 语义：最后一次专注完成时刻，非勾选时刻
  PRIMARY KEY (owner_id, task_id)
);  -- completed_pomodoros 不建列：由 fact_focus_session 聚合（消除派生依赖）

CREATE TABLE dwd_art_instance (          -- 粒度：一部功法实例一行
  owner_id uuid NOT NULL,
  art_id   bigint NOT NULL,              -- 原 arts[].id
  name     text NOT NULL,
  rank     smallint NOT NULL DEFAULT 0,  -- 已修至阶
  status   text NOT NULL DEFAULT 'learning' CHECK (status IN ('learning','mastered','sold')),
  PRIMARY KEY (owner_id, art_id)
);

CREATE TABLE dwd_equipment_instance (    -- 粒度：一件装备一行
  owner_id uuid NOT NULL,
  item_id  bigint NOT NULL,              -- 原 equipment[].id
  name     text NOT NULL,
  tier     smallint NOT NULL,            -- 阶
  star     smallint NOT NULL DEFAULT 1,  -- 星（最低停 1 星模型）
  slot     smallint CHECK (slot BETWEEN 1 AND 4),  -- 非空=已装备（原 equipped 4 槽）
  PRIMARY KEY (owner_id, item_id)
);

CREATE TABLE dwd_seed_stack (            -- 粒度：一(属性,灵草,阶,品)叠一行
  owner_id uuid NOT NULL,
  attribute text NOT NULL CHECK (attribute IN ('root','strength','mana','agility')),
  herb      text,                        -- NULL=普通种子；'wisdom'/'insight'=灵草
  tier      smallint NOT NULL CHECK (tier BETWEEN 1 AND 10),
  grade     text NOT NULL DEFAULT 'yellow',  -- 枚举: HERB_GRADE
  count     integer NOT NULL DEFAULT 0,
  PRIMARY KEY (owner_id, attribute, herb, tier, grade)
);

CREATE TABLE dwd_plot (                  -- 粒度：一块灵田一行
  owner_id uuid NOT NULL,
  plot_no  smallint NOT NULL,            -- 1..MAX_PLOT_COUNT(12)
  crop     jsonb,                        -- CropRecord（含种子名/属性/成熟时刻）；NULL=空田
  PRIMARY KEY (owner_id, plot_no)
);

CREATE TABLE dwd_building (              -- 粒度：一建筑一行（原 JSON 8 个摊平列 → 行存）
  owner_id uuid NOT NULL,
  building_kind text NOT NULL CHECK (building_kind IN ('workshop','warehouse','smithy','dojo')),
  level    smallint NOT NULL DEFAULT 0,
  PRIMARY KEY (owner_id, building_kind)
);

CREATE TABLE dwd_skill (                 -- 粒度：一技能一行
  owner_id uuid NOT NULL,
  skill_kind text NOT NULL CHECK (skill_kind IN ('woodcut','stonecut')),
  level    smallint NOT NULL DEFAULT 0,
  xp       integer NOT NULL DEFAULT 0,
  PRIMARY KEY (owner_id, skill_kind)
);

-- ============ DWD 事实 ============
CREATE TABLE fact_focus_session (        -- 粒度：一次番茄钟完成一行；追加不改
  session_seq  bigint GENERATED ALWAYS AS IDENTITY,
  owner_id     uuid NOT NULL,
  task_id      bigint,                   -- NULL = 修仙采集/纯专注会话
  completed_at timestamptz NOT NULL,
  day          date NOT NULL,            -- completed_at 的业务日（冗余，见 §3.1）
  focus_minutes integer NOT NULL,        -- 单次时长（原全局 focus_minutes 属于配置，会话层固化历史值）
  mode         text NOT NULL DEFAULT 'study' CHECK (mode IN ('study','woodcut','mine')),
  growth_gain  integer NOT NULL DEFAULT 0,   -- 本次修为增量（端侧需新增落点）
  result       jsonb NOT NULL DEFAULT '{}',  -- 长尾产出（掉落/采集明细），核心口径必须列存
  PRIMARY KEY (owner_id, session_seq)
);

CREATE TABLE fact_cultivation_event (    -- 粒度：一次修仙事件一行；追加不改
  event_seq   bigint GENERATED ALWAYS AS IDENTITY,
  owner_id    uuid NOT NULL,
  event_at    timestamptz NOT NULL,      -- 原 EventRecord.at_unix
  kind        text NOT NULL,             -- 枚举: daily/walk/job/battle/trial/shop/harvest/session_gather/breakthrough（以 events.rs EventDetail 为准）
  won         boolean,                   -- NULL=该事件无胜负语义
  stones_delta bigint,                   -- 灵石收支（正=获得，负=消耗；与 dwd_player.stones 可对账）
  art_gain    text,  gear_gain text,  seed_name text,   -- 原事件的三个可选产物
  raw         jsonb NOT NULL DEFAULT '{}',              -- 完整 EventDetail 留档（战报等）
  PRIMARY KEY (owner_id, event_seq)
);

-- ============ ADS ============
CREATE VIEW ads_player_day AS            -- 粒度：玩家 × 自然日；日历/面板直读
SELECT f.owner_id, f.day,
       SUM(f.focus_minutes)                                AS focus_minutes,
       COUNT(*)                                            AS pomodoro_count,
       COUNT(*) FILTER (WHERE f.mode = 'study')            AS study_count,
       COUNT(*) FILTER (WHERE f.mode <> 'study')           AS gather_count
FROM fact_focus_session f
GROUP BY f.owner_id, f.day;
-- 灵石净额/事件数可按需 UNION fact_cultivation_event 同构聚合
```

## 5. 核心指标口径（四要素）

| 指标 | 原子指标 | 修饰词 | 统计周期 | 统计粒度 | 口径 SQL |
|------|----------|--------|----------|----------|----------|
| 每日专注分钟 | focus_minutes | 无 | 自然日 | 玩家×日 | `SELECT SUM(focus_minutes) FROM fact_focus_session WHERE day=$d` |
| 每日番茄完成数 | session 行数 | 无 | 自然日 | 玩家×日 | `COUNT(*)` 同上 |
| 任务完成进度 | 已完成番茄数 | 按任务 | 任务生命周期 | 玩家×任务 | `COUNT(*) WHERE task_id=$t`（替代原 completed_pomodoros 列） |
| 每日灵石净额 | stones_delta 求和 | 无 | 自然日 | 玩家×日 | `SUM(stones_delta) FROM fact_cultivation_event` |
| 累计番茄完成数 | session 行数 | 全历史 | 至今 | 玩家 | `COUNT(*)`（替代原 total_completions 列） |

One Metric One Definition：`total_completions`、`completed_pomodoros` 不再是存储字段，只允许由事实表推导——口径唯一，杜绝双源漂移。

## 6. 迁移策略（三步，可回滚）

1. **双写期**：端上仍写 JSON 档；新增同步器把 Document 投影到云库（ODS 整档快照 + DWD/事实表 upsert/追加）。JSON 是权威源，云库只读验证。
2. **切读**：登录态下改为从云库读状态渲染，JSON 降级为离线缓存；两源 diff 校验跑一周。
3. **收尾**：云库为唯一权威源，JSON 仅作导出/备份；`ods_document_raw` 保留每次写前快照（轮换），升级兼容问题由"事实表只追加 + 实体表 serde(default)"双保险兜底。

⚠️ 端侧新增义务：`fact_focus_session.growth_gain` 等增量字段需在 `record_cultivation_session` 落点补埋点；桌面端接云库需 Rust HTTP 客户端改造（另立 TODO，见假设 A3）。

## 7. 待确认问题

| # | 问题 | 建议默认 |
|---|------|----------|
| 1 | 目标库是否就是现有云 Postgres？还是本地 SQLite（桌面离线优先）？ | 云 Postgres（web 端已具备登录/RLS） |
| 2 | 桌面端本期是否接入，还是先只迁 web 端？ | 先 web 端，桌面端后续 |
| 3 | 灵石是否要独立流水表 `fact_stones_ledger`（每笔收支一行，强对账）？ | P1 暂缓，事件表 `stones_delta` 已可对账 |
| 4 | 多设备并发写：last-write-wins 还是事件合并？ | 先 LWW（单用户多设备低频），冲突以 ODS 快照可回滚兜底 |

## 8. AI 友好度评估（自动嵌入）

| 维度 | 结论 | 说明 |
|------|------|------|
| ① 命名与注释自描述 | ✅ | 全表 COMMENT 含粒度声明（§4 DDL）；dwd_/fact_/ads_ 前缀分层可读；枚举 CHECK 约束即字典 |
| ② 指标语义结构化 | ✅ | §5 四要素齐备，口径 SQL 可执行；`total_completions` 类模糊计数已消灭 |
| ③ 口径唯一与归属 | ✅ | 计数类指标只从事实表推导；`stones` 余额与 `stones_delta` 流水可对账（One Metric） |
| ④ 时间与类型语义 | ⚠️ | 业务日（`day`/`last_active_day`，DATE）与技术时间（`*_at`，timestamptz）已分离；⚠️ 遗留点：原 JSON `daily_event_day` 是 epoch-day 整数，迁移映射须显式转 DATE，且玩家本地时区口径需在注释固化 |
| ⑤ 上下文可发现性 | ⚠️ | 分层/域前缀齐备；⚠️ 血缘工具与样例数据查询依赖云库控制台能力，建议端侧改造时补一份表清单 README 指向本设计 |

**总体结论：AI 就绪（2 项 ⚠️ 均为迁移期动作，落地后闭环）。**

### 评审结论分级

- **P0（本次方案消除）**：整档原子替换 = 任何升级都可能全丢（已发生一次）；`completed_pomodoros`/`total_completions` 双源计数漂移风险。
- **P1（本方案消除）**：事件历史只留最近若干条、无对账能力；建筑/技能摊平列导致结构改动即改表。
- **P2**：`day` 冗余列、`result/raw` JSONB 长尾——已按"声明收益 + 核心口径列存"原则受控保留。
