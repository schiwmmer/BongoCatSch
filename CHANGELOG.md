# Changelog

## Unreleased

### ✨ Features

- A Pomodoro timer, started from the "Start Pomodoro" menu entry on macOS: name a task, set how many focus sessions it needs, and a countdown floats above the cat as the session runs. A finished session plays a short chime, and a right-click on the countdown pauses or abandons it. Tasks are kept in a separate `pomodoro-tasks.json`, and a countdown does not survive a restart.
- A "Pomodoro" settings page with a task list and a month calendar: tick a task done as you finish it, and see each day's finished focus sessions laid out on a calendar. The whole feature is localized into both supported languages.
- The "Pomodoro" settings page grows a "New Task" button beside its title, so a first task can be created without hunting for the list.
- Right-clicking a task in the Pomodoro settings opens a small menu to edit it — name and expected focus sessions — or delete it, which also drops the task's running session and history.
- Each task row in the Pomodoro settings list now carries a "Start" button at its right edge, so a focus session begins straight from the list without opening the picker; it is disabled while a session is already running for that task.
- The task right-click menu gains a "Duplicate" item that opens the new-task editor pre-filled with the same name and focus-session count, so a copy is confirmed before it is saved as a separate task.
- The interaction-count ladder is replaced by a cultivation ladder: ten realms from Qi Refining to Immortal, a hundred layers each and a thousand levels in all, every layer bought with growth rather than keystrokes. A finished focus session pays five hundred growth, every further session the same day pays a tenth more than the last, and the realm scales the whole day's charge.
- Growth buys four attributes — constitution, strength, mana and agility — that climb with the level and jump by a whole realm's worth at each breakthrough, plus the two power figures every fight reads: spell power, fed by constitution and mana, and martial power, fed by strength and agility.
- A cultivator carries a weapon, a robe, trousers and boots, each dropped at one of four rarities; the piece in a slot adds its attributes to the wearer's, and a better piece simply replaces the one it displaces.
- A herb garden of four plots: plant a seed of one attribute and tier, wait half an hour per tier, and harvest a crop whose power raises that attribute for good.
- Every finished focus session rolls a low chance of one drop — six percent a piece of gear, five percent a seed, never both at once.
- Breaking into the next realm needs a guardian beaten in a fight the growth alone cannot win: the guardian is a fifth stronger than a bare cultivator at the same realm's cap, so gear and herbs are what carry it. A loss is recorded and leaves the realm where it was.
- A day without a finished focus session costs eight hundred growth, every further consecutive missed day charges a fifth more, and the realm scales the charge, so a lapse is answered without ever undoing a breakthrough.
- A task's completion date is now the moment its last focus session finished, not the moment its checkbox was ticked; only a task finished without a single session keeps the tick time.
- The pomodoro picker folds finished tasks behind a "Finished (N)" toggle that expands on click; every unfinished task starts a session straight from its own row; and every task — finished or running included — opens the same right-click menu as the settings page: edit, duplicate, delete.
- Every level now pays a free attribute point: the level keeps raising all four attributes on its own, and on top of that one point per level is placed by hand, on whichever attribute the user picks, into the same power figure. Points are paid against the highest level ever reached, so an absence penalty lowers the ladder without taking back a point already earned.
- A harvest can give a seed back: about one picking in seven returns a seed of the same attribute, and a third of those come back one tier higher. The garden still has to be fed by sessions, but it no longer runs dry.
- A shelf of five hundred 功法 in five grades from 黄 to 神: an inner art pays each of its ranks into the four attributes, an outer art joins a fight and multiplies its blows, and taking one to 圆满 pays a further bonus on top of both. Choose one to train and every finished pomodoro advances it a step, each rank asking more sessions than the last — the first a single one, the last of a 神 art twenty-nine — while a failed attempt only ever eats the session it failed on.
- A 功法 row reads as a single line — name and grade on the left, the exact bonus on the right — and a trained 功法 can be sold for 灵石: the tome goes to the market, the knowledge and everything it pays stay exactly as they were, and the market buys a given tome only once. Equipped gear is never for sale, and neither is the 功法 being trained.
- Spirit stones are the one currency — twelve from every finished session, the rest from selling what the satchel no longer needs. The market shelf refreshes daily and is rolled pure: the same realm on the same day always sees the same five to ten pieces, gear and 功法 and seeds alike, priced off the same numbers the drops use, and everything sells back for half of what it cost.
- Breaking into the next realm is a real fight now: an action bar decides who strikes when, a blow adds whatever spell power gets through the defender's resistance to the attacker's own, and the hit chance falls off as the two sides drift apart — even footing lands four blows in five, a two-to-one edge one in two. Thirty blows settle it either way. Winning advances the realm with a 功法 and a further roll at gear and a seed; losing costs twelve hours and a small chance at one equipped piece.
- Exploration opens once every twelve hours: a seed, a piece of gear, a 功法, or a wanderer to fight at four fifths of a bare cultivator's strength — the same engine as a breakthrough, and winning that fight rolls again. A third of the time a handful of stones comes back as well.
- A trial tower sits under the walk: five floors a day, starting at the first and climbing one at a time, its warden the bare cultivator of the level that floor stands for — floor one is a level-one cultivator — so a deeper floor is both harder and worth more. A cleared floor may pay a seed, gear or a 功法; a loss costs the attempt and never the ground already held. The tower is climbed at home: while a walk is still out the door is shut, which makes the tower and the walk one choice rather than two.
- Every fight and every walk now pays something: spirit stones are the floor under all four sources, and the four differ by what they emphasise — a quiet walk is about herbs, a wanderer about gear and money, a guardian about the 功法 it guards, a trial floor about drilling. The four purses run 3–8 / 5–14 / 10–24 / 2–6, and a walk that comes home never comes back empty.
- The first launch of a day — or midnight on a machine left running — opens a "morning event": about 28% someone attacks (a real encounter, and winning it pays), about 12% the day simply hands something over (stones, a seed or gear; deliberately rarer than the ambush), and the remaining 60% a coin or two in the dust. All three land in the event log, and an ambush replays its fight.
- Spirit stones are now priced by difficulty: every purse is written as "what it pays in the first realm" and multiplied by the realm it is paid in — a walk 6–10, a cleared floor 12–18, a wanderer or a morning ambush 20–34, a guardian 38–66 (first-realm figures; doubled by 筑基 and on up), and a session from 12 to 16 scaled the same way. A fight always pays more than a walk, and that ordering is a compile-time assertion.
- The cultivation page keeps an event log of the last sixteen things that happened — sessions, walks, training, breakthroughs, buys, sales and harvests — right above the attributes.
- Growth reads as what the cultivator has and what the next layer still asks for, in compact numbers, and the levels inside a layer are called 阶 rather than levels.
- Every derived figure on the cultivation page carries the formula that computes it, naming the attributes it reads the way the page names them everywhere else.
- The growth curve is retuned so a realm is a stretch worth remembering: filling the first realm's layer takes about six days of sessions instead of one, and the whole ladder runs about two years. An existing save keeps its level, its free points, its gear and its 功法 — the curve is re-labeled once, underneath them.
- The cultivation page's event log keeps the last five events on the page and hands the rest to a popup: one click opens the full history the save still remembers, scrollable, with the backdrop or the close button closing it.
- The garden sells plots: the four beds it starts with are free, and five more can be bought with 灵石, each dearer than the last, up to nine in all, three to a row.
- Seeds of the third tier take four hours to mature and the fourth twelve, and every tier past that triples again; the wait reads in minutes, hours or days, whichever fits.
- A fight reaches for one of the three strongest 外功 at random, and a book that has not been trained to its first 阶 stays on the shelf; an 外功's 内力 price has doubled.
- A harvest no longer says only that a crop was pulled: the line names the herb, the attribute it raised and how many points it paid. A seed keeps its name from the shelf to the plot, so the garden shows which herb is growing in it.
- A seed can be inspected: click its name on the market shelf, in the satchel or in a plot and it says which attribute it will raise, how much it pays and how long it takes to mature; the shelf and the satchel also say what a sale would bring.
- The realm name's hover card lists the whole ladder: all ten realms in climbing order, with the one the save stands in marked in red and a caret, so "元婴 · 3 层" finally says how far from the top that is.
- The market marks two rows a day as specials at 60–80% of the list price. Which two, and how deep the cut, come from the day and the realm, so reopening the page shows the same shelf; each row says the fraction it is cut to and the price it was cut from, and the purchase charges exactly that.
- Opponents wear gear and swing 功法: a guardian, a wanderer and a warden each roll common, fine, rare or immortal pieces for their realm and a named move they use in the fight log, and a win takes the very gear and book they carried rather than rolling fresh ones. Their strength is held where it was — the gear is a slice of the opponent's own attribute budget and the art's three bursts are paid for out of its own attack, so the power the panel promises is still the power the fight is worth.
- A walk's cooldown drops from twelve hours to ten: a day still holds two walks, but a morning walk means the afternoon has to be spent on something else first.

