# ADR-0091: 番茄钟在设置中的入口是顶级页，任务列表只读 + 勾选

状态：已接受（2026-10-15）

## 背景

番茄钟有三个展示面：猫头顶的倒计时浮层（AppKit）、托盘与模型右键菜单（平台层）、
设置窗口里的任务列表与月历（GPUI）。ADR-0090 定下了领域边界，但没规定设置内怎么放。
最初的执行计划把设置内入口写成「模型行为」页下的「番茄时钟」条目 + 任务列表子视图。

实现时撞到一个硬约束：`gpui-kit` 0.7.3 的 `SettingPage` **没有子页/路由栈**，
`SettingItem::render` 是整块自绘元素。所谓「模型行为页内的子视图」在这套组件里并不存在，
只能靠自绘一个覆盖层或一整页来模拟。

## 决策

### 顶级页，不嵌套在「模型行为」下

「番茄时钟」是 `SettingsNavigationPage` 的一个新变体（index 3，位于「模型行为」与
「模型窗口」之间，About 仍最后）。理由是它与「模型行为」不同类：模型行为是**每个模型**
的配置，而番茄计时是用户在会话之间使用的、**模型无关**的活动。把它塞进模型行为页会让
一个全局功能看起来像是某个模型的属性。顶级页还让它在侧栏直接可达，不需要先选一个模型。

### 任务列表只读 + 勾选完成，不做增删改

设置页的任务列表只展示任务，并提供勾选/取消勾选（`PomodoroCommand::SetTaskDone`）。
新建、编辑、开始仍走第 4 步的独立任务弹窗。理由：任务的创建/删除是写操作，
若设置页也能增删改，就会出现两个写入口对同一份 worker 状态双写而分歧的风险。
只读 + 勾选让设置页成为任务状态的第二个**读面**加一个**同源勾选写面**，
与弹窗共用同一条命令通道，不会双写。

### 用 `SettingItem::render` 自绘整页

因为 `SettingPage` 无子页，任务列表 + 月历这种复杂管理视图唯一的落地方式就是一个
自绘的 `SettingItem::Element`：一个 `SettingItem::render` 闭包返回整页内容，
列表/日历用页内一个 `PomodoroSettingsView` 开关切换（切换只换 body，不清空任务数据）。

### 设置页靠 revision 轮询跟随 worker

`PomodoroClient` 不提供订阅 API，因此 `SettingsView` 起一个 500ms 的后台轮询
`poll_pomodoro`：比对 `client.revision()`，仅在 revision 变动时 `cx.notify()`。
这复用了第 4 步 `pomodoro_window` 的 `poll_state` 模式，空闲窗口零快照克隆。

## 后果

- 侧栏多一个顶级项「番茄时钟」；`SettingsNavigationPage` 的顺序、`index()`、
  `.pages(vec![...])` 三处必须同步，由 `navigation_uses_the_agreed_task_order_with_about_last`
  等测试强制。
- `SettingsView` 需要持有一个 `PomodoroClient`：它从 `open_settings_window` 一路透传
  （app 侧 `ensure_settings_window` 已持有 `coordinator.pomodoro_client`）。
  该字段**不加平台 cfg**——Windows 上菜单入口隐藏但 worker 照常起（ADR-0090）。
- 任务的创建/编辑只存在于弹窗；设置页是第二读面 + 勾选写面。
- edition 2024 下自绘函数若返回 `impl IntoElement` 会捕获 `&mut Window/App` 生命周期，
  与 `SettingItem::render` 要求的 `'static` 闭包冲突，故统一返回 `gpui_kit::AnyElement`。

## 验证

- `cargo test -p bongocat-ui`：导航顺序、`every_localized_page_title_is_present_and_distinct`
  （含新 `navigation.pomodoro.title` 的 8 语言去重）。
- `python3 tools/validate-locales.py` + `cargo test -p bongocat-i18n`：新增
  `pomodoro.settings.*` 键 8 语言一致，且双向覆盖（每个键都在源码有字面量引用）。
- `cargo clippy -p bongocat-ui --all-targets -- -D warnings`。
