// 网页端常驻门禁：真 wasm 内核 + 真视图层，不需要浏览器、不需要云会话。
//
//   node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON web/tools/selftest.mjs
//
// 覆盖的是网页端最容易和另外两端漂移的那几层：实时任务卡的开始/停止入口、
// 采集点阶的等级门禁、两条实时进度条的读数与宽度、六条 `cult-*` 命令的映射。
// 内核是 `just web` 刚编出来的那份 `web/bongocat_pomodoro_web.wasm`，所以这里
// 断言的数字与 App、小程序读的是同一份 Rust 代码。
//
// 之前这套断言是临时脚本，跑完即删（`docs/0103` §9.1 记着这件事）；现在它常驻，
// 因为「三端一致」不能靠每次改完手工核对。

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { eventLine, handleCultivationAction, noticeStrip, quantity, renderCultivation } from '../cultivate.js';

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, '..', 'bongocat_pomodoro_web.wasm');

/* ---------- 真实内核 ---------- */

const bytes = readFileSync(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, {});
const ex = instance.exports;
assert.equal(typeof ex.web_task_progress, 'function', '内核要导出 web_task_progress');
assert.equal(typeof ex.web_battle_progress, 'function', '内核要导出 web_battle_progress');

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const memory = () => new Uint8Array(ex.memory.buffer);

function writeInput(text) {
  const encoded = encoder.encode(text);
  const pointer = ex.web_input_ptr();
  memory().set(encoded, pointer);
  return encoded.length;
}

function readOutput() {
  const pointer = ex.web_output_ptr();
  const length = ex.web_output_len();
  return decoder.decode(memory().slice(pointer, pointer + length));
}

// 一个整秒刻度：`start_unix` 是整秒，内核按毫秒读这一回合已经走了多少。
const T0 = 1_800_000_000;

function boot(now = T0) {
  ex.web_boot(0, 0, now, 7);
}

function command(payload, now) {
  return ex.web_command(writeInput(JSON.stringify(payload)), now);
}

function tick(now, elapsedMs) {
  return ex.web_tick(now, elapsedMs);
}

function snapshot(now) {
  ex.web_render(now);
  return JSON.parse(readOutput());
}

// 视图上下文：`live` 与 `app.js` 的 `cultivationCtx().live` 同形——问内核要两条
// 条的读数，页面不自己算。
function ctxFor(snap, nowMs) {
  const c = snap.cultivation;
  return {
    send() {},
    toast() {},
    live: () => ({
      task: c.task
        ? ex.web_task_progress(c.task.start_unix, c.task.tick_seconds, nowMs)
        : null,
      battle: c.battle
        ? ex.web_battle_progress(
            c.battle.rounds_done,
            c.battle.rounds_total,
            c.battle.start_unix,
            c.battle.tick_seconds,
            nowMs,
          )
        : null,
    }),
  };
}

const cases = [];
function check(name, fn) {
  cases.push([name, fn]);
}

/* ---------- 数量口径 ---------- */

// 资源与采集经验的显示口径，与内核 `quantity_number` 同一把尺子。
check('资源与经验数量按 千位符 → K → M 收短', () => {
  assert.equal(quantity(999), '999');
  assert.equal(quantity(1234), '1,234');
  assert.equal(quantity(9999), '9,999');
  assert.equal(quantity(10000), '10K');
  assert.equal(quantity(12345), '12.3K');
  assert.equal(quantity(999999), '1M');
  assert.equal(quantity(1234567), '1.23M');
});

/* ---------- 空转 ---------- */

boot();
const idle = snapshot(T0);
const idleCard = renderCultivation('workshop', idle, ctxFor(idle, T0 * 1000));

check('空存档没有进行中的任务', () => {
  assert.equal(idle.cultivation.task, null);
  assert.equal(idle.cultivation.seclusion, null);
});

