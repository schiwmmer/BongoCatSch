'use strict';

// 页面数据层的冒烟测试：用真实内核快照跑一遍所有 view 构建器。
//
// 抓的是最容易漏的三类问题：字段名拼错（取到 undefined）、除零（NaN）、
// 格式化函数漏分支（拼出 "undefined"）。这些在真机上表现成「页面一片空白」，
// 但在 Node 里能提前看见。
//
//   node miniprogram/tools/views-smoke.js

const assert = require('assert');
const fs = require('fs');
const path = require('path');

const core = require('../cloudfunctions/game/core');
const views = require('../utils/views');
const labels = require('../utils/labels');

const wasm = fs.readFileSync(path.join(__dirname, '..', 'cloudfunctions', 'game', 'kernel.wasm'));
const run = core.createRunner(wasm);

let now = 1_800_000_000;
let result = run({ document: '', commands: [], nowUnix: now });
let snapshot = result.snapshot;
let calendar = result.calendar;

const step = (commands, seconds) => {
  now += seconds || 0;
  result = run({ document: result.document, commands, nowUnix: now });
  snapshot = result.snapshot;
  calendar = result.calendar;
};

// 造一个「玩过一阵」的状态：有任务、有番茄记录、出过门、打过塔、种过地。
step([{ op: 'create_task', name: '写小程序', estimated: 4 }], 0);
const taskId = snapshot.tasks[0].id;
step([{ op: 'set_focus_minutes', minutes: 25 }], 0);
step([{ op: 'start_task', task_id: taskId }], 0);
step([], 26 * 60); // 完成一次，发一次掉落
step([{ op: 'explore', hours: 4 }], 0);
step([], 4 * 3600 + 30);
step([{ op: 'tower' }], 0);
step([{ op: 'prove' }], 0);
step([{ op: 'refresh_jobs' }], 0);
step([{ op: 'accept_job', slot: 0 }], 0);
step([{ op: 'upgrade_building', building: 'workshop' }], 0);

const outputs = {
  pomodoro: views.pomodoro(snapshot, calendar, null, 1500),
  pomodoroTicking: views.pomodoro(snapshot, calendar, { year: 2027, month: 0 }, 900),
  outing: views.outing(snapshot.cultivation),
  tower: views.tower(snapshot.cultivation),
  workshop: views.workshop(snapshot.cultivation),
  garden: views.garden(snapshot.cultivation),
  market: views.market(snapshot.cultivation),
  status: views.statusSection(snapshot.cultivation),
  arts: views.artsSection(snapshot.cultivation),
  gear: views.gearSection(snapshot.cultivation),
  me: views.me(snapshot, { nickname: '休谟', avatar: 'cloud://x/y.png' }),
  meFresh: views.me(snapshot, null),
};

let failures = 0;

function scan(name, value, pathText) {
  if (value === undefined) {
    failures += 1;
    console.log(`  FAIL ${name}: ${pathText} 是 undefined`);
    return;
  }
  if (typeof value === 'number' && Number.isNaN(value)) {
    failures += 1;
    console.log(`  FAIL ${name}: ${pathText} 是 NaN`);
    return;
  }
  if (typeof value === 'string') {
    if (value.includes('undefined') || value.includes('NaN') || value === 'null') {
      failures += 1;
      console.log(`  FAIL ${name}: ${pathText} = "${value}"`);
    }
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => scan(name, item, `${pathText}[${index}]`));
    return;
  }
  if (value && typeof value === 'object') {
    Object.keys(value).forEach((key) => scan(name, value[key], `${pathText}.${key}`));
  }
}

for (const [name, value] of Object.entries(outputs)) {
  try {
    assert.ok(value && typeof value === 'object', `${name} 应该返回对象`);
    scan(name, value, name);
    console.log(`  ok   ${name}`);
  } catch (error) {
    failures += 1;
    console.log(`  FAIL ${name}: ${error.message}`);
  }
}

/* ---------- 关键字段的存在性 ---------- */

const checks = [
  ['番茄钟有任务列表', outputs.pomodoro.tasks.length === 1],
  ['番茄钟日历有格子', outputs.pomodoro.cal.cells.length > 0],
  ['修仙外出有 3 个时长', outputs.outing.hours.length === 3],
  ['修仙外出有 5 个区域', outputs.outing.regions.length === 5],
  ['塔有战报记录', outputs.tower.record.length >= 1],
  ['洞府有 4 个建筑', outputs.workshop.buildings.length === 4],
  ['灵草园有 4 块地', outputs.garden.plots.length >= 4],
  ['集市有报价', outputs.market.offers.length >= 1],
  ['状态有 4 个属性', outputs.status.attributes.length === 4],
  ['我的页显示昵称', outputs.me.nickname === '休谟'],
  ['我的页无资料时有兜底昵称', outputs.meFresh.nickname === '未命名道友'],
  ['格式化：千分位', labels.num(1234567) === '1,234,567'],
  ['格式化：倒计时', labels.clock(1500) === '25:00'],
  ['格式化：拒绝原因', labels.refusalText('JobGone') === '这份活已经没了'],
];

for (const [name, ok] of checks) {
  if (ok) {
    console.log(`  ok   ${name}`);
  } else {
    failures += 1;
    console.log(`  FAIL ${name}`);
  }
}

/* ---------- 模板字段 vs 数据字段 ---------- */

// 模板里写了 `view.xxx` 但数据里没有这个键，真机上就是一片空白。
// 这里把每个页面的模板字段与它可能用到的 view 对象取并集比对。
const PAGES = {
  'pages/pomodoro/index.wxml': ['pomodoro'],
  'pages/cultivate/index.wxml': ['outing', 'tower', 'workshop', 'garden', 'market'],
  'pages/arts/index.wxml': ['status', 'arts', 'gear'],
  'pages/me/index.wxml': ['me'],
};

for (const [relative, names] of Object.entries(PAGES)) {
  const wxml = fs.readFileSync(path.join(__dirname, '..', relative), 'utf8');
  const used = new Set();
  const pattern = /\bview\.([a-zA-Z_]+)/g;
  let match = pattern.exec(wxml);
  while (match) {
    used.add(match[1]);
    match = pattern.exec(wxml);
  }
  const available = new Set();
  names.forEach((name) => Object.keys(outputs[name]).forEach((key) => available.add(key)));
  // 页面里同时用了 header.*，它在 pomodoro/cultivate/arts 里是独立对象
  const headerUsed = /header\.([a-zA-Z_]+)/.test(wxml);
  const headerKeys = new Set(Object.keys(outputs.pomodoro.header));
  used.forEach((key) => {
    if (!available.has(key)) {
      failures += 1;
      console.log(`  FAIL ${relative}: view.${key} 在页面数据里不存在`);
    }
  });
  if (headerUsed) {
    const headerPattern = /header\.([a-zA-Z_]+)/g;
    let headerMatch = headerPattern.exec(wxml);
    while (headerMatch) {
      if (!headerKeys.has(headerMatch[1])) {
        failures += 1;
        console.log(`  FAIL ${relative}: header.${headerMatch[1]} 不存在`);
      }
      headerMatch = headerPattern.exec(wxml);
    }
  }
  console.log(`  ok   ${relative} 字段对齐（${used.size} 个）`);
}

console.log(`\n${failures === 0 ? '全部通过' : `${failures} 处失败`}`);
process.exit(failures === 0 ? 0 : 1);
