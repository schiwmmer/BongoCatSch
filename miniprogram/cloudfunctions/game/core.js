'use strict';

// 服务端权威内核。
//
// 这里跑的是 `crates/bongocat-pomodoro` 编译出的同一份 wasm（`kernel.wasm`），
// 和桌面版、网页版完全同源——数值不会各自漂移，也不需要把三万多行 Rust 重写成 JS。
//
// 三条防刷规则写在这一层，客户端无法绕过：
//
// 1. **时间只认服务器。** 每个请求都 `boot(服务器 now)` 重新结算，倒计时只按
//    服务器时钟推进；客户端上报的毫秒数一律丢弃，所以改本机时间没有用。
// 2. **时区固定 UTC+8。** 业务日（每日机缘、日历）不吃客户端时区，否则往东
//    挪一天就能多刷一次每日结算。
// 3. **单次专注不得少于 25 分钟。** `set_focus_minutes` 小于 25 直接拒绝；
//    而内核本身只在剩余秒数被真实时间扣到 0 才结算，两者叠加就是「开始到
//    结束至少 25 分钟」。

const UTC_OFFSET_SECONDS = 8 * 3600;

/// 一次专注的最短时长（分钟）。内核允许更短，服务端不允许。
const FOCUS_FLOOR_MINUTES = 25;

/// 存档体积上限，超过说明文档已经不该是这个形态了。
const MAX_DOCUMENT_BYTES = 400 * 1024;

/// 一个请求最多带几条命令。客户端一次只发一条，这里只是设个上限。
const MAX_COMMANDS = 4;

/// 内核不认识的操作（白名单以 `bongocat-pomodoro-web` 的 parse_command 为准）。
/// 这里不重复列全量，只挡掉明显不属于游戏的键。
const RESERVED_OPS = new Set(['sync', 'profile_get', 'profile_set']);

function seedNow() {
  // 首次出生的潜质要用一个随机种子，云函数里没有 `SystemTime` 可用。
  // eslint-disable-next-line no-bitwise
  return Math.floor(Math.random() * 0x100000000) >>> 0;
}

/// 把一个 wasm 模块包成「一次请求一次结算」的运行器。
///
/// 每次请求新建实例是刻意的：内核状态挂在 wasm 的 thread_local 上，复用实例
/// 会把上一位玩家的存档带进下一位玩家的请求里。模块本身编译一次就够。
function createRunner(wasmBytes) {
  const module = new WebAssembly.Module(wasmBytes);

  return function run(request) {
    const document = typeof request.document === 'string' ? request.document : '';
    const nowUnix = Math.floor(request.nowUnix);
    const commands = Array.isArray(request.commands) ? request.commands : [];

    const ex = new WebAssembly.Instance(module, {}).exports;
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    // 内存可能被内核扩容过，视图每次重建。
    const view = () => new Uint8Array(ex.memory.buffer);
    const write = (text) => {
      const bytes = encoder.encode(text);
      const pointer = ex.web_input_ptr();
      view().set(bytes, pointer);
      return bytes.length;
    };
    const read = () => {
      const pointer = ex.web_output_ptr();
      const length = ex.web_output_len();
      return decoder.decode(view().slice(pointer, pointer + length));
    };

    const bootLength = document ? write(document) : 0;
    ex.web_boot(bootLength, UTC_OFFSET_SECONDS, nowUnix, seedNow());
    // 结算「关档期间」的欠账：探索回程、打工收工、远征归来、灵草成熟、每日机缘。
    // elapsed 传 0——`web_boot` 里的 resume_live_session 已经按墙上时间把倒计时
    // 扣过一次，再传时长就是重复收费。
    const settled = ex.web_tick(nowUnix, 0);

    let beforeRefusals = null;
    if (commands.length > 0) {
      ex.web_render(nowUnix);
      beforeRefusals = refusalCount(read());
    }

    let accepted = 0;
    for (const command of commands.slice(0, MAX_COMMANDS)) {
      const code = ex.web_command(write(JSON.stringify(command)), nowUnix);
      if (code === 0) accepted += 1;
    }

    ex.web_render(nowUnix);
    const snapshot = JSON.parse(read());
    ex.web_export();
    const exported = read();
    ex.web_calendar(UTC_OFFSET_SECONDS);
    const calendar = JSON.parse(read());

    const afterRefusals = snapshot && snapshot.cultivation
      ? snapshot.cultivation.refusal_count ?? 0
      : 0;

    // 「这一次请求里有没有真的完成一次专注」不能用 `web_tick` 的返回值判断：
    // 关档期间跑完的那一次是在 `web_boot` 的 resume_live_session 里结算的，
    // tick 拿到的是 0。用存档里的累计数做差才准确。
    const completed = (snapshot ? snapshot.total_pomodoros ?? 0 : 0) > totalBefore(document);

    return {
      document: exported,
      snapshot,
      calendar,
      accepted,
      // 命令被内核拒绝时 refusal_count 会加一；客户端据此提示「为什么没生效」，
      // 而不是让按钮看起来像卡住了。
      refused: beforeRefusals !== null && afterRefusals > beforeRefusals,
      completed,
      settled: (settled & 2) === 2,
    };
  };
}

function refusalCount(text) {
  try {
    const snapshot = JSON.parse(text);
    return snapshot && snapshot.cultivation ? snapshot.cultivation.refusal_count ?? 0 : 0;
  } catch {
    return 0;
  }
}

/// 存档里已经完成过多少次专注。空档或坏档都按 0 算。
function totalBefore(document) {
  if (!document) return 0;
  try {
    const parsed = JSON.parse(document);
    return Number(parsed.total_completions) || 0;
  } catch {
    return 0;
  }
}

/// 服务端自己的校验：返回 `null` 表示放行，返回字符串表示拒绝理由。
function guard(command) {
  if (!command || typeof command !== 'object') return '命令格式不对';
  const op = command.op;
  if (typeof op !== 'string' || op.length === 0) return '命令缺少操作名';
  if (RESERVED_OPS.has(op)) return '这个操作由服务端处理';

  if (op === 'set_focus_minutes') {
    const minutes = Number(command.minutes);
    if (!Number.isFinite(minutes) || minutes < FOCUS_FLOOR_MINUTES) {
      return `每次专注不能少于 ${FOCUS_FLOOR_MINUTES} 分钟`;
    }
  }
  return null;
}

/// 把一条请求走完整流程：读档 → 校验 → 内核结算 → 新档。
///
/// `runner` 由 `createRunner` 产出，`nowUnix` 必须来自服务端时钟。
function settle(runner, document, commands, nowUnix) {
  if (document && document.length > MAX_DOCUMENT_BYTES) {
    return { error: '存档过大，请联系客服' };
  }
  const accepted = [];
  for (const command of commands) {
    const reason = guard(command);
    if (reason) return { error: reason };
    accepted.push(command);
  }
  const result = runner({ document, commands: accepted, nowUnix });
  if (!result.document || result.document.length < 2) {
    return { error: '结算失败，请稍后重试' };
  }
  return result;
}

module.exports = {
  FOCUS_FLOOR_MINUTES,
  MAX_DOCUMENT_BYTES,
  UTC_OFFSET_SECONDS,
  createRunner,
  guard,
  settle,
};
