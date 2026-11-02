'use strict';

// 本地自测：不需要 AppID、不需要云环境，直接跑真实云函数入口。
//
// 做法是把 `wx-server-sdk` 替换成内存版（同样的 doc/where/add/update 语义 +
// 版本号乐观锁），其余代码路径与线上完全一致——包括内核加载、服务端时钟、
// 25 分钟下限和存档乐观锁。
//
//   node miniprogram/tools/selftest.js

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const Module = require('module');

const FUNCTION_DIR = path.join(__dirname, '..', 'cloudfunctions', 'game');

/* ---------- 内存版 wx-server-sdk ---------- */

const rows = new Map();
let clock = 1_800_000_000; // 假的开服时间，便于把「25 分钟」压成几行

function missing() {
  const error = new Error('document with _id does not exist');
  error.errCode = -1;
  error.errMsg = 'document.get:fail document with _id does not exist';
  return error;
}

function collectionOf(name) {
  assert.strictEqual(name, 'saves');
  const collection = {
    doc(id) {
      return {
        async get() {
          if (!rows.has(id)) throw missing();
          return { data: JSON.parse(JSON.stringify(rows.get(id))) };
        },
        async update({ data }) {
          if (!rows.has(id)) throw missing();
          const row = rows.get(id);
          Object.assign(row, data);
          return { stats: { updated: 1 } };
        },
      };
    },
    where(condition) {
      return {
        async update({ data }) {
          const row = rows.get(condition._id);
          if (!row || row.version !== condition.version) return { stats: { updated: 0 } };
          Object.assign(row, data);
          return { stats: { updated: 1 } };
        },
      };
    },
    async add({ data }) {
      if (rows.has(data._id)) throw new Error('duplicate key');
      rows.set(data._id, JSON.parse(JSON.stringify(data)));
      return { _id: data._id };
    },
  };
  return collection;
}

const fakeSdk = {
  DYNAMIC_CURRENT_ENV: 'local',
  init() {},
  getWXContext() {
    return { OPENID: currentUser };
  },
  database() {
    return {
      collection: collectionOf,
      serverDate: () => new Date(clock * 1000).toISOString(),
    };
  },
};

let currentUser = 'openid-alice';

const load = Module._load;
Module._load = function patched(request, parent, isMain) {
  if (request === 'wx-server-sdk') return fakeSdk;
  return load.call(this, request, parent, isMain);
};

/* ---------- 时间控制 ---------- */

const realNow = Date.now;
Date.now = () => clock * 1000;
const advance = (seconds) => {
  clock += seconds;
};

/* ---------- 调用云函数 ---------- */

const game = require(path.join(FUNCTION_DIR, 'index.js'));
const call = (event) => game.main(event);
const expectOk = async (event) => {
  const result = await call(event);
  assert.strictEqual(result.ok, true, `${event.op} 应该成功：${JSON.stringify(result)}`);
  return result;
};

/* ---------- 用例 ---------- */

const cases = [];
const test = (name, body) => cases.push({ name, body });

test('首次进入自动建档，落在数据库而不是客户端', async () => {
  const result = await expectOk({ op: 'sync' });
  assert.strictEqual(result.snapshot.cultivation.realm, 'QiRefining');
  assert.strictEqual(result.version, 1);
  assert.ok(rows.has('openid-alice'), '数据库里必须有这一行');
  assert.ok(rows.get('openid-alice').document.startsWith('{'));
});

test('创建任务与开始专注', async () => {
  const created = await expectOk({ op: 'create_task', name: '写小程序', estimated: 2 });
  assert.strictEqual(created.snapshot.tasks.length, 1);
  const taskId = created.snapshot.tasks[0].id;

  const started = await expectOk({ op: 'start_task', task_id: taskId });
  assert.strictEqual(started.snapshot.timer.state, 'Running');
  assert.strictEqual(started.snapshot.timer.remaining_seconds, 25 * 60);
});

test('服务端时钟推进：60 秒后剩 1440 秒，且没有发钱', async () => {
  advance(60);
  const result = await expectOk({ op: 'sync' });
  assert.strictEqual(result.snapshot.timer.remaining_seconds, 1440);
  assert.strictEqual(result.snapshot.total_pomodoros, 0);
});

test('客户端伪造时间无效：命令里夹带的 now/elapsed 不进内核', async () => {
  // 一次性把「已过时间」和「服务器时间」都往前推，期望被忽略。
  const result = await expectOk({
    op: 'sync',
    now: clock + 100000,
    elapsed: 100000,
    serverNow: clock + 100000,
  });
  assert.strictEqual(result.snapshot.timer.remaining_seconds, 1440);
  assert.strictEqual(result.snapshot.total_pomodoros, 0);
});

