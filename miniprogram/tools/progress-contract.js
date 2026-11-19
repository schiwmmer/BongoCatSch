// 小程序实时进度条与内核的一致性契约。
//
//   node miniprogram/tools/progress-contract.js
//
// 小程序页面跑不了 wasm（内核在云函数里），所以 `utils/views.js` 仍要自己算这
// 两条条的千分比。这份契约把同一批样例喂给 JS 与内核导出，要求两边逐位相等：
// 改了内核公式而忘了改小程序，`just mp-check` 立刻红，而不是等玩家看见一条
// 会撒谎的条。
//
// 样例覆盖：回合边前/边/后、毫秒份数、跨多个回合、战斗整场、以及缺
// `start_unix` 的坏存档（三端统一读 0）。

'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

const views = require('../utils/views');

const WASM = path.join(__dirname, '..', 'cloudfunctions', 'game', 'kernel.wasm');
const bytes = fs.readFileSync(WASM);
const wasmModule = new WebAssembly.Module(bytes);
const instance = new WebAssembly.Instance(wasmModule, {});
const ex = instance.exports;

assert.equal(typeof ex.web_task_progress, 'function', 'kernel.wasm 要带 web_task_progress');
assert.equal(typeof ex.web_battle_progress, 'function', 'kernel.wasm 要带 web_battle_progress');

// 整秒刻度：`start_unix` 是整秒，内核按毫秒读这一回合已经走了多少。
const T0 = 1_800_000_000;
const TICKS = [1, 4, 6, 8];
const OFFSETS_MS = [0, 250, 500, 999, 1000, 1001, 3000, 8000 * 2];

let checked = 0;

function compare(name, expected, actual, detail) {
  checked += 1;
  assert.equal(
    actual,
    expected,
    `${name} 不一致：内核 ${expected} vs 小程序 ${actual}（${detail}）`,
  );
}

for (const tick of TICKS) {
  for (const offset of OFFSETS_MS) {
    const nowMs = (T0 - 1) * 1000 + offset;
    const expected = ex.web_task_progress(T0, tick, nowMs);
    const actual = views.liveProgress(
      { task: { kind: 'Woodcut', start_unix: T0, tick_seconds: tick, ticks_done: 0 } },
      nowMs,
    ).task;
    compare('任务条', expected, actual, `tick=${tick}s offset=${offset}ms`);
  }
}

for (const total of [7, 10, 30]) {
  for (const done of [0, 3, 6]) {
    for (const offset of OFFSETS_MS) {
      const nowMs = T0 * 1000 + offset;
      const expected = ex.web_battle_progress(done, total, T0, 1, nowMs);
      const actual = views.liveProgress(
        {
          battle: {
            rounds_done: done,
            rounds_total: total,
            start_unix: T0,
            tick_seconds: 1,
          },
        },
        nowMs,
      ).battle;
      compare('战斗条', expected, actual, `done=${done}/${total} offset=${offset}ms`);
    }
  }
}

// 什么都不跑：两端都不画条。
const idle = views.liveProgress({}, T0 * 1000);
assert.equal(idle.task, null);
assert.equal(idle.battle, null);
checked += 2;

// 坏存档：缺 `start_unix` 读空，不读满（内核与网页端同一条规则）。
compare('缺 start_unix', ex.web_task_progress(0, 8, T0 * 1000), views.liveProgress(
  { task: { start_unix: 0, tick_seconds: 8 } },
  T0 * 1000,
).task, 'start_unix=0');

// 越界夹满：内核不会画出一条超过 100% 的条。
assert.equal(ex.web_task_progress(T0, 8, (T0 + 60) * 1000), 1000);
checked += 1;

console.log(`mp-progress: ${checked} 条断言全绿（内核导出 vs utils/views.js 逐位相等）`);
