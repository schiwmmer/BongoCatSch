// 全局游戏状态：一份服务端快照 + 一条串行请求队列。
//
// 请求必须串行：服务端用版本号做乐观锁，两个操作同时飞过去，后到的那个会
// 被判定为冲突。串行发能让每一次点击都落在最新的存档上。

const { call } = require('./api');

const state = {
  snapshot: null,
  calendar: [],
  profile: null,
  account: null,
  version: 0,
  serverNow: 0,
  receivedAt: 0, // 本地收到快照的时刻（毫秒），用于平滑倒计时
  status: 'idle', // idle | loading | ready | busy | error
  error: '',
  booted: false,
};

const listeners = new Set();
let chain = Promise.resolve();
let pending = 0;
let ticker = null;
let pollAt = 0;

function notify() {
  listeners.forEach((fn) => {
    try {
      fn();
    } catch (error) {
      console.warn('listener failed', error);
    }
  });
}

function apply(result) {
  if (result.snapshot) state.snapshot = result.snapshot;
  if (result.calendar) state.calendar = result.calendar;
  if (result.profile !== undefined) state.profile = result.profile;
  if (result.account) state.account = result.account;
  if (typeof result.version === 'number') state.version = result.version;
  if (typeof result.serverNow === 'number') state.serverNow = result.serverNow;
  state.receivedAt = Date.now();
  state.status = 'ready';
  state.error = '';
  state.booted = true;
}

function run(op, payload) {
  pending += 1;
  state.status = 'busy';
  notify();
  return call(op, payload)
    .then((result) => {
      apply(result);
      return result;
    })
    .catch((error) => {
      state.error = error.message || '请求失败';
      state.status = state.booted ? 'ready' : 'error';
      throw error;
    })
    .finally(() => {
      pending -= 1;
      notify();
    });
}

/// 串行执行：上一次请求结束（无论成败）才发下一次。
function enqueue(op, payload) {
  chain = chain.then(
    () => run(op, payload),
    () => run(op, payload),
  );
  return chain;
}

/// 首次进入：拉一次存档。失败不阻断页面，页面会显示重试按钮。
function boot() {
  if (state.booted) return Promise.resolve(state.snapshot);
  state.status = 'loading';
  notify();
  return enqueue('sync').catch(() => null);
}

const refresh = () => enqueue('sync');

/// 发一条游戏操作。`completed` 由服务端判定，客户端不自己算。
function send(op, payload) {
  return enqueue(op, payload);
}

function profile(input) {
  return enqueue('profile_set', { profile: input }).then((result) => {
    if (result.profile) state.profile = result.profile;
    notify();
    return result;
  });
}

/* ---------- 倒计时 ---------- */

/// 当前应该显示多少秒。服务端给的是权威剩余秒数，这里只按本地流逝平滑它。
function remainingSeconds() {
  const timer = state.snapshot && state.snapshot.timer;
  if (!timer) return (state.snapshot && state.snapshot.focus_minutes ? state.snapshot.focus_minutes : 25) * 60;
  if (timer.state !== 'Running') return timer.remaining_seconds;
  const drifted = (Date.now() - state.receivedAt) / 1000;
  return Math.max(0, timer.remaining_seconds - drifted);
}

/// 本地计时归零时补一次同步，让服务端把这一次结算掉。
function settleIfDue() {
  const timer = state.snapshot && state.snapshot.timer;
  if (!timer || timer.state !== 'Running') return;
  if (remainingSeconds() > 0.5) return;
  if (state.status === 'busy') return;
  refresh().catch(() => {});
}

/* ---------- 轮询 ---------- */

/// 页面显示时调用。`full` 用来重绘整页，`clock` 只重绘倒计时数字。
function startTick(handlers) {
  stopTick();
  pollAt = 0;
  ticker = setInterval(() => {
    if (handlers.clock) handlers.clock();
    settleIfDue();
    if (Date.now() >= pollAt) {
      pollAt = Date.now() + 30000;
      if (pending === 0) {
        refresh()
          .then((result) => {
            // 页面开着的时候倒计时走完：服务端结算完会带回 completed。
            if (result && result.completed && wx.showToast) {
              wx.showToast({ title: '一次专注完成，修为到账', icon: 'none' });
            }
            if (handlers.full) handlers.full(true);
          })
          .catch(() => {});
      }
    }
  }, 1000);
}

function stopTick() {
  if (ticker) {
    clearInterval(ticker);
    ticker = null;
  }
}

function subscribe(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

module.exports = {
  state,
  boot,
  refresh,
  send,
  profile,
  subscribe,
  startTick,
  stopTick,
  remainingSeconds,
  isBusy: () => pending > 0,
};
