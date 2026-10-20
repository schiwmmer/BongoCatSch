# TODO

## Phase 4：设置与模型管理

- [ ] **P4-WINDOW-MODE（issue #1130）**：Windows 普通窗口、可配置纯色背景、OBS 窗口捕获与抠图、旧 v1 配置读取、禁用颜色预览和统一客户区等比例缩放已实现。依赖 ADR-0057、ADR-0054；协议见 ADR-0089。退出条件：配置/UI/runtime/HWND/D3D11 回归、`just check`、Windows product smoke、OBS BitBlt/WGC/抠图验收，以及 Windows 10 和 125/150/200% 跨屏实机验证。当前 Windows 11 OBS 与自动化验收已有证据，缺少的 Windows 10、跨屏与设置布局验收保持未勾选；macOS 不在本任务范围。证据见 `docs/benchmark/windows-window-mode-2026-10-08.md`。
- [ ] **P4-MOTION-OVERLAP（issue #1107）**：默认关闭的动作叠加开关、旧 v1 文档读取、独立计时/淡出/停止和按启动顺序求值已实现，标准模型跨组重复资源重播有自动化回归。依赖 ADR-0061、ADR-0012（动作音效）；协议见 ADR-0088。退出条件：workspace 与独立 config-store contract、`just check`、双平台 CI smoke，以及 Windows/macOS 实机多动作叠加、跨组重播和 800×600/DPI/Retina 目视验收。尚缺的平台实机证据保持未勾选。
- [ ] **P4-NESTED-MODEL-IMPORT（issue #1105）**：嵌套模型发现、单模型自动继续、多模型复用选择弹框、模式选择及顺序导入已实现。依赖 ADR-0037、ADR-0055、ADR-0059；协议与扫描边界见 ADR-0087。退出条件：自动化扫描/服务/GPUI 交互回归、`just check`、macOS smoke，以及 Windows/macOS 实机文件夹选择与拖入、800×600 和 DPI/Retina 目视验收。缺少的平台实机证据仍保持未勾选，不由另一平台编译或 headless 测试代替。

## Phase 5：番茄钟与修仙（feat/pomodoro 分支，未合入 master）

- [ ] **P5-POMODORO-CULTIVATION（ADR-0090、ADR-0092、ADR-0094）**：番茄钟（任务、月历、倒计时浮层、右键菜单）与修仙体系（十境阶梯、四属性与两系战力、装备、仙草园、功法、灵石集市、探索、破境战斗、事件日志）已实现；领域边界、纯值状态机、注入式时钟、两份 `schema_version = 1` 文档与旧档曲线迁移均有自动化 contract，`--cultivation-page-smoke` 覆盖页面挂载，i18n 收敛为 zh-CN + en-US。退出条件：双平台 `just check` 全绿、macOS 实机验收（倒计时浮层表演、修仙页各面板交互、破境与探索战斗弹窗）、Windows 菜单 cfg 分支与安装包 smoke、ADR-0094 旧档迁移实机验证、CHANGELOG 中英条目随版本定稿（`just release-notes` 可取到条目），以及合入 master 后 sync-to-gitee 同步。当前自动化证据已具备，实机验收与发版保持未勾选。