check('空转的任务卡画两个开始钮、没有停止钮', () => {
  assert.match(idleCard, /data-act="cult-woodcut"/);
  assert.match(idleCard, /data-act="cult-stonecut"/);
  assert.doesNotMatch(idleCard, /data-act="cult-meditate"/, '吐纳已下线');
  assert.doesNotMatch(idleCard, /data-act="cult-stop-task"/);
  assert.match(idleCard, /当前没有进行中的任务/);
});

check('空转没有实时条', () => {
  assert.doesNotMatch(idleCard, /id="live-task-bar"/);
  assert.doesNotMatch(idleCard, /id="live-battle-bar"/);
});

check('采集深度没有可点的阶，只按等级报出所在的地名与倍率', () => {
  const card = renderCultivation('workshop', idle, ctxFor(idle, T0 * 1000));
  assert.doesNotMatch(card, /data-act="cult-tier"/, '不再有阶的选择器');
  assert.match(card, /伐木 凡木林 Ⅰ · ×1\.0/, '新存档站在最浅的一层');
  assert.match(card, /采石 凡石山 Ⅰ · ×1\.0/, '两个行业各读各的等级');
});

check('探索时长、倍率与区域规模读内核 rules', () => {
  const page = renderCultivation('outing', idle, ctxFor(idle, T0 * 1000));
  assert.match(page, /4 小时 · ×1\.0/, '第一档来自 ExplorationHours');
  assert.match(page, /12 小时 · ×3\.2/, '第三档倍率也来自内核');
  assert.match(page, /区域 · 10 区 10 层/, '区域数与层数来自 Region::ALL / LEVELS_PER_REGION');
  assert.match(page, /当前层 1\/10/, '当前层上限不再是写死的 /10');
});

check('闭关回合与分钟读内核 rules', () => {
  const inSeclusion = { ...idle, cultivation: { ...idle.cultivation, seclusion: { ticks_done: 10 } } };
  const page = renderCultivation('workshop', inSeclusion, ctxFor(inSeclusion, T0 * 1000));
  assert.match(page, /坐下 25 分钟/, 'SECLUSION_TICKS × 每回合秒数');
  assert.match(page, /回合 10\/250/, '回合总数来自快照');
});

/* ---------- 开始 → 每回合发奖 ---------- */

check('开始伐木后一回合恰好发 2 木与 100 经验', () => {
  assert.equal(command({ op: 'cult_start_task', kind: 'woodcut', site: 'forest' }, T0), 0);
  tick(T0 + 8, 8_000);
  const snap = snapshot(T0 + 8);
  const c = snap.cultivation;
  assert.equal(c.task.kind, 'Woodcut');
  assert.equal(c.task.site, 'Forest');
  assert.equal(c.task.tier, 'Common');
  assert.equal(c.task.ticks_done, 1, '8 秒一整回合');
  assert.equal(c.task.tick_seconds, 8);
  // §5.2：凡木林 L1、无工坊、炼气 → 2 木/回合；道场 0 → 100 经验/回合。
  assert.equal(c.resources.bag.wood, 2);
  assert.equal(c.resources.woodcut.xp, 100);
});

check('进行中的任务卡画停止钮、开始钮整排置灰', () => {
  const snap = snapshot(T0 + 8);
  const card = renderCultivation('workshop', snap, ctxFor(snap, (T0 + 8) * 1000));
  assert.match(card, /data-act="cult-stop-task"/);
  assert.doesNotMatch(card, /data-act="cult-meditate"/, '吐纳已下线');
  assert.match(card, /data-act="cult-woodcut" disabled/);
  assert.match(card, /data-act="cult-stonecut" disabled/);
  assert.match(card, /已进行/);
});

