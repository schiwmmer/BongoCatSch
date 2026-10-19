# Changelog

## Unreleased

### ✨ Features

- macOS interaction stats and levels: hovering the model window shows a small panel underneath with the current level and lifetime interaction count. One keyboard or mouse press is one interaction, and the level grows with the lifetime total.
- Levels stretch to 1000: the original ten-level pacing stays intact across the first hundred levels (level 100 = 200,000, one title every ten levels), then scales tenfold to level 1000 = 2,000,000; the panel gains an upgrade progress bar with the percentage.
- All 1000 levels carry a fully authored name of their own: ten stages (Kittenhood to Cat God) of a hundred levels each, every name globally distinct with no ordinal tiers, the names themselves climbing from humble beginnings to godhood. The Simplified Chinese and English tables are complete; the remaining six languages land in the next batch.
- Tightened the stats panel copy: the progress line drops the leading labels ("Total 10,005 · 5 to go · 3%"), so the panel and its progress bar shrink with the shorter text.
- The level number now rides the cat's forehead: a small bare number pinned to the top center of the model window, following the cat through moves and resizes and hiding whenever the cat hides.
- A Pomodoro timer, started from the "Start Pomodoro" menu entry on macOS: name a task, set how many focus sessions it needs, and a countdown floats above the cat as the session runs. A finished session plays a short chime, and a right-click on the countdown pauses or abandons it. Tasks are kept in a separate `pomodoro-tasks.json`, and a countdown does not survive a restart.
- A "Pomodoro" settings page with a task list and a month calendar: tick a task done as you finish it, and see each day's finished focus sessions laid out on a calendar. The whole feature is localized into all eight supported languages.
- The "Pomodoro" settings page grows a "New Task" button beside its title, so a first task can be created without hunting for the list.
- Right-clicking a task in the Pomodoro settings opens a small menu to edit it — name and expected focus sessions — or delete it, which also drops the task's running session and history.
- Each task row in the Pomodoro settings list now carries a "Start" button at its right edge, so a focus session begins straight from the list without opening the picker; it is disabled while a session is already running for that task.
- The task right-click menu gains a "Duplicate" item that opens the new-task editor pre-filled with the same name and focus-session count, so a copy is confirmed before it is saved as a separate task.
- The interaction-count ladder is replaced by a cultivation ladder: ten realms from Qi Refining to Immortal, a hundred layers each and a thousand levels in all, every layer bought with growth rather than keystrokes. A finished focus session pays ten growth, and a longer session pays proportionally more.
- Growth buys four attributes — constitution, strength, spirit power and agility — that climb with the level and jump by a whole realm's worth at each breakthrough, plus the single power figure that sums them for every fight.
- A cultivator carries a weapon, a robe, trousers and boots, each dropped at one of four rarities; the piece in a slot adds its attributes to the wearer's, and a better piece simply replaces the one it displaces.
- A herb garden of four plots: plant a seed of one attribute and tier, wait one real hour per tier, and harvest a crop whose power raises that attribute for good.
- Every finished focus session rolls a low chance of one drop — six percent a piece of gear, five percent a seed, never both at once.
- Breaking into the next realm needs a guardian beaten in a fight the growth alone cannot win: the guardian is a fifth stronger than a bare cultivator at the same realm's cap, so gear and herbs are what carry it. A loss is recorded and leaves the realm where it was.
- A day without a finished focus session costs growth, and consecutive missed days double the charge up to a cap of 320, so a lapse is answered without ever undoing a breakthrough.
- A task's completion date is now the moment its last focus session finished, not the moment its checkbox was ticked; only a task finished without a single session keeps the tick time.
- The pomodoro picker folds finished tasks behind a "Finished (N)" toggle that expands on click; every unfinished task starts a session straight from its own row; and every task — finished or running included — opens the same right-click menu as the settings page: edit, duplicate, delete.

### 🗑️ Removals

- The interaction-count ladder is gone: the thousand level names, the ten-title pacing, the 2,000,000-keystroke ceiling and the chime that fired as the count crossed a threshold. Presses are still counted and stored, but nothing displays them any more; levels now come from the Pomodoro cultivation ladder.

### 🎨 Interface

- The Pomodoro trend chart now draws focus minutes in red and finished tasks in blue, so the two series never share a colour.
- The forehead badge and the hover panel name the realm and the layer instead of a level number, and the panel's bar measures progress through the current layer rather than towards the next level.
- The Pomodoro settings page gains a "Cultivation" tab: the realm and its growth bar, the four attributes and the power they add up to, the four gear slots beside the satchel holding the rest, the seeds and the plots they can be planted in, and the guardian with the one button that fights it.
- Cultivation moves out of the Pomodoro page's tabs into its own sidebar entry, right beneath Pomodoro; the Pomodoro page returns to its two views, the task list and the calendar.

### 🌍 Localization

