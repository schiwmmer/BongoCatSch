# ADR-0093: 语言只保留中文与英文

状态：已接受（2026-10-19）

## 背景

产品原先随包八份 catalog：`en-US`、`zh-CN`、`zh-TW`、`ar-SA`、`vi-VN`、`pt-BR`、`ko-KR`、
`fr-FR`。其中两个中文只看字形差别，其余六种没有任何一版是逐句校对过的：新增一个键要写
八遍，`stats.stage.name.*`、`cultivation.*` 这类成组新增一次就是几十行重复，而校对只能覆盖
到中英两份，别国用户读到的其实是没人看过的机器翻译。产品只需要中文和英文，多出来的六份
不是能力而是负债。

## 决策

**只发两份 catalog。** `crates/bongocat-i18n/locales/` 只剩 `en-US.json` 与 `zh-CN.json`；
`SHIPPED_LOCALES` 收敛为两项，`build.rs` 的 `CATALOGS` 同步。`tools/validate-locales.py`
的 `EXPECTED_LOCALES` 与打包契约测试都断言这两项，因此「加语言」不再是改文档的事，而是
必须同时改四处并让契约测试通过。

**回退规则按 RFC 4647 主语言子标签。** `locale_code` 只剩一个判断：`zh` 开头的任何标签
（`zh`、`zh-CN`、`zh-Hans`、`zh-TW`、`zh-Hant-HK`）都落到 `zh-CN`，其余一律落到 `en-US`。
繁体标签读简体是刻意的取舍：宁可给一份读得懂的简体，也不给一份读不懂的英文。原本
「简体/繁体各一份」的脚本子标签分支因此消失，`is_simplified_chinese` 删除。

**枚举只留三个值。** `bongocat_config::Language` 与 `SettingsLanguage` 都是
`System | ChineseSimplified | English`，持久化值为 `system` / `zh-CN` / `en-US`，
其余六个变体从枚举、投影（`settings/projection.rs`）、语言选择器与 UI 名称函数里一起删掉。

**安装器跟着收。** Windows NSIS 的 `INSTALLER_LANGUAGES` 只剩 `English`、`SimpChinese`，
仓库自带的 `Vietnamese.nsh` 与 `custom_language_files` 一并删除——两份 catalog 都在
`cargo-packager` 自带的安装器文案里。

## 后果

- 繁体用户、以及任何非中英系统（`de-DE`、`ja-JP`、`ar-EG`……）都落到英文；繁体系统落到简体。
  这条写进 `Language::from_system_locale` 的注释与测试，不再是隐含行为。
- 旧配置里若存着 `"language": "zh-TW"` 之类已经不合法的值，整份配置会解析失败并走既有的
  「备份回滚 / 回到默认」路径：结果是主题与语言回到系统默认，而不是崩溃。这比静默把
  `fr-FR` 读成英文更保守，代价是那一份配置里用户的其他设置也一起回默认。
- 每份 catalog 仍是 377 键，`catalog_keys_are_referenced_by_source` 与
  `source_referenced_keys_exist_in_the_catalog` 双向覆盖不变：键少了，规则没松。
- `settings/appearance/language/options` 下六条 `chinese_traditional`/`arabic`/`vietnamese`/
  `portuguese`/`korean`/`french` 文案删除；语言选择器从 8 项变 3 项（跟随系统 + 简体 + 英文）。
- 后续要加语言，成本是「一份新 catalog + 语言枚举 + 安装器语言 + 两处契约断言」，
  而不是「再翻译一遍」。

## 验证

- `python3 tools/validate-locales.py`：2 份 catalog，377 键，占位符一致。
- `cargo test -p bongocat-i18n`：区域子标签回退表（`en-GB`、`zh_TW`、`zh-Hant` 等）、
  非中英语言落到英文、`SHIPPED_LOCALES` 与磁盘上的文件一致。
- `cargo test -p bongocat-config`：`from_system_locale` 的全部分支与显式选择不跟随系统。
- `python3 -m unittest tools.tests.test_packaging_contract`：catalog 集合与安装器语言集合相等。