check('任务条：回合边读 0、半回合读 500、下一回合边读 1000', () => {
  const c = snapshot(T0 + 8).cultivation;
  const task = c.task;
  assert.equal(ex.web_task_progress(task.start_unix, task.tick_seconds, (T0 + 8) * 1000), 0);
  assert.equal(ex.web_task_progress(task.start_unix, task.tick_seconds, (T0 + 12) * 1000), 500);
  assert.equal(
    ex.web_task_progress(task.start_unix, task.tick_seconds, (T0 + 16) * 1000),
    1000,
    '走到下一回合边正好满，不越界',
  );
});

check('页面画出来的条宽就是内核读数（50% 不是页面自己算的）', () => {
  const snap = snapshot(T0 + 8);
  const card = renderCultivation('workshop', snap, ctxFor(snap, (T0 + 12) * 1000));
  assert.match(card, /id="live-task-bar" style="width:50%"/);
});

check('缺 start_unix 的存档读空而不是读满', () => {
  assert.equal(ex.web_task_progress(0, 8, (T0 + 8) * 1000), 0);
});

/* ---------- 停止 ---------- */

check('停止时结清已走满的回合，停手之后不再有收入', () => {
  assert.equal(command({ op: 'cult_stop_task' }, T0 + 16), 0);
  const snap = snapshot(T0 + 16);
  assert.equal(snap.cultivation.task, null);
  // 8→16 秒这一回合在「停止」那一刻已经走满，停止要把它结清（4 = 2 回合 × 2 木）。
  assert.equal(snap.cultivation.resources.bag.wood, 4);
  tick(T0 + 24, 8_000);
  const later = snapshot(T0 + 24);
  assert.equal(later.cultivation.resources.bag.wood, 4, '停手之后不再有回合收入');
});

/* ---------- 升级后深度自己加深 ---------- */

check('升级不会换地方：Lv1 仍站凡木林，深度随等级而不是随点击', () => {
  command({ op: 'cult_start_task', kind: 'woodcut', site: 'forest' }, T0 + 24);
  // 30 回合 × 100 经验 = 3000，过 Lv1 的门（`skill_xp_total(1) = 2000`）。回合
  // 要一个一个地走：一次 240 秒的跨度会被内核当成离开（离线半额），这也是页面
  // 真实的样子——它每秒都问一次。
  for (let n = 1; n <= 30; n += 1) tick(T0 + 24 + n * 8, 8_000);
  const snap = snapshot(T0 + 24 + 30 * 8);
  assert.equal(snap.cultivation.resources.woodcut.level, 1);
  const card = renderCultivation('workshop', snap, ctxFor(snap, (T0 + 24 + 30 * 8) * 1000));
  assert.match(card, /伐木 凡木林 Ⅰ · ×1\.0/, 'Lv1 还在凡木林');
  assert.doesNotMatch(card, /data-act="cult-tier"/, '没有可点的阶可以误点');
  command({ op: 'cult_stop_task' }, T0 + 24 + 30 * 8);
});

/* ---------- 战斗条 ---------- */

check('战斗条读整场：已完成回合 + 本回合份数', () => {
  assert.equal(ex.web_battle_progress(0, 10, T0, 1, T0 * 1000), 0);
  assert.equal(ex.web_battle_progress(0, 10, T0, 1, (T0 + 0.5) * 1000), 50);
  assert.equal(ex.web_battle_progress(5, 10, T0, 1, (T0 + 0.5) * 1000), 550);
  assert.equal(ex.web_battle_progress(40, 10, T0, 1, (T0 + 5) * 1000), 1000, '越界夹到满');
});

/* ---------- 六条命令与八页渲染 ---------- */