- All eight catalogs: the thousand level names give way to ten realm names, alongside the cultivation copy — attributes, slots, gear labels, the garden, the breakthrough and the absence notice.
- The forehead level badge keeps its own pointer gate: it steps aside only after the pointer has rested on the cat for two seconds and returns the moment the pointer leaves, instead of riding the window's hover and idle hide fades.
- Task rows no longer clip the focus-session icons to a fixed count: past the old cap the icons wrap onto a second line instead of being truncated with an ellipsis, so the whole count stays visible and the row fills its width.
- Days that carry finished focus sessions get a light red wash in the Pomodoro calendar, so an active day is recognisable at a glance.

## 2.3.1 - 2026-10-13

### 🐛 Bug Fixes

- Fixed "Hide on mouse hover" only working in some cases while "Force mouse movement" was on.
- Fixed the settings window being unexpectedly hidden on macOS after turning off the Dock icon.
- Fixed the model window not being able to move above the menu bar on macOS.

### 🎨 Interface

- Renamed "Show Dock icon" to "Always show Dock icon".
- Improved the descriptions on the About page.

## 2.3.0 - 2026-10-12

### ✨ Features

- Model folders containing multiple levels of nested directories can now be imported.
- The Windows installer now asks which language to use.
- Added a "Model behavior → Allow overlapping motions" setting.
- Added a "Model window → Window behavior → Window mode" setting on Windows.
- Added a "Model window → Window appearance → Window background color" setting on Windows.
- Window mode and window background color can be used together, so OBS can capture the model window properly on Windows.

### 🐛 Bug Fixes

- Fixed the cat window missing from OBS's default window list on macOS.
- Fixed model windows losing their aspect ratio in some cases.
- Fixed "The selected model could not be activated" appearing when switching to some models.

### 🌍 Localization

- Added French.

## 2.2.0 - 2026-10-07

### ✨ Features

- Added a "Model window → Window behavior → Hide when idle" setting.
- Added a "Model window → Window behavior → Idle hide delay" setting.
- Added a "Model window → Window behavior → Hold a modifier key to interact" setting.
- Added an "Input & interaction → Mouse → Force mouse movement" setting.
- Added a "Model behavior → Turn off an expression when triggered again" setting.
- Added renaming support to "Shortcuts → Model behavior shortcuts".

### 🐛 Bug Fixes

- Fixed noticeable soft fringes around model texture edges.
- Fixed misplaced key images in some converted Bongo-Cat-Mver models.

### 🎨 Interface

- Renamed "App & system → Startup & desktop → Open at login" to "Run at startup".

## 2.1.1 - 2026-10-03

### 🐛 Bug Fixes

- Fixed the app crashing on macOS when opening System Settings from the permission prompt.

## 2.1.0 - 2026-10-03

### ✨ Features

- Added a "Show every held key" setting under Input & interaction → Keyboard.
- Improved the macOS Input Monitoring permission flow with a guided panel that walks you through the authorization.

### 🐛 Bug Fixes

- Fixed some BongoCat Mver models failing to import with an "invalid model package" message.
- Fixed BongoCat freezing and quitting on Windows after clicking "Report an issue".
- Fixed the menu bar icon missing on macOS after updating to a new version and restarting.
- Fixed Xbox-mode controllers going unresponsive on Windows once another window took focus.
- Fixed flickering on Windows when a model uses a gamepad mode.
- Fixed dropdown menus in Settings being too narrow to show their options.

### 🌍 Localization

- Added Korean.

## 2.0.1 - 2026-09-29

### 🐛 Bug Fixes

- Fixed some models failing to import with an "invalid model package" message.

### 🌍 Localization

Arabic, Vietnamese, Traditional Chinese, and Portuguese are now available.

## 2.0.0 - 2026-09-29

### ⚠️ Upgrade Notice

- BongoCat 2.0.0 uses a new native Rust architecture.
- Uninstall the previous version first, then download and install the new version manually.
- BongoCat is now licensed under the Apache License 2.0 instead of MIT.

### ✨ Features

- Mouse, keyboard and gamepad input can each be ignored separately, and each has a global shortcut.
- Gamepad stick and trigger dead zones can be adjusted.
- Models can switch automatically when a gamepad connects or disconnects, or you can pin the switch to a specific model.
- Model import captures a cover image automatically.
- BongoCat Mver apps can be imported and converted automatically.
- Built-in and imported models can be renamed and given a new cover.
- Random motions or expressions can be played, choosing expressions only, motions only, or both.
- Each model remembers the expression you last used on it, and restores it on the next launch or when you switch back to that model.
- Mouse tracking can also be flipped vertically.
- Logs have an adjustable level and retention, and old logs are cleaned up automatically.
- Window shortcuts and model-behaviour shortcuts have separate switches.

### 🐛 Bug Fixes

- Held keys and gamepad buttons no longer stay stuck after device, lock, sleep and permission changes.
- Switching models or resizing the window no longer leaves the model window black, transparent or flickering.

### 🗑️ Removals

- The "Key release timeout" setting is removed.
- Traditional Chinese, Portuguese and Vietnamese are no longer available.

### 💻 Support Changes

- Supported platforms are Windows 10 1903+ (x64) and macOS 12+ (Intel/Apple Silicon).
- Windows ARM runs the x64 build through emulation.
- **Windows x86, Windows ARM64 and Linux builds are not provided.**
