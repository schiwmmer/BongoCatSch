# Changelog

## Unreleased

### ⚖️ Balance

- A fight's turns now track the speeds instead of the banks: a tick pays at most one action a side, so when both speeds cleared the shared bar the banks — not the speeds — handed out the turns, and a 1.6-to-1 edge in 速度 compounded into a fourteen-to-one edge in turns (281 : 19 over three hundred rounds) while most short fights ended before the slower side swung at all. The bar a fight fills to is now the faster side's speed where that asks more than the shared pace, which puts the slower fighter back on the clock near the ratio of the speeds — the same edge now pays about 2.2 to 1 in turns, and slow fights are unchanged.
- A job's wage is a figure now, not a range: a range is something a roll opens, and a job is a wage — a number the user cannot read before taking it is not one. It now pays the **middle** of the road's own purse, a fifth over, times the grade, which is exactly the expectation the range carried (twelve hours at the fifth tier: 54 at 黄 and 291 at 神). No row on the board is a gamble any more.
- 潜质 reaches money for the first time: each whole point is a 0.11% chance a finished job comes home at **double** and a 0.01% chance at **triple**, rolled the moment it comes due. Until now 潜质 only moved a 功法's success rate and its pace. The panel prints what the last job actually paid — windfall and all — and the event log names the multiplier beside the purse, because "no crit" is the other half of that sentence.
- A missed day is now charged in "layers of the realm you are standing in": one day costs **five layers** of growth. The flat 800 it replaced was a layer and a half in 炼气 and barely half a layer by 元婴, so the further up the ladder the less the charge meant — a 炼气 layer costs 500 growth and a day now costs 2,500; a 元婴 layer costs 2,500 and a day costs 12,500. Consecutive missed days still compound at 1.2^days and scale with the realm, and the charge still stops at the realm's floor.
- The pairing bonus is gone: neither half of 战力 adds a second share of the smaller of its two attributes any more, and both are a plain weighted sum again. The four weights are scaled up by exactly the share the bonus was carrying — 根骨 19 / 力量 26 / 仙力 26 / 身法 15 — so the ladder's power curve lands where it did before the cut. The 配对 line leaves the breakdown with it, which now prints seven parts that sum exactly to the total. Opponents wear more for it: the share of an opponent's worth spent on gear goes from 150‰ to 180‰, because removing the pairing shrank a body more than it shrank a garment, which left guardians undressed and broke the tower's climb at its sixty-first floor.
- 悟心草 and 慧心草 no longer roll their own "one day in four / one day in ten"; they share the ordinary seed pool, so a shelf's seed row is one of four attributes plus the two 潜质 herbs at equal odds. Each herb now turns up exactly as often as any single seed does, and the 洗髓丹 remains the one 潜质 item rolled on its own, at one day in two hundred.
- The two 潜质 herbs pay a fixed 潜质 by grade now rather than a roll: 悟心草 is 0.7 at 地 and 慧心草 1.3 — the middle of the ranges they used to open — each multiplied by the grade's own figure and rounded. They were the only reward on the page that gambled its amount, so a herb never pays a coin-flip's worth of 潜质 and two of the same grade always pay the same.
- Every 内功's per-阶 payout is raised by about a third: 3 / 8 / 12 / 18 / 27 by grade (was 2 / 6 / 9 / 14 / 21), so a 玄阶 first 阶 pays eighty hit or evade rather than sixty.
- A harvest pays by the hour spent growing it, anchored on 0.4 points an hour at 地: the grade ladder moves from 400 / 700 / 1000 / 1500 / 2200 to 250 / 600 / 1000 / 1600 / 2500, and the growing times run 8 / 12 / 18 hours / 1 / 1.5 / 2 / 3 / 4 / 6 / 10 days. A 地阶 crop therefore pays 3 / 5 / 7 / 10 / 14 / 19 / 29 / 38 / 58 / 96 points (was 17 at the fourth tier and 235 at the tenth), and the first tier reads 1 / 2 / 3 / 5 / 8 — every tier tells all five grades apart rather than printing a row of ones.
- A seed is priced at "its points × 14 灵石": the fourth-tier 地阶 seed is 140 and its 玄阶 neighbour 84, and a cheap low-grade herb is now cheap because it pays little rather than because it is the efficient row.
- Exploration pays on return: setting out only writes down the road, and the find, the fight and the purse are all rolled when the walk is home. A walk can be called home early; a called walk finds nothing, and the return trip costs a third of the time already spent. The 锁妖塔 and 试炼 are therefore closed only while the cultivator is genuinely out, and open the moment the walk is due home.
- A failed attempt now goes into the same exponent as 潜质: success = the grade's base × 1.01^(aptitude + failures). The count belongs to the whole book — it carries past a 阶, stands until the art is finished, and every art keeps its own. Each row prints how many attempts that art has lost.
- Equipment no longer shows the "spell / martial / power" line, and `ItemView` has lost its power field with it. The four attribute cards drop their "pairs with X in the spell/martial school" note, and the power hint now says the number follows from the attributes, the arts and the gear together. Spell power and spell resist stay as combat numbers on the attribute page, on gear and in the fight report.
- A won 试炼 always pays 灵石: the courtyard used to roll one exclusive find — a herb, gear, a 功法, or nothing — and "nothing" was the biggest slice, so a clean win could pay the cultivator nothing at all and leave the log reading a bare "Proving won." Every win now pays a purse of its own, priced by realm like every other purse (above a walk, below a tower floor), named in the event log and in the fight report.
- A 功法 can now be found twice, the way a piece of gear can: the drop pool used to be "the books not on the shelf", which made every 功法 drop a guaranteed new one. The roll now runs over the whole grade, and landing on a book already on the shelf folds the second copy into it — the 阶, the banked progress and the failures it has cost all stay exactly where they were (a 5/10 stays 5/10) and the reading carries on. A 功法 learned outright is never handed over again: there is nothing left in that book to learn.
- A harvest now returns a spare seed at odds that climb with the garden: each garden rank adds 1% (ten per mille per rank), so a first-rank field pays a spare seed about one time in a hundred and a full fifteenth-rank field about one time in seven — exactly the flat rate the garden used to give. The spare rolls a random grade, the same pool a walk or the market rolls from, and it is settled the moment the crop is pulled.

