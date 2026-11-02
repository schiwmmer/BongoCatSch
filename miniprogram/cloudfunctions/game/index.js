'use strict';

// 小程序唯一的服务端入口：所有番茄钟 / 修仙 / 功法操作都走这里。
//
// 客户端没有任何游戏逻辑——它只发 `{op, ...参数}`，然后渲染返回的快照。
// 数据库里的 `document` 字段是 Rust 内核导出的整档 JSON，客户端永远拿不到
// 写权限（集合权限设为「仅管理端可读写」），所以绕过服务端改数值这条路是断的。

const fs = require('fs');
const path = require('path');
const cloud = require('wx-server-sdk');

const core = require('./core');

cloud.init({ env: cloud.DYNAMIC_CURRENT_ENV });

const db = cloud.database();
const COLLECTION = 'saves';
const MAX_ATTEMPTS = 3;
const NICKNAME_MAX = 24;

let runner = null;

/// 内核编译一次，之后每个请求只新建实例。
function kernel() {
  if (!runner) {
    const bytes = fs.readFileSync(path.join(__dirname, 'kernel.wasm'));
    runner = core.createRunner(bytes);
  }
  return runner;
}

function isMissing(error) {
  const code = error && (error.errCode ?? error.code);
  const message = String((error && (error.errMsg || error.message)) || '');
  return code === -1 || code === -502005 || /does not exist|not exist|DOCUMENT_NOT_FOUND/i.test(message);
}

async function load(openid) {
  try {
    const result = await db.collection(COLLECTION).doc(openid).get();
    return result.data || null;
  } catch (error) {
    if (isMissing(error)) return null;
    throw error;
  }
}

/// 乐观锁写入：版本没变才算写成功，并发时返回 `null` 让上层重来一次。
async function save(openid, current, document) {
  if (!current) {
    try {
      await db.collection(COLLECTION).add({
        data: {
          _id: openid,
          document,
          version: 1,
          profile: null,
          created_at: db.serverDate(),
          updated_at: db.serverDate(),
        },
      });
      return 1;
    } catch {
      // 并发首次建档：让上层重读后重试。
      return null;
    }
  }
  const version = current.version || 0;
  const result = await db
    .collection(COLLECTION)
    .where({ _id: openid, version })
    .update({ data: { document, version: version + 1, updated_at: db.serverDate() } });
  const updated = result && result.stats ? result.stats.updated : 0;
  return updated > 0 ? version + 1 : null;
}

function cleanProfile(input) {
  const profile = {};
  if (input && typeof input.nickname === 'string') {
    const nickname = input.nickname.trim().slice(0, NICKNAME_MAX);
    if (nickname) profile.nickname = nickname;
  }
  if (input && typeof input.avatar === 'string') {
    const avatar = input.avatar.trim();
    // 只收本云存储的 fileID 或 https 图床，别的一律不存。
    if (avatar.startsWith('cloud://') || avatar.startsWith('https://')) {
      profile.avatar = avatar.slice(0, 512);
    }
  }
  return profile;
}

function ok(payload) {
  return { ok: true, code: 0, ...payload };
}

function fail(code, message) {
  return { ok: false, code, message };
}

exports.main = async (event = {}) => {
  const { OPENID } = cloud.getWXContext();
  if (!OPENID) {
    return fail(401, '没有拿到微信身份，请从小程序里打开');
  }

  const op = typeof event.op === 'string' && event.op ? event.op : 'sync';
  const nowUnix = Math.floor(Date.now() / 1000);

  // 「我的」页面的资料不属于游戏状态，单独一条更新，不动版本号。
  if (op === 'profile_set') {
    const profile = cleanProfile(event.profile);
    const current = await load(OPENID);
    if (!current) {
      return fail(409, '还没有存档，先开始一次专注');
    }
    await db
      .collection(COLLECTION)
      .doc(OPENID)
      .update({ data: { profile: Object.assign({}, current.profile || {}, profile) } });
    return ok({ profile: Object.assign({}, current.profile || {}, profile), serverNow: nowUnix });
  }

  const commands = op === 'sync' || op === 'profile_get' ? [] : [commandOf(op, event)];

  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt += 1) {
    const current = await load(OPENID);
    const document = current && typeof current.document === 'string' ? current.document : '';

    let result;
    try {
      result = core.settle(kernel(), document, commands, nowUnix);
    } catch (error) {
      console.error('settle failed', error);
      return fail(500, '结算失败，请稍后重试');
    }
    if (result.error) {
      return fail(422, result.error);
    }
    // 内核不认识的操作（op 拼错、旧版客户端发了已删除的指令）要明确报错，
    // 不能静默返回「成功」让人以为点点就好了。
    if (commands.length > 0 && result.accepted === 0) {
      return fail(422, '这个操作不被识别');
    }

    const changed = result.document !== document;
    let version = current ? current.version || 0 : 0;
    if (changed) {
      const written = await save(OPENID, current, result.document);
      if (written === null) continue; // 版本冲突，重读后重算
      version = written;
    }

    return ok({
      snapshot: result.snapshot,
      calendar: result.calendar,
      serverNow: nowUnix,
      version,
      changed,
      accepted: result.accepted,
      refused: result.refused,
      completed: result.completed,
      profile: current ? current.profile || null : null,
      // 「我的」页要显示的账号信息。openid 只露后六位，够认人又不可反推。
      account: {
        tail: OPENID.slice(-6),
        joinedAt: current && current.created_at ? new Date(current.created_at).getTime() : Date.now(),
      },
    });
  }

  return fail(409, '存档正在被另一个请求改写，请重试');
};

/// 把客户端参数还原成内核认识的一条命令。`op` 之外的字段原样透传。
function commandOf(op, event) {
  const command = { op };
  for (const key of Object.keys(event)) {
    if (key === 'op') continue;
    command[key] = event[key];
  }
  // 小程序端可能带进来的非游戏字段，不进内核。
  delete command.userInfo;
  delete command.scene;
  return command;
}