### 🗑️ Removals

- The interaction-count ladder is gone: the thousand level names, the ten-title pacing, the 2,000,000-keystroke ceiling and the chime that fired as the count crossed a threshold. Presses are still counted and stored, but nothing displays them any more; levels now come from the Pomodoro cultivation ladder.
- A breakthrough no longer rolls an extra piece of gear on top of its trophy: if the drop is the piece the guardian wore, a second piece conjured out of nowhere has no business in the same line.

### 🎨 Interface

- The Pomodoro trend chart now draws focus minutes in red and finished tasks in blue, so the two series never share a colour.
- The forehead badge and the hover panel name the realm and the layer instead of a level number, and the panel's bar measures progress through the current layer rather than towards the next level.
- The Pomodoro settings page gains a "Cultivation" tab: the realm and its growth bar, the four attributes and the power they add up to, the four gear slots beside the satchel holding the rest, the seeds and the plots they can be planted in, and the guardian with the one button that fights it.
- Cultivation moves out of the Pomodoro page's tabs into its own sidebar entry, right beneath Pomodoro; the Pomodoro page returns to its two views, the task list and the calendar.
- The hover panel keeps the two pomodoro counts on a line of their own: how many focus sessions have ever finished, and how many are still ahead — the running task's remainder, or every open task's when no countdown is running. Deleting a task later does not take its finished sessions out of the total.
- The cultivation page grows the sections the ladder now feeds: the 功法 being trained and the shelf it sits on, the market's daily wares, the exploration button with its cooldown, and the event log above the attributes.
- A duel — a breakthrough's guardian or an exploration's wanderer — opens a popup with both fighters' health bars and their attributes side by side, the verdict and the spoils only at the end, and a log that jumps to the last blow when clicked.
- The breakthrough button is gated by its cooldown rather than by the power comparison, and while it cools it says how long is left.
- Market rows keep to a name and a price: tap the name for the piece's detail, tap the price to buy, and every refusal — stones short, already bought — arrives as one toast whatever the reason.
- Free points can be placed ten at a time once ten are banked, beside the single one, and the pomodoro picker's right-click menu gains a jump straight to the cultivation page.
- The sticky notices under the next-pomodoro line no longer repeat a walk: what a walk found is in the event log alone, so one event is printed once.
- The sticky notices no longer repeat training and breakthroughs either: both belong to the event log, whose breakthrough line now names the realm it opened, and the notices keep only what the log cannot carry — a charge for missed days, gear lost to a defeat, and a seed the harvest gave back.
- A 功法 on the shelf says where its training stands under its name: the 阶 the grade has, what the next one costs in pomodoros, and how far into it the book already is. A finished one says it stands complete.
- A 功法 trained to its first 阶 shows in the learned panel as far as it has gone: the 阶 it stands at beside the name, what it has paid for so far, and no mastery while any is missing.
- A purchase in the event log says what it cost, and a bought seed says which tier it is.
- The learned 功法 panel keeps the three most recently trained and folds the rest behind one click, so it answers "what am I working on" rather than listing every shelf.
- The floating countdown is white now instead of following the system appearance, and keeps its dark shadow so it still reads on a light wallpaper.
- Every derived number's hover card gains an even-spread line: what the same four attributes would buy spread evenly, and how far the current build is from it (`+`, `-` or `±0`). The 潜质 card now explains what 潜质 actually decides — how often 功法 training fails — instead of borrowing the generic sentence.
- Counts in the millions are printed to two decimals (`1.23M`) while thousands keep one (`54.7K`): a tenth of a million is a hundred thousand, so one decimal printed figures tens of thousands apart as the same number.

### 🌍 Localization

- Both catalogs: the thousand level names give way to ten realm names, alongside the cultivation copy — attributes, slots, gear labels, the garden, the breakthrough and the absence notice.
- The forehead level badge keeps its own pointer gate: it steps aside only after the pointer has rested on the cat for two seconds and returns the moment the pointer leaves, instead of riding the window's hover and idle hide fades.
- Task rows no longer clip the focus-session icons to a fixed count: past the old cap the icons wrap onto a second line instead of being truncated with an ellipsis, so the whole count stays visible and the row fills its width.
- Days that carry finished focus sessions get a light red wash in the Pomodoro calendar, so an active day is recognisable at a glance.
- The product ships Chinese and English only: the other six catalogs, their language options, the language enum values and the Windows installer languages are gone. Any Chinese tag — Traditional included — reads the Simplified catalog; every other language reads English.
- Both catalogs grow the copy the new sections need: 功法 names and grades, the market's refusals and prices, exploration's findings, the duel popup and the event log.

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