### ✨ Features

- 悟心草 and 慧心草 become plantable seeds: the two 潜质 herbs go from "bought and found, then eaten on the spot" to seeds with the five grades 黄 to 神, at fixed tiers (慧心草 grows at the sixth, 悟心草 at the eighth), grown in the garden and eaten automatically when they come up. The grade is what the body pays at (a low band rounds the tenth up to its point, a high band rounds to the nearest). The market and the walk no longer feed a herb the instant it is found — both hand over the seed, planted or lost on a failed sowing.
- A walk pays more with the realm: the find, the gear, the book and the purse an exploration rolls are now anchored to the realm's own worth rather than the body's attribute-plus-gear total, so a higher realm finds more every time it sets out.
- A job in hand reads as taken: the board keeps the row but swaps its take button for a 接取中 tag, so a job already being worked cannot be read as open or taken twice. It carries across the day (the work keeps going), and coming home hands over a fresh three.
- A state line sits above the recent events, a line of its own in the page's normal ink: 在家 at home, and 外出 when the cultivator is out — naming the road or the job and the hours and minutes left before they are back. At home the cultivator can fight (破境, a 守卫, a 试炼, a tower floor), walk or work; away, every one of those waits for the road home. The 锁妖塔 and 试炼 answer a closed door with a toast rather than a countdown: the "back in Xh Ym" line is gone, and a click on a door the road, a job, or a spent day has closed is refused by the domain and answered as a toast — the button never guesses, and the state line above already says where the cultivator is.
- A walk or a job coming home plays its own figure: two soft knocks and a warm bell, a texture none of the tonal clips share, so a homecoming is not heard as a finished session or a level. The frame loop watches the state line's own answer and plays once on the away → home edge — an app opened after the return stays silent about it.
- The 打工 board's refresh button moves to the far right of the title line, with its price beside it: a re-roll now costs half of what a four-hour 黄阶 job at that tier pays, so the price is read off the board itself rather than out of a formula.
- An overwhelmingly strong cultivator sweeps the 锁妖塔: when the power is more than half again the warden's, the 闯塔 button becomes 横扫, and one click settles ten floors at once — the fights skipped because none of them was a contest, each swept floor paying its own reward, and the stretch and its counts named in the event log. One sweep spends one of the day's climbs, so the floors are the reward for being the stronger body, not a second ration.