check('五条 cult-* 实时命令映射到内核 op', () => {
  const sent = [];
  const ctx = { send: (payload) => sent.push(payload), live() {} };
  const acts = [
    ['cult-woodcut', { op: 'cult_start_task', kind: 'woodcut', site: 'forest' }],
    ['cult-stonecut', { op: 'cult_start_task', kind: 'stonecut', site: 'mountain' }],
    ['cult-stop-task', { op: 'cult_stop_task' }],
    ['cult-start-seclusion', { op: 'cult_start_seclusion' }],
    ['cult-stop-seclusion', { op: 'cult_stop_seclusion' }],
  ];
  for (const [act, expected] of acts) {
    sent.length = 0;
    assert.equal(handleCultivationAction({ act }, ctx), true, `${act} 要被处理`);
    assert.deepEqual(sent[0], expected, `${act} 的命令体`);
  }
  assert.equal(handleCultivationAction({ act: 'cult-nope' }, ctx), false, '未知动作不吞');
  sent.length = 0;
  assert.equal(
    handleCultivationAction({ act: 'cult-meditate' }, ctx),
    false,
    '吐纳已下线，旧动作不再有命令'
  );
  assert.deepEqual(sent, [], '旧动作不发出任何命令');
});

check('重铸动作映射到内核 op', () => {
  const sent = [];
  const ctx = { send: (payload) => sent.push(payload), live() {} };
  assert.equal(
    handleCultivationAction({ act: 'cult-reforge', id: 44 }, ctx),
    true,
    '重铸按钮要被处理',
  );
  assert.deepEqual(sent[0], { op: 'reforge_equipment', item_id: 44 });
});

check('没有「选阶」这个动作了', () => {
  const sent = [];
  const ctx = { send: (p) => sent.push(p), live() {} };
  assert.equal(
    handleCultivationAction({ act: 'cult-tier', tier: 'chaos' }, ctx),
    false,
    '页面不再有阶的选择器',
  );
  assert.equal(sent.length, 0);
});

check('九个页签都渲染成字符串、不抛异常', () => {
  const snap = snapshot(T0);
  for (const tab of [
    'status',
    'record',
    'gear',
    'arts',
    'garden',
    'market',
    'outing',
    'tower',
    'workshop',
  ]) {
    const html = renderCultivation(tab, snap, ctxFor(snap, T0 * 1000));
    assert.equal(typeof html, 'string', `${tab} 页要有 HTML`);
    assert.ok(html.length > 0, `${tab} 页不能是空的`);
  }
});

/* ---------- 数学只有一份 ---------- */

check('页面源码里不再有进度数学的第二份', () => {
  const source = readFileSync(join(here, '..', 'cultivate.js'), 'utf8');
  assert.doesNotMatch(source, /function tickPermille/, '进度数学要留在内核');
  assert.doesNotMatch(source, /Math\.floor\(\(elapsed \* 1000\) \/ tick\)/);
});

/* ---------- 记录与通知（docs/0106） ---------- */

check('记录页把事件画成带时间戳的行', () => {
  // 事件日志只在真番茄钟结算时写；这里造一条同一 schema 的记录，验的是页面怎么读它。
  const snap = JSON.parse(JSON.stringify(idle));
  snap.cultivation.at_home = true;
  snap.cultivation.events = [
    { at_unix: T0, detail: { Session: { growth: 5 } } },
    { at_unix: T0 + 60, detail: { Walk: { outcome: 'Nothing', gain: 0 } } },
  ];
  const card = renderCultivation('record', snap, ctxFor(snap, T0 * 1000));
  assert.match(card, /最近事件/);
  assert.match(card, /完成修炼，成长 +/, '完成过的番茄钟要留下一条记录');
  assert.match(card, /\d{4}-\d{2}-\d{2} \d{2}:\d{2}/, '事件行带时间戳');
});

