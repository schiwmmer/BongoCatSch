# ADR-0101: 一次性表情在保持时长后自动恢复

状态：已接受（2026-11-01）
补充：ADR-0083（重复触发同一表情可以把它关掉）、ADR-0061（Live2D 播放边界）
需求：用户报告「眨右眼 / 闭左眼做完后不自动恢复，一直卡在表情状态」

## 背景

预置模型的 `wink`（眨右眼）、`exp_2`（闭左眼）、`exp_1`（眯眼）是标准 Live2D 表达式
（exp3）。触发后（设置窗口播放按钮或快捷键，都收敛为 `RuntimeCommand::SetExpression`）它们
**永不自动恢复**：`ExpressionPlayback` 的 `fade_out_started_at` 只在「重复触发」（ADR-0083）、
`ClearExpression` 命令或新表情替换时被设置。于是眨眼类表情一旦播放就固定住，用户看到的是
眼睛一直闭着，与「眨一下」的动作直觉不符。

Live2D 表达式本质上是持续状态，没有「时长」概念；「一次性动作」语义在 Live2D 里由 motion 承担。
但这三个资源是表达式，且语义（眨一下眼）显然是一次性。因此需要在表达式层补上一次性语义，而不是
把它们改成 motion（那要改资源格式、绑定与行为声明，超出本次修复范围）。

## 决策

### 1. 一次性语义是 exp3 的可选扩展字段 `AutoRecoverTime`

在标准 exp3 JSON 顶层加可选字段 `AutoRecoverTime`（秒，本应用扩展）。语义：表情 fade in 完成后
保持该秒数，然后自动 fade out 回到模型默认表情。

- 缺失、`null`、`<= 0` 都视为「持续表情」（保持现状：直到被替换或清除）。这是默认值，所以
  标准 Live2D exp3（无该字段）解析行为逐字不变（§3.1 兼容性）。
- `> 0` 视为一次性：`auto_recover_at = started_at + fade_in_duration + AutoRecoverTime`。
- 非有限值在 `> 0` 分支拒绝（防御；JSON 数字恒有限，此分支实际不可达）。

### 2. 判定与恢复都在 renderer，因为那里才持有单调时钟和 clip

`RuntimeRenderer::set_expression` 在压入 `ExpressionPlayback` 时计算 `auto_recover_at`。
`evaluate` 每帧在保留图层之前检查：`fade_out_started_at.is_none() && now >= auto_recover_at`
时，把 `fade_out_started_at` 设为 `Some(recover_at)`（不是当前帧 `now`），这样晚到的 tick 不会给
恢复叠加一整帧死时间。之后走与手动清除相同的 fade-out 与 retain 路径，模型自然回到默认表情。

一次性表情不新增模型状态：它复用每次替换都有的那套 fade-out。

### 3. 一次性表情不被「记住」，也不作为「当前表情」展示

`set_expression` 返回 `ExpressionSetOutcome::{Persistent, AutoRecover}`。worker 的
`SetExpression` 分支在 `AutoRecover` 时**不**写 `active_expression` 也不写
`user_expression_memory`：一次性眨眼不是用户要保留的选择，写记忆会让模型下次激活时又眨一下。

idle 随机表情（`maybe_trigger_random_behavior`）保持原行为：它选中表情后仍写
`active_expression`（作为「猫正在做的脸」的展示），但从不写记忆。无论选中的是否一次性，该
展示语义不变；一次性与否只影响 renderer 何时自动恢复。

### 4. 资源

`resources/models/{standard,keyboard,gamepad}/{wink,exp_1,exp_2}.exp3.json` 加
`AutoRecoverTime: 1.5`（9 个文件，内容按表情分 3 组一致）。`focus`（专注）、
`live2d_expression0/1/2`（换脸）不加字段，保持持续语义。

### 5. 解析器双入口

exp3 有两个解析器：`bongocat-live2d-playback` 的 `ExpressionClip`（语义）与
`bongocat-model` 的 `RawExpressionResource`（资源结构校验，`deny_unknown_fields`）。
两者都接受 `AutoRecoverTime`；model 侧只做结构校验，值语义归 playback，故该字段在
model 侧显式不读（`_` 前缀）。

## 验证

- `bongocat-live2d-playback`：`parses_auto_recover_time`（缺失/`null`/`0`/负值 → 持续，
  `1.5` → `Some`）。
- `bongocat-runtime` rendering：`one_shot_expression_recovers_to_default_after_its_hold`
  （用预置 `wink` 端到端：fade in + hold 内闭眼，到期 mid fade-out，结束后回到默认）、
  `persistent_expression_does_not_recover_on_its_own`（无字段 60s 后仍满权重）。
- 回归：`bongocat-runtime --lib` 115 过、`bongocat-model` 31 过、`bongocat-packaging` 46 过、
  `bongocat-app` expression_memory 5 过；`cargo clippy -p bongocat-live2d-playback -p bongocat-model
  -p bongocat-runtime --lib --tests` 无警告。