- A 功法 book can be found twice, and the shelf counts **copies**: a second copy of a book already in hand goes beside the one being read, and the 阶, the pomodoros banked toward the next one and the failures already spent stay exactly where they were — 修炼 is the reader's, not the paper's. The market marks a book already held as 已有 and keeps its price, since what the stones buy is the spare; a 功法 learned outright shows 已习得 where the price would be and takes no stones at all. Selling takes the spare first and still refuses the last copy of a book being trained, and finishing a book leaves its spares on the shelf as a fresh unread book rather than sweeping them away.
- Every panel's rule is read beside its title now: the walk, the tower, the 试炼, the job board, 外显属性 and the market each carried an explanation that either took a row of its own or sat in a hover card behind the title — and a hover is something a user has to think of moving the pointer onto, so a rule nobody reads is a rule the page does not have. Each rule is now short enough to be one line beside its title, and the row truncates with an ellipsis rather than wrapping to a second; every panel also starts a row higher.
- A new 打工 board sits under the walk: three jobs at a time, each printed with its tier, its hours and the purse it opens between. A job's tier is the cultivator's own plus or minus one — one within reach, one that costs a longer day — and its hours run four to twenty-four, with the purse following the hours linearly because a job is a wage rather than a roll. The grade runs 黄 to 神 on the odds an art is graded on, multiplied by 1 / 1.6 / 2.4 / 3.6 / 5.4: twelve hours at the fifth tier pays 36–72 灵石 at 黄 and 194–388 at 神, against 30–60 for the same hours on the road — a fifth over, exactly. The base is a fifth over what the same hours on the road pay, which is the whole trade — the road may come home with a herb, a garment, a book or a fight, and a job is the certain half that pays only in coin.
- A job and a walk are the same hours, so they exclude each other: a cultivator out on the road, or already at work, cannot take one. A job is paid the moment it comes due, settled by the tick, so hours spent with the app shut are hours worked; coming home hands over a fresh board and writes a dated "打工 N 小时，得 X 灵石" into the event log. A board can be re-rolled for 战力阶级 × 50 灵石 — otherwise three draws at a ten-thousandth, refreshed until a 神阶 job turns up, would make the board a slot machine.
- The pomodoro countdown and the walk both survive a restart now: the countdown is in `pomodoro-tasks.json` (the instant it started running, not the seconds it had left), so an upgrade or a quit no longer ends the session — reopening charges the wall-clock time the app was closed and carries on. A paused session comes back paused and spends nothing while it is paused, and a session whose time ran out while the app was shut settles on the next launch, growth and completion count included. The walk was already stored; it now has the regression test it was missing.
- The power breakdown is complete and adds up: the panel listed only 境界 · 装备 · 仙草 · 内功 · 外功 and the five did not reach the total in the corner — the points the user had placed and the gear's outright combat numbers were missing, and the pairing bonus the two halves of 战力 pay was counted source by source instead of once over the whole body. It now prints 境界 / 已分配 / 装备 / 装备外显 / 仙草 / 内功 / 内功外显 and ends on ＝ 总战力 X, with the seven lines summing exactly to the total. 外功 is what a blow lands for, not what the body is worth, so it is printed on its own line as 外功每击 +N.
- The absence charge is in the event log: it used to be a notice in memory that the next restart forgot, which made "I missed a day and nothing happened" a reasonable report. It is now a dated line — "N 天未修炼，成长值 -X" — that a user can still read days later.
- The model window's right-click menu now follows the session: "Start Pomodoro" when there is none, "Pause" and "Abandon" while one counts, "Resume" and "Abandon" while one is paused. The countdown's own right-click offers the same abandon — a session the user no longer wants no longer has to be paused before it can be thrown away.
- The breakthrough prompt is now a banner at the very top of the page, in the largest type the page uses for a line, and it appears only when the realm is full — a save still climbing is told nothing, and the breakthrough panel itself is drawn only while a guardian is actually standing there.
- The tower is renamed 锁妖塔; the panel's rules and every line it prints follow the new name.
- A new 试炼 panel sits directly under the walk: every finished pomodoro banks one trial chance (ten at most), and a chance buys a randomised fight levelled to the cultivator. A win pays the tower's base table — a seed most often, gear sometimes, an art almost never — and a loss costs nothing but the chance.
- Walks now come in three lengths — 4, 8 and 12 hours — picked with a chip above the button. Every chance a walk rolls is multiplied by the chip's own figure (×1, ×2.1, ×3.2), and the walk holds the cultivator for exactly that length, so a day holds six short walks or two long ones for about the same total.