test('不足 25 分钟的专注无法完成：24 分钟时仍未结算', async () => {
  advance(24 * 60 - 60); // 累计 25 分钟差 60 秒
  const result = await expectOk({ op: 'sync' });
  assert.strictEqual(result.snapshot.total_pomodoros, 0, '这时不能有产出');
  assert.strictEqual(result.snapshot.timer.state, 'Running');
  assert.strictEqual(result.snapshot.timer.remaining_seconds, 60);
});

test('满 25 分钟才结算：修为与番茄数同时到账', async () => {
  advance(60);
  const result = await expectOk({ op: 'sync' });
  assert.strictEqual(result.completed, true);
  assert.strictEqual(result.snapshot.total_pomodoros, 1);
  assert.ok(result.snapshot.cultivation.growth > 0, '修为必须真的涨');
  assert.strictEqual(result.snapshot.timer, null);
});

test('把每次时长改到 25 分钟以下会被服务端拒绝', async () => {
  const result = await call({ op: 'set_focus_minutes', minutes: 10 });
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 422);
  assert.ok(/25/.test(result.message), '提示里要写清下限');
  const after = await expectOk({ op: 'sync' });
  assert.strictEqual(after.snapshot.focus_minutes, 25, '被拒绝的修改不能生效');
});

test('合法时长可以改', async () => {
  const result = await expectOk({ op: 'set_focus_minutes', minutes: 45 });
  assert.strictEqual(result.snapshot.focus_minutes, 45);
  await expectOk({ op: 'set_focus_minutes', minutes: 25 });
});

test('修仙操作同样在服务端：外出后 at_home 变 false', async () => {
  const before = await expectOk({ op: 'sync' });
  assert.strictEqual(before.snapshot.cultivation.at_home, true);
  const result = await expectOk({ op: 'explore', hours: 4 });
  assert.strictEqual(result.snapshot.cultivation.at_home, false);
  assert.ok(result.snapshot.cultivation.outing_return_seconds > 0);
  advance(4 * 3600 + 60);
  const home = await expectOk({ op: 'sync' });
  assert.strictEqual(home.snapshot.cultivation.at_home, true, '四小时后应该自己回来');
});

test('内核拒绝的操作会如实回报，不会被吞掉', async () => {
  const result = await expectOk({ op: 'accept_job', slot: 9 });
  assert.strictEqual(result.refused, true, '不存在的活计必须被拒绝并留痕');
  assert.strictEqual(result.snapshot.cultivation.last_refusal, 'JobGone');
});

test('拼错的操作名直接报错，不会假装成功', async () => {
  const result = await call({ op: 'start_taks', task_id: 1 });
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 422);
});

test('资料写入与读取', async () => {
  const set = await expectOk({ op: 'profile_set', profile: { nickname: '休谟' } });
  assert.strictEqual(set.profile.nickname, '休谟');
  const read = await expectOk({ op: 'sync' });
  assert.strictEqual(read.profile.nickname, '休谟');
});

test('另一个微信号拿到的是自己的独立存档', async () => {
  currentUser = 'openid-bob';
  const result = await expectOk({ op: 'sync' });
  assert.strictEqual(result.snapshot.total_pomodoros, 0);
  assert.strictEqual(result.profile, null);
  assert.ok(rows.has('openid-bob'));
  currentUser = 'openid-alice';
  const back = await expectOk({ op: 'sync' });
  assert.strictEqual(back.snapshot.total_pomodoros, 1, 'alice 的档不能被 bob 覆盖');
});

test('没有微信身份直接拒绝', async () => {
  currentUser = null;
  const result = await call({ op: 'sync' });
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 401);
  currentUser = 'openid-alice';
});

/* ---------- 跑起来 ---------- */

(async () => {
  assert.ok(
    fs.existsSync(path.join(FUNCTION_DIR, 'kernel.wasm')),
    '缺少 kernel.wasm，先跑 `just mp-kernel`',
  );
  let failed = 0;
  for (const item of cases) {
    try {
      await item.body();
      console.log(`  ok   ${item.name}`);
    } catch (error) {
      failed += 1;
      console.log(`  FAIL ${item.name}`);
      console.log(`       ${error.message}`);
    }
  }
  Date.now = realNow;
  console.log(`\n${cases.length - failed}/${cases.length} 通过`);
  process.exit(failed === 0 ? 0 : 1);
})();