check('事件行按 App 的模板读，不把字段摊给用户', () => {
  assert.equal(
    eventLine({ Walk: { outcome: 'Seed', seed: '幽菊', stones: 300, gain: 12, hint: 'cultivation.record.walk_hint_steady' } }),
    '外出探索：寻得仙草 · 仙草「幽菊」 · 灵石「300」 · 探索 +1.2% · 稳步推进',
  );
  assert.equal(
    eventLine({ TrialSweep: { from_floor: 12, floors: 10, seeds: 2, gear: 1, arts: 0, stones: 500 } }),
    '横扫第 12~21 层 · 仙草 ×2 · 装备 ×1 · 灵石「500」',
  );
  assert.equal(
    eventLine({ Star: { name: '青锋', star: 3, success: false, cost: { stones: 200, resources: { iron: 3 } } } }),
    '青锋 升星失败，降至 3 星（花 200 灵石、铁×3）',
  );
  assert.equal(
    eventLine({
      Reforge: {
        name: '青锋',
        from: 'mana',
        to: 'agility',
        from_points: 40,
        to_points: 69,
        cost: { stones: 1_800, resources: { wood: 180, stone: 180, iron: 70, coal: 70 } },
      },
    }),
    '青锋 重铸：灵力 40 → 身法 69（花 1,800 灵石、木×180 · 石×180 · 铁×70 · 煤×70）',
  );
});

check('装备行给两线装备一个重铸钮，单线装备置灰', () => {
  const snap = snapshot(T0);
  const base = {
    slot: 'weapon',
    tier: 5,
    rarity: 'rare',
    attributes: { root: 0, strength: 120, mana: 40, agility: 0 },
    bonus: { attack: 0 },
    equipped: false,
    star: 0,
    effect: null,
    star_cost: null,
    name: '试剑',
    set: null,
    sell_price: 700,
  };
  snap.cultivation.inventory = [
    {
      ...base,
      id: 44,
      reforge_cost: {
        stones: 1_800,
        resources: { wood: 180, stone: 180, iron: 70, coal: 70 },
      },
    },
    { ...base, id: 45, reforge_cost: null },
  ];
  const card = renderCultivation('gear', snap, ctxFor(snap, T0 * 1000));
  assert.match(card, /data-act="cult-reforge" data-id="44"[^>]*title="重铸需要 1,800 灵石/, '两线装备显示重铸');
  assert.match(card, /data-act="cult-reforge" data-id="45"[^>]*disabled/, '单线装备的重铸置灰');
});

check('通知条读得出最近一次重铸', () => {
  const snap = snapshot(T0);
  snap.cultivation.last_reforge = {
    item_id: 44,
    name: '青锋',
    from: 'mana',
    to: 'agility',
    from_points: 40,
    to_points: 69,
    cost: { stones: 1_800, resources: {} },
  };
  const strip = noticeStrip(snap.cultivation);
  assert.match(strip, /重铸 青锋：灵力 40 → 身法 69/, '重铸结果要出现在通知条');
});

check('通知条画最近一次动作的结果', () => {
  const snap = snapshot(T0 + 16);
  const strip = noticeStrip(snap.cultivation);
  assert.match(strip, /class="notices"/);
  assert.match(strip, /森林·伐木：获得 木×/, '采集完一回合要有结果行');
});

check('仙草园给出一键收获/一键种植与成活率口径', () => {
  const snap = JSON.parse(JSON.stringify(idle));
  snap.cultivation.seeds = [
    { attribute: 'root', herb: null, tier: 1, grade: 'Yellow', count: 2, name: '青芝', sell_price: 30, plant_permille: 850 },
  ];
  const card = renderCultivation('garden', snap, ctxFor(snap, T0 * 1000));
  assert.match(card, /data-act="cult-harvest-all"/);
  assert.match(card, /data-act="cult-plant-all"/);
  assert.match(card, /成活率/, '种子上要读内核给的 plant_permille');
});

/* ---------- run ---------- */

let passed = 0;
const failures = [];
for (const [name, fn] of cases) {
  try {
    fn();
    passed += 1;
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures.push([name, error]);
    console.log(`  FAIL ${name}`);
  }
}
if (failures.length) {
  console.log('');
  for (const [name, error] of failures) {
    console.log(`--- ${name}`);
    console.log(error.message);
  }
  console.log(`\nweb-check: ${passed}/${cases.length} passed, ${failures.length} failed`);
  process.exit(1);
}
console.log(`\nweb-check: ${passed}/${cases.length} passed`);