- The realm line is a control now: clicking it unfolds the whole ladder — the level on the thousand ("347 / 1000"), the rule that converts a realm, layer and rank into it (`(realm − 1) × 100 + layer × 10 + rank`, layers counted from 0), and all ten rungs with the current one marked.
- The attribute panel drops the two school figures that decided nothing and shows the two halves that do: 内功 power, what the inner arts are worth at the same scale as the four attributes, and 外功 power, what the best outer art adds per blow over a bare hand.
- 潜质 is rebuilt as its own fractional number — born somewhere on a −10..+10 band, capped at 1000, stored in tenths — and it now sets both how reliably and how fast an art is learned: at 0 an attempt succeeds 50 / 42 / 35 / 20 / 5% by grade and a rank costs three times the printed pomodoros, with every point multiplying the rate by 1.01 and the cost by 0.99. Two herbs are the only way up (悟心草 +0.1–1.2, 慧心草 +0.5–2.0), and every even hundred needs its own number of 洗髓丹 — the hundredth one pill, the two-hundredth two — before it can move again.
- 洗髓丹 is the rarest thing a save can find: one day in ten thousand on the shelf at 100 000 灵石, one quiet walk or morning event in ten thousand, and a fight drop that climbs from one in a hundred thousand at even odds to one in a thousand against something three times as strong.
- 功法 get steeper by grade: each 阶 of an 内功 pays 2 / 6 / 9 / 14 / 21 by grade (玄 is one and a half times what it was), an 外功 lands 1.2 / 2.03 / 2.7 / 3.6 / 4.8 times a bare hand, and discovery is far rarer at the top — 60% / 25% / 11% / 4% / one in ten thousand. Every grade also carries its own colour — 黄, 蓝, 紫, 橙, 红 — on the shelf, in the training list and on every learned row.

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
- Opponents wear gear and swing 功法: a guardian, a wanderer and a warden each roll common, fine, rare or immortal pieces for their realm and a named move they use in the fight log, and a win takes the very gear and book they carried rather than rolling fresh ones. The gear is an **extra** bonus — the body is still the naked rung, so whoever is better dressed is the harder fight — but it is capped (fifteen percent of the body's power, with a single attribute allowed to drift no more than three quarters), and the art pays for three swings with four tenths of its uplift taken out of the opponent's own hand: the same rung now has stronger and weaker guardians, and none of them is a wall.
- A walk's cooldown drops from twelve hours to ten: a day still holds two walks, but a morning walk means the afternoon has to be spent on something else first.
- The guardian's own body is trimmed (health 1.5 → 1.3, attack 1.2 → 1.15) to make room for what it wears: turning up bare-handed is harder than it was, and turning up in the four pieces of the realm is more reliable than it was.

- Every "Recent events" line now leads with the local date and time (e.g. `2026-10-26 21:03`), on the page and in the full-history popup alike: the record piles up across days, and a list that says what happened but never when reads yesterday and this morning as one entry.
- The event log stops dropping rewards: a won breakthrough writes its art, gear, herb and purse into the same line, a harvest adds the seed it gave back, and a pomodoro's ordinary find — a herb or a piece of gear — gets a line of its own. Those used to appear once in a popup or a notice and nowhere else, which left a seed sitting in the satchel with no history to explain where it came from.
- Aptitude herbs are fed on arrival: Insight and Wisdom Herbs go into the body the moment a drop, a purchase or the morning gift hands one over, so the count stays at zero and the row of items with its feed buttons is gone. A herb is kept only when the body cannot take it — the band is full, or the gate ahead has no pill — and the satchel's button feeds it then.
- Marrow Pills move into the satchel and can be spent on their own: "Use · N pills" opens the gate ahead at exactly the price that gate always charged (one for the first hundred, two for the second), carrying aptitude to that hundred itself. It is the only way to spend a pill other than crossing a gate with a herb.
- 悟心草 and 慧心草 are graded now, the way seeds are: each herb the market sells or a walk turns up carries one of the five grades, and the higher the grade the more 潜质 one dose gives — a 地阶 dose is the printed band and each grade above scales the band up. The satchel keeps them in separate piles by grade and spends the cheapest pile first, so a better herb is never burned on a band the body has nearly filled. A save from before the change folds its herbs into one 地阶 pile.

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
- A seed row on the market shelf now carries its grade beside the name — 黄 / 玄 / 地 / 天 / 神 in the grade's own colour, the same ramp the 功法 and gear wear. The name was already painted that way, but the name is also the first thing a narrow row truncates and it spells the grade out last; with a tag of its own, the grade survives a squeezed row.

- The attribute panel no longer prints the external-art per-blow line: an 外功's multiplier, cost and mastery share are already on the learned-功法 panel, and a second copy under the attributes only asked the user which number to believe.
- 潜质 no longer owns a row: the three items and their feed buttons are gone (herbs are fed on arrival, pills live in the satchel), while 潜质 itself stays as one of the derived-attribute chips.
- The satchel lists only what it holds: the 洗髓丹 row disappears entirely at zero and comes back, with its "Use · N pills" button, once one is in hand; a held herb behaves the same way.
- A harvest in the event log now says how much 造化 it banked beside the attribute points: pulling a crop feeds the body and the garden at once, and the log used to print only the first half, so the garden bar moved with nothing saying why. A save from before the field says nothing rather than printing a "+0" the pull never paid.

### 🌍 Localization

- Both catalogs: the thousand level names give way to ten realm names, alongside the cultivation copy — attributes, slots, gear labels, the garden, the breakthrough and the absence notice.
- The forehead level badge keeps its own pointer gate: it steps aside only after the pointer has rested on the cat for two seconds and returns the moment the pointer leaves, instead of riding the window's hover and idle hide fades.
- Task rows no longer clip the focus-session icons to a fixed count: past the old cap the icons wrap onto a second line instead of being truncated with an ellipsis, so the whole count stays visible and the row fills its width.
- Days that carry finished focus sessions get a light red wash in the Pomodoro calendar, so an active day is recognisable at a glance.
- The product ships Chinese and English only: the other six catalogs, their language options, the language enum values and the Windows installer languages are gone. Any Chinese tag — Traditional included — reads the Simplified catalog; every other language reads English.
- Both catalogs grow the copy the new sections need: 功法 names and grades, the market's refusals and prices, exploration's findings, the duel popup and the event log.

### 🐛 Bug Fixes

- Fixed the countdown freezing while the user typed or moved the pointer: the worker threw away every fraction of a second it could not hand over, and each keystroke or mouse move wakes it once — far more often than once a second, so every fraction was dropped and the clock stopped dead for as long as the input lasted. The fraction now carries until it completes a whole second, and the worker sleeps exactly as long as the running session has left.
- Fixed 试炼 (proving) still being playable while a job was in hand: only one place can be stood in at a time, the tower kept that gate and the proving missed its "at work" half, so a cultivator on the job could still spend a banked chance on a fight they were not in. Both of the courtyard's fights now share one gate — still on the road or still on the clock keeps it closed, and once the hours are up the door opens straight away with not one chance lost. The proving panel prints which wall holds the door, the way the exploration panel does.
- Fixed the market showing a 慧心草 as 已购 the moment a realm was broken through: the shelves re-roll when the realm changes, but a bought flag was remembered by slot alone and cleared only once a day, so an old slot landed on a new herb and read it as already bought. The market now records the realm its shelves were rolled for and clears the bought flags whenever the day or the realm changes.

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
