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

import { handleCultivationAction, renderCultivation } from '../cultivate.js';

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
function ctxFor(snap, nowMs, tier = 'common') {
  const c = snap.cultivation;
  return {
    tier,
    send() {},
    toast() {},
    setTier() {},
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

/* ---------- 空转 ---------- */

boot();
const idle = snapshot(T0);
const idleCard = renderCultivation('workshop', idle, ctxFor(idle, T0 * 1000));

check('空存档没有进行中的任务', () => {
  assert.equal(idle.cultivation.task, null);
  assert.equal(idle.cultivation.seclusion, null);
});

check('空转的任务卡画三个开始钮、没有停止钮', () => {
  assert.match(idleCard, /data-act="cult-meditate"/);
  assert.match(idleCard, /data-act="cult-woodcut"/);
  assert.match(idleCard, /data-act="cult-stonecut"/);
  assert.doesNotMatch(idleCard, /data-act="cult-stop-task"/);
  assert.match(idleCard, /当前没有进行中的任务/);
});

check('空转没有实时条', () => {
  assert.doesNotMatch(idleCard, /id="live-task-bar"/);
  assert.doesNotMatch(idleCard, /id="live-battle-bar"/);
});

check('采集点阶按采集等级门禁（Lv0 全锁、Lv1 只解凡）', () => {
  const locked = renderCultivation('workshop', idle, ctxFor(idle, T0 * 1000));
  assert.match(locked, /data-tier="spirit" disabled>灵 Lv10/);
  assert.match(locked, /data-tier="divine" disabled>神 Lv20/);
  assert.match(locked, /data-tier="chaos" disabled>混沌 Lv30/);
});

/* ---------- 开始 → 每回合发奖 ---------- */

check('开始伐木后一回合恰好发 2 木与 100 经验', () => {
  assert.equal(command({ op: 'cult_start_task', kind: 'woodcut', site: 'forest', tier: 'common' }, T0), 0);
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
  assert.match(card, /data-act="cult-meditate" disabled/);
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

/* ---------- 升级后阶解锁 ---------- */

check('伐木到 Lv1 后凡解锁、灵仍锁在 Lv10', () => {
  command({ op: 'cult_start_task', kind: 'woodcut', site: 'forest', tier: 'common' }, T0 + 24);
  // 30 回合 × 100 经验 = 3000，正好到 Lv1（`skill_xp_total(1) = 3000`）。
  tick(T0 + 24 + 30 * 8, 30 * 8_000);
  const snap = snapshot(T0 + 24 + 30 * 8);
  assert.equal(snap.cultivation.resources.woodcut.level, 1);
  const card = renderCultivation('workshop', snap, ctxFor(snap, (T0 + 24 + 30 * 8) * 1000));
  assert.match(card, /data-tier="common" >凡</, '凡 在 Lv1 解禁');
  assert.match(card, /data-tier="spirit" disabled>灵 Lv10/);
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

check('六条 cult-* 实时命令映射到内核 op', () => {
  const sent = [];
  const ctx = { tier: 'spirit', send: (payload) => sent.push(payload), setTier() {}, live() {} };
  const acts = [
    ['cult-meditate', { op: 'cult_start_task', kind: 'meditate' }],
    ['cult-woodcut', { op: 'cult_start_task', kind: 'woodcut', site: 'forest', tier: 'spirit' }],
    ['cult-stonecut', { op: 'cult_start_task', kind: 'stonecut', site: 'mountain', tier: 'spirit' }],
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
});

check('采集点阶只改页面状态、不发命令', () => {
  const sent = [];
  let tier = 'common';
  const ctx = { send: (p) => sent.push(p), setTier: (next) => (tier = next), live() {} };
  assert.equal(handleCultivationAction({ act: 'cult-tier', tier: 'chaos' }, ctx), true);
  assert.equal(sent.length, 0, '选阶不该发命令');
  assert.equal(tier, 'chaos');
});

check('八个页签都渲染成字符串、不抛异常', () => {
  const snap = snapshot(T0);
  for (const tab of [
    'status',
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
